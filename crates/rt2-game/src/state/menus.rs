//! The single-player menu flow (manual chapter 3; `setup.win` and `campaign.win` of the original):
//! the main menu column, the Single Player submenu, New Scenario (Game Setup, the Map list with
//! description and thumbnail, the Options page) and New Campaign.
//!
//! `setup.win` is one 1024x768 window that serves every page (docs/reference/screens.md): the
//! six main buttons stay on the left, the submenu column opens beside them and the dialogs open
//! on the right. Here the pieces are placed by the canvas size: wide canvases keep the
//! original's arrangement, narrow ones show one panel at a time.
//!
//! TODO(sim): the options (difficulty, models, sandbox, revenue modifiers, computer players) and
//! the campaign difficulty and bonus are shown and remembered but not applied to the world, and
//! the starting date of a map cannot be changed (the flag that allows it is not decoded); see
//! `setup.rs`. The campaign's progression (which scenarios are unlocked, the medals kept per
//! scenario, `scenarios.md` 5) is not stored: the campaign asks for the scenarios in order like
//! the original but lets the player go on.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_formats::map::Save;
use rt2_ui::{
    Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Stack, Ui, UiEvent, game_font,
};

use super::{AppState, Screen, StartMode};
use crate::catalog::{self, Entry};
use crate::radar::Radar;
use crate::scenario_info::{Medal, ScenarioInfo, wrap_text};
use crate::setup::{CampaignStart, Level, Model, Setup};
use crate::text::id as lng;

pub(super) const A_SINGLE: u32 = 400;
pub(super) const A_MULTI: u32 = 401;
pub(super) const A_EDITOR: u32 = 402;
pub(super) const A_SCORES: u32 = 403;
pub(super) const A_CREDITS: u32 = 404;
pub(super) const A_SUB_NEW_CAMPAIGN: u32 = 410;
pub(super) const A_SUB_LOAD_CAMPAIGN: u32 = 411;
pub(super) const A_SUB_NEW_SCENARIO: u32 = 412;
pub(super) const A_SUB_LOAD_SCENARIO: u32 = 413;
pub(super) const A_SETUP_MAP: u32 = 420;
pub(super) const A_SETUP_OPTIONS: u32 = 421;
pub(super) const A_LEVEL: u32 = 430;
pub(super) const A_PLAYERS_UP: u32 = 440;
pub(super) const A_PLAYERS_DOWN: u32 = 441;
pub(super) const A_SETUP_OK: u32 = 442;
pub(super) const A_MAP_LIST: u32 = 450;
pub(super) const A_MAP_OK: u32 = 451;
pub(super) const A_MAP_CANCEL: u32 = 452;
pub(super) const A_OPT_SANDBOX: u32 = 460;
pub(super) const A_OPT_FIN: u32 = 461;
pub(super) const A_OPT_IND: u32 = 464;
pub(super) const A_OPT_HUMAN_MINUS: u32 = 470;
pub(super) const A_OPT_HUMAN_PLUS: u32 = 471;
pub(super) const A_OPT_COMP_MINUS: u32 = 472;
pub(super) const A_OPT_COMP_PLUS: u32 = 473;
pub(super) const A_OPT_OK: u32 = 474;
pub(super) const A_OPT_CANCEL: u32 = 475;
pub(super) const A_CAMP_SET: u32 = 480;
pub(super) const A_CAMP_DIFF: u32 = 484;
pub(super) const A_CAMP_LIST: u32 = 490;
pub(super) const A_CAMP_BONUS: u32 = 491;
pub(super) const A_CAMP_PLAY: u32 = 495;
pub(super) const A_NOTICE_OK: u32 = 496;
pub(super) const A_OPEN_OPTIONS: u32 = 497;

const CREAM: Color = Color::rgb(0xf2, 0xe9, 0xd0);
const GOLD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Page {
    Main,
    Single,
    /// New Scenario: the Game Setup dialog (with the Map or Options dialog beside it).
    NewScenario,
    NewCampaign,
    LoadGame,
    LoadCampaign,
    /// High Scores and Credits: not built yet.
    Scores,
    Credits,
}

/// The dialog open beside the Game Setup dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    None,
    Map,
    Options,
}

/// The selected scenario's description, goals and thumbnail.
#[derive(Clone)]
pub(super) struct Preview {
    pub info: ScenarioInfo,
    pub radar: Radar,
}

/// The campaign page's choices (`scenarios.md` 5: difficulty 1..3, bonus 1..3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Campaign {
    /// 0 the classic campaign (`#NN*.MAP`), 1 "The Second Century" (`#eNNcamp.MP2`).
    pub set: usize,
    /// 0 Easy, 1 Normal, 2 Hard.
    pub difficulty: usize,
    pub scenario: usize,
    pub bonus: usize,
}

#[derive(Clone)]
pub(super) struct MenuData {
    pub setup: Setup,
    pub setup_backup: Setup,
    pub side: Side,
    pub sel: usize,
    pub sel_backup: usize,
    pub preview: Option<Preview>,
    pub campaign: Campaign,
    /// An explanation shown over the page (campaign order, missing files).
    pub notice: Option<String>,
}

impl Default for MenuData {
    fn default() -> Self {
        MenuData {
            setup: Setup::default(),
            setup_backup: Setup::default(),
            side: Side::None,
            sel: 0,
            sel_backup: 0,
            preview: None,
            campaign: Campaign::default(),
            notice: None,
        }
    }
}

/// Scale of the menu panels with the canvas height.
fn mz(ch: f32) -> f32 {
    (ch / 600.0).clamp(0.85, 1.8)
}

fn mfont(n: u8, z: f32) -> FontStyle {
    let f = game_font(n);
    f.with_size((f.size * z).round().max(10.0))
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

fn label(text: &str, f: FontStyle, color: Color, align: Align) -> Kind {
    Kind::Label {
        text: text.to_owned(),
        font: f,
        color,
        align,
        wrap: false,
    }
}

fn button(text: &str, f: FontStyle, action: u32, hotkey: Option<char>) -> Kind {
    Kind::Button {
        text: text.to_owned(),
        font: f,
        action,
        enabled: true,
        hotkey,
    }
}

fn toggle(text: &str, f: FontStyle, action: u32, on: bool) -> Kind {
    Kind::Toggle {
        text: text.to_owned(),
        font: f,
        action,
        on,
        enabled: true,
    }
}

/// Stand-alone scenarios: the maps that are not campaign scenarios (campaign files start with
/// `#`).
pub(super) fn scenario_entries(install: &std::path::Path) -> Vec<Entry> {
    catalog::list_maps(install)
        .into_iter()
        .filter(|e| {
            !e.path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('#'))
        })
        .collect()
}

/// The 18 files of a campaign in order: set 0 `#01..#18` (`.MAP`), set 1 `#e01camp..` (`.MP2`).
pub(super) fn campaign_entries(install: &std::path::Path, set: usize) -> Vec<Entry> {
    let lower = |e: &Entry| {
        e.path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    let mut v: Vec<Entry> = catalog::list_maps(install)
        .into_iter()
        .filter(|e| {
            let n = lower(e);
            n.starts_with('#') && n.starts_with("#e") == (set == 1)
        })
        .collect();
    // Case-insensitive order: `#E09camp` belongs between `#e08camp` and `#e10camp`.
    v.sort_by_key(lower);
    v
}

/// LNG ids of a classic campaign scenario's name and its three bonuses (`2765 + 4 i`, then
/// `+1..+3`; `default2.lng` "campaign scenario names and bonuses").
pub(super) fn campaign_text_ids(index: usize) -> Option<(u32, [u32; 3])> {
    (index < 18).then(|| {
        let b = 2765 + 4 * index as u32;
        (b, [b + 1, b + 2, b + 3])
    })
}

/// Reads a scenario for the description panel.
fn load_preview(entry: &Entry) -> Option<Preview> {
    let raw = std::fs::read(&entry.path).ok()?;
    let save = Save::parse(&raw).ok()?;
    let info = ScenarioInfo::from_save(&save, &raw, &entry.title);
    let radar = Radar::new(&save.map);
    Some(Preview { info, radar })
}

impl AppState {
    // ---- state ------------------------------------------------------------------------------

    fn menu_data(&mut self) -> Option<&mut MenuData> {
        match &mut self.screen {
            Screen::Menu(m) => Some(&mut m.data),
            Screen::Game(_) => None,
        }
    }

    /// Opens a menu page, loading what it lists.
    pub(super) fn open_page(&mut self, page: Page) {
        let own = super::paths::saves_dir();
        let (entries, mut data) = match &self.screen {
            Screen::Menu(m) => (Vec::new(), m.data.clone()),
            Screen::Game(_) => (Vec::new(), MenuData::default()),
        };
        let mut entries: Vec<Entry> = entries;
        match page {
            Page::NewScenario => {
                entries = scenario_entries(&self.install);
                data.sel = data.sel.min(entries.len().saturating_sub(1));
                data.side = Side::None;
                data.preview = entries.get(data.sel).and_then(load_preview);
            }
            Page::NewCampaign => {
                entries = campaign_entries(&self.install, data.campaign.set);
                data.campaign.scenario =
                    data.campaign.scenario.min(entries.len().saturating_sub(1));
                data.preview = entries.get(data.campaign.scenario).and_then(load_preview);
            }
            Page::LoadGame | Page::LoadCampaign => {
                entries = catalog::list_saves(&self.install, own.as_deref());
            }
            _ => {}
        }
        self.screen = Screen::Menu(super::MenuState {
            page,
            entries,
            data,
        });
        self.rebuild_ui();
    }

    /// Menu actions; true if `action` was one of them.
    pub(super) fn menu_activated(&mut self, action: u32) -> bool {
        if !matches!(self.screen, Screen::Menu(_)) {
            return false;
        }
        match action {
            A_SINGLE => self.open_page(Page::Single),
            A_MULTI | A_EDITOR => {}
            A_SCORES => self.open_page(Page::Scores),
            A_CREDITS => self.open_page(Page::Credits),
            A_SUB_NEW_CAMPAIGN => self.open_page(Page::NewCampaign),
            A_SUB_LOAD_CAMPAIGN => self.open_page(Page::LoadCampaign),
            A_SUB_NEW_SCENARIO => self.open_page(Page::NewScenario),
            A_SUB_LOAD_SCENARIO => self.open_saves(super::saves_ui::SavesMode::Load),
            A_OPEN_OPTIONS => self.open_options(crate::options::Tab::Display),
            A_SETUP_MAP => self.set_side(Side::Map),
            A_SETUP_OPTIONS => self.set_side(Side::Options),
            a if (A_LEVEL..A_LEVEL + 5).contains(&a) => {
                self.edit_setup(|s| s.set_level(Level::ALL[(a - A_LEVEL) as usize]));
            }
            A_PLAYERS_UP => self.edit_setup(|s| s.extra_computers = (s.extra_computers + 1).min(8)),
            A_PLAYERS_DOWN => {
                self.edit_setup(|s| s.extra_computers = s.extra_computers.saturating_sub(1))
            }
            A_SETUP_OK => self.start_new_scenario(),
            A_MAP_OK | A_MAP_LIST => self.set_side(Side::None),
            A_CAMP_LIST => self.campaign_play(),
            A_MAP_CANCEL => {
                let back = self.menu_data().map_or(0, |d| d.sel_backup);
                self.select_scenario(back);
                self.set_side(Side::None);
            }
            A_OPT_SANDBOX => self.edit_setup(|s| s.set_sandbox(!s.sandbox)),
            a if (A_OPT_FIN..A_OPT_FIN + 3).contains(&a) => {
                self.edit_setup(|s| s.set_financial(Model::ALL[(a - A_OPT_FIN) as usize]));
            }
            a if (A_OPT_IND..A_OPT_IND + 3).contains(&a) => {
                self.edit_setup(|s| s.set_industrial(Model::ALL[(a - A_OPT_IND) as usize]));
            }
            A_OPT_HUMAN_MINUS => self.edit_setup(|s| s.step_human(-1)),
            A_OPT_HUMAN_PLUS => self.edit_setup(|s| s.step_human(1)),
            A_OPT_COMP_MINUS => self.edit_setup(|s| s.step_computer(-1)),
            A_OPT_COMP_PLUS => self.edit_setup(|s| s.step_computer(1)),
            A_OPT_OK => self.set_side(Side::None),
            A_OPT_CANCEL => {
                if let Some(d) = self.menu_data() {
                    d.setup = d.setup_backup;
                }
                self.set_side(Side::None);
            }
            a if (A_CAMP_SET..A_CAMP_SET + 2).contains(&a) => {
                self.campaign_set((a - A_CAMP_SET) as usize)
            }
            a if (A_CAMP_DIFF..A_CAMP_DIFF + 3).contains(&a) => {
                if let Some(d) = self.menu_data() {
                    d.campaign.difficulty = (a - A_CAMP_DIFF) as usize;
                }
                self.rebuild_ui();
            }
            a if (A_CAMP_BONUS..A_CAMP_BONUS + 3).contains(&a) => {
                if let Some(d) = self.menu_data() {
                    d.campaign.bonus = (a - A_CAMP_BONUS) as usize;
                }
                self.rebuild_ui();
            }
            A_CAMP_PLAY => self.campaign_play(),
            A_NOTICE_OK => {
                if let Some(d) = self.menu_data() {
                    d.notice = None;
                }
                self.rebuild_ui();
            }
            _ => return false,
        }
        true
    }

    pub(super) fn menu_event(&mut self, e: &UiEvent) -> bool {
        match *e {
            UiEvent::Selected(A_MAP_LIST, row) => {
                self.select_scenario(row);
                true
            }
            UiEvent::Selected(A_CAMP_LIST, row) => {
                self.select_campaign_scenario(row);
                true
            }
            _ => false,
        }
    }

    fn edit_setup(&mut self, f: impl FnOnce(&mut Setup)) {
        if let Some(d) = self.menu_data() {
            f(&mut d.setup);
            if d.setup.level != Level::Custom {
                // A preset keeps the label; relabel already ran inside the setters.
            }
        }
        self.rebuild_ui();
    }

    fn set_side(&mut self, side: Side) {
        if let Some(d) = self.menu_data() {
            if side == Side::Options {
                d.setup_backup = d.setup;
            }
            if side == Side::Map {
                d.sel_backup = d.sel;
            }
            d.side = side;
        }
        self.rebuild_ui();
    }

    fn select_scenario(&mut self, row: usize) {
        let entry = match &self.screen {
            Screen::Menu(m) => m.entries.get(row).cloned(),
            Screen::Game(_) => None,
        };
        let preview = entry.as_ref().and_then(load_preview);
        if let Some(d) = self.menu_data() {
            d.sel = row;
            d.preview = preview;
        }
        self.rebuild_ui();
    }

    fn select_campaign_scenario(&mut self, row: usize) {
        let entry = match &self.screen {
            Screen::Menu(m) => m.entries.get(row).cloned(),
            Screen::Game(_) => None,
        };
        let preview = entry.as_ref().and_then(load_preview);
        if let Some(d) = self.menu_data() {
            d.campaign.scenario = row;
            d.campaign.bonus = 0;
            d.preview = preview;
        }
        self.rebuild_ui();
    }

    fn campaign_set(&mut self, set: usize) {
        if let Some(d) = self.menu_data() {
            d.campaign.set = set;
            d.campaign.scenario = 0;
            d.campaign.bonus = 0;
        }
        self.open_page(Page::NewCampaign);
    }

    /// OK of the Game Setup dialog: start the chosen scenario.
    fn start_new_scenario(&mut self) {
        let (path, setup) = match &self.screen {
            Screen::Menu(m) => (
                m.entries.get(m.data.sel).map(|e| e.path.clone()),
                m.data.setup,
            ),
            Screen::Game(_) => return,
        };
        if let Some(p) = path {
            self.start_flow(&p, StartMode::Scenario(setup));
        }
    }

    /// "<Play this Scenario>" on the campaign page.
    fn campaign_play(&mut self) {
        let (path, setup, scenario, set, difficulty, bonus) = match &self.screen {
            Screen::Menu(m) => (
                m.entries
                    .get(m.data.campaign.scenario)
                    .map(|e| e.path.clone()),
                // The campaign difficulty picks the presets of the first three levels (manual:
                // Easy, Normal, Hard) -- TODO(sim): `CampaignDifficulty1To3`.
                Setup::preset(
                    [Level::Easy, Level::Medium, Level::Hard][m.data.campaign.difficulty.min(2)],
                ),
                m.data.campaign.scenario,
                m.data.campaign.set,
                m.data.campaign.difficulty.min(2),
                m.data.campaign.bonus.min(2),
            ),
            Screen::Game(_) => return,
        };
        let Some(path) = path else {
            return;
        };
        // The original asks for the scenarios in order (LNG 2703); without stored progress only
        // the first is "next", so a later one gets the notice once and then starts.
        let first_time =
            scenario > 0 && !matches!(&self.screen, Screen::Menu(m) if m.data.notice.is_some());
        if first_time && set == 0 {
            let names = campaign_text_ids(0).map(|(n, _)| self.texts.get(n, "The Iron Seed"));
            let msg = self.texts.get(
                2703,
                "You have to at least TRY the maps in order.\n\nIf you lose, you can bypass and play the next map.\n\nTry playing '%1' next.",
            );
            let msg = crate::text::subst(&msg, &[names.unwrap_or_default()]);
            if let Some(d) = self.menu_data() {
                d.notice = Some(msg);
            }
            self.rebuild_ui();
            return;
        }
        let c = CampaignStart {
            set,
            index: scenario,
            difficulty,
            bonus,
        };
        self.start_flow(&path, StartMode::Campaign(setup, c));
    }

    /// Starts a scenario from the menus: the game opens with the briefing on top and, for a
    /// scenario without a company, the Start New Company dialog after it.
    fn start_flow(&mut self, path: &std::path::Path, mode: StartMode) {
        if let Err(e) = self.start_mode(path, mode) {
            eprintln!("cannot start {}: {e:#}", path.display());
            self.go_to_menu();
            return;
        }
        self.queue_opening_dialogs();
    }

    /// The dialogs of a game that was just opened: the briefing and, when the player has no
    /// company yet (a new scenario), Start New Company on top of it.
    pub fn queue_opening_dialogs(&mut self) {
        let founding = self.founding_dialog();
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        // The last one is on top: Start New Company at the bottom...
        if let Some(d) = founding {
            g.dialogs.push(d);
        }
        // ...then what the start sequence said (a new game runs the briefing rule, trigger 6,
        // which arrives as a message dialog)...
        let shown = self.poll_world();
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        // ...else (a loaded game) the briefing and goals from the file's rules.
        if !shown
            && g.scenario
                .as_ref()
                .is_some_and(|s| !s.briefing.is_empty() || !s.goals.is_empty())
        {
            g.dialogs.push(super::dialogs::Dialog::Goals);
        }
        self.rebuild_ui();
    }

    // ---- building ---------------------------------------------------------------------------

    /// Builds the current menu page.
    pub(super) fn build_menu(&mut self, ui: &mut Ui) {
        let Screen::Menu(m) = &self.screen else {
            return;
        };
        let page = m.page;
        let (cw, ch) = ui.logical_size();
        self.backdrop(ui);
        let layer = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let z = mz(ch);
        let cover = (cw / 1024.0).max(ch / 768.0);
        let logo_bottom = 265.0 * cover + 10.0;
        // The status line along the bottom (hints, like label 3276 of the window).
        let status = ui.add(
            layer,
            label("", mfont(2, z * 0.9).with_shadow(), CREAM, Align::Left),
            Layout::at(Anchor::BottomLeft, 0.0, 20.0 * z).offset(14.0, 6.0),
        );
        self.menu_handles.status = Some(status);
        // The Options screen from any menu page (docs/ui.md "Opcoes e saves").
        let ob = ui.add(
            layer,
            button(
                &self.texts.a("Options"),
                mfont(2, z * 0.9),
                A_OPEN_OPTIONS,
                None,
            ),
            Layout::at(Anchor::TopRight, 130.0 * z, 36.0 * z).offset(14.0 * z, 14.0 * z),
        );
        let mut tips: Vec<(NodeId, String)> = Vec::new();
        tips.push((
            ob,
            self.texts
                .a("Display, audio, language, keys and gameplay settings."),
        ));
        let wide = cw >= 760.0;
        match page {
            Page::Main => self.build_main(ui, layer, cw, ch, z, logo_bottom, &mut tips),
            Page::Single => {
                self.build_nav(ui, layer, cw, ch, z, logo_bottom, Some(A_SINGLE), &mut tips);
                self.build_sub(ui, layer, cw, ch, z, logo_bottom, &mut tips);
            }
            Page::NewScenario => {
                let side_open = self.menu_side() != Side::None;
                if wide && !side_open {
                    self.build_nav(ui, layer, cw, ch, z, logo_bottom, Some(A_SINGLE), &mut tips);
                    self.build_sub(ui, layer, cw, ch, z, logo_bottom, &mut tips);
                }
                self.build_new_scenario(ui, layer, cw, ch, z, logo_bottom, &mut tips);
            }
            Page::NewCampaign => {
                self.build_campaign(ui, layer, cw, ch, z, logo_bottom, &mut tips);
            }
            Page::LoadGame | Page::LoadCampaign => {
                self.build_load(ui, layer, cw, ch, z, logo_bottom, page)
            }
            Page::Scores | Page::Credits => {
                self.build_soon(ui, layer, cw, ch, z, logo_bottom, page)
            }
        }
        self.menu_handles.tips = tips;
    }

    fn menu_side(&self) -> Side {
        match &self.screen {
            Screen::Menu(m) => m.data.side,
            Screen::Game(_) => Side::None,
        }
    }

    /// Geometry of the left button column: `(x, top, width, height, gap)`.
    fn nav_geometry(cw: f32, ch: f32, z: f32, logo_bottom: f32) -> (f32, f32, f32, f32, f32) {
        let gap = 8.0 * z;
        let top = logo_bottom.min(ch * 0.46);
        let avail = ch - top - 40.0 * z;
        let bh = ((avail - 5.0 * gap) / 6.0).clamp(26.0, 46.0 * z);
        let w = (cw * 0.16).clamp(120.0, 190.0 * z);
        (16.0 * z, top, w, bh, gap)
    }

    /// The six buttons of `setup.win` (3135..3140); `active` is drawn pressed.
    #[allow(clippy::too_many_arguments)]
    fn build_nav(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        active: Option<u32>,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let t = &self.texts;
        let (x, top, w, bh, gap) = Self::nav_geometry(cw, ch, z, logo_bottom);
        let f = mfont(2, z * 0.95);
        let items: [(String, u32, u32, &str, bool); 6] = [
            (
                t.get(1674, "Single Player"),
                A_SINGLE,
                1675,
                "Play a single player game.",
                true,
            ),
            (
                t.get(1676, "Multi Player"),
                A_MULTI,
                1677,
                "Play a game with multiple human players",
                false,
            ),
            (
                t.get(1678, "Editor"),
                A_EDITOR,
                1679,
                "Create or edit a map.",
                false,
            ),
            (
                t.get(1680, "High Scores"),
                A_SCORES,
                1681,
                "View the high scores list.",
                true,
            ),
            (
                t.get(1682, "Credits"),
                A_CREDITS,
                1683,
                "View the list of people who created this game.",
                true,
            ),
            (
                t.get(lng::QUIT, "Quit"),
                super::A_QUIT,
                1685,
                "Quit the game and return to windows.",
                true,
            ),
        ];
        for (i, (text, action, tip_id, tip_en, enabled)) in items.into_iter().enumerate() {
            let n = ui.add(
                layer,
                Kind::Toggle {
                    text,
                    font: f,
                    action,
                    on: active == Some(action),
                    enabled,
                },
                at(x, top + i as f32 * (bh + gap), w, bh),
            );
            let mut tip = t.get(tip_id, tip_en);
            if !enabled {
                tip = format!("{tip}  ({})", t.a("Not available"));
            }
            tips.push((n, tip));
        }
    }

    /// The Single Player submenu column beside the main buttons.
    #[allow(clippy::too_many_arguments)]
    fn build_sub(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let t = &self.texts;
        let (x, top, w, bh, gap) = Self::nav_geometry(cw, ch, z, logo_bottom);
        let f = mfont(2, z * 0.95);
        let x2 = x + w + 14.0 * z;
        let items = [
            (
                t.get(2578, "New Campaign"),
                A_SUB_NEW_CAMPAIGN,
                2579,
                "Start a new campaign.",
            ),
            (
                t.get(2580, "Load Campaign"),
                A_SUB_LOAD_CAMPAIGN,
                2581,
                "Load an existing campaign that you've already started and saved partway through.",
            ),
            (
                t.get(2582, "New Scenario"),
                A_SUB_NEW_SCENARIO,
                2583,
                "Start a new stand-alone scenario.",
            ),
            (
                t.get(2584, "Load Scenario"),
                A_SUB_LOAD_SCENARIO,
                2585,
                "Load an existing scenario that you've already started and saved partway through.",
            ),
        ];
        for (i, (text, action, tip_id, tip_en)) in items.into_iter().enumerate() {
            let n = ui.add(
                layer,
                button(&text, f, action, None),
                at(x2, top + i as f32 * (bh + gap), w, bh),
            );
            tips.push((n, t.get(tip_id, tip_en)));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_main(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        self.build_nav(ui, layer, cw, ch, z, logo_bottom, None, tips);
        self.language_row(ui, layer, cw);
        let version = self.texts.af(
            "rrt2 %1   |   UI scale %2%  (Ctrl +/- to change)   |   %3",
            &[
                crate::version::LONG.to_owned(),
                self.scale.percent().to_string(),
                self.install.display().to_string(),
            ],
        );
        ui.add(
            layer,
            label(
                &version,
                game_font(1).with_size(12.0).with_shadow(),
                Color::rgb(0xd0, 0xc8, 0xb0),
                Align::Right,
            ),
            Layout::at(Anchor::BottomRight, 0.0, 20.0).offset(12.0, 8.0),
        );
    }

    /// The panel rectangle of a dialog: `(x, y, w, h)`.
    fn dialog_rect(
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        w: f32,
        h: f32,
        right: bool,
    ) -> (f32, f32, f32, f32) {
        let w = w.min(cw - 16.0);
        let h = h.min(ch - 16.0);
        let y = logo_bottom.min(ch - h - 8.0).max(8.0);
        let x = if right { cw - 16.0 * z - w } else { 16.0 * z };
        (x.max(8.0), y, w, h)
    }

    /// New Scenario: the Game Setup dialog on the right, Map or Options on its left.
    #[allow(clippy::too_many_arguments)]
    fn build_new_scenario(
        &mut self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let (setup, side, sel, preview, entries, base_y) = match &self.screen {
            Screen::Menu(m) => (
                m.data.setup,
                m.data.side,
                m.data.sel,
                m.data.preview.clone(),
                m.entries.clone(),
                0.0f32,
            ),
            Screen::Game(_) => return,
        };
        let _ = base_y;
        let (pw, ph) = (420.0 * z, 390.0 * z);
        let two = cw >= 2.0 * pw + 48.0 * z;
        // With a side dialog open on a narrow canvas only that dialog shows.
        let show_setup = side == Side::None || two;
        let (sx, sy, sw, sh) = Self::dialog_rect(cw, ch, z, logo_bottom, pw, ph, true);
        if show_setup {
            self.build_setup_panel(
                ui,
                layer,
                (sx, sy, sw, sh),
                z,
                &setup,
                sel,
                &entries,
                preview.as_ref(),
                tips,
            );
        }
        match side {
            Side::None => {}
            Side::Map => {
                let (x, y, w, h) = if two {
                    (sx - 14.0 * z - sw, sy, sw, sh)
                } else {
                    Self::dialog_rect(cw, ch, z, logo_bottom, pw, ph, true)
                };
                self.build_map_panel(ui, layer, (x, y, w, h), z, sel, &entries, preview.as_ref());
            }
            Side::Options => {
                let (x, y, w, h) = if two {
                    (sx - 14.0 * z - sw, sy, sw, sh)
                } else {
                    Self::dialog_rect(cw, ch, z, logo_bottom, pw, ph, true)
                };
                self.build_options_panel(ui, layer, (x, y, w, h), z, &setup);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_setup_panel(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        (x, y, w, h): (f32, f32, f32, f32),
        z: f32,
        setup: &Setup,
        sel: usize,
        entries: &[Entry],
        preview: Option<&Preview>,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let t = &self.texts;
        let p = ui.add(layer, Kind::Panel { fill: Fill::Panel }, at(x, y, w, h));
        let pad = 18.0 * z;
        let rowh = 32.0 * z;
        let f = mfont(1, z);
        let f2 = mfont(2, z * 0.9);
        // Map button and the map's name.
        let b = ui.add(
            p,
            button(&t.get(1733, "Map"), f, A_SETUP_MAP, Some('m')),
            at(pad, pad, 76.0 * z, rowh),
        );
        tips.push((b, t.get(1734, "Select the map to play.")));
        let name = entries
            .get(sel)
            .map_or_else(String::new, |e| e.title.clone());
        ui.add(
            p,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(pad + 84.0 * z, pad, w - 2.0 * pad - 84.0 * z, rowh),
        );
        ui.add(
            p,
            label(&name, f2, CREAM, Align::Center),
            at(pad + 90.0 * z, pad, w - 2.0 * pad - 96.0 * z, rowh),
        );
        // Options button and the five skill levels.
        let y2 = pad + rowh + 10.0 * z;
        let b = ui.add(
            p,
            button(&t.get(1735, "Options"), f, A_SETUP_OPTIONS, Some('o')),
            at(pad, y2, 76.0 * z, rowh),
        );
        tips.push((b, t.get(1736, "Customize the options for this game.")));
        let levels = [
            (1737, "Easy"),
            (1739, "Medium"),
            (1741, "Hard"),
            (1743, "Expert"),
            (1745, "Custom"),
        ];
        let lw = (w - 2.0 * pad - 84.0 * z) / 5.0;
        for (i, (id, en)) in levels.iter().enumerate() {
            let n = ui.add(
                p,
                toggle(
                    &t.get(*id, en),
                    mfont(1, z * 0.82),
                    A_LEVEL + i as u32,
                    setup.level == Level::ALL[i],
                ),
                at(pad + 84.0 * z + i as f32 * lw, y2, lw - 3.0 * z, rowh),
            );
            tips.push((n, t.get(*id + 1, "Select a difficulty.")));
        }
        // Computer players: counter with arrows beside the player list.
        let y3 = y2 + rowh + 12.0 * z;
        let scen_computers = preview.map_or(0, |pv| pv.info.persons.saturating_sub(1));
        let list_h = h - y3 - pad - rowh - 40.0 * z;
        let col_w = 100.0 * z;
        ui.add(
            p,
            label(
                &t.get(1747, "Computer Players"),
                mfont(1, z * 0.8),
                GOLD,
                Align::Left,
            ),
            at(pad, y3, col_w + 20.0 * z, 34.0 * z),
        );
        ui.add(
            p,
            label(
                &(scen_computers + setup.extra_computers).to_string(),
                mfont(3, z * 0.9),
                CREAM,
                Align::Center,
            ),
            at(pad, y3 + 38.0 * z, 40.0 * z, 30.0 * z),
        );
        let b = ui.add(
            p,
            button("+", mfont(3, z * 0.7), A_PLAYERS_UP, None),
            at(pad + 46.0 * z, y3 + 36.0 * z, 30.0 * z, 17.0 * z),
        );
        tips.push((
            b,
            t.get(
                379,
                "You cannot add any more computer players for this scenario.",
            ),
        ));
        ui.add(
            p,
            button("-", mfont(3, z * 0.7), A_PLAYERS_DOWN, None),
            at(pad + 46.0 * z, y3 + 55.0 * z, 30.0 * z, 17.0 * z),
        );
        // The player list: you, the scenario's computer players, the ones added.
        let mut players = vec![t.a("You (human player)")];
        for i in 0..scen_computers {
            players.push(t.af("Computer player %1", &[(i + 1).to_string()]));
        }
        for i in 0..setup.extra_computers {
            players.push(t.af(
                "Computer player %1",
                &[(scen_computers + i + 1).to_string()],
            ));
        }
        let list = ui.add(
            p,
            Kind::List {
                items: players,
                selected: None,
                scroll: 0,
                row_h: 22.0 * z,
                font: mfont(2, z * 0.8),
                action: 0,
            },
            at(
                pad + col_w + 28.0 * z,
                y3,
                w - 2.0 * pad - col_w - 28.0 * z,
                list_h.max(40.0 * z),
            ),
        );
        tips.push((
            list,
            t.get(
                1748,
                "The human and computer players currently in the game.",
            ),
        ));
        // Difficulty rating, OK and Cancel.
        let rating = setup.rating(scen_computers);
        let ry = h - pad - rowh - 28.0 * z;
        ui.add(
            p,
            label(
                &crate::text::subst(
                    &t.get(186, "Difficulty Rating: %1"),
                    &[format!("{rating}%")],
                ),
                mfont(1, z * 0.82),
                CREAM,
                Align::Center,
            ),
            at(pad, ry, w - 2.0 * pad, 22.0 * z),
        );
        let row = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(pad, h - pad - rowh, w - 2.0 * pad, rowh),
        );
        ui.set_stack(row, Stack::row(16.0 * z, 0.0).centered());
        let bw = 110.0 * z;
        let b = ui.add(
            row,
            button(&t.get(lng::OK, "OK"), f, A_SETUP_OK, None),
            Layout::at(Anchor::Left, bw, rowh),
        );
        tips.push((b, t.get(1749, "Launch the game as currently set up.")));
        let b = ui.add(
            row,
            button(&t.get(lng::CANCEL, "Cancel"), f, super::A_BACK, Some('c')),
            Layout::at(Anchor::Left, bw, rowh),
        );
        tips.push((b, t.get(1751, "Cancel back to the last menu.")));
    }

    /// The Map dialog: scenario list, description, thumbnail and start date.
    #[allow(clippy::too_many_arguments)]
    fn build_map_panel(
        &mut self,
        ui: &mut Ui,
        layer: NodeId,
        (x, y, w, h): (f32, f32, f32, f32),
        z: f32,
        sel: usize,
        entries: &[Entry],
        preview: Option<&Preview>,
    ) {
        let t = &self.texts;
        let p = ui.add(layer, Kind::Panel { fill: Fill::Panel }, at(x, y, w, h));
        let pad = 18.0 * z;
        let rowh = 32.0 * z;
        let f = mfont(1, z);
        let list_h = (h * 0.34).max(70.0 * z);
        let items: Vec<String> = entries.iter().map(|e| e.title.clone()).collect();
        let list = ui.add(
            p,
            Kind::List {
                items,
                selected: Some(sel).filter(|&s| s < entries.len()),
                scroll: 0,
                row_h: 22.0 * z,
                font: mfont(2, z * 0.85),
                action: A_MAP_LIST,
            },
            at(pad, pad, w - 2.0 * pad, list_h),
        );
        self.menu_handles.list = Some(list);
        self.menu_scroll_to(ui, list, sel);
        ui.set_focus(Some(list));
        let y2 = pad + list_h + 10.0 * z;
        let below = h - y2 - pad - rowh - 8.0 * z;
        let thumb = (below - 46.0 * z).min(w * 0.36).max(60.0 * z);
        let desc_w = w - 2.0 * pad - thumb - 10.0 * z;
        if let Some(pv) = preview {
            let size = 12.5 * z;
            let chars = (desc_w / (size * 0.56)).floor() as usize;
            let lines = wrap_text(&pv.info.description, chars);
            ui.add(
                p,
                Kind::Panel {
                    fill: Fill::Solid(INSET),
                },
                at(pad, y2, desc_w, below),
            );
            ui.add(
                p,
                Kind::Text {
                    lines,
                    scroll: 0,
                    row_h: (size * 1.3).round(),
                    font: game_font(8).with_size(size.round()),
                    color: CREAM,
                },
                at(
                    pad + 6.0 * z,
                    y2 + 4.0 * z,
                    desc_w - 12.0 * z,
                    below - 8.0 * z,
                ),
            );
            // The thumbnail: the map from the terrain grid.
            let tx = pad + desc_w + 10.0 * z;
            ui.add(
                p,
                Kind::Panel {
                    fill: Fill::Solid(INSET),
                },
                at(tx, y2, thumb, thumb),
            );
            let (cols, rows) = (pv.radar.cols as f32, pv.radar.rows as f32);
            let fit = (thumb - 4.0) / cols.max(rows);
            ui.add(
                p,
                Kind::Cells {
                    cols: pv.radar.cols,
                    rows: pv.radar.rows,
                    colors: Arc::new(pv.radar.colors().to_vec()),
                    action: 0,
                },
                at(
                    tx + (thumb - cols * fit) / 2.0,
                    y2 + (thumb - rows * fit) / 2.0,
                    cols * fit,
                    rows * fit,
                ),
            );
            // Date (the arrows only appear on maps that allow a choice; TODO: flag not decoded).
            ui.add(
                p,
                label(
                    &format!("{}: {}", t.get(1753, "Date"), pv.info.year),
                    mfont(1, z * 0.8),
                    GOLD,
                    Align::Center,
                ),
                at(
                    tx - 6.0 * z,
                    y2 + thumb + 4.0 * z,
                    thumb + 12.0 * z,
                    22.0 * z,
                ),
            );
            ui.add(
                p,
                label(
                    &format!("{} x {}", pv.info.width, pv.info.height),
                    mfont(1, z * 0.7),
                    MUTED,
                    Align::Center,
                ),
                at(
                    tx - 6.0 * z,
                    y2 + thumb + 24.0 * z,
                    thumb + 12.0 * z,
                    20.0 * z,
                ),
            );
        }
        let row = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(pad, h - pad - rowh, w - 2.0 * pad, rowh),
        );
        ui.set_stack(row, Stack::row(16.0 * z, 0.0).centered());
        let bw = 110.0 * z;
        ui.add(
            row,
            button(&t.get(lng::OK, "OK"), f, A_MAP_OK, None),
            Layout::at(Anchor::Left, bw, rowh),
        );
        ui.add(
            row,
            button(&t.get(lng::CANCEL, "Cancel"), f, A_MAP_CANCEL, Some('c')),
            Layout::at(Anchor::Left, bw, rowh),
        );
    }

    /// Scrolls a freshly built list so row `sel` is visible.
    fn menu_scroll_to(&self, ui: &mut Ui, list: NodeId, sel: usize) {
        ui.layout(self.viewport);
        let rect = ui.rect(list);
        if let Kind::List { row_h, scroll, .. } = &mut ui.node_mut(list).kind {
            let vis = ((rect.h / *row_h).floor() as usize).max(1);
            if sel >= vis {
                *scroll = sel + 1 - vis;
            }
        }
    }

    /// The Options dialog: sandbox, financial and industrial models, revenue modifiers.
    fn build_options_panel(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        (x, y, w, h): (f32, f32, f32, f32),
        z: f32,
        s: &Setup,
    ) {
        let t = &self.texts;
        let p = ui.add(layer, Kind::Panel { fill: Fill::Panel }, at(x, y, w, h));
        let pad = 20.0 * z;
        let rowh = 26.0 * z;
        let f = mfont(1, z * 0.9);
        let mut yy = pad;
        ui.add(
            p,
            toggle(
                &format!(
                    "[{}] {}",
                    if s.sandbox { "x" } else { " " },
                    t.get(2595, "Sandbox mode")
                ),
                f,
                A_OPT_SANDBOX,
                s.sandbox,
            ),
            at(pad, yy, w - 2.0 * pad, rowh + 4.0 * z),
        );
        yy += rowh + 16.0 * z;
        // Two columns: financial and industrial model.
        let col_w = (w - 2.0 * pad - 12.0 * z) / 2.0;
        for (col, (title, base, current)) in [
            (t.get(1754, "Financial Model"), A_OPT_FIN, s.financial),
            (t.get(1758, "Industrial Model"), A_OPT_IND, s.industrial),
        ]
        .into_iter()
        .enumerate()
        {
            let cx = pad + col as f32 * (col_w + 12.0 * z);
            ui.add(
                p,
                label(&title, mfont(1, z * 0.8), GOLD, Align::Center),
                at(cx, yy, col_w, rowh),
            );
            for (i, (id, en)) in [(1757, "Basic"), (1756, "Advanced"), (1755, "Expert")]
                .iter()
                .enumerate()
            {
                ui.add(
                    p,
                    toggle(
                        &format!(
                            "({}) {}",
                            if Model::ALL[i] == current { "o" } else { " " },
                            t.get(*id, en)
                        ),
                        mfont(1, z * 0.82),
                        base + i as u32,
                        Model::ALL[i] == current,
                    ),
                    at(cx, yy + (rowh + 2.0 * z) * (i as f32 + 1.0), col_w, rowh),
                );
            }
        }
        yy += (rowh + 2.0 * z) * 4.0 + 12.0 * z;
        ui.add(
            p,
            label(
                &t.get(1761, "Revenue Modifier"),
                mfont(1, z * 0.8),
                GOLD,
                Align::Center,
            ),
            at(pad, yy, w - 2.0 * pad, rowh),
        );
        yy += rowh + 4.0 * z;
        let rows = [
            (
                t.get(1762, "Human"),
                s.human_revenue,
                A_OPT_HUMAN_MINUS,
                A_OPT_HUMAN_PLUS,
            ),
            (
                t.get(1763, "Computer"),
                s.computer_revenue,
                A_OPT_COMP_MINUS,
                A_OPT_COMP_PLUS,
            ),
        ];
        for (name, value, minus, plus) in rows {
            ui.add(
                p,
                label(&name, mfont(1, z * 0.8), CREAM, Align::Left),
                at(pad, yy, 90.0 * z, rowh),
            );
            let bx = pad + 96.0 * z;
            ui.add(
                p,
                button("<", mfont(3, z * 0.7), minus, None),
                at(bx, yy, 30.0 * z, rowh),
            );
            ui.add(
                p,
                label(
                    &format!("{value:+}%"),
                    mfont(2, z * 0.9),
                    CREAM,
                    Align::Center,
                ),
                at(bx + 34.0 * z, yy, 70.0 * z, rowh),
            );
            ui.add(
                p,
                button(">", mfont(3, z * 0.7), plus, None),
                at(bx + 108.0 * z, yy, 30.0 * z, rowh),
            );
            yy += rowh + 6.0 * z;
        }
        ui.add(
            p,
            Kind::Label {
                text: t.a("TODO: these options are kept but not applied to the simulation yet."),
                font: mfont(1, z * 0.62),
                color: MUTED,
                align: Align::Left,
                wrap: true,
            },
            at(pad, yy + 2.0 * z, w - 2.0 * pad, 34.0 * z),
        );
        let row = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(pad, h - pad - rowh - 6.0 * z, w - 2.0 * pad, rowh + 6.0 * z),
        );
        ui.set_stack(row, Stack::row(16.0 * z, 0.0).centered());
        let bw = 110.0 * z;
        ui.add(
            row,
            button(&t.get(lng::OK, "OK"), mfont(1, z), A_OPT_OK, None),
            Layout::at(Anchor::Left, bw, rowh + 6.0 * z),
        );
        ui.add(
            row,
            button(
                &t.get(lng::CANCEL, "Cancel"),
                mfont(1, z),
                A_OPT_CANCEL,
                Some('c'),
            ),
            Layout::at(Anchor::Left, bw, rowh + 6.0 * z),
        );
    }

    /// New Campaign (`campaign.win`): the campaign and difficulty, the 18 scenarios, the map and
    /// briefing of the chosen one, its three bonuses and "<Play this Scenario>".
    #[allow(clippy::too_many_arguments)]
    fn build_campaign(
        &mut self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let (camp, preview, entries, notice) = match &self.screen {
            Screen::Menu(m) => (
                m.data.campaign,
                m.data.preview.clone(),
                m.entries.clone(),
                m.data.notice.clone(),
            ),
            Screen::Game(_) => return,
        };
        let t = &self.texts;
        let (_, y, w, h) = Self::dialog_rect(
            cw,
            ch,
            z,
            logo_bottom.min(ch * 0.3),
            880.0 * z,
            440.0 * z,
            false,
        );
        let x = (cw - w) / 2.0;
        let p = ui.add(layer, Kind::Panel { fill: Fill::Panel }, at(x, y, w, h));
        let pad = 18.0 * z;
        let rowh = 30.0 * z;
        let left_w = ((w - 3.0 * pad) * 0.46).max(180.0);
        // Campaign choice and difficulty.
        let names = [t.a("Classic Campaign"), t.a("The Second Century")];
        let sw = (left_w - 6.0 * z) / 2.0;
        for (i, n) in names.iter().enumerate() {
            let b = ui.add(
                p,
                toggle(n, mfont(1, z * 0.7), A_CAMP_SET + i as u32, camp.set == i),
                at(pad + i as f32 * (sw + 6.0 * z), pad, sw, rowh),
            );
            tips.push((
                b,
                t.get(if i == 0 { 3388 } else { 3386 }, "Play a campaign."),
            ));
        }
        let diffs = [
            (2756, "Easy", 2645),
            (2757, "Normal", 2647),
            (2758, "Hard", 2649),
        ];
        let dw = (left_w - 12.0 * z) / 3.0;
        for (i, (id, en, tip_id)) in diffs.iter().enumerate() {
            let b = ui.add(
                p,
                toggle(
                    &t.get(*id, en),
                    mfont(1, z * 0.8),
                    A_CAMP_DIFF + i as u32,
                    camp.difficulty == i,
                ),
                at(
                    pad + i as f32 * (dw + 6.0 * z),
                    pad + rowh + 6.0 * z,
                    dw,
                    rowh,
                ),
            );
            tips.push((b, t.get(*tip_id, "Choose the campaign difficulty.")));
        }
        // The scenarios.
        let ly = pad + 2.0 * rowh + 16.0 * z;
        let items: Vec<String> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let name = if camp.set == 0 {
                    campaign_text_ids(i)
                        .map(|(n, _)| t.get(n, &e.title))
                        .unwrap_or_else(|| e.title.clone())
                } else {
                    e.title.clone()
                };
                format!("{:>2}. {name}", i + 1)
            })
            .collect();
        let list = ui.add(
            p,
            Kind::List {
                items,
                selected: Some(camp.scenario).filter(|&s| s < entries.len()),
                scroll: 0,
                row_h: 22.0 * z,
                font: mfont(2, z * 0.85),
                action: A_CAMP_LIST,
            },
            at(pad, ly, left_w, h - ly - pad - rowh - 12.0 * z),
        );
        tips.push((list, t.get(2253, "Single click to view the map for this scenario.  Double click to play this scenario.")));
        self.menu_scroll_to(ui, list, camp.scenario);
        ui.set_focus(Some(list));
        // Map, briefing and bonuses on the right.
        let rx = pad * 2.0 + left_w;
        let rw = w - rx - pad;
        let thumb = (rw * 0.4).min(h * 0.34);
        if let Some(pv) = &preview {
            ui.add(
                p,
                Kind::Panel {
                    fill: Fill::Solid(INSET),
                },
                at(rx, pad, thumb, thumb),
            );
            let (cols, rows) = (pv.radar.cols as f32, pv.radar.rows as f32);
            let fit = (thumb - 4.0) / cols.max(rows);
            ui.add(
                p,
                Kind::Cells {
                    cols: pv.radar.cols,
                    rows: pv.radar.rows,
                    colors: Arc::new(pv.radar.colors().to_vec()),
                    action: 0,
                },
                at(
                    rx + (thumb - cols * fit) / 2.0,
                    pad + (thumb - rows * fit) / 2.0,
                    cols * fit,
                    rows * fit,
                ),
            );
            let size = 12.5 * z;
            let tw = rw - thumb - 10.0 * z;
            let title = if camp.set == 0 {
                campaign_text_ids(camp.scenario)
                    .map(|(n, _)| t.get(n, &pv.info.title))
                    .unwrap_or_else(|| pv.info.title.clone())
            } else {
                pv.info.title.clone()
            };
            let mut lines = vec![
                title.to_uppercase(),
                format!("{}", pv.info.year),
                String::new(),
            ];
            lines.extend(wrap_text(
                &pv.info.description,
                (tw / (size * 0.56)).floor() as usize,
            ));
            ui.add(
                p,
                Kind::Text {
                    lines,
                    scroll: 0,
                    row_h: (size * 1.3).round(),
                    font: game_font(8).with_size(size.round()),
                    color: CREAM,
                },
                at(rx + thumb + 10.0 * z, pad, tw, thumb),
            );
        }
        // Bonuses (classic campaign only: the second campaign's texts are not in the LNG).
        let by = pad + thumb + 12.0 * z;
        ui.add(
            p,
            label(&t.a("Starting bonus"), mfont(1, z * 0.8), GOLD, Align::Left),
            at(rx, by, rw, 22.0 * z),
        );
        if camp.set == 0
            && let Some((_, bonus)) = campaign_text_ids(camp.scenario)
        {
            for (i, id) in bonus.iter().enumerate() {
                let text = t.get(*id, "");
                let b = ui.add(
                    p,
                    toggle(
                        &format!("({}) {}", if camp.bonus == i { "o" } else { " " }, text),
                        mfont(1, z * 0.62),
                        A_CAMP_BONUS + i as u32,
                        camp.bonus == i,
                    ),
                    at(
                        rx,
                        by + 24.0 * z + i as f32 * (rowh * 0.86 + 3.0 * z),
                        rw,
                        rowh * 0.86,
                    ),
                );
                tips.push((
                    b,
                    t.get(
                        2254,
                        "Choose this item as your starting bonus in this scenario.",
                    ),
                ));
            }
        } else {
            ui.add(
                p,
                Kind::Label {
                    text: t.a("TODO: the bonuses of this campaign are not decoded."),
                    font: mfont(1, z * 0.62),
                    color: MUTED,
                    align: Align::Left,
                    wrap: true,
                },
                at(rx, by + 24.0 * z, rw, 40.0 * z),
            );
        }
        // Play / Back.
        let row = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(rx, h - pad - rowh, rw, rowh),
        );
        ui.set_stack(row, Stack::row(12.0 * z, 0.0));
        let pw = (rw - 12.0 * z) * 0.62;
        ui.add(
            row,
            button(
                &t.get(2255, "<Play this Scenario>"),
                mfont(1, z * 0.72),
                A_CAMP_PLAY,
                Some('p'),
            ),
            Layout::at(Anchor::Left, pw, rowh),
        );
        ui.add(
            row,
            button(
                &t.get(lng::CANCEL, "Cancel"),
                mfont(1, z * 0.8),
                super::A_BACK,
                Some('c'),
            ),
            Layout::at(Anchor::Left, rw - 12.0 * z - pw, rowh),
        );
        if let Some(msg) = notice {
            self.build_notice(ui, layer, cw, ch, z, &msg);
        }
    }

    /// A small modal note over a menu page.
    fn build_notice(&self, ui: &mut Ui, layer: NodeId, cw: f32, ch: f32, z: f32, msg: &str) {
        ui.add(
            layer,
            Kind::Panel {
                fill: Fill::Solid(Color(0, 0, 0, 150)),
            },
            Layout::fill(),
        );
        let (w, h) = ((420.0 * z).min(cw - 16.0), (230.0 * z).min(ch - 16.0));
        let p = ui.add(
            layer,
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, w, h),
        );
        let size = 14.0 * z;
        let lines = wrap_text(msg, ((w - 40.0 * z) / (size * 0.56)).floor() as usize);
        ui.add(
            p,
            Kind::Text {
                lines,
                scroll: 0,
                row_h: (size * 1.3).round(),
                font: game_font(8).with_size(size.round()),
                color: CREAM,
            },
            at(20.0 * z, 20.0 * z, w - 40.0 * z, h - 80.0 * z),
        );
        ui.add(
            p,
            button(
                &self.texts.get(lng::OK, "OK"),
                mfont(1, z),
                A_NOTICE_OK,
                Some('o'),
            ),
            Layout::at(Anchor::Bottom, 110.0 * z, 32.0 * z).offset(0.0, 16.0 * z),
        );
    }

    /// Load Game / Load Campaign: the saved games list with OK and Cancel (`filerq.win`).
    #[allow(clippy::too_many_arguments)]
    fn build_load(
        &mut self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        page: Page,
    ) {
        let Screen::Menu(m) = &self.screen else {
            return;
        };
        let items: Vec<String> = m
            .entries
            .iter()
            .map(|e| match e.source {
                catalog::Source::Own => {
                    self.texts.af("%1  (saved)", std::slice::from_ref(&e.title))
                }
                _ => e.title.clone(),
            })
            .collect();
        let t = &self.texts;
        let title = match page {
            Page::LoadCampaign => t.get(2580, "Load Campaign"),
            _ => t.get(2584, "Load Scenario"),
        };
        let (_, y, w, h) = Self::dialog_rect(cw, ch, z, logo_bottom, 600.0 * z, 400.0 * z, false);
        let x = (cw - w) / 2.0;
        let p = ui.add(layer, Kind::Panel { fill: Fill::Panel }, at(x, y, w, h));
        let pad = 22.0 * z;
        ui.add(
            p,
            label(
                &title,
                mfont(3, z * 0.8).with_shadow(),
                CREAM,
                Align::Center,
            ),
            at(pad, pad, w - 2.0 * pad, 36.0 * z),
        );
        let rowh = 36.0 * z;
        let list = ui.add(
            p,
            Kind::List {
                items,
                selected: None,
                scroll: 0,
                row_h: 26.0 * z,
                font: mfont(6, z * 0.85),
                action: super::A_LIST,
            },
            at(
                pad,
                pad + 46.0 * z,
                w - 2.0 * pad,
                h - 2.0 * pad - 46.0 * z - rowh - 10.0 * z,
            ),
        );
        self.menu_handles.list = Some(list);
        ui.set_focus(Some(list));
        let row = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(pad, h - pad - rowh, w - 2.0 * pad, rowh),
        );
        ui.set_stack(row, Stack::row(16.0 * z, 0.0).centered());
        let f = mfont(1, z);
        ui.add(
            row,
            button(&t.get(lng::OK, "OK"), f, super::A_OK, None),
            Layout::at(Anchor::Left, 130.0 * z, rowh),
        );
        ui.add(
            row,
            button(&t.get(lng::CANCEL, "Cancel"), f, super::A_BACK, Some('c')),
            Layout::at(Anchor::Left, 130.0 * z, rowh),
        );
    }

    /// High Scores and Credits: not built yet.
    #[allow(clippy::too_many_arguments)]
    fn build_soon(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        page: Page,
    ) {
        let t = &self.texts;
        let (_, y, w, h) = Self::dialog_rect(cw, ch, z, logo_bottom, 520.0 * z, 220.0 * z, false);
        let x = (cw - w) / 2.0;
        let p = ui.add(layer, Kind::Panel { fill: Fill::Panel }, at(x, y, w, h));
        let (title, tip) = match page {
            Page::Scores => (
                t.get(1680, "High Scores"),
                t.a("TODO: the high scores list is not built yet."),
            ),
            _ => (
                t.get(1682, "Credits"),
                t.a("TODO: the credits are not built yet."),
            ),
        };
        ui.add(
            p,
            label(
                &title,
                mfont(3, z * 0.8).with_shadow(),
                CREAM,
                Align::Center,
            ),
            at(20.0 * z, 24.0 * z, w - 40.0 * z, 36.0 * z),
        );
        ui.add(
            p,
            Kind::Label {
                text: tip,
                font: mfont(1, z * 0.8),
                color: MUTED,
                align: Align::Center,
                wrap: true,
            },
            at(30.0 * z, 80.0 * z, w - 60.0 * z, 60.0 * z),
        );
        ui.add(
            p,
            button(
                &t.get(lng::CANCEL, "Cancel"),
                mfont(1, z),
                super::A_BACK,
                Some('c'),
            ),
            Layout::at(Anchor::Bottom, 130.0 * z, 34.0 * z).offset(0.0, 20.0 * z),
        );
    }

    /// The list rows of a start path and the entries that [`campaign_entries`] returns, for
    /// tests and screenshots.
    #[allow(dead_code)]
    pub(super) fn first_campaign_path(&self, set: usize) -> Option<PathBuf> {
        campaign_entries(&self.install, set)
            .first()
            .map(|e| e.path.clone())
    }
}

/// Medal of a finished game, for the campaign progress (TODO: persisted progress).
#[allow(dead_code)]
pub(super) fn medal_label(m: Medal) -> &'static str {
    match m {
        Medal::Bronze => "Bronze",
        Medal::Silver => "Silver",
        Medal::Gold => "Gold",
        Medal::Loss => "Loss",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_texts_follow_the_lng_layout() {
        assert_eq!(campaign_text_ids(0), Some((2765, [2766, 2767, 2768])));
        assert_eq!(campaign_text_ids(17), Some((2833, [2834, 2835, 2836])));
        assert_eq!(campaign_text_ids(18), None);
    }

    #[test]
    fn scenarios_and_campaigns_split_the_maps() {
        let dir = std::env::temp_dir().join(format!("rrt2-menus-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("maps")).unwrap();
        for n in [
            "#01Iron.MAP",
            "#e01camp.MP2",
            "#E09camp.Mp2",
            "Alaska.MP2",
            "Austria.MP2",
        ] {
            std::fs::write(dir.join("maps").join(n), b"junk").unwrap();
        }
        let names = |v: Vec<Entry>| -> Vec<String> {
            v.iter()
                .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            names(scenario_entries(&dir)),
            vec!["Alaska.MP2", "Austria.MP2"]
        );
        assert_eq!(names(campaign_entries(&dir, 0)), vec!["#01Iron.MAP"]);
        assert_eq!(
            names(campaign_entries(&dir, 1)),
            vec!["#e01camp.MP2", "#E09camp.Mp2"]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
