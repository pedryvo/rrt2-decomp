//! The application state machine: menu and game screens, input routing, the fixed-timestep
//! simulation driver and the HUD. No GPU and no window in here (the window loop is `app.rs`, the
//! offscreen path `shot.rs`), so it is testable.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use rt2_audio::Event;
use rt2_formats::map::Save;
use rt2_render::live::LiveFrame;
use rt2_render::{Camera, GameAssets, Scene, SceneOptions};
use rt2_sim::calendar::{Date, SpeedTable};
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::{Command, SpeedChange, World};
use rt2_ui::{
    Align, Anchor, Color, Fill, Fit, Input, Key, Kind, Layout, MouseButton, NodeId, SpriteId,
    Stack, Ui, UiEvent, UiScale, game_font,
};
use winit::keyboard::KeyCode;

use crate::catalog::{self, Entry};
use crate::config::Config;
use crate::info::Info;
use crate::keys::Bindings;
use crate::paths;
use crate::sound::{Cue, MusicRequester};
use crate::text::Texts;

mod bldinfo;
mod build_dialogs;
mod build_ui;
mod campaign;
mod companybook;
mod credits;
mod dialogs;
mod editor_modals;
mod editor_options_ui;
mod editor_panel;
mod editor_ui;
mod groupa;
mod hiscores;
mod info_ui;
mod mainwin;
mod maplabels;
mod menus;
mod options_ui;
mod overview;
mod saves_ui;
mod ticker;
mod train_panel;
mod video;
use dialogs::Dialog;
use info_ui::InfoState;
use mainwin::{CenterState, MainIds, NewsItem};
use menus::{MenuData, Page};

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
/// Gap between language buttons and between the selector's lines.
const LANG_GAP: f32 = 8.0;

/// Wall time of one simulation frame (`calendar::NOMINAL_FRAME_MS`).
const STEP_MS: f32 = rt2_sim::calendar::NOMINAL_FRAME_MS as f32;
/// Simulation frames run per rendered frame at most (the rest of a long stall is dropped).
const MAX_STEPS_PER_FRAME: u32 = 5;
/// Keyboard pan speed in physical pixels per second.
const PAN_SPEED: f32 = 900.0;
const ZOOM_STEP: f32 = 1.15;
/// How a game file is opened (`AppState::start_mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartMode {
    /// By extension: `.MAP` / `.MP2` start a scenario with the default options, else load.
    Auto,
    /// A saved game, as it is.
    Load,
    /// New Scenario with the options of the Game Setup screen.
    Scenario(crate::setup::Setup),
    /// New Campaign: the options plus the campaign block.
    Campaign(crate::setup::Setup, crate::setup::CampaignStart),
}

/// Seed of the game RNG after a load (it is not in the file, docs/formats/map-gam.md 5.2).
pub const RNG_SEED: u32 = 1;
const TOAST_SECS: f32 = 4.0;

/// Things the window loop must do for the state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Quit,
    /// Window mode, size, monitor or vsync changed (`Config::display` has the new values).
    ApplyDisplay(crate::config::Display),
    /// Something in the configuration changed: write it to `config.toml`. Only the last one of a
    /// batch matters.
    SaveConfig(Box<Config>),
}

/// A screen over the menu or the game: Options, or the Save / Load dialogs.
enum Overlay {
    Options(options_ui::OptionsState),
    Saves(saves_ui::SavesState),
}

struct MenuState {
    page: Page,
    entries: Vec<Entry>,
    data: MenuData,
}

/// Camera (target x, y, zoom, rotation) and viewport the radar outline was drawn for.
type RadarView = ((f32, f32, f32, u8), (u32, u32));

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
    hud: MainIds,
    /// The scenario's description, briefing and goals (`scenario_info.rs`).
    scenario: Option<Box<crate::scenario_info::ScenarioInfo>>,
    /// The options the game was started with (`setup.rs`); the ones the simulation does not take
    /// yet (sandbox, revenue modifiers, extra computers) wait here for their hooks.
    #[allow(dead_code)]
    setup: crate::setup::Setup,
    /// Modal dialogs, the last one on top (`dialogs.rs`).
    dialogs: Vec<Dialog>,
    center: CenterState,
    /// The activity box, newest first.
    news: Vec<NewsItem>,
    radar: Option<crate::radar::Radar>,
    /// Camera and viewport the radar outline was last drawn for.
    radar_view: Option<RadarView>,
    /// The game year the year-end report was last offered for.
    last_year: u16,
    /// Simulated day the center list was last refreshed for.
    center_day: i32,
    /// The win / lose plaque was already opened for this game.
    over_shown: bool,
    /// The open information screen (finance, trains, ...), if any.
    info: Option<InfoState>,
    /// Fingerprint of what the static map layers show (buildings, track, stations).
    scene_fp: u64,
    /// The construction tools (`build_ui.rs`).
    build: build_ui::BuildState,
    /// Game year and month of the last autosave (or of the start).
    last_autosave: (u16, u8),
    /// The map and scenario editor, when this game is the editor (`editor_ui.rs`).
    editor: Option<Box<editor_ui::EditorState>>,
    /// `(campaign set, scenario index)` when this game is a campaign scenario (medals).
    campaign: Option<(usize, usize)>,
}

/// Hash of the parts of the save the static layers are built from: building type and tile,
/// track piece shapes and the tiles' track/structure ids (`scene.rs`). Cheap enough to run after
/// every batch of simulation steps; changes only when something is built or removed.
fn scene_fingerprint(save: &rt2_formats::map::Save) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |b: u8| {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    for r in &save.objects.records {
        put(r.slot as u8);
        for &o in &[6usize, 0x22, 0x23, 0x24, 0x25, 0x52] {
            put(r.data.get(o).copied().unwrap_or(0));
        }
    }
    for r in &save.track_pieces.records {
        put(r.slot as u8);
        put(r.data.first().copied().unwrap_or(0));
        put(r.data.get(0x23).copied().unwrap_or(0) & 1);
    }
    for t in &save.map.tiles {
        put(t.track_id);
        put(t.struct_id);
    }
    h
}

#[allow(clippy::large_enum_variant)]
enum Screen {
    Menu(MenuState),
    Game(Box<GameState>),
}

/// Everything the state needs from the outside world.
pub struct Init {
    pub install: PathBuf,
    pub assets: Arc<GameAssets>,
    pub backdrop: Option<SpriteId>,
    pub icons: crate::artwork::Icons,
    pub dialogs: crate::artwork::DialogArt,
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
    icons: crate::artwork::Icons,
    dialog_art: crate::artwork::DialogArt,
    /// Sprites to re-upload to the renderer's atlas (the composed station picture), and the key
    /// of what the station sprite holds now. Built from `&self` while the windows are laid out.
    /// Answers to scenario Choice messages, queued for the simulation (`answer_choice`).
    choice_answers: Vec<(u32, u8)>,
    sprite_uploads: std::cell::RefCell<Vec<(SpriteId, Vec<u8>)>>,
    station_view_key: std::cell::RefCell<Option<(u8, i32, u32)>>,
    texts: Texts,
    binds: Bindings,
    data: Option<GameData>,
    vehicle_names: Arc<Vec<String>>,
    /// Live layer update for the renderer: (frame, reset interpolation).
    pending_live: Option<(LiveFrame, bool)>,
    live_alpha: f32,
    /// The scene the renderer shows, and whether its static layers are out of date.
    current_scene: Option<Arc<Scene>>,
    rebuild_scene: bool,
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
    /// Camera, scene and UI scale the map names were laid out for (they move with the view).
    labels_key: Option<(f32, f32, f32, u8, u64, u32)>,
    cursor: Option<(f32, f32)>,
    menu_handles: MenuHandles,
    /// Audio triggers, drained by the window loop (one way: the state never reads audio).
    cues: Vec<Cue>,
    music: MusicRequester,
    /// True while the UI events being handled come from a mouse button.
    via_mouse: bool,
    /// Where the left button went down on the map, to tell a click from a drag.
    press: Option<(f32, f32)>,
    /// Seed of the next start (`set_seed`).
    seed: u32,
    /// The configuration being edited by the Options screen (the window loop writes it out).
    config: Config,
    overlay: Option<Overlay>,
    /// Monitors the window loop reported, for the Options screen.
    monitors: Vec<String>,
    /// DPI scale factor of the window (kept even while the UI scale is pinned).
    dpi: f64,
    /// Our saves folder (`paths::saves_dir`); tests point it at a temporary folder.
    saves_dir: Option<PathBuf>,
    /// Autosaves and other writes the player did not ask for in this very moment; off until the
    /// window loop enables them, so tests and screenshots never write into the user's folders.
    persist: bool,
    /// State of the Credits, High Scores, Map Overview, video and right-click screens.
    ga: groupa::GroupA,
    /// Scenario Choice messages wait for the player (the window loop turns this on); off, they
    /// are answered with button 0 at once like a headless run.
    interactive_choices: bool,
}

#[derive(Default, Clone)]
struct MenuHandles {
    list: Option<NodeId>,
    status: Option<NodeId>,
    /// Hints of the buttons, shown on the status line while the mouse is over them.
    tips: Vec<(NodeId, String)>,
}

fn format_money(v: f64) -> String {
    crate::info::money(v)
}

fn format_date(d: Date, texts: &Texts) -> String {
    texts.date(d.month, d.day, d.year)
}

/// Cash of the player's company: account 0 of the human player's company (`finance.md` 2);
/// `None` while the player has no company (a new scenario before Start New Company).
fn player_cash(save: &Save) -> Option<f64> {
    let c = crate::buildtools::human_company(save)?;
    Some(crate::buildtools::cash(save, c)).filter(|v| v.is_finite())
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
            icons: init.icons,
            dialog_art: init.dialogs,
            choice_answers: Vec::new(),
            sprite_uploads: Default::default(),
            station_view_key: Default::default(),
            texts,
            binds,
            data: None,
            vehicle_names: Arc::default(),
            pending_live: None,
            live_alpha: 1.0,
            current_scene: None,
            rebuild_scene: false,
            ui: Ui::new(init.scale),
            scale: init.scale,
            scale_pinned: init.scale_pinned,
            viewport: init.viewport,
            screen: Screen::Menu(MenuState {
                page: Page::Main,
                entries: Vec::new(),
                data: MenuData::default(),
            }),
            pending_scene: None,
            effects: Vec::new(),
            keys_down: BTreeSet::new(),
            ctrl: false,
            shift: false,
            drag: false,
            labels_key: None,
            cursor: None,
            menu_handles: MenuHandles::default(),
            cues: Vec::new(),
            music: MusicRequester::default(),
            via_mouse: false,
            press: None,
            seed: RNG_SEED,
            config: init.config,
            overlay: None,
            monitors: Vec::new(),
            dpi: 1.0,
            saves_dir: paths::saves_dir(),
            persist: false,
            ga: Default::default(),
            interactive_choices: false,
        };
        s.rebuild_ui();
        s
    }

    /// Seed of the next game's random numbers (game seed, game RNG and CRT state of a start).
    /// Fixed by default so tests and screenshots repeat; the window loop passes the wall clock
    /// so each real game differs, as the original's `clock_ms()` seeds did.
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed.max(1);
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

    /// Scenario Choice messages wait for the player (the window loop turns this on); off, a
    /// headless run answers button 0 at once, as `rt2-sim` does by default.
    pub fn set_interactive_choices(&mut self, on: bool) {
        self.interactive_choices = on;
        if let Screen::Game(g) = &mut self.screen {
            g.world.set_interactive_choices(on);
        }
    }

    /// Lets the state write files the player did not ask for in this moment (autosaves). Off by
    /// default, so tests and screenshots never write into the user's folders.
    pub fn set_persistence(&mut self, on: bool) {
        self.persist = on;
    }

    /// Where our saves go (default `paths::saves_dir()`); tests use a temporary folder.
    #[cfg(test)]
    pub fn set_saves_dir(&mut self, dir: Option<PathBuf>) {
        self.saves_dir = dir;
    }

    /// Pixels the renderer must write into reserved sprites before drawing (the station picture).
    pub fn take_sprite_uploads(&mut self) -> Vec<(SpriteId, Vec<u8>)> {
        std::mem::take(&mut *self.sprite_uploads.borrow_mut())
    }

    /// A new scene for the renderer: a freshly opened game, or the same one with its static
    /// layers rebuilt after something was built or removed.
    pub fn take_scene(&mut self) -> Option<Arc<Scene>> {
        if let Some(s) = self.pending_scene.take() {
            // Something may have been built before the first frame (screenshots, tests).
            let s = match (std::mem::take(&mut self.rebuild_scene), &self.screen) {
                (true, Screen::Game(g)) if g.world.save_ref().is_some() => {
                    let save = g.world.save_ref().expect("checked");
                    Arc::new(s.rebuild(&save.map, Some(save)))
                }
                _ => s,
            };
            self.current_scene = Some(s.clone());
            return Some(s);
        }
        if std::mem::take(&mut self.rebuild_scene)
            && let (Some(cur), Screen::Game(g)) = (&self.current_scene, &self.screen)
            && let Some(save) = g.world.save_ref()
        {
            let s = Arc::new(cur.rebuild(&save.map, Some(save)));
            self.current_scene = Some(s.clone());
            return Some(s);
        }
        None
    }

    /// Live-layer update since the last call: the frame and whether to restart interpolation.
    pub fn take_live(&mut self) -> Option<(LiveFrame, bool)> {
        self.pending_live.take()
    }

    /// Interpolation position between the last two simulation steps (render-side only).
    pub fn live_alpha(&self) -> f32 {
        self.live_alpha
    }

    /// Audio triggers since the last call: the music category of the current screen (menu and
    /// its sub-screens 0, game 1; `sound.md` 2.2) first, then UI sounds in order.
    pub fn take_cues(&mut self) -> Vec<Cue> {
        if self.video_active() {
            // The clip has its own sound: music requests wait until it ends.
            return std::mem::take(&mut self.cues);
        }
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

    /// Centres the camera on a tile (screenshots); `zoom` is in native pixels per pixel.
    pub fn set_view(&mut self, x: f32, y: f32, zoom: f32, rotation: u8) {
        if let Screen::Game(g) = &mut self.screen {
            g.camera.target = rt2_render::camera::Vec2::new(x, y);
            g.camera.zoom = (zoom * rt2_render::NATIVE_ZOOM).clamp(0.02, 200.0);
            g.camera.rotation = rotation % 4;
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
        self.dpi = scale_factor;
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

    /// Which way the cursor pushes the camera at the window edge: `(right, up)` in -1..=1.
    fn edge_direction(&self) -> (f32, f32) {
        const EDGE: f32 = 4.0;
        let Some((x, y)) = self.cursor else {
            return (0.0, 0.0);
        };
        let (w, h) = (self.viewport.0 as f32, self.viewport.1 as f32);
        let axis = |v: f32, size: f32| {
            if v <= EDGE {
                -1.0
            } else if v >= size - 1.0 - EDGE {
                1.0
            } else {
                0.0
            }
        };
        (axis(x, w), -axis(y, h))
    }

    pub fn cursor_moved(&mut self, x: f32, y: f32) {
        if self.drag
            && let (Some((px, py)), Screen::Game(g)) = (self.cursor, &mut self.screen)
        {
            let k = if self.config.gameplay.invert_drag {
                -1.0
            } else {
                1.0
            };
            g.camera.drag(k * (x - px), k * (y - py));
            if g.build.tool.is_some() || self.ga.overview.open {
                // The tool's overlay follows the map.
                self.cursor = Some((x, y));
                self.rebuild_ui();
            }
        }
        self.cursor = Some((x, y));
        let ev = self.ui.input(Input::CursorMoved { x, y });
        self.handle_events(ev);
        self.tool_hover((x, y));
        self.editor_hover((x, y));
        self.overview_hover();
        self.update_status();
    }

    pub fn cursor_left(&mut self) {
        self.cursor = None;
        self.drag = false;
        self.ui.input(Input::CursorLeft);
    }

    pub fn mouse_button(&mut self, left: bool, pressed: bool) {
        if pressed && self.video_skip() {
            return;
        }
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
            // With a construction tool the left button acts on the map instead of panning it.
            let tool = left && self.tool_active() && !self.ui.over_ui() && !self.has_dialog();
            let tool = tool || (left && self.editor_active() && !self.ui.over_ui());
            self.drag = self.in_game() && !self.ui.over_ui() && !tool;
            self.press = (left && self.drag).then_some(self.cursor).flatten();
            if tool && self.editor_active() {
                self.editor_press();
            } else if tool {
                self.tool_press();
            }
            if !left && self.tool_active() {
                self.cancel_tool_drag();
            }
            // A right click on the map (no drag) opens the information box of what is there.
            self.ga.right_press = (!left && self.drag).then_some(self.cursor).flatten();
            if left && self.ui.over_ui() {
                // sound.md 1.3 id 2: button pressed (ButtonDown, 0x50cc60).
                self.cues.push(Cue::Event(Event::ButtonDown));
            }
        } else {
            self.drag = false;
            if left {
                self.tool_release();
                self.editor_release();
            }
            if left && let Some(p) = self.press.take() {
                self.map_click(p);
            }
            if !left && let Some(p) = self.ga.right_press.take() {
                self.map_right_click(p);
            }
        }
        self.via_mouse = true;
        self.handle_events(ev);
        self.via_mouse = false;
    }

    pub fn wheel(&mut self, notches: f32) {
        self.ui.input(Input::Wheel(notches));
        if self.credits_open() {
            self.credits_scroll(-notches * 60.0);
            return;
        }
        if matches!(self.overlay, Some(Overlay::Options(_))) {
            // The rows of a settings tab that does not fit scroll (a list handles its own).
            if self
                .ui
                .hovered()
                .is_none_or(|h| !matches!(self.ui.node(h).kind, rt2_ui::Kind::List { .. }))
            {
                self.options_scroll(-notches.round() as i32);
            }
            return;
        }
        if self.has_dialog() {
            return;
        }
        if self.info_screen().is_none()
            && let Screen::Game(g) = &self.screen
            && g.hud.center.is_some()
            && self.ui.hovered() == g.hud.center
        {
            self.center_scroll(-(notches * 3.0).round() as i32);
            return;
        }
        if self.info_screen().is_some() {
            self.info_scroll(-(notches * 3.0).round() as i32);
            return;
        }
        if !self.ui.over_ui()
            && let Screen::Game(g) = &mut self.screen
        {
            let p = &self.config.gameplay;
            let step = 1.0 + (ZOOM_STEP - 1.0) * p.zoom_speed as f32 / 100.0;
            let dir = if p.invert_zoom { -1.0 } else { 1.0 };
            g.camera.zoom_by(step.powf(dir * notches));
            if g.build.tool.is_some() || self.ga.overview.open {
                self.rebuild_ui();
            }
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
        // Any key skips a playing video.
        if self.video_skip() {
            return;
        }
        // The Options screen and the Save / Load dialogs take every key (rebinding must see F11
        // and the like as plain keys).
        if self.options_open() {
            self.options_key(code, repeat);
            return;
        }
        if self.saves_open() {
            self.saves_key(code, repeat, text);
            return;
        }
        let (ctrl, b) = (self.ctrl, self.binds.clone());
        if b.ui_scale_up.matches(code, ctrl) {
            self.step_ui_scale(1);
            return;
        }
        if b.ui_scale_down.matches(code, ctrl) {
            self.step_ui_scale(-1);
            return;
        }
        if !repeat && b.mute.matches(code, ctrl) {
            self.toggle_mute();
            return;
        }
        if !repeat && b.next_track.matches(code, ctrl) {
            self.cues.push(Cue::NextTrack);
            return;
        }
        if !repeat && b.fullscreen.matches(code, ctrl) {
            self.toggle_fullscreen();
            return;
        }
        if !repeat && b.options.matches(code, ctrl) {
            self.open_options(crate::options::Tab::Display);
            return;
        }
        if !repeat && b.quick_load.matches(code, ctrl) {
            self.quick_load();
            return;
        }
        if !repeat && b.load_dialog.matches(code, ctrl) {
            self.open_saves(saves_ui::SavesMode::Load);
            return;
        }
        match &self.screen {
            Screen::Game(_) => {
                if self.editor_active() {
                    if !repeat || self.editor_modal_open() {
                        self.editor_key(code, text);
                    }
                    return;
                }
                // Start New Company takes typed text for the company name.
                if self.founding_key(code, text) {
                    return;
                }
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
                if self.credits_key(code) {
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

    /// Escape / Cancel: one level up (a side dialog or a notice first).
    fn menu_back(&mut self) {
        let Screen::Menu(m) = &mut self.screen else {
            return;
        };
        if m.data.notice.take().is_some() {
            self.rebuild_ui();
            return;
        }
        match m.page {
            Page::Main => self.effects.push(Effect::Quit),
            Page::NewScenario if m.data.side != menus::Side::None => {
                m.data.side = menus::Side::None;
                self.rebuild_ui();
            }
            Page::Single | Page::Editor | Page::Scores | Page::Credits => {
                self.open_page(Page::Main)
            }
            Page::NewScenario | Page::NewCampaign | Page::LoadGame | Page::LoadCampaign => {
                self.open_page(Page::Single);
            }
        }
    }

    fn game_key(&mut self, code: KeyCode, ctrl: bool) {
        let b = self.binds.clone();
        let fit = match &self.screen {
            Screen::Game(g) => Some(self.fit_camera(g.map_size.0, g.map_size.1)),
            Screen::Menu(_) => None,
        };
        if !ctrl && code == KeyCode::Escape && !self.has_dialog() && self.cancel_tool() {
            return;
        }
        if !ctrl && code == KeyCode::Escape && !self.has_dialog() && self.overview_cancel() {
            return;
        }
        if !ctrl && self.mainwin_key(code) {
            return;
        }
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
            self.quick_save();
        } else if b.save_dialog.matches(code, ctrl) {
            self.open_saves(saves_ui::SavesMode::Save);
        } else if b.menu.matches(code, ctrl) {
            // File Options (`fileopt.win`): Load, Save, New Game, Main Menu, Quit.
            self.push_dialog(Dialog::FileOptions);
        } else if let Screen::Game(g) = &mut self.screen {
            if b.rotate_left.matches(code, ctrl) {
                g.camera.rotate(-1);
            } else if b.rotate_right.matches(code, ctrl) {
                g.camera.rotate(1);
            } else if b.reframe.matches(code, ctrl)
                && let Some(f) = fit
            {
                g.camera = f;
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
        self.ga_tick(dt_ms);
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
        let covered = self.overlay.is_some();
        // Edge scrolling: the cursor in the border strip of the window and over no widget.
        let (edx, edy) = if self.config.gameplay.edge_scroll && !covered && !self.ui.over_ui() {
            self.edge_direction()
        } else {
            (0.0, 0.0)
        };
        let pan_pct = self.config.gameplay.pan_speed as f32 / 100.0;
        let (dx, dy) = ((dx + edx).clamp(-1.0, 1.0), (dy + edy).clamp(-1.0, 1.0));
        let mut overlay_stale = false;
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        if (dx != 0.0 || dy != 0.0) && !ctrl && !covered && g.info.is_none() && g.dialogs.is_empty()
        {
            let k = PAN_SPEED * pan_pct * dt_ms / 1000.0;
            g.camera.pan(dx * k, dy * k);
            overlay_stale = g.build.tool.is_some() || self.ga.overview.open;
        }
        // Dialogs and the information screens hold the simulation (the original's "keep the
        // game running in dialogs" option defaults to off for modal windows); so do the Options
        // and Save / Load screens.
        if g.editor.is_some() {
            g.accum_ms = 0.0;
            if overlay_stale {
                self.rebuild_ui();
            }
            self.editor_tick(dt_ms);
            return;
        }
        let on_hold = !g.dialogs.is_empty() || covered;
        if !on_hold {
            g.accum_ms += dt_ms.min(250.0);
        }
        let mut steps = 0;
        while g.accum_ms >= STEP_MS {
            g.accum_ms -= STEP_MS;
            if steps < MAX_STEPS_PER_FRAME {
                g.world.advance(g.speeds.minutes_per_frame(g.world.speed()));
                steps += 1;
            }
        }
        if steps > 0
            && let Some(save) = g.world.save_ref()
        {
            self.pending_live = Some((LiveFrame::from_save(save), false));
            let fp = scene_fingerprint(save);
            if fp != g.scene_fp {
                // Something was built or removed: rebuild the static layers.
                g.scene_fp = fp;
                self.rebuild_scene = true;
            }
        }
        self.live_alpha = (g.accum_ms / STEP_MS).clamp(0.0, 1.0);
        g.cash_timer -= dt_ms;
        // Messages and the end of the game, after the steps (and once at the start).
        self.poll_world();
        self.autosave_tick();
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
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
        // A new year: offer the year-end report (`finance.md` 8: the ledgers were archived).
        let year = g.world.date().year;
        if year > g.last_year {
            g.last_year = year;
            self.offer_year_end(i32::from(year) - 1);
        }
        let labels_stale = self.map_labels_stale();
        if overlay_stale || labels_stale {
            self.rebuild_ui();
        }
        self.mainwin_refresh();
        self.center_refresh();
        self.info_refresh();
        self.ticker_advance(dt_ms);
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
                UiEvent::Pointer(a, fx, fy) => {
                    if options_ui::owns(a) {
                        self.options_event(&UiEvent::Pointer(a, fx, fy));
                    } else if a == mainwin::A_RADAR {
                        self.radar_pointer(fx, fy);
                    } else if !self.book_pointer(a, fx, fy)
                        && !(self.via_mouse && self.build_pointer(a, fx, fy))
                    {
                        // Neither the company book nor a build tool took the pointer.
                        self.dialog_pointer(a, fx);
                    }
                }
                UiEvent::Selected(a, row) if a != info_ui::A_ROW && self.row_event(a, row) => {
                    self.cues.push(Cue::Event(if self.via_mouse {
                        Event::ListClick
                    } else {
                        Event::ListHover
                    }));
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

    /// A list selection that belongs to the menus or the main window; true if consumed.
    fn row_event(&mut self, action: u32, row: usize) -> bool {
        let e = UiEvent::Selected(action, row);
        self.options_event(&e)
            || self.saves_event(&e)
            || self.editor_row(action, row)
            || self.menu_event(&e)
            || self.mainwin_event(&e)
            || self.build_row_event(action, row)
    }

    fn activated(&mut self, action: u32) {
        if options_ui::owns(action) && self.options_activated(action) {
            return;
        }
        if saves_ui::owns(action) && self.saves_activated(action) {
            return;
        }
        if self.editor_activated(action) {
            return;
        }
        if self.in_game() {
            if self.overview_activated(action)
                || self.dialog_activated(action)
                || self.build_dialog_activated(action)
                || self.build_activated(action)
                || self.mainwin_activated(action)
            {
                return;
            }
            if self.info_activated(action) {
                return;
            }
        } else if self.menu_activated(action) {
            return;
        }
        match action {
            A_QUIT => self.effects.push(Effect::Quit),
            A_BACK => self.menu_back(),
            A_LIST | A_OK => self.menu_start(),
            A_PAUSE => self.apply_command(Command::TogglePause),
            A_FASTER => self.apply_command(Command::SetSpeed(SpeedChange::Delta(1))),
            A_SLOWER => self.apply_command(Command::SetSpeed(SpeedChange::Delta(-1))),
            A_SAVE => self.quick_save(),
            A_MENU => self.go_to_menu(),
            a if (A_LANG..A_LANG + 32).contains(&a) && !self.in_game() => {
                if let Some((locale, _)) = self.texts.languages().get((a - A_LANG) as usize) {
                    let locale = locale.clone();
                    self.choose_language(&locale);
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
            self.go_to_menu();
        } else {
            // A saved game keeps its own state; its briefing is offered like a loaded game's
            // (`scenarios.md` 2.2: trigger 6 runs on load too).
            self.queue_opening_dialogs();
        }
    }

    /// Shows the New Scenario page (Game Setup with the map dialog beside it); used by screenshots.
    pub fn show_new_game(&mut self) {
        self.open_page(Page::NewScenario);
    }

    /// Shows the Load Game page; used by screenshots.
    pub fn show_load_game(&mut self) {
        self.open_page(Page::LoadGame);
    }

    /// Shows a menu page by name (screenshots): `single`, `new-scenario`, `map`, `options`,
    /// `campaign`, `load`, `scores`, `credits`; false for an unknown name.
    pub fn show_menu_page(&mut self, name: &str) -> bool {
        match name {
            "main" => self.open_page(Page::Main),
            "single" => self.open_page(Page::Single),
            "editor" => self.open_page(Page::Editor),
            "new-scenario" => self.open_page(Page::NewScenario),
            "map" | "options" => {
                self.open_page(Page::NewScenario);
                self.menu_activated(if name == "map" {
                    menus::A_SETUP_MAP
                } else {
                    menus::A_SETUP_OPTIONS
                });
            }
            "campaign" => self.open_page(Page::NewCampaign),
            "load" => self.open_page(Page::LoadGame),
            "scores" => self.open_page(Page::Scores),
            "credits" => self.open_page(Page::Credits),
            // Screenshots of the roll and of a clip: `credits@NNN` (design pixels scrolled),
            // `video:NAME[@ms]`.
            n if n.starts_with("credits@") => {
                self.open_page(Page::Credits);
                self.ga.credits.scroll = n[8..].parse().unwrap_or(0.0);
                self.rebuild_ui();
            }
            n if n.starts_with("video:") => {
                let (clip, ms) = n[6..].split_once('@').unwrap_or((&n[6..], "0"));
                if !self.play_video(clip) {
                    return false;
                }
                self.video_wait(ms.parse().unwrap_or(0.0));
            }
            _ => return false,
        }
        true
    }

    fn go_to_menu(&mut self) {
        self.screen = Screen::Menu(MenuState {
            page: Page::Main,
            entries: Vec::new(),
            data: MenuData::default(),
        });
        self.rebuild_ui();
    }

    fn game_data(&mut self) -> Result<&GameData> {
        if self.data.is_none() {
            let path = self.install.join("RT2_PLAT.EXE");
            let exe =
                Exe::from_bytes(std::fs::read(&path).with_context(|| path.display().to_string())?)
                    .context("reading RT2_PLAT.EXE")?;
            // `vehicle_type_table` (0x53ddb8, 0x39 bytes, 175 records): short names, `+0`.
            let names = (0..rt2_sim::trains::VEHICLE_COUNT as u32)
                .map(|t| {
                    exe.read(0x0053_ddb8 + t * 0x39, 5)
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                        .unwrap_or_default()
                })
                .collect();
            self.vehicle_names = Arc::new(names);
            self.data = Some(GameData::load(&exe).context("loading the data tables")?);
        }
        Ok(self.data.as_ref().expect("just loaded"))
    }

    /// Opens a game file: a saved game (`.GAM`) loads as it is, a scenario map (`.MAP`, `.MP2`)
    /// starts a new game with the default options.
    pub fn start(&mut self, path: &Path) -> Result<()> {
        self.start_mode(path, StartMode::Auto)
    }

    /// Starts `path` as `mode` says. A load goes through `World::from_save_seeded` (the old
    /// `0x233C` layout is converted as the original's loader does); a new scenario or campaign
    /// game through `World::new_game` with the options of the setup screens
    /// ([`Setup::new_game_options`]).
    pub fn start_mode(&mut self, path: &Path, mode: StartMode) -> Result<()> {
        let raw = std::fs::read(path).with_context(|| path.display().to_string())?;
        let (map, save) = catalog::load_any(&raw).with_context(|| path.display().to_string())?;
        let Some(save) = save else {
            bail!(
                "{} has no game state (neither a saved game nor a scenario)",
                path.display()
            );
        };
        let is_map = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("map") || e.eq_ignore_ascii_case("mp2"));
        let mode = match mode {
            StartMode::Auto if is_map => StartMode::Scenario(crate::setup::Setup::default()),
            StartMode::Auto => StartMode::Load,
            m => m,
        };
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "game".into());
        let data = self.game_data()?.clone();
        let seed = self.seed;
        let (world, setup) = match &mode {
            StartMode::Load | StartMode::Auto => {
                let mut w = World::from_save_seeded(&save, &data, seed);
                w.apply(Command::SeedRng(seed));
                (w, crate::setup::Setup::default())
            }
            StartMode::Scenario(setup) => {
                let mut o = setup.new_game_options(&save, &name, None, seed);
                o.lng = self.texts.english().cloned().map(std::sync::Arc::new);
                (World::new_game(&save, &o, &data, seed), *setup)
            }
            StartMode::Campaign(setup, c) => {
                let mut o = setup.new_game_options(&save, &name, Some(c), seed);
                o.lng = self.texts.english().cloned().map(std::sync::Arc::new);
                (World::new_game(&save, &o, &data, seed), *setup)
            }
        };
        // Everything below reads the converted state the world keeps, not the file's layout.
        let speeds = data.speed_table.clone();
        let live = world.save_ref().unwrap_or(&save);
        let scene = Scene::build_with(
            self.assets.clone(),
            &map,
            Some(live),
            SceneOptions { sprites: true },
            self.vehicle_names.clone(),
        );
        let live_frame = LiveFrame::from_save(live);
        let scene_fp = scene_fingerprint(live);
        let cash = player_cash(live);
        self.pending_scene = Some(Arc::new(scene));
        self.pending_live = Some((live_frame, true));
        self.live_alpha = 1.0;
        let camera = self.fit_camera(map.width, map.height);
        let scenario = Some(Box::new(crate::scenario_info::ScenarioInfo::from_save(
            &save,
            &raw,
            &catalog::map_title(&map, path),
        )));
        let welcome = NewsItem {
            date: format_date(world.date(), &self.texts),
            text: self.texts.af(
                "Game started: %1.",
                &[scenario
                    .as_ref()
                    .map_or_else(|| name.clone(), |s| s.title.clone())],
            ),
        };
        let last_year = world.date().year;
        let world_month = world.date().month;
        self.screen = Screen::Game(Box::new(GameState {
            cash,
            radar: Some(crate::radar::Radar::new(&map)),
            radar_view: None,
            scenario,
            setup,
            dialogs: Vec::new(),
            center: CenterState::default(),
            news: vec![welcome],
            last_year,
            center_day: -1,
            world,
            name,
            map_size: (map.width, map.height),
            camera,
            speeds,
            accum_ms: 0.0,
            cash_timer: 500.0,
            toast: None,
            hud: MainIds::default(),
            over_shown: false,
            info: None,
            scene_fp,
            build: build_ui::BuildState::default(),
            last_autosave: (last_year, world_month),
            editor: None,
            campaign: match &mode {
                StartMode::Campaign(_, c) => Some((c.set, c.index)),
                _ => None,
            },
        }));
        // Scenario choices wait for the player in the real game; headless they answer button 0.
        if let Screen::Game(g) = &mut self.screen {
            g.world.set_interactive_choices(self.interactive_choices);
            // The configured starting speed goes through the same command as the HUD buttons.
            let speed = self.config.gameplay.default_speed;
            if speed > 0 {
                g.world
                    .apply(Command::SetSpeed(SpeedChange::Absolute(speed)));
            }
        }
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
            // Years crossed here (screenshots, tests) do not offer a report.
            g.last_year = g.world.date().year;
            if let Some(save) = g.world.save_ref() {
                self.pending_live = Some((LiveFrame::from_save(save), true));
                if scene_fingerprint(save) != g.scene_fp {
                    g.scene_fp = scene_fingerprint(save);
                    self.rebuild_scene = true;
                }
            }
        }
    }

    fn toast(&mut self, msg: String) {
        if let Screen::Game(g) = &mut self.screen {
            g.toast = Some((msg, TOAST_SECS));
        }
    }

    // ---- UI construction ------------------------------------------------------------------

    fn rebuild_ui(&mut self) {
        let mut ui = Ui::new(self.scale);
        ui.layout(self.viewport);
        self.menu_handles = MenuHandles::default();
        match &self.screen {
            Screen::Menu(_) => self.build_menu(&mut ui),
            Screen::Game(_) if self.editor_active() => self.build_editor(&mut ui),
            Screen::Game(_) => {
                self.build_map_labels(&mut ui);
                self.build_overlay(&mut ui);
                self.build_overview_overlay(&mut ui);
                self.build_mainwin(&mut ui);
                self.build_info(&mut ui);
                self.build_dialogs(&mut ui);
            }
        }
        self.build_overlay_screen(&mut ui);
        self.build_video(&mut ui);
        ui.layout(self.viewport);
        self.ui = ui;
        if let Some((x, y)) = self.cursor {
            self.ui.input(Input::CursorMoved { x, y });
        }
        if self.in_game() && !self.editor_active() {
            self.mainwin_refresh();
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

    /// Sizes of the language selector for a canvas width: `(label_w, button_w, font, stacked)`.
    /// There is no text measurement here, so the advance is an estimate (em per script) that is
    /// generous enough for no label to clip. When the labels would need a font under 9 logical
    /// pixels (high UI scales shrink the canvas) the selector stacks into two button lines with
    /// its caption above them.
    fn language_plan(&self, canvas_w: f32) -> (f32, f32, rt2_ui::FontStyle, bool) {
        let langs = self.texts.languages();
        let em = |t: &str| {
            t.chars()
                .map(|c| if (c as u32) >= 0x2e80 { 1.05 } else { 0.72 })
                .sum::<f32>()
        };
        let label_w = (em(&self.texts.a("Language")) * game_font(6).size + 16.0).max(60.0);
        let widest = langs.iter().map(|(_, name)| em(name)).fold(1.0, f32::max);
        let fit = |n: f32, avail: f32| {
            let bw = ((avail - 16.0) / n - LANG_GAP).clamp(60.0, 190.0);
            (bw, ((bw - 16.0) / widest).floor().min(game_font(2).size))
        };
        let n = langs.len() as f32;
        let (bw, size) = fit(n, canvas_w - label_w);
        let (bw, size, stacked) = if size >= 9.0 {
            (bw, size, false)
        } else {
            let (bw2, size2) = fit((n / 2.0).ceil(), canvas_w);
            (bw2, size2.max(8.0), true)
        };
        let mut font = game_font(2);
        font.size = size;
        (label_w, bw, font, stacked)
    }

    /// One button per available language, labelled with its own name; the current one is
    /// disabled (it reads as selected).
    fn language_row(&self, ui: &mut Ui, layer: NodeId, canvas_w: f32) {
        let langs = self.texts.languages();
        let (label_w, bw, font, stacked) = self.language_plan(canvas_w);
        let caption = self.texts.a("Language");
        let cream = Color::rgb(0xf2, 0xe9, 0xd0);
        let per = if stacked {
            langs.len().div_ceil(2)
        } else {
            langs.len()
        };
        let line_h = 34.0 + LANG_GAP;
        let mut rows = Vec::new();
        for line in 0..(langs.len().div_ceil(per)) {
            let count = per.min(langs.len() - line * per) as f32;
            let w = count * (bw + LANG_GAP) + if stacked { 0.0 } else { label_w };
            // The last line sits on the bottom margin; earlier lines stack above it.
            let up = 40.0 + (if stacked { 1 - line } else { 0 }) as f32 * line_h;
            let row = ui.add(
                layer,
                Kind::Panel { fill: Fill::None },
                Layout::at(Anchor::Bottom, w, 34.0).offset(0.0, up),
            );
            ui.set_stack(row, Stack::row(LANG_GAP, 0.0));
            if !stacked {
                ui.add(
                    row,
                    label(&caption, game_font(6).with_shadow(), Align::Right, cream),
                    Layout::at(Anchor::Left, label_w, 34.0),
                );
            }
            rows.push(row);
        }
        if stacked {
            ui.add(
                layer,
                label(&caption, game_font(6).with_shadow(), Align::Center, cream),
                Layout::at(Anchor::Bottom, canvas_w, 34.0).offset(0.0, 40.0 + 2.0 * line_h),
            );
        }
        for (i, (locale, name)) in langs.iter().enumerate() {
            ui.add(
                rows[i / per],
                Kind::Button {
                    text: name.clone(),
                    font,
                    action: A_LANG + i as u32,
                    enabled: locale != self.texts.locale(),
                    hotkey: None,
                },
                Layout::at(Anchor::Left, bw, 34.0),
            );
        }
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

#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod editor_panel_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod fin_tests;
#[cfg(test)]
mod groupa_tests;
#[cfg(test)]
mod groupb_tests;
#[cfg(test)]
mod options_tests;
#[cfg(test)]
mod play_tests;
