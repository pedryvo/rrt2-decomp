//! A10 Smacker videos (`smackwin.win`, `smackwi2..4.win`; docs/formats/sound.md 3). The clone
//! decodes `anim\*.SMK` with the `ffmpeg` command line tool: frames come through a pipe into a
//! reserved sprite of the skin atlas, the sound plays from an `ffplay` child (both are optional:
//! without them the video is skipped). Nothing is written next to the install.
//!
//! Played full screen with Esc, Space, Enter or a click to skip: the intro (`INTRO_01`) when the
//! game starts (setting `[gameplay] intro_video`, on by default) and `scenwin` / `scenlose` when a
//! scenario ends (the two triggers the executable's code shows, `sound.md` 3). Gap: the clips
//! of the Activity Box (tool tutorials) and the campaign clips (`cvNN`, `caNN`) are not wired:
//! which screen starts each of them is not decoded (`sound.md` section 4).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, sync_channel};

use rt2_ui::{Align, Anchor, Color, Fill, Fit, Kind, Layout, Ui};

use super::{AppState, Screen};

/// Size of the frame sprite (`DialogArt::video_frame`): clips are scaled into it.
pub const FRAME_W: u32 = crate::artwork::VIDEO_FRAME.0;
pub const FRAME_H: u32 = crate::artwork::VIDEO_FRAME.1;

/// A running clip.
pub(super) struct Player {
    frames: Receiver<Vec<u8>>,
    decoder: Child,
    sound: Option<Child>,
    fps: f32,
    elapsed_ms: f32,
    shown: u32,
    pub name: String,
    /// The reader hung up: nothing more will come.
    done: bool,
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.decoder.kill();
        let _ = self.decoder.wait();
        if let Some(s) = &mut self.sound {
            let _ = s.kill();
            let _ = s.wait();
        }
    }
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Player({})", self.name)
    }
}

/// The file of `name` in the `anim` folder, ignoring case (the install mixes `.SMK` and `.smk`).
pub(super) fn find_clip(install: &Path, name: &str) -> Option<PathBuf> {
    let dir = install.join("anim");
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|e| {
            let p = e.path();
            let stem = p.file_stem()?.to_string_lossy().to_ascii_lowercase();
            let ext = p.extension()?.to_string_lossy().to_ascii_lowercase();
            (stem == name.to_ascii_lowercase() && ext == "smk").then_some(p)
        })
}

/// `(width, height, frames per second)` of the first video stream, with `ffprobe`.
fn probe(file: &Path) -> Option<(u32, u32, f32)> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=width,height,r_frame_rate"])
        .args(["-of", "csv=p=0"])
        .arg(file)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut it = text.trim().split(',');
    let w: u32 = it.next()?.parse().ok()?;
    let h: u32 = it.next()?.parse().ok()?;
    let rate = it.next()?;
    let fps = match rate.split_once('/') {
        Some((n, d)) => n.parse::<f32>().ok()? / d.parse::<f32>().ok()?.max(1.0),
        None => rate.parse().ok()?,
    };
    Some((
        w,
        h,
        if fps.is_finite() && fps > 1.0 {
            fps
        } else {
            12.0
        },
    ))
}

/// ffmpeg filter that puts a `w` x `h` clip into the 640x480 frame: very wide clips (the
/// double-height 640x240 ones) fill it, the others keep their aspect with black bars.
pub(super) fn scale_filter(w: u32, h: u32, fps: f32) -> String {
    let fit = if h > 0 && w as f32 / h as f32 > 2.0 {
        format!("scale={FRAME_W}:{FRAME_H}")
    } else {
        format!(
            "scale={FRAME_W}:{FRAME_H}:force_original_aspect_ratio=decrease,pad={FRAME_W}:{FRAME_H}:(ow-iw)/2:(oh-ih)/2"
        )
    };
    format!("{fit},fps={fps}")
}

fn start(file: &Path, volume: u8, name: &str) -> Option<Player> {
    let (w, h, fps) = probe(file)?;
    // A clip with a token 4x4 picture is only a sound track: show black.
    let (w, h) = if w < 16 || h < 16 {
        (FRAME_W, FRAME_H)
    } else {
        (w, h)
    };
    let mut decoder = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-i"])
        .arg(file)
        .args(["-an", "-vf", &scale_filter(w, h, fps)])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = decoder.stdout.take()?;
    let (tx, rx) = sync_channel::<Vec<u8>>(8);
    std::thread::spawn(move || {
        let n = (FRAME_W * FRAME_H * 4) as usize;
        loop {
            let mut buf = vec![0u8; n];
            if out.read_exact(&mut buf).is_err() || tx.send(buf).is_err() {
                break;
            }
        }
    });
    let sound = (volume > 0)
        .then(|| {
            Command::new("ffplay")
                .args(["-nodisp", "-autoexit", "-loglevel", "quiet", "-vn"])
                .args(["-volume", &volume.to_string()])
                .arg(file)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok()
        })
        .flatten();
    Some(Player {
        frames: rx,
        decoder,
        sound,
        fps,
        elapsed_ms: 0.0,
        shown: 0,
        name: name.to_owned(),
        done: false,
    })
}

impl Player {
    /// Advances the clock; returns the frame to show now, if a new one is due.
    fn tick(&mut self, dt_ms: f32) -> Option<Vec<u8>> {
        self.elapsed_ms += dt_ms;
        let due = (self.elapsed_ms * self.fps / 1000.0) as u32 + 1;
        let mut newest = None;
        while self.shown < due {
            match self.frames.try_recv() {
                Ok(f) => {
                    self.shown += 1;
                    newest = Some(f);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.done = true;
                    break;
                }
            }
        }
        newest
    }
}

/// Video state kept by the application.
#[derive(Debug, Default)]
pub(super) struct VideoState {
    pub player: Option<Player>,
    /// Name of the clip a test or screenshot asked for, to verify the choice.
    pub last_name: Option<String>,
    /// Clips tied to events (intro, scenario end) play only when the window loop enabled them.
    pub enabled: bool,
}

impl AppState {
    pub fn video_active(&self) -> bool {
        self.ga.video.player.is_some()
    }

    /// Starts clip `name` full screen. False when there is no frame sprite, no clip or no
    /// `ffmpeg`.
    pub fn play_video(&mut self, name: &str) -> bool {
        if self.dialog_art.video_frame.is_none() {
            return false;
        }
        let Some(file) = find_clip(&self.install, name) else {
            return false;
        };
        let a = &self.config.audio;
        let volume = if a.mute {
            0
        } else {
            (u32::from(a.master) * u32::from(a.sfx) / 100) as u8
        };
        let Some(p) = start(&file, volume, name) else {
            return false;
        };
        self.ga.video.last_name = Some(name.to_owned());
        self.ga.video.player = Some(p);
        self.rebuild_ui();
        true
    }

    /// The opening clip, unless the setting turned it off.
    pub fn play_intro(&mut self) {
        if self.ga.video.enabled && self.config.gameplay.intro_video {
            self.play_video("INTRO_01");
        }
    }

    /// Stops the clip (Esc, Space, Enter, a click or its end).
    pub(super) fn video_stop(&mut self) {
        if self.ga.video.player.take().is_some() {
            self.rebuild_ui();
        }
    }

    /// A key or click while a clip plays skips it; true if it was consumed.
    pub(super) fn video_skip(&mut self) -> bool {
        if self.video_active() {
            self.video_stop();
            true
        } else {
            false
        }
    }

    pub(super) fn video_tick(&mut self, dt_ms: f32) {
        let Some(p) = &mut self.ga.video.player else {
            return;
        };
        let frame = p.tick(dt_ms);
        let done = p.done && frame.is_none();
        if let (Some(f), Some(sprite)) = (frame, self.dialog_art.video_frame) {
            self.sprite_uploads.borrow_mut().push((sprite, f));
        }
        if done {
            self.video_stop();
        }
    }

    /// The black screen with the picture and the hint line.
    pub(super) fn build_video(&self, ui: &mut Ui) {
        if !self.video_active() {
            return;
        }
        let Some(sprite) = self.dialog_art.video_frame else {
            return;
        };
        let root = ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(Color::rgb(0, 0, 0)),
            },
            Layout::fill(),
        );
        ui.add(
            root,
            Kind::Image {
                sprite,
                fit: Fit::Contain,
            },
            Layout::fill(),
        );
        ui.add(
            root,
            Kind::Label {
                text: self.texts.a("Press Esc, Space or click to skip"),
                font: super::menus::mfont(1, 1.1),
                color: Color::rgb(0xb4, 0xaa, 0x8e),
                align: Align::Right,
                wrap: false,
            },
            Layout::at(Anchor::BottomRight, 360.0, 20.0).offset(12.0, 8.0),
        );
    }

    /// Screenshots and tests: is the game showing a video over the screen.
    #[allow(dead_code)]
    pub(super) fn video_screen(&self) -> bool {
        self.video_active() && matches!(self.screen, Screen::Menu(_) | Screen::Game(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_clips_fill_the_frame_and_others_keep_their_aspect() {
        assert!(!scale_filter(640, 240, 9.0).contains("pad"));
        assert!(scale_filter(320, 240, 12.0).contains("pad"));
        assert!(scale_filter(236, 216, 10.0).ends_with("fps=10"));
    }

    #[test]
    fn clips_are_found_ignoring_case() {
        let dir = std::env::temp_dir().join(format!("rrt2-anim-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("anim")).unwrap();
        std::fs::write(dir.join("anim/INTRO_01.SMK"), b"").unwrap();
        std::fs::write(dir.join("anim/scenwin.smk"), b"").unwrap();
        assert!(find_clip(&dir, "intro_01").is_some());
        assert!(find_clip(&dir, "SCENWIN").is_some());
        assert!(find_clip(&dir, "missing").is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
