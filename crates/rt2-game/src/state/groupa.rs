//! State of the "map and general" screens: Credits, High Scores, the Map Overview, the videos and
//! the right-click box (docs/reference/screens.md A8, A9, A10, C3..C6).

use super::credits::CreditsState;
use super::hiscores::HiscoreState;
use super::overview::OverviewState;
use super::video::VideoState;

#[derive(Debug, Default)]
pub(super) struct GroupA {
    pub credits: CreditsState,
    pub hiscores: HiscoreState,
    pub overview: OverviewState,
    pub video: VideoState,
    /// Where the right button went down on the map (a click shows the information box).
    pub right_press: Option<(f32, f32)>,
}

use super::AppState;
use crate::scenario_info::Medal;

impl AppState {
    /// Per-frame work of these screens (before the simulation steps).
    pub(super) fn ga_tick(&mut self, dt_ms: f32) {
        if !self.in_game() {
            self.ga.overview.open = false;
        }
        self.credits_tick(dt_ms);
        self.video_tick(dt_ms);
        if self.overview_refresh() {
            self.rebuild_ui();
        }
    }

    /// The window loop turns clips on (tests and screenshots leave them off, so no key or click
    /// is ever swallowed by a clip nobody asked for).
    pub fn set_videos(&mut self, on: bool) {
        self.ga.video.enabled = on;
    }

    /// Plays a clip tied to an event when videos are on.
    pub(super) fn video_event(&mut self, name: &str) {
        if self.ga.video.enabled {
            self.play_video(name);
        }
    }

    /// The scenario ended: a victory goes into the High Scores table and each outcome plays its
    /// clip (`scenwin` / `scenlose`, docs/formats/sound.md 3).
    pub(super) fn game_over_hook(&mut self, medal: Medal) {
        match medal {
            Medal::Loss => self.video_event("scenlose"),
            m => {
                self.record_score(match m {
                    Medal::Bronze => 1,
                    Medal::Silver => 2,
                    _ => 3,
                });
                self.video_event("scenwin");
            }
        }
    }

    /// Screenshots: lets a clip run for `ms` of its own time.
    pub(super) fn video_wait(&mut self, ms: f32) {
        let mut t = 0.0;
        while t < ms && self.video_active() {
            std::thread::sleep(std::time::Duration::from_millis(25));
            self.video_tick(83.0);
            t += 83.0;
        }
    }
}

impl AppState {
    /// Screenshots: opens the Map Overview on a layer. `resource[:all|:ID,ID...]`, `stations`,
    /// `grade`, `traffic`, `company`.
    pub fn show_overview(&mut self, spec: &str) -> bool {
        use super::overview::Layer;
        if !self.in_game() || self.editor_active() {
            return false;
        }
        let (name, arg) = spec.split_once(':').unwrap_or((spec, ""));
        let layer = match name {
            "resource" => Layer::Resource,
            "stations" => Layer::Stations,
            "grade" => Layer::Grade,
            "traffic" => Layer::Traffic,
            "company" => Layer::Company,
            _ => return false,
        };
        if !self.ga.overview.open {
            self.overview_toggle();
        }
        self.ga.overview.layer = layer;
        if layer == Layer::Resource {
            let roles = self.ga.overview.roles.clone();
            self.ga.overview.cargo_on = if arg == "all" {
                roles.0.union(&roles.1).copied().collect()
            } else {
                arg.split(',')
                    .filter_map(|c| c.trim().parse().ok())
                    .collect()
            };
        }
        self.rebuild_ui();
        true
    }
}
