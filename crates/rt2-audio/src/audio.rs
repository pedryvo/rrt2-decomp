//! The audio facade: sound effects, music and volumes on one kira manager.

use std::fmt::Debug;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend, Panning, PlaybackRate, Tween,
    backend::Backend,
    sound::{
        FromFileError, PlaybackState,
        streaming::{StreamingSoundData, StreamingSoundHandle},
    },
    track::{TrackBuilder, TrackHandle},
};
use rt2_formats::pk2::Pk2;

use crate::{
    AudioError,
    events::Event,
    files::GameFiles,
    ids::SoundId,
    music::{MusicCommand, MusicDirector, PlayOrder, Track},
    rng::AudioRng,
    sfx::{SfxBank, Voice, percent_to_db},
    spatial::{self, View},
};

/// Options for one effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayParams {
    /// Volume in percent, the original's scale (0..=100).
    pub volume: u8,
    /// -1 left .. 1 right (a modern extra; the original has no panning).
    pub pan: f32,
    pub looping: bool,
    /// Playback rate in Hz (DirectSound frequency); `None` = the entry's own rate.
    pub frequency: Option<u32>,
}

impl Default for PlayParams {
    fn default() -> Self {
        Self {
            volume: 100,
            pan: 0.0,
            looping: false,
            frequency: None,
        }
    }
}

pub struct Audio<B: Backend = DefaultBackend> {
    manager: AudioManager<B>,
    sfx_track: TrackHandle,
    music_track: TrackHandle,
    bank: SfxBank,
    director: MusicDirector,
    music_dir: PathBuf,
    music_handle: Option<StreamingSoundHandle<FromFileError>>,
    started: Instant,
    rng: AudioRng,
    master: u8,
}

impl Audio<DefaultBackend> {
    /// Opens the default output device with the effects and music of an install.
    pub fn open(files: &GameFiles) -> Result<Self, AudioError> {
        let bank = load_bank(files)?;
        Self::with_backend(
            AudioManagerSettings::default(),
            bank,
            files.music_files().unwrap_or_default(),
            files.music_dir(),
        )
    }
}

/// Reads the install's PK2 archives and decodes every sound effect.
pub fn load_bank(files: &GameFiles) -> Result<SfxBank, AudioError> {
    let raws = files.read_archives()?;
    let archives = raws
        .iter()
        .enumerate()
        .map(|(i, r)| {
            Pk2::parse(r).map_err(|e| AudioError::Archive {
                name: format!("#{i}"),
                message: e.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    SfxBank::from_archives(&archives)
}

impl<B: Backend> Audio<B>
where
    B::Error: Debug,
{
    /// Builds the audio engine on any kira backend (the tests use the mock backend).
    pub fn with_backend(
        settings: AudioManagerSettings<B>,
        bank: SfxBank,
        music_files: Vec<String>,
        music_dir: PathBuf,
    ) -> Result<Self, AudioError> {
        let backend_err = |e: &dyn Debug| AudioError::Backend(format!("{e:?}"));
        let mut manager = AudioManager::<B>::new(settings).map_err(|e| backend_err(&e))?;
        let sfx_track = manager
            .add_sub_track(TrackBuilder::new().sound_capacity(256))
            .map_err(|e| backend_err(&e))?;
        let music_track = manager
            .add_sub_track(TrackBuilder::new().sound_capacity(8))
            .map_err(|e| backend_err(&e))?;
        let mut rng = AudioRng::from_clock();
        let director = MusicDirector::new(music_files, AudioRng::new(rng.next_u64()));
        Ok(Self {
            manager,
            sfx_track,
            music_track,
            bank,
            director,
            music_dir,
            music_handle: None,
            started: Instant::now(),
            rng,
            master: 100,
        })
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    // ---- sound effects -------------------------------------------------------------------

    /// Plays an effect at full volume. `None` if the id has no sound or the voice limit is hit.
    pub fn play(&mut self, id: SoundId) -> Option<Voice> {
        self.play_with(id, &PlayParams::default())
    }

    pub fn play_with(&mut self, id: SoundId, params: &PlayParams) -> Option<Voice> {
        let entry = id.entry()?;
        let mut data = self
            .bank
            .get(id)?
            .clone()
            .volume(percent_to_db(params.volume))
            .panning(Panning(params.pan.clamp(-1.0, 1.0)));
        if let Some(hz) = params.frequency {
            data = data.playback_rate(PlaybackRate(f64::from(hz) / f64::from(entry.sample_rate)));
        }
        if params.looping {
            data = data.loop_region(..);
        }
        let handle = self.sfx_track.play(data).ok()?;
        Some(Voice {
            handle,
            base_rate: entry.sample_rate,
        })
    }

    /// Plays the sound of a game event with the volume and looping the original used.
    pub fn play_event(&mut self, event: Event) -> Option<Voice> {
        let row = event.row();
        self.play_with(
            row.sound,
            &PlayParams {
                volume: row.volume,
                looping: row.looping,
                ..PlayParams::default()
            },
        )
    }

    /// Plays an effect located on the map: volume falls off and pan follows the position relative
    /// to the view. Sounds the original would drop (volume 5 or less) are skipped.
    pub fn play_at(
        &mut self,
        id: SoundId,
        view: &View,
        pos: (f32, f32),
        volume: u8,
    ) -> Option<Voice> {
        let (v, pan) = spatial::locate(view, pos)?;
        self.play_with(
            id,
            &PlayParams {
                volume: (u32::from(v) * u32::from(volume) / 100) as u8,
                pan,
                ..PlayParams::default()
            },
        )
    }

    /// Picks one of the station chime / brake variants with the audio RNG (the original uses the
    /// CRT rand here, which never touches the simulation).
    pub fn random_variant(&mut self, count: u32) -> u32 {
        self.rng.range(0, count - 1)
    }

    pub fn sfx_loaded(&self) -> usize {
        self.bank.len()
    }

    // ---- music ---------------------------------------------------------------------------

    /// The game's music request (`0` main menu, `1` in game, `-2` keep category).
    pub fn music_request(&mut self, category: i32) -> Result<(), AudioError> {
        let now = self.now_ms();
        let cmds = self.director.request(category, now);
        self.run(cmds)
    }

    /// Modern option: skip to the next track.
    pub fn music_skip(&mut self) -> Result<(), AudioError> {
        let now = self.now_ms();
        let cmds = self.director.skip(now);
        self.run(cmds)
    }

    pub fn music_stop(&mut self) -> Result<(), AudioError> {
        let cmds = self.director.stop();
        self.run(cmds)
    }

    /// Modern option: random-avoiding-recent (original) or alphabetical order.
    pub fn set_music_order(&mut self, order: PlayOrder) {
        self.director.set_order(order);
    }

    pub fn music_tracks(&self) -> &[Track] {
        self.director.tracks()
    }

    pub fn music_current(&self) -> Option<&Track> {
        self.director.current().map(|i| &self.director.tracks()[i])
    }

    /// Plays one specific file of the playlist (CLI/testing); category rules are bypassed.
    pub fn music_play_file(&mut self, file: &str) -> Result<(), AudioError> {
        let track = self
            .director
            .tracks()
            .iter()
            .position(|t| t.file.eq_ignore_ascii_case(file))
            .ok_or_else(|| AudioError::Music {
                file: file.to_string(),
                message: "not in the playlist".into(),
            })?;
        self.run(vec![MusicCommand::Play {
            track,
            fade_in_ms: 0,
        }])
    }

    /// Call once per frame: finishes crossfades and advances to the next track.
    pub fn update(&mut self) -> Result<(), AudioError> {
        let now = self.now_ms();
        let mut cmds = self.director.tick(now);
        let ended = self
            .music_handle
            .as_ref()
            .is_some_and(|h| h.state() == PlaybackState::Stopped);
        if ended {
            self.music_handle = None;
            cmds.extend(self.director.track_ended(now));
        }
        self.run(cmds)
    }

    fn run(&mut self, cmds: Vec<MusicCommand>) -> Result<(), AudioError> {
        for cmd in cmds {
            match cmd {
                MusicCommand::FadeOut { ms } => {
                    if let Some(mut h) = self.music_handle.take() {
                        h.stop(Tween {
                            duration: Duration::from_millis(ms),
                            ..Tween::default()
                        });
                    }
                }
                MusicCommand::Stop => {
                    if let Some(mut h) = self.music_handle.take() {
                        h.stop(Tween::default());
                    }
                }
                MusicCommand::Play { track, fade_in_ms } => {
                    if let Some(mut h) = self.music_handle.take() {
                        h.stop(Tween::default());
                    }
                    let t = &self.director.tracks()[track];
                    let music_err = |message: String| AudioError::Music {
                        file: t.file.clone(),
                        message,
                    };
                    let mut data = StreamingSoundData::from_file(self.music_dir.join(&t.file))
                        .map_err(|e| music_err(e.to_string()))?;
                    if t.looped {
                        data = data.loop_region(..);
                    }
                    if fade_in_ms > 0 {
                        data = data.fade_in_tween(Tween {
                            duration: Duration::from_millis(fade_in_ms),
                            ..Tween::default()
                        });
                    }
                    let handle = self
                        .music_track
                        .play(data)
                        .map_err(|e| music_err(format!("{e}")))?;
                    self.music_handle = Some(handle);
                }
            }
        }
        Ok(())
    }

    // ---- volumes -------------------------------------------------------------------------

    /// Master volume in percent.
    pub fn set_master_volume(&mut self, percent: u8) {
        self.master = percent.min(100);
        self.manager
            .main_track()
            .set_volume(percent_to_db(self.master), Tween::default());
    }

    pub fn master_volume(&self) -> u8 {
        self.master
    }

    pub fn set_sfx_volume(&mut self, percent: u8) {
        self.sfx_track
            .set_volume(percent_to_db(percent.min(100)), Tween::default());
    }

    pub fn set_music_volume(&mut self, percent: u8) {
        self.music_track
            .set_volume(percent_to_db(percent.min(100)), Tween::default());
    }
}

#[cfg(test)]
mod tests {
    use kira::backend::mock::{MockBackend, MockBackendSettings};
    use rt2_formats::sound::{Pcm, SOUNDS};

    use super::*;
    use crate::sfx::to_static;

    fn mock_audio(music: Vec<String>) -> Audio<MockBackend> {
        let mut sounds = vec![None; SOUNDS.len()];
        let pcm = Pcm {
            channels: 1,
            sample_rate: 11025,
            samples: vec![1000; 2000],
        };
        sounds[1] = Some(to_static(&pcm));
        sounds[19] = Some(to_static(&pcm));
        let bank = SfxBank { sounds };
        Audio::with_backend(
            AudioManagerSettings {
                backend_settings: MockBackendSettings { sample_rate: 44100 },
                ..AudioManagerSettings::default()
            },
            bank,
            music,
            PathBuf::from("/nonexistent"),
        )
        .unwrap()
    }

    #[test]
    fn plays_known_sounds_only() {
        let mut a = mock_audio(Vec::new());
        assert!(a.play(SoundId::BUTTON_UP).is_some());
        assert!(a.play(SoundId::BUTTON_DOWN).is_none(), "not in the bank");
        assert!(a.play(SoundId(0)).is_none());
        assert_eq!(a.sfx_loaded(), 2);
    }

    #[test]
    fn loop_voice_can_be_adjusted_and_stopped() {
        let mut a = mock_audio(Vec::new());
        let mut v = a
            .play_with(
                SoundId::CHUG_FIRST,
                &PlayParams {
                    looping: true,
                    volume: 60,
                    ..PlayParams::default()
                },
            )
            .unwrap();
        assert!(v.is_playing());
        v.set_frequency(16000);
        v.set_volume(30);
        v.stop();
    }

    #[test]
    fn positional_sounds_respect_the_threshold() {
        let mut a = mock_audio(Vec::new());
        let view = View {
            center: (0.0, 0.0),
            half_width: 10.0,
            half_height: 10.0,
        };
        assert!(
            a.play_at(SoundId::BUTTON_UP, &view, (1.0, 1.0), 100)
                .is_some()
        );
        assert!(
            a.play_at(SoundId::BUTTON_UP, &view, (100.0, 0.0), 100)
                .is_none()
        );
    }

    #[test]
    fn music_without_files_is_a_no_op() {
        let mut a = mock_audio(Vec::new());
        a.music_request(1).unwrap();
        a.music_skip().unwrap();
        a.update().unwrap();
        assert!(a.music_current().is_none());
    }

    #[test]
    fn missing_music_file_reports_an_error() {
        let mut a = mock_audio(vec!["a.mp3".into()]);
        let err = a.music_request(1).unwrap_err();
        assert!(matches!(err, AudioError::Music { .. }), "{err}");
    }

    #[test]
    fn volumes_apply() {
        let mut a = mock_audio(Vec::new());
        a.set_master_volume(50);
        a.set_sfx_volume(20);
        a.set_music_volume(80);
        assert_eq!(a.master_volume(), 50);
        assert!(a.random_variant(4) < 4);
    }
}
