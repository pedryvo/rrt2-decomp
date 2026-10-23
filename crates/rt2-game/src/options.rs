//! The model behind the Options screen: which settings each tab has, how a setting reads as a row
//! (toggle, choice or slider) and how a change edits the [`Config`]. No UI and no I/O here, so it
//! is testable; `state/options_ui.rs` draws it and applies the side effects (window, audio,
//! language) of a change.

use crate::config::{Config, RESOLUTIONS, WindowMode};
use crate::notices::Category;
use crate::text::Texts;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Display,
    Audio,
    Language,
    Keys,
    Gameplay,
    Notices,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Display,
        Tab::Audio,
        Tab::Language,
        Tab::Keys,
        Tab::Gameplay,
        Tab::Notices,
    ];

    pub fn english(self) -> &'static str {
        match self {
            Tab::Display => "Display",
            Tab::Audio => "Audio",
            Tab::Language => "Language",
            Tab::Keys => "Keys",
            Tab::Gameplay => "Gameplay",
            Tab::Notices => "Notices",
        }
    }

    /// The settings rows of the tab (empty for the language and key tabs, which are lists).
    pub fn settings(self) -> &'static [Setting] {
        match self {
            Tab::Display => &[
                Setting::WindowMode,
                Setting::Resolution,
                Setting::Monitor,
                Setting::Vsync,
                Setting::UiScaleAuto,
                Setting::UiScale,
            ],
            Tab::Audio => &[Setting::Master, Setting::Music, Setting::Sfx, Setting::Mute],
            Tab::Gameplay => &[
                Setting::DefaultSpeed,
                Setting::AutosaveMonths,
                Setting::AutosaveKeep,
                Setting::EdgeScroll,
                Setting::PanSpeed,
                Setting::InvertDrag,
                Setting::InvertZoom,
                Setting::ZoomSpeed,
                Setting::IntroVideo,
            ],
            Tab::Notices => &[
                Setting::NoticeScenario,
                Setting::NoticeFinance,
                Setting::NoticeTowns,
                Setting::NoticeCompetitors,
            ],
            Tab::Language | Tab::Keys => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    WindowMode,
    Resolution,
    Monitor,
    Vsync,
    UiScaleAuto,
    UiScale,
    Master,
    Music,
    Sfx,
    Mute,
    DefaultSpeed,
    AutosaveMonths,
    AutosaveKeep,
    EdgeScroll,
    PanSpeed,
    InvertDrag,
    InvertZoom,
    ZoomSpeed,
    IntroVideo,
    NoticeScenario,
    NoticeFinance,
    NoticeTowns,
    NoticeCompetitors,
}

impl Setting {
    pub const ALL: [Setting; 23] = [
        Setting::WindowMode,
        Setting::Resolution,
        Setting::Monitor,
        Setting::Vsync,
        Setting::UiScaleAuto,
        Setting::UiScale,
        Setting::Master,
        Setting::Music,
        Setting::Sfx,
        Setting::Mute,
        Setting::DefaultSpeed,
        Setting::AutosaveMonths,
        Setting::AutosaveKeep,
        Setting::EdgeScroll,
        Setting::PanSpeed,
        Setting::InvertDrag,
        Setting::InvertZoom,
        Setting::ZoomSpeed,
        Setting::IntroVideo,
        Setting::NoticeScenario,
        Setting::NoticeFinance,
        Setting::NoticeTowns,
        Setting::NoticeCompetitors,
    ];

    pub fn index(self) -> u32 {
        Setting::ALL.iter().position(|s| *s == self).unwrap_or(0) as u32
    }

    pub fn from_index(i: u32) -> Option<Setting> {
        Setting::ALL.get(i as usize).copied()
    }

    /// The notification category of a `Notice*` setting.
    pub fn notice_category(self) -> Option<Category> {
        match self {
            Setting::NoticeScenario => Some(Category::Scenario),
            Setting::NoticeFinance => Some(Category::Finance),
            Setting::NoticeTowns => Some(Category::Towns),
            Setting::NoticeCompetitors => Some(Category::Competitors),
            _ => None,
        }
    }

    pub fn english(self) -> &'static str {
        match self {
            Setting::WindowMode => "Window mode",
            Setting::Resolution => "Window size",
            Setting::Monitor => "Monitor",
            Setting::Vsync => "Vertical sync",
            Setting::UiScaleAuto => "Interface scale from the monitor",
            Setting::UiScale => "Interface scale",
            Setting::Master => "Master volume",
            Setting::Music => "Music volume",
            Setting::Sfx => "Effects volume",
            Setting::Mute => "Mute all sound",
            Setting::DefaultSpeed => "Game speed at start",
            Setting::AutosaveMonths => "Autosave every",
            Setting::AutosaveKeep => "Autosaves kept",
            Setting::EdgeScroll => "Scroll at the window edge",
            Setting::PanSpeed => "Camera speed",
            Setting::InvertDrag => "Reverse mouse dragging",
            Setting::InvertZoom => "Reverse wheel zoom",
            Setting::ZoomSpeed => "Wheel zoom speed",
            Setting::IntroVideo => "Play the intro video",
            Setting::NoticeScenario => Category::Scenario.english(),
            Setting::NoticeFinance => Category::Finance.english(),
            Setting::NoticeTowns => Category::Towns.english(),
            Setting::NoticeCompetitors => Category::Competitors.english(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Control {
    Toggle(bool),
    /// Radio buttons; `enabled` false greys them out.
    Choice {
        options: Vec<String>,
        selected: usize,
        enabled: bool,
    },
    /// `text` is the value as shown ("70%", "Off").
    Slider {
        value: i32,
        min: i32,
        max: i32,
        step: i32,
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub setting: Setting,
    pub label: String,
    pub control: Control,
}

/// What the player did to a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Toggle,
    /// Radio option index.
    Pick(usize),
    /// Slider to this value (snapped and clamped).
    Set(i32),
    /// Slider by this many steps.
    Step(i32),
}

/// What the screen knows besides the config.
#[derive(Clone, Debug)]
pub struct Env {
    pub monitors: Vec<String>,
    /// The UI scale in force now, percent.
    pub ui_scale: u32,
    /// The scale follows the monitor's DPI (not pinned by the player, the config or `--ui-scale`).
    pub ui_auto: bool,
}

fn percent(v: i32) -> String {
    format!("{v}%")
}

fn slider(
    setting: Setting,
    label: String,
    value: i32,
    range: (i32, i32, i32),
    text: String,
) -> Row {
    Row {
        setting,
        label,
        control: Control::Slider {
            value,
            min: range.0,
            max: range.1,
            step: range.2,
            text,
        },
    }
}

fn toggle(setting: Setting, label: String, on: bool) -> Row {
    Row {
        setting,
        label,
        control: Control::Toggle(on),
    }
}

/// The window sizes offered: the presets plus the configured one if it is none of them.
pub fn resolution_list(c: &Config) -> Vec<(u32, u32)> {
    let mut v = RESOLUTIONS.to_vec();
    let cur = (c.display.width, c.display.height);
    if !v.contains(&cur) {
        v.push(cur);
        v.sort_unstable();
    }
    v
}

/// The rows of a tab, with the texts translated.
pub fn rows(tab: Tab, c: &Config, env: &Env, t: &Texts) -> Vec<Row> {
    tab.settings().iter().map(|&s| row(s, c, env, t)).collect()
}

fn row(s: Setting, c: &Config, env: &Env, t: &Texts) -> Row {
    let label = t.a(s.english());
    let g = &c.gameplay;
    match s {
        Setting::WindowMode => Row {
            setting: s,
            label,
            control: Control::Choice {
                options: vec![t.a("Windowed"), t.a("Borderless fullscreen")],
                selected: usize::from(c.display.mode == WindowMode::Borderless),
                enabled: true,
            },
        },
        Setting::Resolution => {
            let list = resolution_list(c);
            let sel = list
                .iter()
                .position(|&r| r == (c.display.width, c.display.height))
                .unwrap_or(0);
            Row {
                setting: s,
                label,
                control: Control::Choice {
                    options: list.iter().map(|(w, h)| format!("{w}x{h}")).collect(),
                    selected: sel,
                    enabled: c.display.mode == WindowMode::Windowed,
                },
            }
        }
        Setting::Monitor => {
            let mut options = vec![t.a("Current")];
            options.extend(env.monitors.iter().cloned());
            let selected = c
                .display
                .monitor
                .map_or(0, |m| (m + 1).min(options.len() - 1));
            Row {
                setting: s,
                label,
                control: Control::Choice {
                    options,
                    selected,
                    enabled: env.monitors.len() > 1,
                },
            }
        }
        Setting::Vsync => toggle(s, label, c.display.vsync),
        Setting::UiScaleAuto => toggle(s, label, env.ui_auto),
        Setting::UiScale => slider(
            s,
            label,
            env.ui_scale as i32,
            (100, 300, 25),
            percent(env.ui_scale as i32),
        ),
        Setting::Master => slider(
            s,
            label,
            i32::from(c.audio.master),
            (0, 100, 5),
            percent(i32::from(c.audio.master)),
        ),
        Setting::Music => slider(
            s,
            label,
            i32::from(c.audio.music),
            (0, 100, 5),
            percent(i32::from(c.audio.music)),
        ),
        Setting::Sfx => slider(
            s,
            label,
            i32::from(c.audio.sfx),
            (0, 100, 5),
            percent(i32::from(c.audio.sfx)),
        ),
        Setting::Mute => toggle(s, label, c.audio.mute),
        Setting::DefaultSpeed => slider(
            s,
            label,
            i32::from(g.default_speed),
            (0, 10, 1),
            if g.default_speed == 0 {
                t.a("As in the save")
            } else {
                g.default_speed.to_string()
            },
        ),
        Setting::AutosaveMonths => slider(
            s,
            label,
            g.autosave_months as i32,
            (0, 24, 1),
            if g.autosave_months == 0 {
                t.a("Off")
            } else {
                t.af("%1 months", &[g.autosave_months.to_string()])
            },
        ),
        Setting::AutosaveKeep => slider(
            s,
            label,
            g.autosave_keep as i32,
            (1, 20, 1),
            g.autosave_keep.to_string(),
        ),
        Setting::EdgeScroll => toggle(s, label, g.edge_scroll),
        Setting::PanSpeed => slider(
            s,
            label,
            g.pan_speed as i32,
            (25, 300, 25),
            percent(g.pan_speed as i32),
        ),
        Setting::InvertDrag => toggle(s, label, g.invert_drag),
        Setting::InvertZoom => toggle(s, label, g.invert_zoom),
        Setting::IntroVideo => toggle(s, label, g.intro_video),
        Setting::NoticeScenario
        | Setting::NoticeFinance
        | Setting::NoticeTowns
        | Setting::NoticeCompetitors => toggle(
            s,
            label,
            c.notices
                .get(s.notice_category().unwrap_or(Category::Scenario)),
        ),
        Setting::ZoomSpeed => slider(
            s,
            label,
            g.zoom_speed as i32,
            (25, 300, 25),
            percent(g.zoom_speed as i32),
        ),
    }
}

/// What a change needs the rest of the program to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    pub display: bool,
    pub audio: bool,
    /// The UI scale to switch to (`Some(None)` = follow the monitor).
    pub ui_scale: Option<Option<u32>>,
}

fn snap(v: i32, min: i32, max: i32, step: i32) -> i32 {
    let v = v.clamp(min, max);
    (min + (v - min + step / 2) / step * step).clamp(min, max)
}

/// Applies `change` to `setting` in `c`. A change that does not fit the setting (a toggle press on a
/// slider) does nothing.
pub fn apply(c: &mut Config, s: Setting, change: Change, env: &Env) -> Effects {
    let mut fx = Effects::default();
    let g = &mut c.gameplay;
    // A slider: the range and the new value.
    let adjust = |cur: i32, (min, max, step): (i32, i32, i32)| match change {
        Change::Set(v) => Some(snap(v, min, max, step)),
        Change::Step(n) => Some((cur + n * step).clamp(min, max)),
        _ => None,
    };
    match s {
        Setting::WindowMode => {
            if let Change::Pick(i) = change {
                c.display.mode = if i == 1 {
                    WindowMode::Borderless
                } else {
                    WindowMode::Windowed
                };
                fx.display = true;
            }
        }
        Setting::Resolution => {
            if let Change::Pick(i) = change
                && let Some(&(w, h)) = resolution_list(c).get(i)
            {
                c.display.width = w;
                c.display.height = h;
                fx.display = true;
            }
        }
        Setting::Monitor => {
            if let Change::Pick(i) = change {
                c.display.monitor = i.checked_sub(1).filter(|&m| m < env.monitors.len());
                fx.display = true;
            }
        }
        Setting::Vsync => {
            if change == Change::Toggle {
                c.display.vsync = !c.display.vsync;
                fx.display = true;
            }
        }
        Setting::UiScaleAuto => {
            if change == Change::Toggle {
                if env.ui_auto {
                    // Pin the scale in force now.
                    c.ui_scale = Some(env.ui_scale);
                    fx.ui_scale = Some(Some(env.ui_scale));
                } else {
                    c.ui_scale = None;
                    fx.ui_scale = Some(None);
                }
            }
        }
        Setting::UiScale => {
            if let Some(v) = adjust(env.ui_scale as i32, (100, 300, 25)) {
                c.ui_scale = Some(v as u32);
                fx.ui_scale = Some(Some(v as u32));
            }
        }
        Setting::Master => {
            if let Some(v) = adjust(i32::from(c.audio.master), (0, 100, 5)) {
                c.audio.master = v as u8;
                fx.audio = true;
            }
        }
        Setting::Music => {
            if let Some(v) = adjust(i32::from(c.audio.music), (0, 100, 5)) {
                c.audio.music = v as u8;
                fx.audio = true;
            }
        }
        Setting::Sfx => {
            if let Some(v) = adjust(i32::from(c.audio.sfx), (0, 100, 5)) {
                c.audio.sfx = v as u8;
                fx.audio = true;
            }
        }
        Setting::Mute => {
            if change == Change::Toggle {
                c.audio.mute = !c.audio.mute;
                fx.audio = true;
            }
        }
        Setting::DefaultSpeed => {
            if let Some(v) = adjust(i32::from(g.default_speed), (0, 10, 1)) {
                g.default_speed = v as u8;
            }
        }
        Setting::AutosaveMonths => {
            if let Some(v) = adjust(g.autosave_months as i32, (0, 24, 1)) {
                g.autosave_months = v as u32;
            }
        }
        Setting::AutosaveKeep => {
            if let Some(v) = adjust(g.autosave_keep as i32, (1, 20, 1)) {
                g.autosave_keep = v as u32;
            }
        }
        Setting::EdgeScroll => {
            if change == Change::Toggle {
                g.edge_scroll = !g.edge_scroll;
            }
        }
        Setting::PanSpeed => {
            if let Some(v) = adjust(g.pan_speed as i32, (25, 300, 25)) {
                g.pan_speed = v as u32;
            }
        }
        Setting::InvertDrag => {
            if change == Change::Toggle {
                g.invert_drag = !g.invert_drag;
            }
        }
        Setting::InvertZoom => {
            if change == Change::Toggle {
                g.invert_zoom = !g.invert_zoom;
            }
        }
        Setting::IntroVideo => {
            if change == Change::Toggle {
                g.intro_video = !g.intro_video;
            }
        }
        Setting::ZoomSpeed => {
            if let Some(v) = adjust(g.zoom_speed as i32, (25, 300, 25)) {
                g.zoom_speed = v as u32;
            }
        }
        Setting::NoticeScenario
        | Setting::NoticeFinance
        | Setting::NoticeTowns
        | Setting::NoticeCompetitors => {
            if change == Change::Toggle
                && let Some(cat) = s.notice_category()
            {
                c.notices.toggle(cat);
            }
        }
    }
    fx
}

/// Puts one tab's settings back to the defaults (the language and the install folder stay; the
/// key tab resets bindings through `Keys::reset`).
pub fn reset_tab(c: &mut Config, tab: Tab) -> Effects {
    let d = Config::default();
    let mut fx = Effects::default();
    match tab {
        Tab::Display => {
            c.display = d.display;
            c.ui_scale = None;
            fx.display = true;
            fx.ui_scale = Some(None);
        }
        Tab::Audio => {
            c.audio = d.audio;
            fx.audio = true;
        }
        Tab::Gameplay => c.gameplay = d.gameplay,
        Tab::Notices => c.notices = d.notices,
        Tab::Keys => c.keys = d.keys,
        Tab::Language => {}
    }
    fx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env {
            monitors: vec!["DP-1".into(), "HDMI-1".into()],
            ui_scale: 100,
            ui_auto: true,
        }
    }

    #[test]
    fn sliders_snap_clamp_and_step() {
        let mut c = Config::default();
        apply(&mut c, Setting::Master, Change::Set(63), &env());
        assert_eq!(c.audio.master, 65);
        apply(&mut c, Setting::Master, Change::Set(1000), &env());
        assert_eq!(c.audio.master, 100);
        apply(&mut c, Setting::Master, Change::Step(-3), &env());
        assert_eq!(c.audio.master, 85);
        apply(&mut c, Setting::Master, Change::Step(-100), &env());
        assert_eq!(c.audio.master, 0);
        // A toggle press on a slider does nothing.
        let fx = apply(&mut c, Setting::Music, Change::Toggle, &env());
        assert_eq!(fx, Effects::default());
        assert_eq!(c.audio.music, 100);
    }

    #[test]
    fn ui_scale_is_pinned_by_the_slider_and_released_by_auto() {
        let mut c = Config::default();
        let fx = apply(&mut c, Setting::UiScale, Change::Step(2), &env());
        assert_eq!(c.ui_scale, Some(150));
        assert_eq!(fx.ui_scale, Some(Some(150)));
        // Pinned now: the toggle goes back to the monitor's scale.
        let pinned = Env {
            ui_auto: false,
            ..env()
        };
        let fx = apply(&mut c, Setting::UiScaleAuto, Change::Toggle, &pinned);
        assert_eq!((c.ui_scale, fx.ui_scale), (None, Some(None)));
        // Switching auto off pins the scale in force now.
        let e = Env {
            ui_scale: 200,
            ..env()
        };
        apply(&mut c, Setting::UiScaleAuto, Change::Toggle, &e);
        assert_eq!(c.ui_scale, Some(200));
        // The slider stays within 100..=300.
        apply(&mut c, Setting::UiScale, Change::Set(900), &e);
        assert_eq!(c.ui_scale, Some(300));
    }

    #[test]
    fn display_changes_ask_the_window_loop() {
        let mut c = Config::default();
        let fx = apply(&mut c, Setting::WindowMode, Change::Pick(1), &env());
        assert!(fx.display);
        assert_eq!(c.display.mode, WindowMode::Borderless);
        let fx = apply(&mut c, Setting::Resolution, Change::Pick(4), &env());
        assert!(fx.display);
        assert_eq!((c.display.width, c.display.height), (1920, 1080));
        apply(&mut c, Setting::Monitor, Change::Pick(2), &env());
        assert_eq!(c.display.monitor, Some(1));
        apply(&mut c, Setting::Monitor, Change::Pick(0), &env());
        assert_eq!(c.display.monitor, None);
        // A monitor that is not there is ignored.
        apply(&mut c, Setting::Monitor, Change::Pick(9), &env());
        assert_eq!(c.display.monitor, None);
        assert!(apply(&mut c, Setting::Vsync, Change::Toggle, &env()).display);
        assert!(!c.display.vsync);
    }

    #[test]
    fn an_unusual_size_from_the_config_is_listed_and_selected() {
        let mut c = Config::default();
        c.display.width = 1500;
        c.display.height = 900;
        let t = Texts::none();
        let r = row(Setting::Resolution, &c, &env(), &t);
        let Control::Choice {
            options, selected, ..
        } = r.control
        else {
            panic!("not a choice");
        };
        assert_eq!(options[selected], "1500x900");
        assert!(options.contains(&"1920x1080".to_owned()));
    }

    #[test]
    fn the_size_row_is_disabled_when_borderless() {
        let mut c = Config::default();
        c.display.mode = WindowMode::Borderless;
        let t = Texts::none();
        let r = row(Setting::Resolution, &c, &env(), &t);
        assert!(matches!(r.control, Control::Choice { enabled: false, .. }));
    }

    #[test]
    fn gameplay_settings_edit_the_gameplay_table() {
        let mut c = Config::default();
        apply(&mut c, Setting::AutosaveMonths, Change::Set(0), &env());
        assert_eq!(c.gameplay.autosave_months, 0);
        apply(&mut c, Setting::AutosaveKeep, Change::Set(0), &env());
        assert_eq!(c.gameplay.autosave_keep, 1, "at least one autosave is kept");
        apply(&mut c, Setting::EdgeScroll, Change::Toggle, &env());
        assert!(c.gameplay.edge_scroll);
        apply(&mut c, Setting::DefaultSpeed, Change::Set(7), &env());
        assert_eq!(c.gameplay.default_speed, 7);
        apply(&mut c, Setting::ZoomSpeed, Change::Step(1), &env());
        assert_eq!(c.gameplay.zoom_speed, 125);
    }

    #[test]
    fn reset_tab_restores_only_that_tab() {
        let mut c = Config::default();
        c.audio.master = 10;
        c.gameplay.edge_scroll = true;
        c.language = Some("pt-BR".into());
        c.game_dir = Some("/x".into());
        let fx = reset_tab(&mut c, Tab::Audio);
        assert!(fx.audio);
        assert_eq!(c.audio, Config::default().audio);
        assert!(c.gameplay.edge_scroll);
        reset_tab(&mut c, Tab::Gameplay);
        assert!(!c.gameplay.edge_scroll);
        assert_eq!(c.language.as_deref(), Some("pt-BR"));
        assert_eq!(c.game_dir.as_deref(), Some(std::path::Path::new("/x")));
    }

    #[test]
    fn every_setting_has_a_unique_index_a_row_and_an_english_name() {
        let t = Texts::none();
        let c = Config::default();
        for (i, s) in Setting::ALL.into_iter().enumerate() {
            assert_eq!(s.index(), i as u32);
            assert_eq!(Setting::from_index(i as u32), Some(s));
            assert_eq!(row(s, &c, &env(), &t).setting, s);
        }
        let in_tabs: usize = Tab::ALL.iter().map(|t| t.settings().len()).sum();
        assert_eq!(in_tabs, Setting::ALL.len());
    }
}
