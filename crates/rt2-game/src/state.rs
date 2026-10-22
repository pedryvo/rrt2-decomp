//! The application state machine: menu and game screens, input routing, the fixed-timestep
//! simulation driver and the HUD. No GPU and no window in here (the window loop is `app.rs`, the
//! offscreen path `shot.rs`), so it is testable.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use rt2_audio::Event;
use rt2_formats::map::Save;
use rt2_render::{Camera, GameAssets, Scene, SceneOptions};
use rt2_sim::calendar::{DEFAULT_SPEED, Date, SpeedTable};
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::{Command, SpeedChange, World};
use rt2_ui::{
    Align, Anchor, Color, Fill, Fit, Input, Key, Kind, Layout, MouseButton, NodeId, SpriteId,
    Stack, Ui, UiEvent, UiScale, game_font,
};
use winit::keyboard::KeyCode;

use crate::catalog::{self, Entry, Source};
use crate::config::Config;
use crate::info::Info;
use crate::keys::Bindings;
use crate::paths;
use crate::sound::{Cue, MusicRequester};
use crate::text::{Texts, id as lng};

mod info_ui;
use info_ui::InfoState;

const A_NEW: u32 = 1;
const A_LOAD: u32 = 2;
const A_QUIT: u32 = 3;
const A_LIST: u32 = 10;
const A_OK: u32 = 11;
const A_BACK: u32 = 12;
const A_PAUSE: u32 = 20;
const A_FASTER: u32 = 21;
const A_SLOWER: u32 = 22;
const A_SAVE: u32 = 23;
const A_MENU: u32 = 24;
/// Language buttons of the main menu: `A_LANG + index` in `Texts::languages()`.
const A_LANG: u32 = 200;

/// Wall time of one simulation frame (`calendar::NOMINAL_FRAME_MS`).
const STEP_MS: f32 = rt2_sim::calendar::NOMINAL_FRAME_MS as f32;
/// Simulation frames run per rendered frame at most (the rest of a long stall is dropped).
const MAX_STEPS_PER_FRAME: u32 = 5;
/// Keyboard pan speed in physical pixels per second.
const PAN_SPEED: f32 = 900.0;
const ZOOM_STEP: f32 = 1.15;
/// Seed of the game RNG after a load (it is not in the file, docs/formats/map-gam.md 5.2).
pub const RNG_SEED: u32 = 1;
const HUD_HEIGHT: f32 = 46.0;
const TOAST_SECS: f32 = 4.0;

/// Things the window loop must do for the state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Quit,
    ToggleFullscreen,
    /// The player chose a language: remember it in `config.toml`.
    SaveLanguage(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Main,
    NewGame,
    LoadGame,
}

struct MenuState {
    page: Page,
    entries: Vec<Entry>,
}

#[derive(Default, Clone, Copy)]
struct HudIds {
    date: Option<NodeId>,
    cash: Option<NodeId>,
    speed: Option<NodeId>,
    pause: Option<NodeId>,
    toast: Option<NodeId>,
}

struct GameState {
    world: World,
    name: String,
    map_size: (usize, usize),
    camera: Camera,
    speeds: SpeedTable,
    accum_ms: f32,
    cash: Option<f64>,
    cash_timer: f32,
    toast: Option<(String, f32)>,
    hud: HudIds,
    /// A map without game state (a new scenario): no save is possible.
    map_only: bool,
    /// The open information screen (finance, trains, ...), if any.
    info: Option<InfoState>,
}

enum Screen {
    Menu(MenuState),
    Game(Box<GameState>),
}

/// Everything the state needs from the outside world.
pub struct Init {
    pub install: PathBuf,
    pub assets: Arc<GameAssets>,
    pub backdrop: Option<SpriteId>,
    pub config: Config,
    pub scale: UiScale,
    /// True when the scale was chosen by the player (config/CLI) and must not follow the DPI.
    pub scale_pinned: bool,
    pub viewport: (u32, u32),
}

pub struct AppState {
    install: PathBuf,
    assets: Arc<GameAssets>,
    backdrop: Option<SpriteId>,
    texts: Texts,
    binds: Bindings,
    data: Option<GameData>,
    pub ui: Ui,
    scale: UiScale,
    scale_pinned: bool,
    viewport: (u32, u32),
    screen: Screen,
    pending_scene: Option<Arc<Scene>>,
    effects: Vec<Effect>,
    keys_down: BTreeSet<KeyCode>,
    ctrl: bool,
    shift: bool,
    drag: bool,
    cursor: Option<(f32, f32)>,
    menu_handles: MenuHandles,
    /// Audio triggers, drained by the window loop (one way: the state never reads audio).
    cues: Vec<Cue>,
    music: MusicRequester,
    /// True while the UI events being handled come from a mouse button.
    via_mouse: bool,
    /// Where the left button went down on the map, to tell a click from a drag.
    press: Option<(f32, f32)>,
}

#[derive(Default, Clone, Copy)]
struct MenuHandles {
    list: Option<NodeId>,
}

fn format_money(v: f64) -> String {
    crate::info::money(v)
}

fn format_date(d: Date, texts: &Texts) -> String {
    texts.date(d.month, d.day, d.year)
}

/// Cash of the player's company: account 0 of the first company (`finance.md` 2: `+0x4766`).
// TODO: which company is the human's is not decoded (companies are listed in pool order).
fn player_cash(save: &Save) -> Option<f64> {
    let rec = save.companies.ordered().next()?;
    let b = rec.data.get(0x4766..0x4766 + 8)?;
    let v = f64::from_le_bytes(b.try_into().ok()?);
    v.is_finite().then_some(v)
}

impl AppState {
    pub fn new(init: Init) -> AppState {
        let locales = crate::locales::discover(crate::locales::override_dir().as_deref());
        let wanted = init
            .config
            .language
            .clone()
            .or_else(crate::locales::system_locale)
            .unwrap_or_else(|| crate::locales::ENGLISH.to_owned());
        let texts = Texts::load(&init.install, locales, &wanted);
        let binds = Bindings::from_config(&init.config.keys);
        let mut s = AppState {
            install: init.install,
            assets: init.assets,
            backdrop: init.backdrop,
            texts,
            binds,
            data: None,
            ui: Ui::new(init.scale),
            scale: init.scale,
            scale_pinned: init.scale_pinned,
            viewport: init.viewport,
            screen: Screen::Menu(MenuState {
                page: Page::Main,
                entries: Vec::new(),
            }),
            pending_scene: None,
            effects: Vec::new(),
            keys_down: BTreeSet::new(),
            ctrl: false,
            shift: false,
            drag: false,
            cursor: None,
            menu_handles: MenuHandles::default(),
            cues: Vec::new(),
            music: MusicRequester::default(),
            via_mouse: false,
            press: None,
        };
        s.rebuild_ui();
        s
    }

    /// The writing system of the current language, for the font choice.
    pub fn script(&self) -> rt2_ui::Script {
        rt2_ui::Script::of_locale(self.texts.locale())
    }

    /// Switches the language at runtime: every text is laid out again.
    pub fn set_language(&mut self, locale: &str) {
        self.texts.set_locale(locale);
        self.rebuild_ui();
    }

    pub fn invalid_keys(&self) -> &[String] {
        &self.binds.invalid
    }

    pub fn take_scene(&mut self) -> Option<Arc<Scene>> {
        self.pending_scene.take()
    }

    /// Audio triggers since the last call: the music category of the current screen (menu and
    /// its sub-screens 0, game 1; `sound.md` 2.2) first, then UI sounds in order.
    pub fn take_cues(&mut self) -> Vec<Cue> {
        let music = match self.screen {
            Screen::Menu(_) => self.music.menu(),
            Screen::Game(_) => self.music.game(),
        };
        let mut out: Vec<Cue> = music.into_iter().collect();
        out.append(&mut self.cues);
        out
    }

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// The map camera while a game is on screen.
    pub fn camera(&self) -> Option<Camera> {
        match &self.screen {
            Screen::Game(g) => Some(g.camera),
            Screen::Menu(_) => None,
        }
    }

    pub fn in_game(&self) -> bool {
        matches!(self.screen, Screen::Game(_))
    }

    // ---- window plumbing ------------------------------------------------------------------

    pub fn resize(&mut self, viewport: (u32, u32)) {
        if viewport.0 == 0 || viewport.1 == 0 || viewport == self.viewport {
            return;
        }
        self.viewport = viewport;
        self.rebuild_ui();
    }

    /// The monitor's DPI changed: follow it unless the scale was pinned by the player.
    pub fn dpi_changed(&mut self, scale_factor: f64) {
        if !self.scale_pinned {
            self.set_ui_scale(UiScale::from_dpi(scale_factor), false);
        }
    }

    pub fn set_ui_scale(&mut self, scale: UiScale, pin: bool) {
        self.scale_pinned |= pin;
        if scale != self.scale {
            self.scale = scale;
            self.rebuild_ui();
        }
    }

    pub fn cursor_moved(&mut self, x: f32, y: f32) {
        if self.drag
            && let (Some((px, py)), Screen::Game(g)) = (self.cursor, &mut self.screen)
        {
            g.camera.drag(x - px, y - py);
        }
        self.cursor = Some((x, y));
        let ev = self.ui.input(Input::CursorMoved { x, y });
        self.handle_events(ev);
    }

    pub fn cursor_left(&mut self) {
        self.cursor = None;
        self.drag = false;
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
        if pressed {
            self.drag = self.in_game() && !self.ui.over_ui();
            self.press = (left && self.drag).then_some(self.cursor).flatten();
            if left && self.ui.over_ui() {
                // sound.md 1.3 id 2: button pressed (ButtonDown, 0x50cc60).
                self.cues.push(Cue::Event(Event::ButtonDown));
            }
        } else {
            self.drag = false;
            if left && let Some(p) = self.press.take() {
                self.map_click(p);
            }
        }
        self.via_mouse = true;
        self.handle_events(ev);
        self.via_mouse = false;
    }

    pub fn wheel(&mut self, notches: f32) {
        self.ui.input(Input::Wheel(notches));
        if self.info_screen().is_some() {
            self.info_scroll(-(notches * 3.0).round() as i32);
            return;
        }
        if !self.ui.over_ui()
            && let Screen::Game(g) = &mut self.screen
        {
            g.camera.zoom_by(ZOOM_STEP.powf(notches));
        }
    }

    pub fn modifiers(&mut self, ctrl: bool, shift: bool) {
        self.ctrl = ctrl;
        self.shift = shift;
    }

    pub fn focus_lost(&mut self) {
        self.keys_down.clear();
        self.drag = false;
    }

    pub fn key(&mut self, code: KeyCode, pressed: bool, repeat: bool, text: Option<&str>) {
        if !pressed {
            self.keys_down.remove(&code);
            return;
        }
        self.keys_down.insert(code);
        let (ctrl, b) = (self.ctrl, self.binds.clone());
        if b.ui_scale_up.matches(code, ctrl) {
            self.set_ui_scale(self.scale.step(1), true);
            return;
        }
        if b.ui_scale_down.matches(code, ctrl) {
            self.set_ui_scale(self.scale.step(-1), true);
            return;
        }
        if !repeat && b.mute.matches(code, ctrl) {
            self.cues.push(Cue::ToggleMute);
            return;
        }
        if !repeat && b.next_track.matches(code, ctrl) {
            self.cues.push(Cue::NextTrack);
            return;
        }
        if !repeat && b.fullscreen.matches(code, ctrl) {
            self.effects.push(Effect::ToggleFullscreen);
            return;
        }
        match &self.screen {
            Screen::Game(_) => {
                if !repeat {
                    self.game_key(code, ctrl);
                }
                // Tab / Enter / arrows still drive the HUD buttons' focus.
                let k = match code {
                    KeyCode::Tab => Some(if self.shift { Key::BackTab } else { Key::Tab }),
                    KeyCode::Enter | KeyCode::NumpadEnter => Some(Key::Enter),
                    _ => None,
                };
                if let Some(k) = k {
                    let ev = self.ui.input(Input::KeyDown(k));
                    self.handle_events(ev);
                }
            }
            Screen::Menu(_) => {
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
                    KeyCode::Escape if !repeat => {
                        self.menu_back();
                        None
                    }
                    _ => text
                        .and_then(|t| t.chars().next())
                        .filter(|c| c.is_alphanumeric() && !ctrl)
                        .map(Key::Char),
                };
                if let Some(k) = k {
                    let ev = self.ui.input(Input::KeyDown(k));
                    self.handle_events(ev);
                }
            }
        }
    }

    fn menu_back(&mut self) {
        if let Screen::Menu(m) = &mut self.screen {
            if m.page == Page::Main {
                self.effects.push(Effect::Quit);
            } else {
                m.page = Page::Main;
                self.rebuild_ui();
            }
        }
    }

    fn game_key(&mut self, code: KeyCode, ctrl: bool) {
        let b = self.binds.clone();
        if !ctrl && self.info_key(code) {
            return;
        }
        if !ctrl {
            let tabs = [
                &b.finance,
                &b.stock,
                &b.trains,
                &b.stations,
                &b.cities,
                &b.industries,
            ];
            if let Some(i) = tabs.iter().position(|k| k.matches(code, false)) {
                self.toggle_tab(i);
                return;
            }
        }
        let mut cmd = None;
        if b.pause.matches(code, ctrl) {
            cmd = Some(Command::TogglePause);
        } else if b.speed_up.matches(code, ctrl) {
            cmd = Some(Command::SetSpeed(SpeedChange::Delta(1)));
        } else if b.speed_down.matches(code, ctrl) {
            cmd = Some(Command::SetSpeed(SpeedChange::Delta(-1)));
        } else if b.save.matches(code, ctrl) {
            self.save_game();
        } else if b.menu.matches(code, ctrl) {
            self.go_to_menu();
        } else if let Screen::Game(g) = &mut self.screen {
            if b.rotate_left.matches(code, ctrl) {
                g.camera.rotate(-1);
            } else if b.rotate_right.matches(code, ctrl) {
                g.camera.rotate(1);
            } else if b.reframe.matches(code, ctrl) {
                g.camera = Camera::fit(g.map_size.0, g.map_size.1, self.viewport);
            }
        }
        if let (Some(c), Screen::Game(g)) = (cmd, &mut self.screen) {
            g.world.apply(c);
        }
    }

    /// A left click on the map that did not move: open the page of what is under it (an
    /// industry, a station, a city or region, else the territory).
    fn map_click(&mut self, pos: (f32, f32)) {
        if self.info_screen().is_some() {
            return;
        }
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let (Some(save), Some(data)) = (g.world.save_ref(), self.data.as_ref()) else {
            return;
        };
        // A press that drifted is a drag, not a click (the camera already followed it).
        if let Some((cx, cy)) = self.cursor
            && ((cx - pos.0).powi(2) + (cy - pos.1).powi(2)).sqrt() > 5.0
        {
            return;
        }
        let Some((tx, ty)) = crate::pick::screen_to_tile(&g.camera, self.viewport, pos, &save.map)
        else {
            return;
        };
        let d = g.world.date();
        let info = Info {
            save,
            data,
            texts: &self.texts,
            year: i32::from(d.year),
            month: i32::from(d.month),
            day: i32::from(d.day),
            minutes: g.world.minutes(),
        };
        if let Some(p) = info.pick(tx, ty) {
            self.open_pick(p);
        }
    }

    // ---- per-frame update -----------------------------------------------------------------

    /// Advances the simulation at the fixed timestep and pans the camera. `dt_ms` is the wall
    /// time since the last call (the only place wall time enters; the simulation itself only
    /// sees whole nominal frames).
    pub fn update(&mut self, dt_ms: f32) {
        let held = |bind: &crate::keys::Binding, keys: &BTreeSet<KeyCode>| bind.held(keys);
        let (b, keys) = (&self.binds, &self.keys_down);
        let axis = |neg: &crate::keys::Binding, pos: &crate::keys::Binding| {
            held(pos, keys) as i32 as f32 - held(neg, keys) as i32 as f32
        };
        let (dx, dy) = (
            axis(&b.pan_left, &b.pan_right),
            axis(&b.pan_down, &b.pan_up),
        );
        let ctrl = self.ctrl;
        let texts = &self.texts;
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        if (dx != 0.0 || dy != 0.0) && !ctrl && g.info.is_none() {
            let k = PAN_SPEED * dt_ms / 1000.0;
            g.camera.pan(dx * k, dy * k);
        }
        g.accum_ms += dt_ms.min(250.0);
        let mut steps = 0;
        while g.accum_ms >= STEP_MS {
            g.accum_ms -= STEP_MS;
            if steps < MAX_STEPS_PER_FRAME {
                g.world.advance(g.speeds.minutes_per_frame(g.world.speed()));
                steps += 1;
            }
        }
        g.cash_timer -= dt_ms;
        if g.cash_timer <= 0.0 {
            g.cash_timer = 500.0;
            g.cash = g.world.save_ref().and_then(player_cash);
        }
        if let Some((_, left)) = &mut g.toast {
            *left -= dt_ms / 1000.0;
            if *left <= 0.0 {
                g.toast = None;
            }
        }
        // HUD texts.
        let date = format_date(g.world.date(), texts);
        let speed = if g.world.speed() == 0 {
            texts.get(lng::PAUSED, "Paused")
        } else {
            format!("{} {}", texts.get(lng::SPEED, "Speed:"), g.world.speed())
        };
        let cash = match g.cash {
            Some(c) => format!("{} {}", texts.get(lng::CASH, "Cash"), format_money(c)),
            None => String::new(),
        };
        let toast = g.toast.as_ref().map(|t| t.0.clone());
        let hud = g.hud;
        if let Some(n) = hud.date {
            self.ui.set_text(n, date);
        }
        if let Some(n) = hud.speed {
            self.ui.set_text(n, speed);
        }
        if let Some(n) = hud.cash {
            self.ui.set_text(n, cash);
        }
        if let Some(n) = hud.pause {
            let paused = matches!(&self.screen, Screen::Game(g) if g.world.speed() == 0);
            self.ui.set_text(n, if paused { ">" } else { "II" });
        }
        if let Some(n) = hud.toast {
            self.ui.set_visible(n, toast.is_some());
            self.ui.set_text(n, toast.unwrap_or_default());
        }
        self.info_refresh();
    }

    // ---- screens --------------------------------------------------------------------------

    fn handle_events(&mut self, events: Vec<UiEvent>) {
        for e in events {
            match e {
                UiEvent::Activated(a) => {
                    // sound.md 1.3 id 1: button released (ButtonUp, 0x50cbd0).
                    self.cues.push(Cue::Event(Event::ButtonUp));
                    self.activated(a);
                }
                UiEvent::Selected(info_ui::A_ROW, row) => {
                    // A click on a list row opens its page (a key press only moves the cursor).
                    if self.via_mouse {
                        self.cues.push(Cue::Event(Event::ListClick));
                        self.info_row(row);
                    }
                }
                UiEvent::Selected(_, _) => {
                    // ids 3 / 4: list click (0x506220) vs soft scroll by wheel or keys (0x5062f0).
                    let ev = if self.via_mouse {
                        Event::ListClick
                    } else {
                        Event::ListHover
                    };
                    self.cues.push(Cue::Event(ev));
                }
            }
        }
    }

    fn activated(&mut self, action: u32) {
        if self.in_game() && self.info_activated(action) {
            return;
        }
        match action {
            A_NEW => self.open_page(Page::NewGame),
            A_LOAD => self.open_page(Page::LoadGame),
            A_QUIT => self.effects.push(Effect::Quit),
            A_BACK => self.menu_back(),
            A_LIST | A_OK => self.menu_start(),
            A_PAUSE => self.apply_command(Command::TogglePause),
            A_FASTER => self.apply_command(Command::SetSpeed(SpeedChange::Delta(1))),
            A_SLOWER => self.apply_command(Command::SetSpeed(SpeedChange::Delta(-1))),
            A_SAVE => self.save_game(),
            A_MENU => self.go_to_menu(),
            a if a >= A_LANG && !self.in_game() => {
                if let Some((locale, _)) = self.texts.languages().get((a - A_LANG) as usize) {
                    let locale = locale.clone();
                    self.set_language(&locale);
                    self.effects.push(Effect::SaveLanguage(locale));
                }
            }
            _ => {}
        }
    }

    /// Every change of the simulation goes through a [`Command`].
    fn apply_command(&mut self, cmd: Command) {
        if let Screen::Game(g) = &mut self.screen {
            g.world.apply(cmd);
        }
    }

    fn open_page(&mut self, page: Page) {
        let own = paths::saves_dir();
        let entries = match page {
            Page::NewGame => catalog::list_maps(&self.install),
            Page::LoadGame => catalog::list_saves(&self.install, own.as_deref()),
            Page::Main => Vec::new(),
        };
        self.screen = Screen::Menu(MenuState { page, entries });
        self.rebuild_ui();
    }

    fn menu_start(&mut self) {
        let Screen::Menu(m) = &self.screen else {
            return;
        };
        let Some(sel) = self.menu_handles.list.and_then(|l| self.ui.selected(l)) else {
            return;
        };
        let Some(entry) = m.entries.get(sel).cloned() else {
            return;
        };
        if let Err(e) = self.start(&entry.path) {
            eprintln!("cannot start {}: {e:#}", entry.path.display());
            self.screen = Screen::Menu(MenuState {
                page: Page::Main,
                entries: Vec::new(),
            });
            self.rebuild_ui();
        }
    }

    /// Shows the "New Game" (scenario list) page; used by screenshots.
    pub fn show_new_game(&mut self) {
        self.open_page(Page::NewGame);
    }

    /// Shows the "Load Game" page; used by screenshots.
    pub fn show_load_game(&mut self) {
        self.open_page(Page::LoadGame);
    }

    fn go_to_menu(&mut self) {
        self.screen = Screen::Menu(MenuState {
            page: Page::Main,
            entries: Vec::new(),
        });
        self.rebuild_ui();
    }

    fn game_data(&mut self) -> Result<&GameData> {
        if self.data.is_none() {
            let path = self.install.join("RT2_PLAT.EXE");
            let exe =
                Exe::from_bytes(std::fs::read(&path).with_context(|| path.display().to_string())?)
                    .context("reading RT2_PLAT.EXE")?;
            self.data = Some(GameData::load(&exe).context("loading the data tables")?);
        }
        Ok(self.data.as_ref().expect("just loaded"))
    }

    /// Opens a save (a world with its companies, industries...) or a plain scenario map (terrain
    /// only, clock running).
    pub fn start(&mut self, path: &Path) -> Result<()> {
        let raw = std::fs::read(path).with_context(|| path.display().to_string())?;
        let (map, save) = catalog::load_any(&raw).with_context(|| path.display().to_string())?;
        let (world, speeds, map_only) = match &save {
            Some(save) => {
                let data = self.game_data()?.clone();
                let mut w = World::from_save(save, &data);
                w.apply(Command::SeedRng(RNG_SEED));
                (w, data.speed_table.clone(), false)
            }
            None => {
                // TODO(time.md): a new game's start date and setup screen are not specified; a
                // scenario map opens as a clock-only world in 1860 (`Date` of the sample saves).
                let data = self.game_data()?.clone();
                let mut w = World::new(Date::new(1860, 1, 1, 0, 0), 1, RNG_SEED);
                w.apply(Command::SetSpeed(SpeedChange::Absolute(DEFAULT_SPEED)));
                (w, data.speed_table, true)
            }
        };
        let scene = Scene::build(
            self.assets.clone(),
            &map,
            save.as_ref(),
            SceneOptions { sprites: true },
        );
        self.pending_scene = Some(Arc::new(scene));
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "game".into());
        let camera = Camera::fit(map.width, map.height, self.viewport);
        self.screen = Screen::Game(Box::new(GameState {
            cash: save.as_ref().and_then(player_cash),
            world,
            name,
            map_size: (map.width, map.height),
            camera,
            speeds,
            accum_ms: 0.0,
            cash_timer: 500.0,
            toast: None,
            hud: HudIds::default(),
            map_only,
            info: None,
        }));
        self.rebuild_ui();
        Ok(())
    }

    /// Advances the open game by `days` (screenshots, tests), like `rt2-advance`.
    pub fn advance_days(&mut self, days: u32) {
        if let Screen::Game(g) = &mut self.screen {
            let total = i64::from(days) * 1440;
            let mut done = 0i64;
            while done < total {
                let dt = 320.min(total - done) as i32;
                g.world.advance(dt);
                done += i64::from(dt);
            }
        }
    }

    fn toast(&mut self, msg: String) {
        if let Screen::Game(g) = &mut self.screen {
            g.toast = Some((msg, TOAST_SECS));
        }
    }

    /// Writes the game to our own saves folder, never into the install.
    fn save_game(&mut self) {
        let result = self.try_save();
        match result {
            Ok(p) => {
                let m = self.texts.af("Saved %1", &[p.display().to_string()]);
                self.toast(m);
            }
            Err(e) => {
                let m = self.texts.af("Cannot save: %1", &[e.to_string()]);
                self.toast(m);
            }
        }
    }

    fn try_save(&self) -> Result<PathBuf> {
        let Screen::Game(g) = &self.screen else {
            bail!("no game");
        };
        if g.map_only {
            bail!("a bare scenario map has no game state yet");
        }
        let dir = paths::saves_dir().context("no data folder")?;
        if dir.starts_with(&self.install) {
            bail!("refusing to write inside the game install");
        }
        std::fs::create_dir_all(&dir).with_context(|| dir.display().to_string())?;
        let d = g.world.date();
        let file = dir.join(format!(
            "{}-{}-{:02}-{:02}.GAM",
            g.name, d.year, d.month, d.day
        ));
        std::fs::write(&file, g.world.to_save().to_bytes())
            .with_context(|| file.display().to_string())?;
        Ok(file)
    }

    // ---- UI construction ------------------------------------------------------------------

    fn rebuild_ui(&mut self) {
        let mut ui = Ui::new(self.scale);
        ui.layout(self.viewport);
        self.menu_handles = MenuHandles::default();
        match &self.screen {
            Screen::Menu(m) => {
                let (page, n_entries) = (m.page, m.entries.len());
                let items: Vec<String> = match &self.screen {
                    Screen::Menu(m) => m
                        .entries
                        .iter()
                        .map(|e| match e.source {
                            Source::Own => {
                                self.texts.af("%1  (saved)", std::slice::from_ref(&e.title))
                            }
                            _ => e.title.clone(),
                        })
                        .collect(),
                    _ => unreachable!(),
                };
                self.build_menu(&mut ui, page, items, n_entries);
            }
            Screen::Game(_) => {
                self.build_hud(&mut ui);
                self.build_toolbar(&mut ui);
                self.build_info(&mut ui);
            }
        }
        ui.layout(self.viewport);
        self.ui = ui;
        if let Some((x, y)) = self.cursor {
            self.ui.input(Input::CursorMoved { x, y });
        }
    }

    fn backdrop(&self, ui: &mut Ui) {
        match self.backdrop {
            Some(sprite) => {
                ui.add_root(
                    Kind::Image {
                        sprite,
                        fit: Fit::CoverTop,
                    },
                    Layout::fill(),
                );
            }
            None => {
                ui.add_root(
                    Kind::Panel {
                        fill: Fill::Solid(Color::rgb(0x16, 0x1a, 0x14)),
                    },
                    Layout::fill(),
                );
            }
        }
    }

    fn build_menu(&mut self, ui: &mut Ui, page: Page, items: Vec<String>, _n: usize) {
        let (cw, ch) = ui.logical_size();
        let t = &self.texts;
        self.backdrop(ui);
        let layer = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        // The backdrop is "cover" fitted at the top edge: its logo ends at 265 of 768 design rows.
        let cover = (cw / 1024.0).max(ch / 768.0);
        let top = 265.0 * cover + 24.0;
        let btn_font = game_font(3);
        match page {
            Page::Main => {
                let (bw, bh, gap, pad) = (320.0, 54.0, 14.0, 26.0);
                let h = pad * 2.0 + 3.0 * bh + 2.0 * gap;
                let panel = ui.add(
                    layer,
                    Kind::Panel { fill: Fill::Panel },
                    Layout::at(Anchor::Top, bw + 2.0 * pad, h)
                        .offset(0.0, top.min((ch - h - 40.0).max(0.0))),
                );
                ui.set_stack(panel, Stack::column(gap, pad).centered());
                for (text, action, hot) in [
                    (t.get(lng::NEW_GAME, "New Game"), A_NEW, 'n'),
                    (t.get(lng::LOAD_GAME, "Load Game"), A_LOAD, 'l'),
                    (t.get(lng::QUIT, "Quit"), A_QUIT, 'q'),
                ] {
                    ui.add(
                        panel,
                        button(&text, btn_font, action, Some(hot)),
                        Layout::at(Anchor::Top, bw, bh),
                    );
                }
                self.language_row(ui, layer, cw);
                let version = t.af(
                    "rrt2 %1   |   UI scale %2%  (Ctrl +/- to change)   |   %3",
                    &[
                        env!("CARGO_PKG_VERSION").to_owned(),
                        self.scale.percent().to_string(),
                        self.install.display().to_string(),
                    ],
                );
                ui.add(
                    layer,
                    label(
                        &version,
                        game_font(1).with_size(12.0).with_shadow(),
                        Align::Left,
                        Color::rgb(0xd0, 0xc8, 0xb0),
                    ),
                    Layout::at(Anchor::BottomLeft, 0.0, 20.0).offset(12.0, 8.0),
                );
            }
            Page::NewGame | Page::LoadGame => {
                let title = match page {
                    Page::NewGame => t.get(lng::NEW_GAME, "New Game"),
                    _ => t.get(lng::LOAD_GAME, "Load Game"),
                };
                let pad = 24.0;
                let w = (640.0f32).min(cw - 24.0);
                let list_h = (ch - top - 2.0 * pad - 60.0 - 70.0 - 24.0).clamp(120.0, 420.0);
                let h = pad * 2.0 + 44.0 + list_h + 16.0 + 48.0 + 12.0;
                let y = top.min((ch - h - 16.0).max(8.0));
                let panel = ui.add(
                    layer,
                    Kind::Panel { fill: Fill::Panel },
                    Layout::at(Anchor::Top, w, h).offset(0.0, y),
                );
                ui.add(
                    panel,
                    label(
                        &title,
                        game_font(3).with_shadow(),
                        Align::Center,
                        Color::rgb(0xf2, 0xe9, 0xd0),
                    ),
                    Layout::at(Anchor::Top, 0.0, 40.0).offset(pad, pad),
                );
                let list = ui.add(
                    panel,
                    Kind::List {
                        items: Vec::new(),
                        selected: None,
                        scroll: 0,
                        row_h: 28.0,
                        font: game_font(6),
                        action: A_LIST,
                    },
                    Layout::at(Anchor::Top, 0.0, list_h).offset(pad, pad + 52.0),
                );
                ui.set_items(list, items);
                self.menu_handles.list = Some(list);
                ui.set_focus(Some(list));
                let row = ui.add(
                    panel,
                    Kind::Panel { fill: Fill::None },
                    Layout::at(Anchor::Bottom, 0.0, 48.0).offset(pad, pad - 6.0),
                );
                ui.set_stack(row, Stack::row(16.0, 0.0).centered());
                let ok = if page == Page::NewGame {
                    t.get(lng::OK, "OK")
                } else {
                    t.get(lng::LOAD_GAME, "Load Game")
                };
                ui.add(
                    row,
                    button(&ok, btn_font, A_OK, None),
                    Layout::at(Anchor::Left, 180.0, 44.0),
                );
                ui.add(
                    row,
                    button(&t.get(lng::CANCEL, "Cancel"), btn_font, A_BACK, Some('c')),
                    Layout::at(Anchor::Left, 180.0, 44.0),
                );
            }
        }
    }

    /// One button per available language, labelled with its own name; the current one is
    /// disabled (it reads as selected).
    fn language_row(&self, ui: &mut Ui, layer: NodeId, canvas_w: f32) {
        let langs = self.texts.languages();
        let (gap, label_w) = (8.0, 100.0);
        let n = langs.len() as f32;
        let bw = ((canvas_w - label_w - 24.0) / n - gap).clamp(90.0, 190.0);
        let w = label_w + n * (bw + gap);
        let row = ui.add(
            layer,
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Bottom, w, 34.0).offset(0.0, 40.0),
        );
        ui.set_stack(row, Stack::row(gap, 0.0));
        ui.add(
            row,
            label(
                &self.texts.a("Language"),
                game_font(6).with_shadow(),
                Align::Right,
                Color::rgb(0xf2, 0xe9, 0xd0),
            ),
            Layout::at(Anchor::Left, label_w, 34.0),
        );
        for (i, (locale, name)) in langs.iter().enumerate() {
            ui.add(
                row,
                Kind::Button {
                    text: name.clone(),
                    font: game_font(2),
                    action: A_LANG + i as u32,
                    enabled: locale != self.texts.locale(),
                    hotkey: None,
                },
                Layout::at(Anchor::Left, bw, 34.0),
            );
        }
    }

    fn build_hud(&mut self, ui: &mut Ui) {
        let bar = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Top, 0.0, HUD_HEIGHT),
        );
        let f = game_font(6);
        let text = Color::rgb(0xf2, 0xe9, 0xd0);
        // Left: menu and save.
        let left = ui.add(
            bar,
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Left, 260.0, HUD_HEIGHT).offset(8.0, 0.0),
        );
        ui.set_stack(left, Stack::row(8.0, 0.0));
        ui.add(
            left,
            button(&self.texts.a("Menu"), f, A_MENU, None),
            Layout::at(Anchor::Left, 110.0, 32.0).offset(0.0, 7.0),
        );
        ui.add(
            left,
            button(&self.texts.a("Save"), f, A_SAVE, None),
            Layout::at(Anchor::Left, 140.0, 32.0).offset(0.0, 7.0),
        );
        // Centre: date.
        let date = ui.add(
            bar,
            label("", f.with_shadow(), Align::Center, text),
            Layout::at(Anchor::Center, 340.0, HUD_HEIGHT),
        );
        // Right: cash, speed label and buttons.
        let right = ui.add(
            bar,
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Right, 560.0, HUD_HEIGHT).offset(8.0, 0.0),
        );
        ui.set_stack(right, Stack::row(8.0, 0.0));
        let cash = ui.add(
            right,
            label("", f.with_shadow(), Align::Right, text),
            Layout::at(Anchor::Left, 230.0, HUD_HEIGHT),
        );
        let speed = ui.add(
            right,
            label("", f.with_shadow(), Align::Center, text),
            Layout::at(Anchor::Left, 120.0, HUD_HEIGHT),
        );
        ui.add(
            right,
            button("-", game_font(3), A_SLOWER, None),
            Layout::at(Anchor::Left, 36.0, 32.0).offset(0.0, 7.0),
        );
        let pause = ui.add(
            right,
            button("II", game_font(6), A_PAUSE, None),
            Layout::at(Anchor::Left, 48.0, 32.0).offset(0.0, 7.0),
        );
        ui.add(
            right,
            button("+", game_font(3), A_FASTER, None),
            Layout::at(Anchor::Left, 36.0, 32.0).offset(0.0, 7.0),
        );
        let toast_root = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let toast = ui.add(
            toast_root,
            label(
                "",
                game_font(6).with_shadow(),
                Align::Left,
                Color::rgb(0xff, 0xe0, 0x80),
            ),
            Layout::at(Anchor::BottomLeft, 0.0, 28.0).offset(14.0, 12.0),
        );
        ui.set_visible(toast, false);
        if let Screen::Game(g) = &mut self.screen {
            g.hud = HudIds {
                date: Some(date),
                cash: Some(cash),
                speed: Some(speed),
                pause: Some(pause),
                toast: Some(toast),
            };
        }
    }
}

fn button(text: &str, font: rt2_ui::FontStyle, action: u32, hotkey: Option<char>) -> Kind {
    Kind::Button {
        text: text.to_owned(),
        font,
        action,
        enabled: true,
        hotkey,
    }
}

fn label(text: &str, font: rt2_ui::FontStyle, align: Align, color: Color) -> Kind {
    Kind::Label {
        text: text.to_owned(),
        font,
        color,
        align,
        wrap: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_has_thousands_separators_and_a_leading_minus() {
        assert_eq!(format_money(0.0), "$0");
        assert_eq!(format_money(1234567.4), "$1,234,567");
        assert_eq!(format_money(-9999.6), "-$10,000");
    }

    #[test]
    fn date_uses_the_language_month_or_the_english_fallback() {
        let d = Date::new(1876, 1, 12, 0, 0);
        assert_eq!(format_date(d, &Texts::none()), "January 12, 1876");
    }
}
