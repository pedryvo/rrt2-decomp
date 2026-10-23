//! The Options screen: a panel over the menu or the game with five tabs (Display, Audio, Language,
//! Keys, Gameplay), reachable from the main menu, File Options and the options key (F10). Every
//! change applies at once and is written to `config.toml` (docs/ui.md "Opcoes e saves"; the
//! original's Control Panel, `settings.win`, is the model for the tabs, docs/reference/screens.md
//! J3..J5).
//!
//! The model (rows, ranges, how a change edits the config) is `options.rs`; the key conflict
//! rules are `keys.rs`. This file draws the panel and turns clicks and key presses into changes.

use std::sync::Arc;

use rt2_ui::{
    Align, Anchor, Color, Fill, FontStyle, Input, Key, Kind, Layout, NodeId, UiEvent, UiScale,
    game_font,
};
use winit::keyboard::KeyCode;

use super::{AppState, Effect, Overlay};
use crate::config::WindowMode;
use crate::keys::{self, BindMode, KeyAction, Rebind};
use crate::options::{self, Change, Control, Env, Row, Setting, Tab};
use rt2_ui::Ui;

pub(super) const A_OPT_CLOSE: u32 = 8000;
pub(super) const A_OPT_RESET: u32 = 8001;
pub(super) const A_OPT_TAB: u32 = 8010;
pub(super) const A_OPT_TOGGLE: u32 = 8100;
pub(super) const A_OPT_DEC: u32 = 8200;
pub(super) const A_OPT_INC: u32 = 8300;
pub(super) const A_OPT_BAR: u32 = 8400;
/// `A_OPT_PICK + setting * 16 + option`.
pub(super) const A_OPT_PICK: u32 = 8500;
pub(super) const A_OPT_LANG: u32 = 8800;
pub(super) const A_OPT_KEYLIST: u32 = 8900;
pub(super) const A_OPT_REBIND: u32 = 8901;
pub(super) const A_OPT_ADDKEY: u32 = 8902;
pub(super) const A_OPT_KEYRESET: u32 = 8903;
/// Scroll buttons of a tab with more rows than the panel shows (up, down).
pub(super) const A_OPT_SCROLL: u32 = 8904;
const A_SCROLL_DOWN: u32 = 8905;
const PICK_SPAN: u32 = 16;
/// Whether `a` is an action of this screen.
pub(super) fn owns(a: u32) -> bool {
    (A_OPT_CLOSE..9000).contains(&a)
}

const DIM: Color = Color(0, 0, 0, 170);
const CREAM: Color = Color::rgb(0xf2, 0xe9, 0xd0);
const GOLD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const WARN: Color = Color::rgb(0xff, 0xb0, 0x60);
const BAR_ON: Color = Color::rgb(0xd8, 0xb0, 0x40);
const BAR_OFF: Color = Color::rgb(0x30, 0x2c, 0x18);
const BAR_CELLS: usize = 20;

/// A key being rebound.
#[derive(Clone, Debug)]
pub(super) struct Rebinding {
    pub action: KeyAction,
    pub mode: BindMode,
    /// A key another action uses, waiting for a second press to take it over.
    pub clash: Option<(String, Vec<KeyAction>)>,
}

pub(super) struct OptionsState {
    pub tab: Tab,
    /// Selected row of the key list.
    pub key_sel: usize,
    pub rebinding: Option<Rebinding>,
    /// First visible row of a tab of settings that does not fit the panel.
    pub scroll: usize,
}

impl OptionsState {
    pub fn new(tab: Tab) -> OptionsState {
        OptionsState {
            tab,
            key_sel: 0,
            rebinding: None,
            scroll: 0,
        }
    }
}

/// Smallest height of a row of settings, in logical units; a tab with more rows than fit scrolls.
const MIN_ROW_H: f32 = 26.0;

/// Rows a body of height `h` shows at the smallest row height.
fn rows_that_fit(h: f32) -> usize {
    ((h / MIN_ROW_H).floor() as usize).max(1)
}

/// The window of rows to draw: first row and count, with `scroll` clamped.
fn row_window(n: usize, fit: usize, scroll: usize) -> (usize, usize) {
    if n <= fit {
        (0, n)
    } else {
        (scroll.min(n - fit), fit)
    }
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

fn text(s: &str, font: FontStyle, color: Color, align: Align) -> Kind {
    Kind::Label {
        text: s.to_owned(),
        font,
        color,
        align,
        wrap: false,
    }
}

fn button(s: &str, font: FontStyle, action: u32) -> Kind {
    Kind::Button {
        text: s.to_owned(),
        font,
        action,
        enabled: true,
        hotkey: None,
    }
}

impl AppState {
    // ---- opening and closing ----------------------------------------------------------------

    /// Opens the Options screen on `tab` (replacing another overlay).
    pub fn open_options(&mut self, tab: Tab) {
        self.overlay = Some(Overlay::Options(OptionsState::new(tab)));
        self.rebuild_ui();
    }

    /// Opens the Options screen by tab name (screenshots): display, audio, language, keys,
    /// gameplay, keys-rebind, keys-conflict.
    pub fn show_options(&mut self, name: &str) -> bool {
        let tab = match name {
            "display" => Tab::Display,
            "audio" => Tab::Audio,
            "language" => Tab::Language,
            "keys" => Tab::Keys,
            "gameplay" => Tab::Gameplay,
            // The key tab while rebinding "Pause / resume", then after pressing Q (a clash).
            "keys-rebind" | "keys-conflict" => {
                self.open_options(Tab::Keys);
                self.options_activated(A_OPT_REBIND);
                if name == "keys-conflict" {
                    self.options_key(KeyCode::KeyQ, false);
                }
                return true;
            }
            _ => return false,
        };
        self.open_options(tab);
        true
    }

    pub(super) fn close_overlay(&mut self) {
        self.overlay = None;
        self.rebuild_ui();
    }

    pub(super) fn options_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Options(_)))
    }

    /// The monitors the window loop found (names), for the monitor choice.
    pub fn set_monitors(&mut self, names: Vec<String>) {
        self.monitors = names;
    }

    /// The configuration as it stands.
    #[cfg(test)]
    pub fn config(&self) -> &crate::config::Config {
        &self.config
    }

    fn env(&self) -> Env {
        Env {
            monitors: self.monitors.clone(),
            ui_scale: self.scale.percent(),
            ui_auto: !self.scale_pinned,
        }
    }

    /// Asks the window loop to write the config (the last request of a batch wins).
    pub(super) fn save_config(&mut self) {
        self.effects.retain(|e| !matches!(e, Effect::SaveConfig(_)));
        self.effects
            .push(Effect::SaveConfig(Box::new(self.config.clone())));
    }

    /// Side effects of a change to the config.
    fn config_changed(&mut self, fx: options::Effects) {
        if fx.audio {
            self.cues
                .push(crate::sound::Cue::Settings(self.config.audio.clone()));
        }
        if fx.display {
            self.effects
                .push(Effect::ApplyDisplay(self.config.display.clone()));
        }
        match fx.ui_scale {
            Some(Some(p)) => {
                self.scale_pinned = true;
                self.set_ui_scale(UiScale::from_percent(p), true);
            }
            Some(None) => {
                self.scale_pinned = false;
                self.set_ui_scale(UiScale::from_dpi(self.dpi), false);
            }
            None => {}
        }
        self.save_config();
        self.rebuild_ui();
    }

    fn change_setting(&mut self, setting: Setting, change: Change) {
        let env = self.env();
        let fx = options::apply(&mut self.config, setting, change, &env);
        self.config_changed(fx);
    }

    /// F11 and friends: the window mode, kept in the config.
    pub(super) fn toggle_fullscreen(&mut self) {
        self.config.display.mode = match self.config.display.mode {
            WindowMode::Windowed => WindowMode::Borderless,
            WindowMode::Borderless => WindowMode::Windowed,
        };
        self.effects
            .push(Effect::ApplyDisplay(self.config.display.clone()));
        self.save_config();
        self.rebuild_ui();
    }

    /// The mute key: flips the setting, so the Options screen shows the same state.
    pub(super) fn toggle_mute(&mut self) {
        self.config.audio.mute = !self.config.audio.mute;
        self.cues
            .push(crate::sound::Cue::Settings(self.config.audio.clone()));
        self.save_config();
    }

    /// Ctrl +/- (or the bound keys): a new UI scale, remembered.
    pub(super) fn step_ui_scale(&mut self, steps: i32) {
        let s = self.scale.step(steps);
        self.config.ui_scale = Some(s.percent());
        self.set_ui_scale(s, true);
        self.save_config();
        if self.overlay.is_some() {
            self.rebuild_ui();
        }
    }

    /// Picks a language: the whole interface is laid out again and the choice is remembered.
    pub(super) fn choose_language(&mut self, locale: &str) {
        self.set_language(locale);
        self.config.language = Some(locale.to_owned());
        self.save_config();
    }

    // ---- input ------------------------------------------------------------------------------

    /// A key press while the Options screen is up; always consumed.
    pub(super) fn options_key(&mut self, code: KeyCode, repeat: bool) {
        let ctrl = self.ctrl;
        let Some(Overlay::Options(o)) = &mut self.overlay else {
            return;
        };
        if let Some(rb) = o.rebinding.clone() {
            if repeat {
                return;
            }
            self.rebind_key(rb, code, ctrl);
            return;
        }
        let tab = o.tab;
        let b = self.binds.clone();
        if !repeat && (code == KeyCode::Escape || b.options.matches(code, ctrl)) {
            self.close_overlay();
            return;
        }
        let k = match code {
            KeyCode::Tab => Some(if self.shift { Key::BackTab } else { Key::Tab }),
            KeyCode::Enter | KeyCode::NumpadEnter => Some(Key::Enter),
            KeyCode::Space => Some(Key::Space),
            KeyCode::ArrowUp => Some(Key::Up),
            KeyCode::ArrowDown => Some(Key::Down),
            KeyCode::PageUp => Some(Key::PageUp),
            KeyCode::PageDown => Some(Key::PageDown),
            KeyCode::Home => Some(Key::Home),
            KeyCode::End => Some(Key::End),
            // Left / Right switch tabs when no list needs them.
            KeyCode::ArrowLeft | KeyCode::ArrowRight if !repeat => {
                let i = Tab::ALL.iter().position(|t| *t == tab).unwrap_or(0) as i32;
                let n = Tab::ALL.len() as i32;
                let j = (i + if code == KeyCode::ArrowRight { 1 } else { -1 }).rem_euclid(n);
                if let Some(Overlay::Options(o)) = &mut self.overlay {
                    o.tab = Tab::ALL[j as usize];
                }
                self.rebuild_ui();
                None
            }
            _ => None,
        };
        if let Some(k) = k {
            let ev = self.ui.input(Input::KeyDown(k));
            self.handle_events(ev);
        }
    }

    /// The key captured while rebinding an action.
    fn rebind_key(&mut self, rb: Rebinding, code: KeyCode, ctrl: bool) {
        let set = |s: &mut AppState, r: Option<Rebinding>| {
            if let Some(Overlay::Options(o)) = &mut s.overlay {
                o.rebinding = r;
            }
            s.rebuild_ui();
        };
        if code == KeyCode::Escape {
            set(self, None);
            return;
        }
        let Some(spec) = keys::spec_string(code, ctrl) else {
            // A lone modifier or a key we have no name for: keep waiting.
            return;
        };
        // A second press of the key that clashed takes it over.
        let steal = rb
            .clash
            .as_ref()
            .is_some_and(|(s, _)| keys::parse_spec(s) == keys::parse_spec(&spec));
        match self.config.keys.rebind(rb.action, &spec, rb.mode, steal) {
            Rebind::Done => {
                self.keys_changed();
                set(self, None);
            }
            Rebind::Conflict(with) => set(
                self,
                Some(Rebinding {
                    clash: Some((spec, with)),
                    ..rb
                }),
            ),
            Rebind::Invalid => {}
        }
    }

    /// The bindings in the config changed: rebuild the lookup and write the file.
    fn keys_changed(&mut self) {
        self.binds = crate::keys::Bindings::from_config(&self.config.keys);
        self.save_config();
    }

    /// Scrolls the rows of a tab of settings by `d` rows (the buttons and the wheel).
    pub(super) fn options_scroll(&mut self, d: i32) {
        let Some(Overlay::Options(o)) = &mut self.overlay else {
            return;
        };
        if matches!(o.tab, Tab::Keys | Tab::Language) {
            return;
        }
        o.scroll = (o.scroll as i32 + d).max(0) as usize;
        self.rebuild_ui();
    }

    pub(super) fn options_activated(&mut self, a: u32) -> bool {
        let Some(Overlay::Options(o)) = &mut self.overlay else {
            return false;
        };
        let tab = o.tab;
        match a {
            A_OPT_CLOSE => self.close_overlay(),
            A_OPT_RESET => {
                if tab == Tab::Keys {
                    self.config.keys = crate::config::Keys::default();
                    self.keys_changed();
                    self.rebuild_ui();
                } else {
                    let fx = options::reset_tab(&mut self.config, tab);
                    self.config_changed(fx);
                }
            }
            a if (A_OPT_TAB..A_OPT_TAB + 5).contains(&a) => {
                o.tab = Tab::ALL[(a - A_OPT_TAB) as usize];
                o.rebinding = None;
                o.scroll = 0;
                self.rebuild_ui();
            }
            A_OPT_SCROLL | A_SCROLL_DOWN => {
                self.options_scroll(if a == A_OPT_SCROLL { -1 } else { 1 });
            }
            a if (A_OPT_TOGGLE..A_OPT_TOGGLE + 64).contains(&a) => {
                if let Some(s) = Setting::from_index(a - A_OPT_TOGGLE) {
                    self.change_setting(s, Change::Toggle);
                }
            }
            a if (A_OPT_DEC..A_OPT_DEC + 64).contains(&a) => {
                if let Some(s) = Setting::from_index(a - A_OPT_DEC) {
                    self.change_setting(s, Change::Step(-1));
                }
            }
            a if (A_OPT_INC..A_OPT_INC + 64).contains(&a) => {
                if let Some(s) = Setting::from_index(a - A_OPT_INC) {
                    self.change_setting(s, Change::Step(1));
                }
            }
            a if (A_OPT_PICK..A_OPT_PICK + Setting::ALL.len() as u32 * PICK_SPAN).contains(&a) => {
                let n = a - A_OPT_PICK;
                if let Some(s) = Setting::from_index(n / PICK_SPAN) {
                    self.change_setting(s, Change::Pick((n % PICK_SPAN) as usize));
                }
            }
            a if (A_OPT_LANG..A_OPT_LANG + 32).contains(&a) => {
                if let Some((locale, _)) = self.texts.languages().get((a - A_OPT_LANG) as usize) {
                    let locale = locale.clone();
                    self.choose_language(&locale);
                }
            }
            A_OPT_REBIND | A_OPT_ADDKEY => {
                let action = KeyAction::ALL[o.key_sel.min(KeyAction::ALL.len() - 1)];
                o.rebinding = Some(Rebinding {
                    action,
                    mode: if a == A_OPT_ADDKEY {
                        BindMode::Add
                    } else {
                        BindMode::Replace
                    },
                    clash: None,
                });
                self.rebuild_ui();
            }
            A_OPT_KEYRESET => {
                let action = KeyAction::ALL[o.key_sel.min(KeyAction::ALL.len() - 1)];
                self.config.keys.reset(action);
                self.keys_changed();
                self.rebuild_ui();
            }
            A_OPT_KEYLIST => {
                // Enter or a double click on a row: rebind it.
                self.options_activated(A_OPT_REBIND);
            }
            _ => return false,
        }
        true
    }

    /// A list selection or a press on a slider bar.
    pub(super) fn options_event(&mut self, e: &UiEvent) -> bool {
        match *e {
            UiEvent::Selected(A_OPT_KEYLIST, row) => {
                if let Some(Overlay::Options(o)) = &mut self.overlay {
                    o.key_sel = row;
                    o.rebinding = None;
                }
                self.rebuild_ui();
                true
            }
            UiEvent::Pointer(a, fx, _) if (A_OPT_BAR..A_OPT_BAR + 64).contains(&a) => {
                if let Some(s) = Setting::from_index(a - A_OPT_BAR) {
                    let rows =
                        options::rows(self.options_tab(), &self.config, &self.env(), &self.texts);
                    if let Some(Row {
                        control: Control::Slider { min, max, .. },
                        ..
                    }) = rows.into_iter().find(|r| r.setting == s)
                    {
                        let v = min as f32 + fx.clamp(0.0, 1.0) * (max - min) as f32;
                        self.change_setting(s, Change::Set(v.round() as i32));
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn options_tab(&self) -> Tab {
        match &self.overlay {
            Some(Overlay::Options(o)) => o.tab,
            _ => Tab::Display,
        }
    }

    // ---- building ---------------------------------------------------------------------------

    pub(super) fn build_options(&self, ui: &mut Ui) {
        let Some(Overlay::Options(o)) = &self.overlay else {
            return;
        };
        let t = &self.texts;
        let (cw, ch) = ui.logical_size();
        ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(DIM),
            },
            Layout::fill(),
        );
        let (pw, ph) = ((cw - 16.0).min(800.0), (ch - 16.0).min(620.0));
        let panel = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, pw, ph),
        );
        let pad = 18.0;
        let inner_w = pw - 2.0 * pad;
        // Everything scales with the room there is.
        let u = (ph / 560.0).clamp(0.62, 1.15);
        let title_h = 34.0 * u;
        ui.add(
            panel,
            text(
                &t.a("Options"),
                game_font(3)
                    .with_size((game_font(3).size * u).round().max(12.0))
                    .with_shadow(),
                CREAM,
                Align::Center,
            ),
            at(pad, pad * 0.7, inner_w, title_h),
        );
        // Tabs.
        let tab_y = pad * 0.7 + title_h + 6.0 * u;
        let tab_h = 34.0 * u;
        let tab_w = (inner_w / Tab::ALL.len() as f32 - 6.0).min(150.0);
        let tab_font = game_font(2).with_size((game_font(2).size * u.max(0.8)).round().max(10.0));
        for (i, tab) in Tab::ALL.iter().enumerate() {
            ui.add(
                panel,
                Kind::Toggle {
                    text: t.a(tab.english()),
                    font: tab_font,
                    action: A_OPT_TAB + i as u32,
                    on: *tab == o.tab,
                    enabled: true,
                },
                at(
                    pad + i as f32 * (tab_w + 6.0)
                        + (inner_w - Tab::ALL.len() as f32 * (tab_w + 6.0) + 6.0) / 2.0,
                    tab_y,
                    tab_w,
                    tab_h,
                ),
            );
        }
        // Footer.
        let foot_h = 36.0 * u;
        let foot_y = ph - pad * 0.8 - foot_h;
        let body_y = tab_y + tab_h + 12.0 * u;
        let body_h = foot_y - body_y - 8.0 * u;
        let fbtn = game_font(2).with_size((game_font(2).size * u.max(0.8)).round().max(10.0));
        let reset_text = if o.tab == Tab::Keys {
            t.a("Reset all keys")
        } else {
            t.a("Reset this tab")
        };
        if o.tab != Tab::Language {
            ui.add(
                panel,
                button(&reset_text, fbtn, A_OPT_RESET),
                at(pad, foot_y, 190.0 * u.max(0.85), foot_h),
            );
        }
        ui.add(
            panel,
            button(&t.a("Close"), fbtn, A_OPT_CLOSE),
            at(
                pw - pad - 140.0 * u.max(0.85),
                foot_y,
                140.0 * u.max(0.85),
                foot_h,
            ),
        );
        let body = Body {
            x: pad,
            y: body_y,
            w: inner_w,
            h: body_h,
            u,
        };
        match o.tab {
            Tab::Language => self.build_language(ui, panel, &body),
            Tab::Keys => self.build_keys(ui, panel, &body, o),
            tab => self.build_rows(ui, panel, &body, tab, o.scroll),
        }
    }

    fn build_rows(&self, ui: &mut Ui, panel: NodeId, b: &Body, tab: Tab, scroll: usize) {
        let t = &self.texts;
        let rows = options::rows(tab, &self.config, &self.env(), t);
        // More rows than fit (a small canvas, the 300 % scale on 1080p): show a window of them
        // with scroll buttons and the wheel instead of running over the footer.
        let (first, shown) = row_window(rows.len(), rows_that_fit(b.h), scroll);
        let scrolls = shown < rows.len();
        let n = shown.max(1) as f32;
        let row_h = (b.h / n).clamp(MIN_ROW_H, 46.0 * b.u.max(0.8));
        let fs = (row_h * 0.46).clamp(11.0, 20.0);
        let label_font = game_font(8).with_size(fs.round());
        let btn_font = game_font(2).with_size((fs * 0.9).round().max(10.0));
        let label_w = b.w * 0.40;
        let cx = b.x + label_w + 8.0;
        let gutter = if scrolls { 34.0 } else { 0.0 };
        let cw = b.w - label_w - 8.0 - gutter;
        if scrolls {
            let bw = 28.0;
            let bx = b.x + b.w - bw;
            ui.add(
                panel,
                button("^", btn_font, A_OPT_SCROLL),
                at(bx, b.y, bw, 28.0),
            );
            ui.add(
                panel,
                button("v", btn_font, A_SCROLL_DOWN),
                at(bx, b.y + b.h - 28.0, bw, 28.0),
            );
            ui.add(
                panel,
                text(
                    &format!("{}-{}/{}", first + 1, first + shown, rows.len()),
                    btn_font,
                    MUTED,
                    Align::Center,
                ),
                at(bx - 8.0, b.y + b.h / 2.0 - 10.0, bw + 16.0, 20.0),
            );
        }
        for (i, r) in rows.iter().enumerate().skip(first).take(shown) {
            let y = b.y + (i - first) as f32 * row_h;
            let h = row_h - 6.0;
            let idx = r.setting.index();
            let enabled = !matches!(r.control, Control::Choice { enabled: false, .. });
            ui.add(
                panel,
                text(
                    &r.label,
                    label_font.with_shadow(),
                    if enabled { CREAM } else { MUTED },
                    Align::Left,
                ),
                at(b.x, y, label_w, h),
            );
            match &r.control {
                Control::Toggle(on) => {
                    ui.add(
                        panel,
                        Kind::Toggle {
                            text: if *on { t.a("On") } else { t.a("Off") },
                            font: btn_font,
                            action: A_OPT_TOGGLE + idx,
                            on: *on,
                            enabled: true,
                        },
                        at(cx, y, 120.0, h),
                    );
                }
                Control::Choice {
                    options,
                    selected,
                    enabled,
                } => {
                    let gap = 6.0;
                    let n = options.len().max(1) as f32;
                    let bw = ((cw - gap * (n - 1.0)) / n).min(190.0);
                    // Long names (monitors) get a smaller font to fit.
                    let longest =
                        options.iter().map(|s| s.chars().count()).max().unwrap_or(1) as f32;
                    // Many short options (window sizes) read better in the plain face.
                    let f = if options.len() >= 5 {
                        game_font(8)
                            .with_size(fs.min((bw - 8.0) / (longest * 0.6)).max(10.0).round())
                    } else {
                        btn_font.with_size(
                            btn_font
                                .size
                                .min(((bw - 12.0) / (longest * 0.72)).max(9.0))
                                .round(),
                        )
                    };
                    for (j, o) in options.iter().enumerate() {
                        ui.add(
                            panel,
                            Kind::Toggle {
                                text: o.clone(),
                                font: f,
                                action: A_OPT_PICK + idx * PICK_SPAN + j as u32,
                                on: j == *selected,
                                enabled: *enabled,
                            },
                            at(cx + j as f32 * (bw + gap), y, bw, h),
                        );
                    }
                }
                Control::Slider {
                    value,
                    min,
                    max,
                    text: value_text,
                    ..
                } => {
                    let bw = h.max(28.0);
                    let val_w = 120.0;
                    ui.add(
                        panel,
                        button("-", btn_font, A_OPT_DEC + idx),
                        at(cx, y, bw, h),
                    );
                    let bar_x = cx + bw + 6.0;
                    let bar_w = (cw - 2.0 * bw - val_w - 18.0).max(40.0);
                    let filled = (((value - min) as f32 / (max - min).max(1) as f32)
                        * BAR_CELLS as f32)
                        .round() as usize;
                    let colors: Vec<Color> = (0..BAR_CELLS)
                        .map(|c| if c < filled { BAR_ON } else { BAR_OFF })
                        .collect();
                    ui.add(
                        panel,
                        Kind::Cells {
                            cols: BAR_CELLS,
                            rows: 1,
                            colors: Arc::new(colors),
                            action: A_OPT_BAR + idx,
                        },
                        at(bar_x, y + 4.0, bar_w, h - 8.0),
                    );
                    ui.add(
                        panel,
                        button("+", btn_font, A_OPT_INC + idx),
                        at(bar_x + bar_w + 6.0, y, bw, h),
                    );
                    ui.add(
                        panel,
                        text(value_text, label_font.with_shadow(), GOLD, Align::Left),
                        at(bar_x + bar_w + 6.0 + bw + 8.0, y, val_w, h),
                    );
                }
            }
        }
        // A hint under the rows of the display tab.
        if tab == Tab::Display {
            let hint_y = b.y + shown as f32 * row_h + 4.0;
            if hint_y + 18.0 < b.y + b.h + 4.0 {
                ui.add(
                    panel,
                    text(
                        &t.a("Window size does not apply in borderless fullscreen, which uses the monitor's size."),
                        label_font.with_size((fs * 0.8).round().max(10.0)),
                        MUTED,
                        Align::Left,
                    ),
                    at(b.x, hint_y, b.w, 20.0),
                );
            }
        }
    }

    fn build_language(&self, ui: &mut Ui, panel: NodeId, b: &Body) {
        let t = &self.texts;
        let langs = t.languages();
        let fs = (20.0 * b.u).round().max(11.0);
        ui.add(
            panel,
            text(
                &t.a("The interface changes language at once."),
                game_font(8).with_size(fs).with_shadow(),
                MUTED,
                Align::Left,
            ),
            at(b.x, b.y, b.w, 28.0 * b.u),
        );
        let cols = if b.w > 560.0 { 2 } else { 1 };
        let bw = (b.w - 8.0) / cols as f32;
        let bh = 36.0 * b.u.max(0.8);
        let f = game_font(2).with_size((fs * 0.95).round().max(10.0));
        for (i, (locale, name)) in langs.iter().enumerate() {
            let (c, r) = (i % cols, i / cols);
            ui.add(
                panel,
                Kind::Toggle {
                    text: name.clone(),
                    font: f,
                    action: A_OPT_LANG + i as u32,
                    on: locale == t.locale(),
                    enabled: true,
                },
                at(
                    b.x + c as f32 * (bw + 8.0),
                    b.y + 36.0 * b.u + r as f32 * (bh + 6.0),
                    bw - 8.0,
                    bh,
                ),
            );
        }
    }

    fn build_keys(&self, ui: &mut Ui, panel: NodeId, b: &Body, o: &OptionsState) {
        let t = &self.texts;
        let fs = (17.0 * b.u).round().max(11.0);
        let keys = &self.config.keys;
        let items: Vec<String> = KeyAction::ALL
            .iter()
            .map(|&a| {
                let names: Vec<String> =
                    keys.get(a).iter().map(|s| keys::display_spec(s)).collect();
                let bound = if names.is_empty() {
                    t.a("(none)")
                } else {
                    names.join(", ")
                };
                format!("{}:  {}", t.a(a.english()), bound)
            })
            .collect();
        let bar_h = 36.0 * b.u.max(0.8);
        let msg_h = 40.0 * b.u;
        let list_h = (b.h - bar_h - msg_h - 12.0).max(60.0);
        let list = ui.add(
            panel,
            Kind::List {
                items,
                selected: Some(o.key_sel.min(KeyAction::ALL.len() - 1)),
                scroll: o.key_sel.saturating_sub(4),
                row_h: (fs * 1.55).round(),
                font: game_font(8).with_size(fs),
                action: A_OPT_KEYLIST,
            },
            at(b.x, b.y, b.w, list_h),
        );
        if o.rebinding.is_none() {
            ui.set_focus(Some(list));
        }
        // What the next key press will do.
        let (msg, color) = match &o.rebinding {
            None => (
                t.a("Select an action, then Rebind (Enter) to replace its keys or Add key for another."),
                MUTED,
            ),
            Some(Rebinding { action, clash: None, .. }) => (
                t.af("Press the new key for \"%1\" (Esc cancels).", &[t.a(action.english())]),
                GOLD,
            ),
            Some(Rebinding {
                action,
                clash: Some((spec, with)),
                ..
            }) => {
                let names = with
                    .iter()
                    .map(|a| t.a(a.english()))
                    .collect::<Vec<_>>()
                    .join(", ");
                (
                    t.af(
                        "%1 is already used by \"%2\". Press it again to move it to \"%3\", or press another key (Esc cancels).",
                        &[keys::display_spec(spec), names, t.a(action.english())],
                    ),
                    WARN,
                )
            }
        };
        ui.add(
            panel,
            Kind::Label {
                text: msg,
                font: game_font(8).with_size((fs * 0.9).round()).with_shadow(),
                color,
                align: Align::Left,
                wrap: true,
            },
            at(b.x, b.y + list_h + 4.0, b.w, msg_h),
        );
        let by = b.y + list_h + msg_h + 8.0;
        let bw = ((b.w - 12.0) / 3.0).min(220.0);
        let f = game_font(2).with_size((game_font(2).size * b.u.max(0.8)).round().max(10.0));
        for (i, (label, action)) in [
            (t.a("Rebind"), A_OPT_REBIND),
            (t.a("Add key"), A_OPT_ADDKEY),
            (t.a("Reset this key"), A_OPT_KEYRESET),
        ]
        .into_iter()
        .enumerate()
        {
            ui.add(
                panel,
                Kind::Button {
                    text: label,
                    font: f,
                    action,
                    enabled: o.rebinding.is_none(),
                    hotkey: None,
                },
                at(b.x + i as f32 * (bw + 6.0), by, bw, bar_h),
            );
        }
    }
}

/// The body area of the panel under the tabs.
struct Body {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    u: f32,
}

impl AppState {
    /// Builds whichever overlay is open (called last by `rebuild_ui`).
    pub(super) fn build_overlay_screen(&mut self, ui: &mut Ui) {
        match &self.overlay {
            Some(Overlay::Options(_)) => self.build_options(ui),
            Some(Overlay::Saves(_)) => self.build_saves(ui),
            None => {}
        }
    }
}
