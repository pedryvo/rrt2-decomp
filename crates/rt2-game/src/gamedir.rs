//! The game-folder screen (docs/ui.md "Tela de pasta do jogo"): shown on the first run, when the
//! configured install is gone, and from Options ("Game folder..."). It lists the installs found on
//! the machine ([`crate::install_detect`]), lets the player pick a folder with the native chooser,
//! says what a folder lacks, remembers the choice in `config.toml` and starts the game.
//!
//! It must work before any original asset exists, so it draws only with the vendored fonts and
//! the project logo, on a flat skin of its own; it is a small state next to [`crate::state`]'s
//! `AppState`, with the same input methods, driven by the same window loop.

use std::path::PathBuf;

use rt2_ui::{
    Align, Anchor, Color, Fill, Fit, Input, Key, Kind, Layout, MouseButton, NodeId, Script,
    SkinBuilder, SkinImage, SpriteId, Theme, Ui, UiEvent, UiScale, game_font,
};
use winit::keyboard::KeyCode;

use crate::config::Config;
use crate::install_detect::{self, Found, Roots, Source};
use crate::locales;
use crate::paths;
use crate::text::Texts;

const A_LIST: u32 = 1;
const A_BROWSE: u32 = 2;
const A_PLAY: u32 = 3;
const A_LEAVE: u32 = 4;

/// Logical size of the logo (a square) at most; smaller windows shrink it.
const LOGO_MAX: f32 = 150.0;

/// Cream text, steel and brass: the logo's own palette (docs/assets/logo.svg).
const BG: Color = Color::rgb(0x10, 0x16, 0x1e);
const OK: Color = Color::rgb(0xa9, 0xd3, 0x93);
const BAD: Color = Color::rgb(0xea, 0x95, 0x80);
const DIM: Color = Color::rgb(0xb9, 0xc0, 0xc8);

fn theme() -> Theme {
    Theme {
        text: Color::rgb(0xf0, 0xe6, 0xc8),
        text_dim: DIM,
        text_disabled: Color::rgb(0x6f, 0x78, 0x82),
        text_on_button: Color::rgb(0x14, 0x1c, 0x26),
        panel_fill: Color(0x1b, 0x24, 0x2f, 0xf2),
        panel_border: Color::rgb(0x8a, 0x96, 0xa3),
        button_fill: Color::rgb(0x2d, 0x39, 0x47),
        button_border: Color::rgb(0xc9, 0xa2, 0x27),
        accent: Color::rgb(0xc9, 0xa2, 0x27),
        row_selected: Color(0xc9, 0xa2, 0x27, 0x66),
        row_hover: Color(0xe8, 0xd9, 0xb5, 0x22),
        shadow: Color(0, 0, 0, 0xb0),
    }
}

/// The renderer's skin for this screen: the flat theme plus the logo rasterized for the scale.
/// The window loop gives it to a [`rt2_ui::UiRenderer`] and uploads nothing else: the logo is
/// the only image and is already in the skin.
pub struct Art {
    pub skin: rt2_ui::Skin,
    pub sprite: SpriteId,
}

fn make_art(px: u32) -> Art {
    let (size, rgba) = match crate::logo::rasterize(px) {
        Some(r) => (r.size, r.rgba),
        None => (16, vec![0; 16 * 16 * 4]),
    };
    let mut b = SkinBuilder::new();
    let sprite = b.add(SkinImage {
        width: size,
        height: size,
        rgba,
        bake: 1,
    });
    Art {
        skin: b.build(theme()),
        sprite,
    }
}

/// What the screen asks the window loop to do.
#[derive(Debug)]
pub enum Effect {
    /// Open the native folder chooser (off the UI thread); the answer comes back to
    /// [`GameDirScreen::picked`].
    Browse {
        start: Option<PathBuf>,
        title: String,
    },
    /// The folder is saved in the configuration: start the game with it.
    Play { path: PathBuf, config: Box<Config> },
    /// Quit (first run) or go back to what was open (from Options).
    Leave,
}

enum Status {
    Hint,
    Browsing,
    Valid,
    Missing(Vec<&'static str>),
    Error(String),
}

pub struct Params {
    pub config: Config,
    pub config_path: Option<PathBuf>,
    /// From Options: Esc / Back returns to the game instead of quitting.
    pub can_cancel: bool,
    /// What to search (default: this machine).
    pub roots: Option<Roots>,
}

pub struct GameDirScreen {
    pub ui: Ui,
    texts: Texts,
    config: Config,
    config_path: Option<PathBuf>,
    can_cancel: bool,
    viewport: (u32, u32),
    scale: UiScale,
    found: Vec<Found>,
    selected: Option<usize>,
    status: Status,
    effects: Vec<Effect>,
    art: Option<Art>,
    logo_px: u32,
    sprite: Option<SpriteId>,
    nodes: Nodes,
}

#[derive(Default)]
struct Nodes {
    list: Option<NodeId>,
    message: Option<NodeId>,
    play: Option<NodeId>,
    browse: Option<NodeId>,
}

impl GameDirScreen {
    pub fn new(params: Params, viewport: (u32, u32), dpi: f64) -> GameDirScreen {
        let Params {
            config,
            config_path,
            can_cancel,
            roots,
        } = params;
        let locale = config
            .language
            .clone()
            .or_else(locales::system_locale)
            .unwrap_or_else(|| locales::ENGLISH.to_owned());
        let texts = Texts::from_parts(
            None,
            locales::discover(locales::override_dir().as_deref()),
            &locale,
        );
        let roots = roots.unwrap_or_else(|| Roots::system(config.game_dir.clone()));
        let found = install_detect::detect(&roots);
        let scale = config
            .ui_scale
            .map_or_else(|| UiScale::from_dpi(dpi), UiScale::from_percent);
        let mut s = GameDirScreen {
            ui: Ui::new(scale),
            texts,
            config,
            config_path,
            can_cancel,
            viewport: (viewport.0.max(1), viewport.1.max(1)),
            scale,
            found,
            selected: None,
            status: Status::Hint,
            effects: Vec::new(),
            art: None,
            logo_px: 0,
            sprite: None,
            nodes: Nodes::default(),
        };
        // The configured folder when it works, else the first usable find.
        if let Some(i) = s.found.iter().position(Found::valid) {
            s.select(i);
        }
        s.build();
        s
    }

    /// The writing system of the language, for the font faces.
    pub fn script(&self) -> Script {
        Script::of_locale(self.texts.locale())
    }

    /// A new skin for the renderer when the logo size changed (the first frame, a new scale).
    pub fn take_art(&mut self) -> Option<Art> {
        self.art.take()
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    #[cfg(test)]
    pub fn found(&self) -> &[Found] {
        &self.found
    }

    #[cfg(test)]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The folder the Play button would use, if it is usable.
    pub fn playable(&self) -> Option<&PathBuf> {
        self.selected
            .and_then(|i| self.found.get(i))
            .filter(|f| f.valid())
            .map(|f| &f.path)
    }

    pub fn is_browsing(&self) -> bool {
        matches!(self.status, Status::Browsing)
    }

    // ---- model ----------------------------------------------------------------------------

    fn select(&mut self, i: usize) {
        let Some(f) = self.found.get(i) else {
            return;
        };
        self.selected = Some(i);
        self.status = if f.valid() {
            Status::Valid
        } else {
            Status::Missing(f.missing.clone())
        };
    }

    /// The answer of the folder chooser (`None`: cancelled).
    pub fn picked(&mut self, path: Option<PathBuf>) {
        if !self.is_browsing() {
            return;
        }
        self.status = Status::Hint;
        if let Some(p) = path {
            let p = p.canonicalize().unwrap_or(p);
            // A folder that holds the install in a child (the player picked `Program Files`).
            let p = descend(&p).unwrap_or(p);
            let key = |x: &PathBuf| x.canonicalize().unwrap_or_else(|_| x.clone());
            let at = self.found.iter().position(|f| key(&f.path) == key(&p));
            let i = at.unwrap_or_else(|| {
                self.found.insert(0, Found::new(p, Source::Picked));
                0
            });
            self.select(i);
        } else if let Some(i) = self.selected {
            self.select(i);
        }
        self.refresh();
    }

    /// The window loop could not load the game from the folder.
    pub fn failed(&mut self, error: &str) {
        self.status = Status::Error(self.texts.af(
            "Cannot load the game from this folder: %1",
            &[error.to_owned()],
        ));
        self.refresh();
    }

    fn start_browse(&mut self) {
        if self.is_browsing() {
            return;
        }
        let start = self
            .selected
            .and_then(|i| self.found.get(i))
            .map(|f| f.path.clone())
            .or_else(|| self.config.game_dir.clone())
            .or_else(dirs::home_dir);
        self.status = Status::Browsing;
        self.effects.push(Effect::Browse {
            start,
            title: self.texts.a("Select the Railroad Tycoon II folder"),
        });
        self.refresh();
    }

    fn play(&mut self) {
        let Some(path) = self.playable().cloned() else {
            return;
        };
        let mut config = self.config.clone();
        config.game_dir = Some(path.clone());
        if let Some(file) = &self.config_path
            && let Err(e) = config.save(file)
        {
            self.status = Status::Error(
                self.texts
                    .af("Cannot save the choice: %1", &[format!("{e:#}")]),
            );
            self.refresh();
            return;
        }
        self.config = config.clone();
        self.effects.push(Effect::Play {
            path,
            config: Box::new(config),
        });
    }

    fn leave(&mut self) {
        self.effects.push(Effect::Leave);
    }

    // ---- window plumbing ------------------------------------------------------------------

    pub fn resize(&mut self, viewport: (u32, u32)) {
        let v = (viewport.0.max(1), viewport.1.max(1));
        if v != self.viewport {
            self.viewport = v;
            self.build();
        }
    }

    pub fn dpi_changed(&mut self, dpi: f64) {
        if self.config.ui_scale.is_none() {
            let s = UiScale::from_dpi(dpi);
            if s != self.scale {
                self.scale = s;
                self.build();
            }
        }
    }

    pub fn update(&mut self, _dt_ms: f32) {}

    pub fn cursor_moved(&mut self, x: f32, y: f32) {
        let ev = self.ui.input(Input::CursorMoved { x, y });
        self.handle(ev);
    }

    pub fn cursor_left(&mut self) {
        self.ui.input(Input::CursorLeft);
    }

    pub fn mouse_button(&mut self, left: bool, pressed: bool) {
        let b = if left {
            MouseButton::Left
        } else {
            MouseButton::Other
        };
        let ev = self.ui.input(if pressed {
            Input::MouseDown(b)
        } else {
            Input::MouseUp(b)
        });
        self.handle(ev);
    }

    pub fn wheel(&mut self, notches: f32) {
        self.ui.input(Input::Wheel(notches));
    }

    pub fn key(&mut self, code: KeyCode, pressed: bool, repeat: bool, shift: bool) {
        if !pressed {
            return;
        }
        let k = match code {
            KeyCode::Escape if !repeat => {
                self.leave();
                return;
            }
            KeyCode::Tab if shift => Key::BackTab,
            KeyCode::Tab => Key::Tab,
            KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
            KeyCode::Space => Key::Space,
            KeyCode::ArrowUp => Key::Up,
            KeyCode::ArrowDown => Key::Down,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            _ => return,
        };
        let ev = self.ui.input(Input::KeyDown(k));
        self.handle(ev);
    }

    fn handle(&mut self, events: Vec<UiEvent>) {
        for e in events {
            match e {
                UiEvent::Selected(A_LIST, i) => {
                    self.select(i);
                    self.refresh();
                }
                // A second click or Enter on the selected row.
                UiEvent::Activated(A_LIST) | UiEvent::Activated(A_PLAY) => self.play(),
                UiEvent::Activated(A_BROWSE) => self.start_browse(),
                UiEvent::Activated(A_LEAVE) => self.leave(),
                _ => {}
            }
        }
    }

    // ---- layout ---------------------------------------------------------------------------

    fn row_text(&self, f: &Found) -> String {
        let t = &self.texts;
        let mut s = format!("{}   -   {}", f.path.display(), t.a(f.source.english()));
        if !f.valid() {
            s.push_str("   (");
            s.push_str(&t.a("incomplete"));
            s.push(')');
        }
        s
    }

    fn status_line(&self) -> (String, Color) {
        let t = &self.texts;
        match &self.status {
            Status::Hint if self.found.is_empty() => (
                t.a("No installation found. Choose the folder yourself."),
                DIM,
            ),
            Status::Hint => (
                t.a("Pick an installation from the list or choose the folder yourself."),
                DIM,
            ),
            Status::Browsing => (t.a("Choosing a folder..."), DIM),
            Status::Valid => (t.a("Looks good: this is a Railroad Tycoon II install."), OK),
            Status::Missing(m) => (
                t.af("Not a usable install. Missing: %1", &[m.join(", ")]),
                BAD,
            ),
            Status::Error(e) => (e.clone(), BAD),
        }
    }

    /// Updates the nodes that depend on the state (no new layout).
    fn refresh(&mut self) {
        let items: Vec<String> = self.found.iter().map(|f| self.row_text(f)).collect();
        let (text, color) = self.status_line();
        let playable = self.playable().is_some();
        let browsing = self.is_browsing();
        let selected = self.selected;
        if let Some(l) = self.nodes.list
            && let Kind::List {
                items: it,
                selected: sel,
                ..
            } = &mut self.ui.node_mut(l).kind
        {
            *it = items;
            *sel = selected;
        }
        if let Some(m) = self.nodes.message
            && let Kind::Label {
                text: t, color: c, ..
            } = &mut self.ui.node_mut(m).kind
        {
            *t = text;
            *c = color;
        }
        if let Some(p) = self.nodes.play {
            self.ui.set_enabled(p, playable && !browsing);
        }
        if let Some(b) = self.nodes.browse {
            self.ui.set_enabled(b, !browsing);
        }
    }

    /// Builds the whole tree for the viewport, scale and language.
    fn build(&mut self) {
        let focus_list = self.nodes.list.is_none_or(|l| self.ui.focused() == Some(l));
        let mut ui = Ui::new(self.scale);
        let (w, h) = self.scale.logical_size(self.viewport);
        let pw = (w - 24.0).clamp(240.0, 780.0);
        let ph = (h - 24.0).clamp(240.0, 720.0);
        let small = ph < 420.0;
        // The logo is rasterized for exactly the size it is drawn at (a new skin when it changes).
        let logo = if small {
            0.0
        } else {
            (ph * 0.22).clamp(56.0, LOGO_MAX)
        };
        let logo_px = ((logo * self.scale.factor()).round() as u32).max(16);
        if logo_px != self.logo_px {
            let art = make_art(logo_px);
            self.sprite = Some(art.sprite);
            self.art = Some(art);
            self.logo_px = logo_px;
        }
        let sprite = self.sprite;
        let t = &self.texts;
        let theme = theme();
        let pad = (pw * 0.045).clamp(10.0, 28.0);
        let inner = pw - 2.0 * pad;
        let at =
            |x: f32, y: f32, ww: f32, hh: f32| Layout::at(Anchor::TopLeft, ww, hh).offset(x, y);

        ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(BG),
            },
            Layout::fill(),
        );
        let panel = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, pw, ph),
        );

        let mut y = pad;
        if logo > 0.0
            && let Some(sprite) = sprite
        {
            ui.add(
                panel,
                Kind::Image {
                    sprite,
                    fit: Fit::Contain,
                },
                Layout::at(Anchor::Top, logo, logo).offset(0.0, y),
            );
            y += logo + 6.0;
        }
        let title_font = game_font(4).with_size(if small { 24.0 } else { 34.0 });
        let title_h = title_font.line_height();
        ui.add(
            panel,
            Kind::Label {
                text: "Railroad Tycoon II".to_owned(),
                font: title_font,
                color: theme.text,
                align: Align::Center,
                wrap: false,
            },
            at(pad, y, inner, title_h),
        );
        y += title_h;
        let sub_font = game_font(3).with_size(if small { 14.0 } else { 18.0 });
        let sub_h = sub_font.line_height();
        ui.add(
            panel,
            Kind::Label {
                text: t.a("Game folder"),
                font: sub_font,
                color: theme.accent,
                align: Align::Center,
                wrap: false,
            },
            at(pad, y, inner, sub_h),
        );
        y += sub_h + 8.0;

        let body = game_font(8).with_size(if small { 13.0 } else { 15.0 });
        let intro = t.a("rrt2 needs the files of your own copy of Railroad Tycoon II Platinum. Pick its folder below or browse for it; nothing is copied or changed there.");
        let intro_h = wrapped_height(&intro, inner, body.size, body.line_height());
        ui.add(
            panel,
            Kind::Label {
                text: intro,
                font: body,
                color: theme.text_dim,
                align: Align::Center,
                wrap: true,
            },
            at(pad, y, inner, intro_h),
        );
        y += intro_h + 10.0;

        let head_font = game_font(2).with_size(13.0);
        ui.add(
            panel,
            Kind::Label {
                text: t.a("Installations found"),
                font: head_font,
                color: theme.text,
                align: Align::Left,
                wrap: false,
            },
            at(pad, y, inner, head_font.line_height()),
        );
        y += head_font.line_height() + 4.0;

        // Bottom: buttons, then the status line; the list takes what is left.
        let btn_h = if small { 30.0 } else { 38.0 };
        let btn_y = ph - pad - btn_h;
        let msg_h = body.line_height() * 2.0 + 4.0;
        let msg_y = btn_y - 8.0 - msg_h;
        let list_h = (msg_y - 8.0 - y).max(40.0);
        let row_h = (body.size * 1.9).round();
        let list = ui.add(
            panel,
            Kind::List {
                items: Vec::new(),
                selected: None,
                scroll: 0,
                row_h,
                font: body,
                action: A_LIST,
            },
            at(pad, y, inner, list_h),
        );
        let message = ui.add(
            panel,
            Kind::Label {
                text: String::new(),
                font: body,
                color: DIM,
                align: Align::Left,
                wrap: true,
            },
            at(pad, msg_y, inner, msg_h),
        );
        let bfont = game_font(2).with_size(if small { 13.0 } else { 15.0 });
        let gap = 10.0;
        let bw = ((inner - 2.0 * gap) / 3.0).clamp(90.0, 210.0);
        let button = |ui: &mut Ui, text: String, action: u32, x: f32| {
            ui.add(
                panel,
                Kind::Button {
                    text,
                    font: bfont,
                    action,
                    enabled: true,
                    hotkey: None,
                },
                at(x, btn_y, bw, btn_h),
            )
        };
        let browse = button(&mut ui, t.a("Choose folder..."), A_BROWSE, pad);
        let leave_text = if self.can_cancel {
            t.a("Back")
        } else {
            t.a("Quit")
        };
        let _ = button(&mut ui, leave_text, A_LEAVE, pad + inner - 2.0 * bw - gap);
        let play = button(&mut ui, t.a("Play"), A_PLAY, pad + inner - bw);

        self.nodes = Nodes {
            list: Some(list),
            message: Some(message),
            play: Some(play),
            browse: Some(browse),
        };
        self.ui = ui;
        self.refresh();
        if focus_list {
            self.ui.set_focus(Some(list));
        }
        self.ui.layout(self.viewport);
    }
}

/// A folder that is not an install but has exactly one install among its children (the player
/// picked `steamapps/common` or `GOG Games` instead of the game's own folder).
fn descend(dir: &std::path::Path) -> Option<PathBuf> {
    if paths::missing_files(dir).is_empty() {
        return None;
    }
    let mut hits = Vec::new();
    for child in std::fs::read_dir(dir).ok()?.filter_map(Result::ok) {
        let p = child.path();
        if p.is_dir() && paths::missing_files(&p).is_empty() {
            hits.push(p);
        }
    }
    hits.sort();
    (hits.len() == 1).then(|| hits.remove(0))
}

/// Height of `text` wrapped to `width` (an estimate: Latin letters are about half an em wide,
/// CJK ideographs a full em).
fn wrapped_height(text: &str, width: f32, size: f32, line_h: f32) -> f32 {
    let em: f32 = text
        .chars()
        .map(|c| if (c as u32) >= 0x2e80 { 1.0 } else { 0.55 })
        .sum();
    let lines = (em * size / width.max(1.0)).ceil().max(1.0) + 0.0;
    lines * line_h + 4.0
}

#[cfg(test)]
#[path = "gamedir_tests.rs"]
mod tests;
