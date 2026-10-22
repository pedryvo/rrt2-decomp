//! The application state machine: menu and game screens, input routing, the fixed-timestep
//! simulation driver and the HUD. No GPU and no window in here (the window loop is `app.rs`, the
//! offscreen path `shot.rs`), so it is testable.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
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
use crate::keys::Bindings;
use crate::paths;
use crate::text::{Texts, id as lng};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Quit,
    ToggleFullscreen,
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
}

#[derive(Default, Clone, Copy)]
struct MenuHandles {
    list: Option<NodeId>,
}

fn format_money(v: f64) -> String {
    let n = v.round() as i64;
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("{}${out}", if n < 0 { "-" } else { "" })
}

fn format_date(d: Date, texts: &Texts) -> String {
    format!("{} {}, {}", texts.month(d.month), d.day, d.year)
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
        let texts = Texts::load(&init.install);
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
        };
        s.rebuild_ui();
        s
    }

    pub fn invalid_keys(&self) -> &[String] {
        &self.binds.invalid
    }

    pub fn take_scene(&mut self) -> Option<Arc<Scene>> {
        self.pending_scene.take()
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
        } else {
            self.drag = false;
        }
        self.handle_events(ev);
    }

    pub fn wheel(&mut self, notches: f32) {
        self.ui.input(Input::Wheel(notches));
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
        if (dx != 0.0 || dy != 0.0) && !ctrl {
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
    }

    // ---- screens --------------------------------------------------------------------------

    fn handle_events(&mut self, events: Vec<UiEvent>) {
        for e in events {
            match e {
                UiEvent::Activated(a) => self.activated(a),
                UiEvent::Selected(_, _) => {}
            }
        }
    }

    fn activated(&mut self, action: u32) {
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
            Ok(p) => self.toast(format!("Saved {}", p.display())),
            Err(e) => self.toast(format!("Cannot save: {e}")),
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
                            Source::Own => format!("{}  (saved)", e.title),
                            _ => e.title.clone(),
                        })
                        .collect(),
                    _ => unreachable!(),
                };
                self.build_menu(&mut ui, page, items, n_entries);
            }
            Screen::Game(_) => self.build_hud(&mut ui),
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
                let version = format!(
                    "rrt2 {}   |   UI scale {}%  (Ctrl +/- to change)   |   {}",
                    env!("CARGO_PKG_VERSION"),
                    self.scale.percent(),
                    self.install.display()
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
            Layout::at(Anchor::Left, 190.0, HUD_HEIGHT).offset(8.0, 0.0),
        );
        ui.set_stack(left, Stack::row(8.0, 0.0));
        ui.add(
            left,
            button("Menu", f, A_MENU, None),
            Layout::at(Anchor::Left, 80.0, 32.0).offset(0.0, 7.0),
        );
        ui.add(
            left,
            button("Save", f, A_SAVE, None),
            Layout::at(Anchor::Left, 80.0, 32.0).offset(0.0, 7.0),
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
