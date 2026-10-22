//! Music selection, mirroring the original's rules (`docs/formats/sound.md`, "Música"). Pure logic
//! with no audio device: the director returns commands, [`crate::Audio`] executes them.
//!
//! Original, addresses in `RT2_PLAT.EXE`:
//! - `0x4f5490` scans `data2\*.mp3` (names up to 40 characters, at most 60 tracks). Every track is
//!   enabled with category 1; the byte-wise smallest name (`1AIntro.mp3`) becomes category 0, the
//!   first name starting with `C` category 2 and the first starting with `W` category 3, and these
//!   three loop. `0x4f56f0` gives each track a random rank in `0..=1000`.
//! - `0x4f6130(category)` is the only request: the game asks for 0 on the main menu and 1 in the
//!   game screens (`-2` = keep the current category, 1 if none). A category change picks the
//!   highest-ranked (most recently played) track of that category, or a random one if none is
//!   ranked, fades the playing track out over 1300 ms (`0x514`), then starts the new one with a
//!   500 ms fade-in (`0x4f5f30`).
//! - `0x4f5ca0` is the random pick: candidates are the enabled tracks of the category (all
//!   enabled tracks if the category has none), minus the `min(n-1, 3)` most recently played; the
//!   choice stamps the track with the rank counter (starts at 2000, +1000 per pick).
//! - `0x4f5de0` runs when a stream ends: a looping track restarts, any other picks the next
//!   track of the current category with `0x4f5ca0`.

use crate::rng::AudioRng;

/// Fade-out of the playing track when the category changes (`0x514` ms, `0x4f6130`).
pub const FADE_OUT_MS: u64 = 1300;
/// Fade-in of a new track (`|remaining| * 100 / 500`, `0x4f5f30`).
pub const FADE_IN_MS: u64 = 500;
/// Capacity of the original's track table (60 records of 0x44 bytes at `0x55d578`).
pub const MAX_TRACKS: usize = 60;
/// Longest accepted file name (`0x4f5490` copies 0x28 bytes and rejects longer names).
pub const MAX_NAME: usize = 40;
const INITIAL_RANK_COUNTER: u32 = 2000;
const RANK_STEP: u32 = 1000;

pub const CAT_MENU: u8 = 0;
pub const CAT_GAME: u8 = 1;

/// One track of `data2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// File name inside `data2` (`Cross Roads.mp3`).
    pub file: String,
    pub category: u8,
    /// Restarts instead of advancing when it ends.
    pub looped: bool,
    /// Higher means played more recently (random `0..=1000` before the first play).
    pub rank: u32,
}

/// How the next track is chosen when a track ends or is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayOrder {
    /// The original's random pick that avoids the last three tracks.
    #[default]
    Original,
    /// Modern option: the next file of the category in name order.
    Sequential,
}

/// What the player should do; the clone's audio layer maps these to streaming sounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicCommand {
    /// Fade the current track out (it will be replaced when [`MusicDirector::tick`] says so).
    FadeOut { ms: u64 },
    /// Start `track` (index into [`MusicDirector::tracks`]).
    Play { track: usize, fade_in_ms: u64 },
    /// Stop the current track now.
    Stop,
}

#[derive(Debug)]
pub struct MusicDirector {
    tracks: Vec<Track>,
    rng: AudioRng,
    order: PlayOrder,
    rank_counter: u32,
    /// Track being played (`DAT_0055e56c`).
    current: Option<usize>,
    /// Category of the music in effect (`DAT_0055e570`), `None` = nothing requested yet.
    category: Option<u8>,
    /// Track waiting for the fade-out to end (`DAT_0055e568`) and the time it starts at.
    pending: Option<(usize, u64)>,
    /// Load/suspend nesting (`DAT_0057bc38`).
    suspended: u32,
    /// Category to restore when the last suspension ends.
    resume_category: Option<i32>,
}

impl MusicDirector {
    /// Builds the playlist from the file names found in `data2` (any order; sorted here the way a
    /// Windows directory listing is: case-insensitive).
    pub fn new(mut files: Vec<String>, mut rng: AudioRng) -> Self {
        files.retain(|f| f.len() <= MAX_NAME);
        files.sort_by(|a, b| {
            a.to_ascii_lowercase()
                .cmp(&b.to_ascii_lowercase())
                .then_with(|| a.cmp(b))
        });
        files.truncate(MAX_TRACKS);
        let mut tracks: Vec<Track> = files
            .into_iter()
            .map(|file| Track {
                file,
                category: CAT_GAME,
                looped: false,
                rank: rng.range(0, 1000),
            })
            .collect();
        // Smallest name, byte-wise, first one wins on ties (0x4f5490).
        let mut min: Option<usize> = None;
        for (i, t) in tracks.iter().enumerate() {
            if min.is_none_or(|m| t.file.as_bytes() < tracks[m].file.as_bytes()) {
                min = Some(i);
            }
        }
        let first_with = |c: u8| {
            tracks.iter().position(|t| {
                t.file
                    .bytes()
                    .next()
                    .is_some_and(|b| b.to_ascii_uppercase() == c)
            })
        };
        let (c_idx, w_idx) = (first_with(b'C'), first_with(b'W'));
        // Applied in this order: a name that is both the smallest and starts with C ends in 2.
        for (idx, cat) in [(min, 0), (c_idx, 2), (w_idx, 3)] {
            if let Some(i) = idx {
                tracks[i].category = cat;
                tracks[i].looped = true;
            }
        }
        Self {
            tracks,
            rng,
            order: PlayOrder::Original,
            rank_counter: INITIAL_RANK_COUNTER,
            current: None,
            category: None,
            pending: None,
            suspended: 0,
            resume_category: None,
        }
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn current(&self) -> Option<usize> {
        self.current
    }

    pub fn set_order(&mut self, order: PlayOrder) {
        self.order = order;
    }

    /// Highest-ranked track of `category` (`0x4f6130` scan).
    fn best_in_category(&self, category: u8) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, t) in self.tracks.iter().enumerate() {
            if t.category == category && best.is_none_or(|b| t.rank > self.tracks[b].rank) {
                best = Some(i);
            }
        }
        best
    }

    /// `0x4f5ca0`: random pick avoiding the most recently played tracks, stamps the rank.
    pub fn pick_random(&mut self, category: u8) -> Option<usize> {
        let n = self.tracks.len();
        if n == 0 {
            return None;
        }
        let mut excluded = vec![false; n];
        let mut count = 0usize;
        for pass in 0..2 {
            for (i, t) in self.tracks.iter().enumerate() {
                if pass == 0 && t.category != category {
                    excluded[i] = true;
                } else {
                    excluded[i] = false;
                    count += 1;
                }
            }
            if count >= 1 {
                break;
            }
        }
        let skip = if count >= 2 { (count - 1).min(3) } else { 0 };
        for _ in 0..skip {
            let mut best: Option<usize> = None;
            for (i, ex) in excluded.iter().enumerate() {
                if !ex && best.is_none_or(|b| self.tracks[i].rank > self.tracks[b].rank) {
                    best = Some(i);
                }
            }
            if let Some(b) = best {
                excluded[b] = true;
            }
        }
        let remaining = count - skip;
        let chosen = if remaining == 0 {
            self.rng.range(0, n as u32 - 1) as usize
        } else {
            let mut r = self.rng.range(0, remaining as u32 - 1);
            let mut idx = 0;
            for (i, ex) in excluded.iter().enumerate() {
                if !ex {
                    if r == 0 {
                        idx = i;
                        break;
                    }
                    r -= 1;
                }
            }
            idx
        };
        self.tracks[chosen].rank = self.rank_counter;
        self.rank_counter += RANK_STEP;
        Some(chosen)
    }

    /// Next track for `category` according to the play order (original random rule or
    /// sequential), stamping its rank.
    fn next_in(&mut self, category: u8) -> Option<usize> {
        match self.order {
            PlayOrder::Original => self.pick_random(category),
            PlayOrder::Sequential => {
                let n = self.tracks.len();
                let start = self.current.map_or(0, |c| c + 1);
                let pick = (0..n)
                    .map(|k| (start + k) % n.max(1))
                    .find(|&i| self.tracks.get(i).is_some_and(|t| t.category == category))
                    .or_else(|| self.pick_random(category))?;
                self.tracks[pick].rank = self.rank_counter;
                self.rank_counter += RANK_STEP;
                Some(pick)
            }
        }
    }

    /// The game's `0x4f6130(category)`; `-2` keeps the current category (1 if none). `now_ms` is
    /// the audio clock.
    pub fn request(&mut self, category: i32, now_ms: u64) -> Vec<MusicCommand> {
        let cat = if category == -2 {
            self.category.unwrap_or(CAT_GAME)
        } else {
            match u8::try_from(category) {
                Ok(c) => c,
                Err(_) => return Vec::new(),
            }
        };
        if self.category == Some(cat) {
            return Vec::new();
        }
        if self.suspended != 0 {
            self.resume_category = Some(i32::from(cat));
            return Vec::new();
        }
        let Some(track) = self.best_in_category(cat).or_else(|| self.pick_random(cat)) else {
            return Vec::new();
        };
        self.category = Some(cat);
        if self.pending.map(|p| p.0) == Some(track) {
            return Vec::new();
        }
        let deadline = match self.current {
            Some(_) => now_ms + FADE_OUT_MS,
            None => now_ms,
        };
        self.pending = Some((track, deadline));
        let mut out = Vec::new();
        if self.current.is_some() {
            out.push(MusicCommand::FadeOut { ms: FADE_OUT_MS });
        }
        out.extend(self.tick(now_ms));
        out
    }

    /// Call every frame: starts the pending track once its fade-out has ended (`0x4f5f30`).
    pub fn tick(&mut self, now_ms: u64) -> Vec<MusicCommand> {
        match self.pending {
            Some((track, deadline)) if now_ms >= deadline => {
                self.pending = None;
                self.current = Some(track);
                vec![MusicCommand::Play {
                    track,
                    fade_in_ms: FADE_IN_MS,
                }]
            }
            _ => Vec::new(),
        }
    }

    /// The playing stream reached its end (`0x4f5de0`).
    pub fn track_ended(&mut self, _now_ms: u64) -> Vec<MusicCommand> {
        if self.pending.is_some() {
            return Vec::new();
        }
        let Some(cur) = self.current else {
            return Vec::new();
        };
        if self.tracks[cur].looped {
            return vec![MusicCommand::Play {
                track: cur,
                fade_in_ms: 0,
            }];
        }
        let cat = self.category.unwrap_or(CAT_GAME);
        match self.next_in(cat) {
            Some(next) => {
                self.current = Some(next);
                vec![MusicCommand::Play {
                    track: next,
                    fade_in_ms: 0,
                }]
            }
            None => {
                self.current = None;
                Vec::new()
            }
        }
    }

    /// Modern option: skip to the next track of the current category with the usual crossfade.
    pub fn skip(&mut self, now_ms: u64) -> Vec<MusicCommand> {
        let cat = self.category.unwrap_or(CAT_GAME);
        let Some(next) = self.next_in(cat) else {
            return Vec::new();
        };
        let deadline = now_ms
            + if self.current.is_some() {
                FADE_OUT_MS
            } else {
                0
            };
        self.pending = Some((next, deadline));
        let mut out = Vec::new();
        if self.current.is_some() {
            out.push(MusicCommand::FadeOut { ms: FADE_OUT_MS });
        }
        out.extend(self.tick(now_ms));
        out
    }

    /// `0x4f6230`: a screen load begins, music stops and requests are held.
    pub fn suspend(&mut self) -> Vec<MusicCommand> {
        self.suspended += 1;
        self.pending = None;
        if self.current.take().is_some() {
            vec![MusicCommand::Stop]
        } else {
            Vec::new()
        }
    }

    /// `0x4f62e0`: the load ended; the held category plays when the last suspension is released.
    pub fn resume(&mut self, now_ms: u64) -> Vec<MusicCommand> {
        self.suspended = self.suspended.saturating_sub(1);
        if self.suspended != 0 {
            return Vec::new();
        }
        self.category = None;
        match self.resume_category.take() {
            Some(c) => self.request(c, now_ms),
            None => Vec::new(),
        }
    }

    pub fn stop(&mut self) -> Vec<MusicCommand> {
        self.pending = None;
        self.category = None;
        if self.current.take().is_some() {
            vec![MusicCommand::Stop]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        [
            "1Intro.mp3",
            "alpha.mp3",
            "Beta.mp3",
            "chorus.mp3",
            "Delta.mp3",
            "echo.mp3",
            "Waltz.mp3",
            "wind.mp3",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
    }

    fn director() -> MusicDirector {
        MusicDirector::new(names(), AudioRng::new(42))
    }

    #[test]
    fn special_categories() {
        let d = director();
        let cat = |f: &str| d.tracks().iter().find(|t| t.file == f).unwrap().category;
        assert_eq!(cat("1Intro.mp3"), 0);
        assert_eq!(cat("chorus.mp3"), 2);
        assert_eq!(cat("Waltz.mp3"), 3);
        assert_eq!(cat("alpha.mp3"), 1);
        let looped: Vec<_> = d.tracks().iter().filter(|t| t.looped).collect();
        assert_eq!(looped.len(), 3);
        // Case-insensitive directory order.
        let files: Vec<_> = d.tracks().iter().map(|t| t.file.as_str()).collect();
        assert_eq!(&files[..3], ["1Intro.mp3", "alpha.mp3", "Beta.mp3"]);
    }

    #[test]
    fn long_and_excess_names_are_dropped() {
        let mut v = names();
        v.push("x".repeat(41));
        for i in 0..70 {
            v.push(format!("t{i:02}.mp3"));
        }
        let d = MusicDirector::new(v, AudioRng::new(1));
        assert_eq!(d.tracks().len(), MAX_TRACKS);
        assert!(d.tracks().iter().all(|t| t.file.len() <= MAX_NAME));
    }

    #[test]
    fn first_request_starts_immediately_with_fade_in() {
        let mut d = director();
        let cmds = d.request(0, 1000);
        assert_eq!(
            cmds,
            vec![MusicCommand::Play {
                track: 0,
                fade_in_ms: FADE_IN_MS
            }]
        );
        assert_eq!(d.tracks()[0].file, "1Intro.mp3");
        // Same category again does nothing.
        assert!(d.request(0, 1100).is_empty());
    }

    #[test]
    fn category_change_fades_out_then_plays() {
        let mut d = director();
        d.request(0, 0);
        let cmds = d.request(1, 5000);
        assert_eq!(cmds, vec![MusicCommand::FadeOut { ms: FADE_OUT_MS }]);
        assert!(d.tick(5000 + FADE_OUT_MS - 1).is_empty());
        let cmds = d.tick(5000 + FADE_OUT_MS);
        let [MusicCommand::Play { track, .. }] = cmds[..] else {
            panic!("{cmds:?}")
        };
        assert_eq!(d.tracks()[track].category, 1);
        assert_eq!(d.current(), Some(track));
    }

    #[test]
    fn minus_two_keeps_category() {
        let mut d = director();
        d.request(0, 0);
        assert!(d.request(-2, 10).is_empty());
        let mut e = director();
        // Nothing playing: -2 means category 1.
        let cmds = e.request(-2, 0);
        let [MusicCommand::Play { track, .. }] = cmds[..] else {
            panic!("{cmds:?}")
        };
        assert_eq!(e.tracks()[track].category, 1);
    }

    #[test]
    fn random_pick_avoids_the_last_three() {
        let mut d = director();
        // Category 1 has 4 tracks (alpha, Beta, Delta, echo); n-1 = 3 are excluded, so the pick
        // is always the least recently played one: a strict rotation after the first round.
        let mut seq = Vec::new();
        for _ in 0..12 {
            seq.push(d.pick_random(1).unwrap());
        }
        for w in seq.windows(4) {
            let mut s = w.to_vec();
            s.sort();
            s.dedup();
            assert_eq!(s.len(), 4, "{seq:?}");
        }
    }

    #[test]
    fn returning_to_a_category_resumes_its_last_track() {
        let mut d = director();
        d.request(1, 0);
        let first = d.current().unwrap();
        d.request(0, 100);
        d.tick(100 + FADE_OUT_MS);
        d.request(1, 10_000);
        d.tick(10_000 + FADE_OUT_MS);
        assert_eq!(d.current(), Some(first));
    }

    #[test]
    fn ended_track_loops_or_advances() {
        let mut d = director();
        d.request(0, 0);
        assert_eq!(
            d.track_ended(5),
            vec![MusicCommand::Play {
                track: 0,
                fade_in_ms: 0
            }]
        );
        let mut d = director();
        d.request(1, 0);
        let first = d.current().unwrap();
        let cmds = d.track_ended(5);
        let [MusicCommand::Play { track, .. }] = cmds[..] else {
            panic!("{cmds:?}")
        };
        assert_ne!(track, first);
    }

    #[test]
    fn sequential_order_walks_the_category() {
        let mut d = director();
        d.set_order(PlayOrder::Sequential);
        d.request(1, 0);
        let cat1: Vec<usize> = (0..d.tracks().len())
            .filter(|&i| d.tracks()[i].category == 1)
            .collect();
        let mut prev = d.current().unwrap();
        for _ in 0..8 {
            d.track_ended(0);
            let cur = d.current().unwrap();
            let pos = cat1.iter().position(|&i| i == prev).unwrap();
            assert_eq!(cur, cat1[(pos + 1) % cat1.len()]);
            prev = cur;
        }
    }

    #[test]
    fn suspend_and_resume() {
        let mut d = director();
        d.request(1, 0);
        assert_eq!(d.suspend(), vec![MusicCommand::Stop]);
        assert!(d.request(0, 10).is_empty());
        let cmds = d.resume(20);
        assert!(matches!(cmds[..], [MusicCommand::Play { .. }]));
        assert_eq!(d.tracks()[d.current().unwrap()].category, 0);
    }

    #[test]
    fn skip_replaces_the_track() {
        let mut d = director();
        d.request(1, 0);
        let first = d.current().unwrap();
        assert_eq!(
            d.skip(1000),
            vec![MusicCommand::FadeOut { ms: FADE_OUT_MS }]
        );
        d.tick(1000 + FADE_OUT_MS);
        assert_ne!(d.current(), Some(first));
    }

    #[test]
    fn menu_game_switching_never_repeats_the_last_three_in_a_category() {
        let mut d = director();
        let mut played = Vec::new();
        let mut now = 0;
        for round in 0..10 {
            // Game screen, then back to the menu, then a track ends in the game.
            d.request(1, now);
            now += FADE_OUT_MS + 1;
            d.tick(now);
            played.push(d.current().unwrap());
            d.request(0, now);
            now += FADE_OUT_MS + 1;
            d.tick(now);
            assert_eq!(
                d.tracks()[d.current().unwrap()].category,
                0,
                "round {round}"
            );
            d.request(1, now);
            now += FADE_OUT_MS + 1;
            d.tick(now);
            // Returning resumes the same game track instead of drawing a new one.
            assert_eq!(d.current(), played.last().copied());
            d.skip(now);
            now += FADE_OUT_MS + 1;
            d.tick(now);
            played.push(d.current().unwrap());
        }
        played.dedup();
        for w in played.windows(4) {
            let mut s = w.to_vec();
            s.sort();
            s.dedup();
            assert_eq!(s.len(), 4, "{played:?}");
        }
    }
}
