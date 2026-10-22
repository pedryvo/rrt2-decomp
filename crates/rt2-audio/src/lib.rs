//! Music and sound effects for the Railroad Tycoon II clone.
//!
//! Playback uses [kira](https://crates.io/crates/kira). The audio side never feeds back into the
//! simulation: events flow one way (sim to game layer to [`Audio`]), random choices use
//! [`rng::AudioRng`] and the audio clock is wall time. Formats and the event table are documented
//! in `docs/formats/sound.md`.

pub mod audio;
pub mod events;
pub mod files;
pub mod ids;
pub mod music;
pub mod rng;
pub mod sfx;
pub mod spatial;
pub mod video;

pub use audio::{Audio, PlayParams};
pub use events::Event;
pub use files::GameFiles;
pub use ids::SoundId;
pub use sfx::{SfxBank, Voice};

/// Errors from loading data or talking to the audio device.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("audio backend: {0}")]
    Backend(String),
    #[error("sound {name}: {source}")]
    Decode {
        name: &'static str,
        source: rt2_formats::sound::SoundError,
    },
    #[error("archive {name}: {message}")]
    Archive { name: String, message: String },
    #[error("music file {file}: {message}")]
    Music { file: String, message: String },
    #[error("video conversion failed: {0}")]
    Video(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
