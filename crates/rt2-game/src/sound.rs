//! Audio glue of the game (`docs/formats/sound.md`, `docs/ui.md` "Som").
//!
//! The data flows one way: [`crate::state::AppState`] pushes [`Cue`]s (what happened on screen),
//! the window loop drains them into a [`SoundSystem`]. Nothing here reads or writes simulation
//! state, so audio can never change a game. Without a sound device (or without the music files)
//! the system degrades to a no-op after logging once.

use std::path::Path;

use rt2_audio::music::{CAT_GAME, CAT_MENU};
use rt2_audio::{Audio, Event, GameFiles};
use serde::{Deserialize, Serialize};

/// `[audio]` of `config.toml`: volumes in percent (0..=100) and the mute switch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub master: u8,
    pub music: u8,
    pub sfx: u8,
    pub mute: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 100,
            music: 100,
            sfx: 100,
            mute: false,
        }
    }
}

impl AudioSettings {
    /// Master volume actually sent to the device: 0 while muted.
    pub fn effective_master(&self) -> u8 {
        if self.mute { 0 } else { self.master.min(100) }
    }
}

/// Something the screen did that has a sound. Produced by the UI state, consumed by audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// `music_request_category` (`0x4f6130`): [`CAT_MENU`] or [`CAT_GAME`].
    Music(i32),
    Event(Event),
    NextTrack,
    ToggleMute,
}

/// Turns "which screen is up" into music requests, only when the category changes (the original
/// also just stores a request for the category already in force, `sound.md` 2.2 rule 3).
#[derive(Debug, Default)]
pub struct MusicRequester {
    last: Option<i32>,
}

impl MusicRequester {
    /// Main menu and its sub-screens (new game, load game) use category 0 (`0x464f60`).
    pub fn menu(&mut self) -> Option<Cue> {
        self.want(i32::from(CAT_MENU))
    }

    /// Entering a game screen asks for category 1 (`0x40de20`, `0x411070`...).
    pub fn game(&mut self) -> Option<Cue> {
        self.want(i32::from(CAT_GAME))
    }

    fn want(&mut self, cat: i32) -> Option<Cue> {
        (self.last != Some(cat)).then(|| {
            self.last = Some(cat);
            Cue::Music(cat)
        })
    }
}

/// The audio engine, or nothing when the device or the files are not available.
pub struct SoundSystem {
    audio: Option<Audio>,
    settings: AudioSettings,
    warned: bool,
}

impl SoundSystem {
    /// Opens the default device with the install's sounds. Never fails: on error it logs once
    /// and the game runs silently.
    pub fn open(install: &Path, settings: AudioSettings) -> Self {
        let audio = match Audio::open(&GameFiles::new(install)) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("audio disabled: {e}");
                None
            }
        };
        let mut s = Self {
            audio,
            settings,
            warned: false,
        };
        s.apply_volumes();
        s
    }

    /// A system without a device (headless runs, tests).
    #[cfg(test)]
    pub fn silent(settings: AudioSettings) -> Self {
        Self {
            audio: None,
            settings,
            warned: false,
        }
    }

    #[cfg(test)]
    pub fn is_active(&self) -> bool {
        self.audio.is_some()
    }

    #[cfg(test)]
    pub fn settings(&self) -> &AudioSettings {
        &self.settings
    }

    fn apply_volumes(&mut self) {
        let Some(a) = &mut self.audio else { return };
        a.set_master_volume(self.settings.effective_master());
        a.set_music_volume(self.settings.music.min(100));
        a.set_sfx_volume(self.settings.sfx.min(100));
    }

    fn warn(&mut self, what: &str, e: &dyn std::fmt::Display) {
        if !self.warned {
            self.warned = true;
            eprintln!("audio: {what}: {e} (further audio errors are not reported)");
        }
    }

    pub fn apply(&mut self, cue: Cue) {
        match cue {
            Cue::ToggleMute => {
                self.settings.mute = !self.settings.mute;
                self.apply_volumes();
            }
            Cue::Music(cat) => {
                if let Some(Err(e)) = self.audio.as_mut().map(|a| a.music_request(cat)) {
                    self.warn("music", &e);
                }
            }
            Cue::NextTrack => {
                if let Some(Err(e)) = self.audio.as_mut().map(Audio::music_skip) {
                    self.warn("music", &e);
                }
            }
            Cue::Event(ev) => {
                if let Some(a) = &mut self.audio {
                    // Fire and forget; the voice is not tracked for one-shot UI sounds.
                    let _ = a.play_event(ev);
                }
            }
        }
    }

    /// Once per frame: crossfades and track changes.
    pub fn update(&mut self) {
        if let Some(Err(e)) = self.audio.as_mut().map(Audio::update) {
            self.warn("music", &e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_is_requested_only_when_the_category_changes() {
        let mut r = MusicRequester::default();
        assert_eq!(r.menu(), Some(Cue::Music(0)));
        // New game / load game pages are still the menu.
        assert_eq!(r.menu(), None);
        assert_eq!(r.game(), Some(Cue::Music(1)));
        assert_eq!(r.game(), None);
        assert_eq!(r.menu(), Some(Cue::Music(0)));
    }

    #[test]
    fn mute_zeroes_the_master_without_losing_it() {
        let mut s = SoundSystem::silent(AudioSettings {
            master: 70,
            ..AudioSettings::default()
        });
        assert_eq!(s.settings().effective_master(), 70);
        s.apply(Cue::ToggleMute);
        assert_eq!(s.settings().effective_master(), 0);
        assert_eq!(s.settings().master, 70);
        s.apply(Cue::ToggleMute);
        assert_eq!(s.settings().effective_master(), 70);
    }

    #[test]
    fn a_missing_device_is_a_no_op() {
        let mut s = SoundSystem::silent(AudioSettings::default());
        assert!(!s.is_active());
        s.apply(Cue::Music(1));
        s.apply(Cue::NextTrack);
        s.apply(Cue::Event(Event::ButtonUp));
        s.update();
    }

    #[test]
    fn audio_settings_parse_and_default() {
        let c = crate::config::Config::parse("[audio]\nmusic = 40\nmute = true\n").unwrap();
        assert_eq!(c.audio.music, 40);
        assert!(c.audio.mute);
        assert_eq!(c.audio.master, 100);
        assert_eq!(c.audio.sfx, 100);
    }
}
