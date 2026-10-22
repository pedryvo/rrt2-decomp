//! Sound-effect bank: the PK2 PCM entries decoded once into kira static sounds.

use kira::{
    Decibels, Frame, PlaybackRate, Tween,
    sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
};
use rt2_formats::{
    pk2::Pk2,
    sound::{self, Pcm, SOUNDS},
};

use crate::{AudioError, ids::SoundId};

/// Converts decoded PCM to a kira sound (mono is kept mono, kira pans it).
pub fn to_static(pcm: &Pcm) -> StaticSoundData {
    let frames: Vec<Frame> = if pcm.channels == 1 {
        pcm.samples
            .iter()
            .map(|&s| Frame::from_mono(f32::from(s) / 32768.0))
            .collect()
    } else {
        pcm.samples
            .chunks_exact(usize::from(pcm.channels))
            .map(|c| {
                Frame::new(
                    f32::from(c[0]) / 32768.0,
                    f32::from(c[1.min(c.len() - 1)]) / 32768.0,
                )
            })
            .collect()
    };
    StaticSoundData {
        sample_rate: pcm.sample_rate,
        frames: frames.into(),
        settings: StaticSoundSettings::new(),
        slice: None,
    }
}

/// Percent (0..=100, the scale of the original's volume argument) to kira decibels.
pub fn percent_to_db(percent: u8) -> Decibels {
    match percent {
        0 => Decibels::SILENCE,
        p if p >= 100 => Decibels::IDENTITY,
        p => Decibels(20.0 * (f32::from(p) / 100.0).log10()),
    }
}

/// All sound effects of the install, indexed by [`SoundId`].
pub struct SfxBank {
    pub(crate) sounds: Vec<Option<StaticSoundData>>,
}

impl SfxBank {
    /// Decodes every named entry found in `archives` (precedence order). Entries missing from the
    /// archives stay empty so a trimmed install still plays what it has.
    pub fn from_archives(archives: &[Pk2<'_>]) -> Result<Self, AudioError> {
        let mut sounds = vec![None; SOUNDS.len()];
        for entry in SOUNDS.iter().filter(|e| e.is_used()) {
            let Some(blob) = sound::blob(archives, entry) else {
                continue;
            };
            let pcm = sound::decode(entry, blob).map_err(|source| AudioError::Decode {
                name: entry.name,
                source,
            })?;
            sounds[usize::from(entry.id)] = Some(to_static(&pcm));
        }
        Ok(Self { sounds })
    }

    pub fn get(&self, id: SoundId) -> Option<&StaticSoundData> {
        self.sounds.get(usize::from(id.0))?.as_ref()
    }

    /// Number of loaded sounds.
    pub fn len(&self) -> usize {
        self.sounds.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A playing effect that the caller can still adjust or stop (looping engine sounds).
pub struct Voice {
    pub(crate) handle: StaticSoundHandle,
    pub(crate) base_rate: u32,
}

impl Voice {
    pub fn set_volume(&mut self, percent: u8) {
        self.handle
            .set_volume(percent_to_db(percent), Tween::default());
    }

    /// Changes the playback rate to `hz` samples per second of the original (the original passes
    /// the DirectSound frequency; `chug*` follow the train speed this way).
    pub fn set_frequency(&mut self, hz: u32) {
        self.handle.set_playback_rate(
            PlaybackRate(f64::from(hz) / f64::from(self.base_rate.max(1))),
            Tween::default(),
        );
    }

    pub fn stop(&mut self) {
        self.handle.stop(Tween::default());
    }

    pub fn is_playing(&self) -> bool {
        use kira::sound::PlaybackState::{Paused, Pausing, Playing, Resuming, WaitingToResume};
        matches!(
            self.handle.state(),
            Playing | Pausing | Paused | WaitingToResume | Resuming
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_scale() {
        assert_eq!(percent_to_db(100), Decibels::IDENTITY);
        assert_eq!(percent_to_db(0), Decibels::SILENCE);
        assert!((percent_to_db(50).0 + 6.02).abs() < 0.01);
    }

    #[test]
    fn pcm_to_frames() {
        let pcm = Pcm {
            channels: 1,
            sample_rate: 11025,
            samples: vec![0, 16384, -32768],
        };
        let s = to_static(&pcm);
        assert_eq!(s.sample_rate, 11025);
        assert_eq!(s.frames.len(), 3);
        assert!((s.frames[1].left - 0.5).abs() < 1e-6);
        assert!((s.frames[2].right + 1.0).abs() < 1e-6);
    }
}
