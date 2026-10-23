//! The construction tools of the main window: Lay Track (`bpaddtra.win`/`bpaddtr2.win`), Build
//! Station (`bpaddstn.win`) and the Bulldozer (`bpbulldz.win`), with the live preview over the
//! map (docs/reference/screens.md D1, D2, C7; docs/spec/commands.md 3.1 to 3.3).
//!
//! A tool replaces the center list box with its panel (the original swaps the `bp*.win` palette
//! into the same place). Over the map the tool shows what it would do: the path of a track drag,
//! the footprint and catchment of a station, the bulldozer's area. The numbers are the real
//! command's, found by applying it to a clone of the world (`buildtools::preview`), so the
//! preview can never disagree with the result. Nothing here changes a rule: every action is a
//! [`BuildCmd`] sent through `World::apply_build`; the checks of the issuing window (permissions,
//! rights, cash) are the ones of `docs/spec/commands.md` 3.2 and 3.3.

use rt2_formats::map::Map;
use rt2_sim::build::bulldoze::BulldozeError;
use rt2_sim::build::cmd::{BuildCmd, CmdError};
use rt2_ui::{Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::dialogs::Dialog;
use super::{AppState, GameState, Screen};
use crate::buildtools::{self as bt, StationSize};
use crate::text::id as lng;

pub(super) const A_BT_SINGLE: u32 = 5001;
pub(super) const A_BT_DOUBLE: u32 = 5002;
pub(super) const A_BT_PLAIN: u32 = 5003;
pub(super) const A_BT_ELECTRIC: u32 = 5004;
/// Bridge type `n` (1 wooden, 2 stone, 3 iron): `A_BT_BRIDGE + n`.
pub(super) const A_BT_BRIDGE: u32 = 5010;
/// Station size `i`: `A_BT_SIZE + i`.
pub(super) const A_BT_SIZE: u32 = 5020;
/// Orientation `0..8` (`8` = automatic): `A_BT_ORIENT + o`.
pub(super) const A_BT_ORIENT: u32 = 5030;
/// Architecture style `i` of [`STYLES`].
pub(super) const A_BT_STYLE: u32 = 5045;
/// Bulldozer brush `i` of [`DOZER_RADIUS`].
pub(super) const A_BT_DOZER: u32 = 5060;
pub(super) const A_BT_CANCEL: u32 = 5070;

/// `0x53656c`: the bulldozer's radius per brush size (commands.md 2.1, `0x0a`).
pub(super) const DOZER_RADIUS: [i32; 5] = [0, 2, 5, 11, 20];

/// Architecture styles: (name, hint byte of the `0x17` packet; `-1` = the nearest town's).
// TODO(commands.md 3.3): the hint values of the six styles are read from `DAT_005534c8`; the
// letters of the art ("VCMTBK") are used as the index.
pub(super) const STYLES: [(&str, i32); 6] = [
    ("Default", -1),
    ("Victorian", 0),
    ("Colonial", 1),
    ("Mission", 2),
    ("Tudor", 3),
    ("Kyoto", 5),
];

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
pub(super) const BAD: Color = Color::rgb(0xf0, 0x8c, 0x7a);
const GOOD: Color = Color::rgb(0x9a, 0xe0, 0x8a);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolKind {
    Track,
    Station,
    Bulldoze,
}

impl ToolKind {
    /// Index of the main window's action icon.
    fn icon(self) -> usize {
        match self {
            ToolKind::Track => 0,
            ToolKind::Station => 1,
            ToolKind::Bulldoze => 2,
        }
    }

    pub(super) fn from_icon(i: usize) -> Option<ToolKind> {
        [ToolKind::Track, ToolKind::Station, ToolKind::Bulldoze]
            .into_iter()
            .find(|t| t.icon() == i)
    }
}

/// What the active tool would do at the cursor.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Plan {
    /// Tiles the command would change (track path, bulldozed area).
    pub tiles: Vec<(i32, i32)>,
    pub footprint: Vec<(i32, i32)>,
    pub cost: f64,
    /// Why the command is refused, if it is.
    pub error: Option<String>,
    /// Station: anchor tile and the orientation found.
    pub station: Option<(i32, i32, i32)>,
    /// Station: catchment radius.
    pub radius: i32,
    /// Station: the anchor tile already has track (`0x48d900` case 0xe: the placement is green
    /// with a connection to existing track, yellow without, red when invalid; the original's
    /// flag is an output of `0x4087a0` that `rt2-sim` does not expose, so the clone tests the
    /// anchor tile for track).
    pub connected: bool,
}

impl Plan {
    fn ok(&self) -> bool {
        self.error.is_none()
    }
}

/// The construction tool state of an open game.
#[derive(Clone, Debug)]
pub(super) struct BuildState {
    pub tool: Option<ToolKind>,
    /// `track_single`: simple (true) or double track.
    pub single: bool,
    pub electric: bool,
    pub bridge: i32,
    pub size: StationSize,
    /// `None` = try the eight orientations.
    pub orient: Option<i32>,
    pub style: usize,
    pub dozer: usize,
    /// Start tile of a track drag.
    pub from: Option<(i32, i32)>,
    pub hover: Option<(i32, i32)>,
    pub plan: Option<Plan>,
    /// The last result, shown in the panel.
    pub message: Option<(String, bool)>,
}

impl Default for BuildState {
    fn default() -> Self {
        BuildState {
            tool: None,
            single: true,
            electric: false,
            bridge: 1,
            size: StationSize::Small,
            orient: None,
            style: 0,
            dozer: 1,
            from: None,
            hover: None,
            plan: None,
            message: None,
        }
    }
}

fn fnt(n: u8, dz: f32) -> FontStyle {
    let f = game_font(n);
    f.with_size((f.size * dz).round())
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

fn lbl(text: &str, f: FontStyle, color: Color, align: Align) -> Kind {
    Kind::Label {
        text: text.to_owned(),
        font: f,
        color,
        align,
        wrap: false,
    }
}

impl AppState {
    // ---- plumbing ---------------------------------------------------------------------------

    pub(super) fn game(&self) -> Option<&GameState> {
        match &self.screen {
            Screen::Game(g) => Some(g),
            Screen::Menu(_) => None,
        }
    }

    /// The player's company when the save can be played (layout `0x2348`).
    pub(super) fn player(&self) -> Option<i32> {
        let g = self.game()?;
        let save = g.world.save_ref()?;
        bt::playable(save)
            .then(|| bt::human_company(save))
            .flatten()
    }

    pub(super) fn tool_active(&self) -> bool {
        self.game()
            .is_some_and(|g| g.build.tool.is_some() || g.editor.is_some())
    }

    pub(super) fn tool_kind(&self) -> Option<ToolKind> {
        self.game().and_then(|g| g.build.tool)
    }

    /// The text of a refusal of the simulation's command (LNG ids of the original's messages).
    pub(super) fn cmd_error_text(&self, e: CmdError, cash_id: u32) -> String {
        let t = &self.texts;
        match e {
            CmdError::NoPath => t.a("No route can be built between those two places."),
            CmdError::InvalidPlacement => t.get(
                535,
                "You can't put a station here!  (It must be on clear ground, next to existing track.)",
            ),
            CmdError::NoSuchObject => t.a("That object no longer exists."),
            CmdError::TooFewStations => t.get(352, "You can't add a train until you have at least 2 stations"),
            CmdError::InsufficientCash => t.get(
                cash_id,
                "Your company doesn't have enough money to build that!",
            ),
            CmdError::EditorOnly => t.get(23, "That action is not allowed in this scenario."),
            CmdError::NotBuyable => t.a("This industry cannot be bought or sold here."),
            CmdError::Unknown => t.a("The command was refused."),
            CmdError::Bulldoze(b) => {
                let (id, en) = match b {
                    BulldozeError::StationTrack => (
                        1121,
                        "You can't bulldoze the track that is a station's primary outlet.  (Try bulldozing the station first.)",
                    ),
                    BulldozeError::ForeignTrack => (1122, "You can't bulldoze another company's track."),
                    BulldozeError::TrainOnTrack => {
                        (1123, "You can't bulldoze track if there's a train on it.")
                    }
                    BulldozeError::IllegalLayout => {
                        (1125, "Bulldozing that would result in an illegal track layout.")
                    }
                    BulldozeError::Cash => {
                        (1126, "Your company doesn't have enough funds to bulldoze that.")
                    }
                    BulldozeError::ForeignBuilding => {
                        (1127, "You can't bulldoze a building owned by another company.")
                    }
                    BulldozeError::ForeignStation => (
                        1128,
                        "You can't bulldoze a building within another company's station radius.",
                    ),
                };
                t.get(id, en)
            }
            _ => t.a("The command was refused."),
        }
    }

    fn refusal_text(&self, r: bt::Refusal) -> String {
        let t = &self.texts;
        match r {
            bt::Refusal::NotAllowed => t.get(23, "That action is not allowed in this scenario."),
            bt::Refusal::NoCompany => t.get(337, "You can't lay track if you don't control a company."),
            bt::Refusal::NoRights => {
                let terr = self.hover_territory_name();
                t.get(2266, "You don't have rights to lay track in %1")
                    .replace("%1", &terr)
            }
            bt::Refusal::NoCash => t.get(
                341,
                "Your company doesn't have enough money to build a new station!",
            ),
            bt::Refusal::BadStationSite => t.get(
                535,
                "You can't put a station here!  (It must be on clear ground, next to existing track.)",
            ),
            bt::Refusal::CannotAfford => {
                t.get(394, "Your company doesn't have enough money to build that!")
            }
            bt::Refusal::Sim(e) => self.cmd_error_text(e, 394),
        }
    }

    fn hover_territory_name(&self) -> String {
        let Some(g) = self.game() else {
            return String::new();
        };
        let (Some((x, y)), Some(save)) = (g.build.hover, g.world.save_ref()) else {
            return String::new();
        };
        self.territory_name(bt::territory_at(save, x, y))
    }

    /// Name of a territory (`territory + 0x4`?): the region number when the record has none.
    pub(super) fn territory_name(&self, region: i32) -> String {
        self.with_info(|i| {
            i.save
                .territories
                .ordered()
                .find(|t| bt::Rec(&t.data).u8(0x2a) as i32 == region)
                .map(|t| i.territory_page(t.slot as u32).title)
        })
        .flatten()
        .unwrap_or_else(|| self.texts.af("Territory %1", &[region.to_string()]))
    }

    /// Runs a command on the real world; the scene, the live layer and the cash follow.
    pub(super) fn issue(&mut self, cmd: BuildCmd) -> Result<i32, CmdError> {
        let Screen::Game(g) = &mut self.screen else {
            return Err(CmdError::NoSuchObject);
        };
        let r = g.world.apply_build(cmd);
        self.sync_after_build();
        r
    }

    /// After a command: rebuild the static layers, restart the live layer's interpolation and
    /// refresh the cash and the center list.
    pub(super) fn sync_after_build(&mut self) {
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        if let Some(save) = g.world.save_ref() {
            self.pending_live = Some((rt2_render::live::LiveFrame::from_save(save), true));
            let fp = super::scene_fingerprint(save);
            if fp != g.scene_fp {
                g.scene_fp = fp;
                self.rebuild_scene = true;
            }
            g.cash = super::player_cash(save);
        }
        g.build.plan = None;
        self.rebuild_ui();
    }

    /// The tile under the cursor as the Lay Track tool takes it: a press on a station building
    /// means the station's tile (`buildtools::snap_to_station`).
    fn track_tile_under(&self, pos: (f32, f32)) -> Option<(i32, i32)> {
        let (x, y) = self.tile_under(pos)?;
        let save = self.game()?.world.save_ref()?;
        Some(bt::snap_to_station(save, x, y))
    }

    fn tile_under(&self, pos: (f32, f32)) -> Option<(i32, i32)> {
        let g = self.game()?;
        let save = g.world.save_ref()?;
        crate::pick::screen_to_tile(&g.camera, self.viewport, pos, &save.map)
    }

    // ---- selecting a tool -------------------------------------------------------------------

    /// A click on one of the three construction icons: select it, or deselect it again.
    pub(super) fn select_tool(&mut self, kind: ToolKind) {
        let denied = self.player().is_none();
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        if denied {
            g.toast = Some((
                self.texts
                    .a("Construction needs a company: start one first (Start New Company)."),
                4.0,
            ));
            return;
        }
        let same = g.build.tool == Some(kind);
        g.build.tool = if same { None } else { Some(kind) };
        g.build.from = None;
        g.build.plan = None;
        g.build.message = None;
        self.rebuild_ui();
    }

    /// Right button during a track drag: drop the drag, keep the tool.
    pub(super) fn cancel_tool_drag(&mut self) {
        if let Screen::Game(g) = &mut self.screen
            && g.build.from.take().is_some()
        {
            g.build.plan = None;
            self.rebuild_ui();
        }
    }

    pub(super) fn cancel_tool(&mut self) -> bool {
        let Screen::Game(g) = &mut self.screen else {
            return false;
        };
        if g.build.tool.is_none() {
            return false;
        }
        if g.build.from.take().is_none() {
            g.build.tool = None;
        }
        g.build.plan = None;
        self.rebuild_ui();
        true
    }

    // ---- the plan under the cursor ----------------------------------------------------------

    /// Called when the cursor moves: when it enters another tile, the plan is computed again.
    pub(super) fn tool_hover(&mut self, pos: (f32, f32)) {
        if !self.tool_active() {
            return;
        }
        let over_ui = self.ui.over_ui();
        let track = self.tool_kind() == Some(ToolKind::Track);
        let tile = if over_ui {
            None
        } else if track {
            self.track_tile_under(pos)
        } else {
            self.tile_under(pos)
        };
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        if g.build.hover == tile {
            return;
        }
        g.build.hover = tile;
        self.recompute_plan();
        self.rebuild_ui();
    }

    pub(super) fn recompute_plan(&mut self) {
        let Some(co) = self.player() else {
            return;
        };
        let plan = self.make_plan(co);
        if let Screen::Game(g) = &mut self.screen {
            g.build.plan = plan;
        }
    }

    fn make_plan(&self, co: i32) -> Option<Plan> {
        let g = self.game()?;
        let b = &g.build;
        let (x, y) = b.hover?;
        let save = g.world.save_ref()?;
        let data = self.data.as_ref()?;
        let d = g.world.date();
        let (year, month) = (i32::from(d.year), i32::from(d.month));
        match b.tool? {
            ToolKind::Track => {
                let from = b.from?;
                if from == (x, y) {
                    return None;
                }
                let mut plan = Plan::default();
                if let Err(r) = bt::check_track(save, co, [from, (x, y)]) {
                    plan.error = Some(self.refusal_text(r));
                    return Some(plan);
                }
                let bridge = b.bridge.clamp(1, bt::max_bridge_type(year));
                let electric =
                    b.electric && bt::electric_track_available(data, save, co, year, month);
                let p = bt::preview(
                    &g.world,
                    co,
                    BuildCmd::LayTrack {
                        company: co,
                        x1: from.0,
                        y1: from.1,
                        x2: x,
                        y2: y,
                        d1: -1,
                        d2: -1,
                        single: b.single,
                        electric,
                        bridge,
                    },
                );
                plan.cost = p.cost;
                plan.tiles = p.tiles;
                if let Err(e) = p.result {
                    plan.error = Some(self.cmd_error_text(e, 396));
                }
                Some(plan)
            }
            ToolKind::Station => {
                let mut plan = Plan {
                    radius: b.size.radius(),
                    ..Plan::default()
                };
                if !bt::permitted(save, bt::PERM_BUILD_STATION) {
                    plan.error = Some(self.refusal_text(bt::Refusal::NotAllowed));
                    return Some(plan);
                }
                let style = STYLES[b.style.min(5)].1;
                let (o, p) = bt::find_station(&g.world, data, co, b.size, x, y, style, b.orient);
                let shape = b.size.shape(o);
                plan.footprint = bt::station_footprint(data, shape, x, y);
                plan.station = Some((x, y, o));
                // A station stands beside its track: track on the anchor tile or one of the
                // eight tiles around it counts as the connection.
                plan.connected = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        let (tx, ty) = (x + dx, y + dy);
                        tx >= 0
                            && ty >= 0
                            && (tx as usize) < save.map.width
                            && (ty as usize) < save.map.height
                            && save.map.tile(tx as usize, ty as usize).track_id != 0
                    })
                });
                // The real price: the station plus the track the command lays under it.
                plan.cost = if p.result.is_ok() {
                    p.cost
                } else {
                    f64::from(bt::station_price(data, save, co, shape, x, y))
                };
                if let Err(e) = p.result {
                    plan.error = Some(self.cmd_error_text(e, 394));
                }
                Some(plan)
            }
            ToolKind::Bulldoze => {
                let r = DOZER_RADIUS[b.dozer.min(4)];
                let mut plan = Plan::default();
                let p = bt::preview(
                    &g.world,
                    co,
                    BuildCmd::Bulldoze {
                        company: co,
                        x,
                        y,
                        radius: r,
                    },
                );
                plan.cost = p.cost;
                plan.tiles = (x - r..=x + r)
                    .flat_map(|a| (y - r..=y + r).map(move |c| (a, c)))
                    .collect();
                if let Err(e) = p.result {
                    plan.error = Some(self.cmd_error_text(e, 1126));
                }
                Some(plan)
            }
        }
    }

    // ---- mouse ------------------------------------------------------------------------------

    /// Left press on the map with a tool: a track drag starts, a station or a bulldoze is placed.
    pub(super) fn tool_press(&mut self) {
        let Some(pos) = self.cursor else {
            return;
        };
        let Some(kind) = self.tool_kind() else {
            return;
        };
        let tile = if kind == ToolKind::Track {
            self.track_tile_under(pos)
        } else {
            self.tile_under(pos)
        };
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        g.build.hover = tile;
        match kind {
            ToolKind::Track => {
                g.build.from = tile;
                g.build.plan = None;
                self.rebuild_ui();
            }
            ToolKind::Station | ToolKind::Bulldoze => {
                self.recompute_plan();
                self.tool_commit();
            }
        }
    }

    /// Left release: a track drag ends and the track is laid.
    pub(super) fn tool_release(&mut self) {
        if self.tool_kind() != Some(ToolKind::Track) {
            return;
        }
        let Some(pos) = self.cursor else {
            return;
        };
        let tile = self.track_tile_under(pos);
        if let Screen::Game(g) = &mut self.screen {
            g.build.hover = tile;
        }
        self.recompute_plan();
        self.tool_commit();
        if let Screen::Game(g) = &mut self.screen {
            g.build.from = None;
        }
        self.rebuild_ui();
    }

    /// Sends the command of the current plan, if the plan is valid; else shows why not.
    pub(super) fn tool_commit(&mut self) {
        let Some(co) = self.player() else {
            return;
        };
        let (kind, plan, hover, from, b) = match self.game() {
            Some(g) => (
                g.build.tool,
                g.build.plan.clone(),
                g.build.hover,
                g.build.from,
                g.build.clone(),
            ),
            None => return,
        };
        let (Some(kind), Some((x, y))) = (kind, hover) else {
            return;
        };
        let Some(plan) = plan else {
            return;
        };
        if let Some(e) = &plan.error {
            self.set_message(e.clone(), false);
            return;
        }
        let d = self.game().map(|g| g.world.date());
        let year = d.map_or(1830, |d| i32::from(d.year));
        match kind {
            ToolKind::Track => {
                let Some(from) = from else {
                    return;
                };
                let bridge = b.bridge.clamp(1, bt::max_bridge_type(year));
                let electric = b.electric && self.electric_ok();
                let cmd = BuildCmd::LayTrack {
                    company: co,
                    x1: from.0,
                    y1: from.1,
                    x2: x,
                    y2: y,
                    d1: -1,
                    d2: -1,
                    single: b.single,
                    electric,
                    bridge,
                };
                match self.issue(cmd) {
                    Ok(cost) => {
                        let m = self
                            .texts
                            .af("Track laid for %1.", &[crate::info::money(f64::from(cost))]);
                        self.set_message(m, true);
                    }
                    Err(e) => {
                        let m = self.cmd_error_text(e, 396);
                        self.set_message(m, false);
                    }
                }
            }
            ToolKind::Station => {
                let Some((sx, sy, o)) = plan.station else {
                    return;
                };
                let Some(data) = self.data.as_ref() else {
                    return;
                };
                let (cash_ok, cash) = match self.game().and_then(|g| g.world.save_ref()) {
                    Some(s) => (bt::cash(s, co) > 0.0, bt::cash(s, co)),
                    None => return,
                };
                if !cash_ok {
                    let m = self.refusal_text(bt::Refusal::NoCash);
                    self.set_message(m, false);
                    return;
                }
                let _ = cash;
                let shape = b.size.shape(o);
                let cmd = bt::station_command(data, co, shape, sx, sy, STYLES[b.style.min(5)].1);
                match self.issue(cmd) {
                    Ok(building) => {
                        let st = self
                            .game()
                            .and_then(|g| g.world.save_ref())
                            .and_then(|s| {
                                bt::rec(&s.objects, building).map(|r| i32::from(r.u16(8)))
                            })
                            .unwrap_or(0);
                        let m = self.texts.a("Station built.");
                        self.set_message(m, true);
                        if st > 0 {
                            self.cancel_tool_silently();
                            self.push_dialog(Dialog::Station { station: st });
                        }
                    }
                    Err(e) => {
                        let m = self.cmd_error_text(e, 394);
                        self.set_message(m, false);
                    }
                }
            }
            ToolKind::Bulldoze => {
                let r = DOZER_RADIUS[b.dozer.min(4)];
                match self.issue(BuildCmd::Bulldoze {
                    company: co,
                    x,
                    y,
                    radius: r,
                }) {
                    Ok(cost) => {
                        let m = self
                            .texts
                            .af("Bulldozed for %1.", &[crate::info::money(f64::from(cost))]);
                        self.set_message(m, true);
                    }
                    Err(e) => {
                        let m = self.cmd_error_text(e, 1126);
                        self.set_message(m, false);
                    }
                }
            }
        }
    }

    fn cancel_tool_silently(&mut self) {
        if let Screen::Game(g) = &mut self.screen {
            g.build.tool = None;
            g.build.from = None;
            g.build.plan = None;
        }
    }

    pub(super) fn set_message(&mut self, text: String, good: bool) {
        if let Screen::Game(g) = &mut self.screen {
            g.build.message = Some((text, good));
        }
        self.rebuild_ui();
    }

    fn electric_ok(&self) -> bool {
        let (Some(g), Some(data), Some(co)) = (self.game(), self.data.as_ref(), self.player())
        else {
            return false;
        };
        let (Some(save), d) = (g.world.save_ref(), g.world.date()) else {
            return false;
        };
        bt::electric_track_available(data, save, co, i32::from(d.year), i32::from(d.month))
    }

    // ---- panel actions ----------------------------------------------------------------------

    pub(super) fn build_activated(&mut self, action: u32) -> bool {
        let Screen::Game(g) = &mut self.screen else {
            return false;
        };
        let b = &mut g.build;
        match action {
            A_BT_SINGLE => b.single = true,
            A_BT_DOUBLE => b.single = false,
            A_BT_PLAIN => b.electric = false,
            A_BT_ELECTRIC => b.electric = true,
            a if (A_BT_BRIDGE + 1..A_BT_BRIDGE + 4).contains(&a) => {
                b.bridge = (a - A_BT_BRIDGE) as i32
            }
            a if (A_BT_SIZE..A_BT_SIZE + 3).contains(&a) => {
                b.size = StationSize::from_index((a - A_BT_SIZE) as usize);
            }
            a if (A_BT_ORIENT..A_BT_ORIENT + 9).contains(&a) => {
                let o = (a - A_BT_ORIENT) as i32;
                b.orient = (o < 8).then_some(o);
            }
            a if (A_BT_STYLE..A_BT_STYLE + 6).contains(&a) => b.style = (a - A_BT_STYLE) as usize,
            a if (A_BT_DOZER..A_BT_DOZER + 5).contains(&a) => b.dozer = (a - A_BT_DOZER) as usize,
            A_BT_CANCEL => {
                b.tool = None;
                b.from = None;
            }
            _ => return false,
        }
        b.plan = None;
        self.recompute_plan();
        self.rebuild_ui();
        true
    }

    // ---- the panel --------------------------------------------------------------------------

    /// The tool's palette, in the place of the center list box.
    pub(super) fn build_tool_panel(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        w: f32,
        _h: f32,
        dz: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let Some(g) = self.game() else {
            return;
        };
        let b = &g.build;
        let Some(kind) = b.tool else {
            return;
        };
        // `bpaddstn.win`: the Add Station palette in the original's own art, when the install has
        // it.
        if kind == ToolKind::Station && self.build_station_art_panel(ui, parent, (w, _h), dz, tips)
        {
            return;
        }
        let t = &self.texts;
        let pad = 8.0 * dz;
        let row_h = (24.0 * dz).round();
        let f1 = fnt(1, dz * 0.92);
        let year = i32::from(g.world.date().year);
        let (title, hint) = match kind {
            ToolKind::Track => (
                t.get(2453, "Lay Track"),
                t.a("Drag from a track end or a station to where the track should go. Esc cancels."),
            ),
            ToolKind::Station => (
                t.get(1853, "Add Station"),
                t.a("Click next to existing track to place the station. The shaded area is its catchment."),
            ),
            ToolKind::Bulldoze => (
                t.get(1943, "Bulldoze"),
                t.a("Click to bulldoze the shaded area. Stations cannot be bulldozed."),
            ),
        };
        ui.add(
            parent,
            lbl(&title.to_uppercase(), fnt(2, dz), HEAD, Align::Left),
            at(pad, 4.0 * dz, w * 0.5, row_h),
        );
        let (cash, cash_txt) = g
            .world
            .save_ref()
            .zip(self.player())
            .map(|(s, c)| bt::cash(s, c))
            .map_or((0.0, String::new()), |c| (c, crate::info::money(c)));
        let _ = cash;
        ui.add(
            parent,
            lbl(
                &format!("{} {}", t.get(lng::CASH, "Cash"), cash_txt),
                f1,
                TEXT,
                Align::Right,
            ),
            at(w * 0.5, 4.0 * dz, w * 0.5 - pad, row_h),
        );
        // A row of toggles from a list of (label, action, on, enabled, tip).
        let mut y = 4.0 * dz + row_h;
        let mut toggle_row =
            |ui: &mut Ui,
             y: &mut f32,
             caption: &str,
             items: &[(String, u32, bool, bool, String)]| {
                ui.add(
                    parent,
                    lbl(caption, f1, MUTED, Align::Left),
                    at(pad, *y, 110.0 * dz, row_h),
                );
                let x0 = pad + 110.0 * dz;
                let n = items.len().max(1) as f32;
                let bw = ((w - x0 - pad) / n).min(120.0 * dz);
                for (i, (text, action, on, enabled, tip)) in items.iter().enumerate() {
                    let node = ui.add(
                        parent,
                        Kind::Toggle {
                            text: text.clone(),
                            font: f1,
                            action: *action,
                            on: *on,
                            enabled: *enabled,
                        },
                        at(x0 + i as f32 * bw, *y + 1.0, bw - 3.0, row_h - 2.0),
                    );
                    tips.push((node, tip.clone()));
                }
                *y += row_h;
            };
        let item = |text: String, action: u32, on: bool, enabled: bool, tip: String| {
            (text, action, on, enabled, tip)
        };
        match kind {
            ToolKind::Track => {
                toggle_row(
                    ui,
                    &mut y,
                    &t.a("Track"),
                    &[
                        item(
                            t.get(243, "Single Track"),
                            A_BT_SINGLE,
                            b.single,
                            true,
                            t.get(1962, "Lay single track."),
                        ),
                        item(
                            t.get(242, "Double Track"),
                            A_BT_DOUBLE,
                            !b.single,
                            true,
                            t.get(1963, "Lay double track."),
                        ),
                    ],
                );
                let elec_ok = self.electric_ok();
                toggle_row(
                    ui,
                    &mut y,
                    &t.a("Power"),
                    &[
                        item(
                            t.a("Ordinary"),
                            A_BT_PLAIN,
                            !b.electric,
                            true,
                            t.get(1964, "Lay ordinary, non-electrified track."),
                        ),
                        item(
                            t.get(2504, "Electric"),
                            A_BT_ELECTRIC,
                            b.electric,
                            elec_ok,
                            t.get(1965, "Lay electrified track, to allow electric trains."),
                        ),
                    ],
                );
                let maxb = bt::max_bridge_type(year);
                let names = [
                    t.a("Wooden bridge"),
                    t.a("Stone bridge"),
                    t.a("Iron bridge"),
                ];
                let items: Vec<_> = (1..=3)
                    .map(|n| {
                        item(
                            names[n as usize - 1].clone(),
                            A_BT_BRIDGE + n,
                            b.bridge.clamp(1, maxb) == n as i32,
                            n as i32 <= maxb,
                            t.a("The kind of bridge to build over water."),
                        )
                    })
                    .collect();
                toggle_row(ui, &mut y, &t.a("Bridges"), &items);
            }
            ToolKind::Station => {
                let (Some(save), Some(data), Some(co)) =
                    (g.world.save_ref(), self.data.as_ref(), self.player())
                else {
                    return;
                };
                let items: Vec<_> = StationSize::ALL
                    .iter()
                    .map(|s| {
                        let shape = s.shape(0);
                        let price = bt::station_price(data, save, co, shape, 0, 0);
                        item(
                            format!(
                                "{} ${}K",
                                t.get(
                                    1043 + s.index() as u32,
                                    ["Small", "Medium", "Large"][s.index() as usize]
                                )
                                .replace(" Station", ""),
                                price / 1000
                            ),
                            A_BT_SIZE + s.index() as u32,
                            b.size == *s,
                            true,
                            t.get(1076 + s.index() as u32, ""),
                        )
                    })
                    .collect();
                toggle_row(ui, &mut y, &t.get(545, "Size"), &items);
                let names = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
                let mut items: Vec<_> = (0..8)
                    .map(|o| {
                        item(
                            names[o].to_owned(),
                            A_BT_ORIENT + o as u32,
                            b.orient == Some(o as i32),
                            true,
                            t.a("The direction of the station's track."),
                        )
                    })
                    .collect();
                items.push(item(
                    t.a("Auto"),
                    A_BT_ORIENT + 8,
                    b.orient.is_none(),
                    true,
                    t.get(2556, "Change the station direction dynamically, to try to best fit nearby track."),
                ));
                toggle_row(ui, &mut y, &t.a("Direction"), &items);
                let items: Vec<_> = STYLES
                    .iter()
                    .enumerate()
                    .map(|(i, (n, _))| {
                        item(
                            t.a(n),
                            A_BT_STYLE + i as u32,
                            b.style == i,
                            true,
                            t.a("The architecture of the station building."),
                        )
                    })
                    .collect();
                toggle_row(ui, &mut y, &t.a("Style"), &items);
            }
            ToolKind::Bulldoze => {
                let items: Vec<_> = (0..5)
                    .map(|i| {
                        item(
                            format!("{}", i + 1),
                            A_BT_DOZER + i as u32,
                            b.dozer == i,
                            true,
                            t.a("The size of the area the bulldozer clears."),
                        )
                    })
                    .collect();
                toggle_row(ui, &mut y, &t.a("Brush"), &items);
            }
        }
        // Cost and the message.
        let plan = b.plan.as_ref();
        let cost_txt = match plan {
            Some(p) if p.ok() || p.cost > 0.0 => t.af("Cost: %1", &[crate::info::money(p.cost)]),
            _ => t.af("Cost: %1", &["-".to_owned()]),
        };
        ui.add(
            parent,
            lbl(&cost_txt, fnt(1, dz), HEAD, Align::Left),
            at(pad, y + 2.0, w * 0.4, row_h),
        );
        let (msg, color) = match (plan.and_then(|p| p.error.clone()), &b.message) {
            (Some(e), _) => (e, BAD),
            (None, Some((m, good))) => (m.clone(), if *good { GOOD } else { BAD }),
            (None, None) => (hint, MUTED),
        };
        let cancel_w = 90.0 * dz;
        ui.add(
            parent,
            Kind::Button {
                text: t.get(lng::CANCEL, "Cancel"),
                font: f1,
                action: A_BT_CANCEL,
                enabled: true,
                hotkey: None,
            },
            Layout::at(Anchor::BottomRight, cancel_w, row_h).offset(pad, 4.0 * dz),
        );
        ui.add(
            parent,
            Kind::Label {
                text: msg,
                font: fnt(8, dz * 0.62),
                color,
                align: Align::Left,
                wrap: true,
            },
            Layout::at(Anchor::BottomLeft, w - cancel_w - 3.0 * pad, row_h * 1.4)
                .offset(pad, 4.0 * dz),
        );
    }

    // ---- the map overlay --------------------------------------------------------------------

    /// Highlights over the map: the tile under the cursor, the path or footprint of the plan and
    /// the catchment of a station (one translucent square per tile).
    pub(super) fn build_overlay(&self, ui: &mut Ui) {
        let Some(g) = self.game() else {
            return;
        };
        let Some(kind) = g.build.tool else {
            return;
        };
        let Some(save) = g.world.save_ref() else {
            return;
        };
        let map = &save.map;
        let root = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let tile = |ui: &mut Ui, x: i32, y: i32, c: Color, shrink: f32| {
            if x < 0 || y < 0 || x as usize >= map.width || y as usize >= map.height {
                return;
            }
            let (px, py, w, h) = self.tile_rect(g, map, x, y, shrink);
            ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Overlay(c),
                },
                at(px, py, w, h),
            );
        };
        let plan = g.build.plan.as_ref();
        let ok = plan.is_none_or(Plan::ok);
        let (good, bad) = (Color(0x60, 0xe0, 0x60, 0x90), Color(0xf0, 0x50, 0x40, 0x90));
        match kind {
            ToolKind::Station => {
                if let Some(p) = plan
                    && let Some((sx, sy, _)) = p.station
                {
                    // The catchment: tiles within the radius of the station's centre.
                    // The manual: "a gold circle around them on the map".
                    let c = Color(0xf0, 0xc8, 0x40, 0x38);
                    let r = p.radius;
                    let (cx, cy) = p
                        .footprint
                        .iter()
                        .fold((0, 0, 0), |a, t| (a.0 + t.0, a.1 + t.1, a.2 + 1))
                        .pipe_center(sx, sy);
                    for dx in -r..=r {
                        for dy in -r..=r {
                            if dx * dx + dy * dy <= r * r + r / 2 {
                                tile(ui, cx + dx, cy + dy, c, 0.15);
                            }
                        }
                    }
                    // The station itself: green (connected to track), yellow (valid, no track
                    // there yet) or red (invalid), as the manual's Build a New Station says.
                    let yellow = Color(0xf0, 0xe0, 0x50, 0x90);
                    let body = match (ok, p.connected) {
                        (true, true) => good,
                        (true, false) => yellow,
                        (false, _) => bad,
                    };
                    for &(x, y) in &p.footprint {
                        tile(ui, x, y, body, 0.0);
                    }
                }
            }
            ToolKind::Track => {
                if let Some(p) = plan {
                    for &(x, y) in p.tiles.iter().take(800) {
                        tile(ui, x, y, if ok { good } else { bad }, 0.1);
                    }
                }
                if let Some((x, y)) = g.build.from {
                    tile(ui, x, y, Color(0xff, 0xff, 0xff, 0xa0), 0.0);
                }
            }
            ToolKind::Bulldoze => {
                if let Some(p) = plan {
                    for &(x, y) in p.tiles.iter().take(900) {
                        tile(
                            ui,
                            x,
                            y,
                            if ok {
                                Color(0xf0, 0x90, 0x30, 0x70)
                            } else {
                                bad
                            },
                            0.1,
                        );
                    }
                }
            }
        }
        if let Some((x, y)) = g.build.hover {
            tile(ui, x, y, Color(0xff, 0xff, 0xff, 0x70), 0.0);
        }
    }

    /// Logical rectangle of a tile: the diamond's bounding box, shrunk by `shrink`.
    pub(super) fn tile_rect(
        &self,
        g: &GameState,
        map: &Map,
        x: i32,
        y: i32,
        shrink: f32,
    ) -> (f32, f32, f32, f32) {
        let k = self.scale.factor();
        let (px, py) = crate::pick::tile_to_screen(
            &g.camera,
            self.viewport,
            map,
            x as f32 + 0.5,
            y as f32 + 0.5,
        );
        let z = g.camera.zoom / k;
        let (w, h) = (z * (1.0 - shrink), z * 0.5 * (1.0 - shrink));
        (px / k - w / 2.0, py / k - h / 2.0, w.max(2.0), h.max(1.0))
    }
}

/// `(sum x, sum y, n)` to the integer centre, or `(sx, sy)` when empty.
trait PipeCenter {
    fn pipe_center(self, sx: i32, sy: i32) -> (i32, i32);
}

impl PipeCenter for (i32, i32, i32) {
    fn pipe_center(self, sx: i32, sy: i32) -> (i32, i32) {
        if self.2 == 0 {
            (sx, sy)
        } else {
            (self.0 / self.2, self.1 / self.2)
        }
    }
}
