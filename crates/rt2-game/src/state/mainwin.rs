//! The in-game main window (manual chapter 5, "The Main Interface"; `mainwin.win`,
//! `docs/formats/ui.md`): the map with, over it,
//!
//! * the eight action icons down the left edge (Lay Track, Build New Station, Bulldoze, Map
//!   Overviews, Purchase Train, Stock Market, Control Panel, File Options),
//! * the bottom panel: radar (B), the center list box with its four tabs (C) and the activity
//!   box (D), with the status line along the bottom (label 255 of the window),
//! * the four navigation icons above the activity box (rotate x2, zoom x2).
//!
//! The original is a fixed 1024x768 layout; here every size derives from the logical canvas
//! (`dz`, grown with the canvas height) so the window is usable from a 640x480 canvas to 4K at
//! any UI scale. The date and cash labels and the speed controls sit where the original puts the
//! date and cash (left of the tabs) and in the top right corner (the original has them only on
//! the `+` / `-` keys).
//!
//! Construction tools are not in the simulation yet: their icons open a clearly marked
//! placeholder panel (`dialogs.rs`).

use std::sync::Arc;

use rt2_ui::{
    Align, Anchor, Color, Fill, Fit, FontStyle, Kind, Layout, NodeId, SpriteId, Stack, Ui, UiEvent,
    game_font,
};
use winit::keyboard::KeyCode;

use super::dialogs::Dialog;
use super::info_ui::{self, InfoScreen};
use super::{AppState, Screen, format_date};
use crate::info::{Info, Table};
use crate::scenario_info::wrap_text;
use crate::text::id as lng;

pub(super) const A_TOOL: u32 = 300;
pub(super) const A_ROTATE_RIGHT: u32 = 310;
pub(super) const A_ROTATE_LEFT: u32 = 311;
pub(super) const A_ZOOM_OUT: u32 = 312;
pub(super) const A_ZOOM_IN: u32 = 313;
pub(super) const A_RADAR: u32 = 320;
pub(super) const A_CENTER_TAB: u32 = 330;
pub(super) const A_CENTER_ROW: u32 = 340;
pub(super) const A_LIST_SCREEN: u32 = 341;
pub(super) const A_BRIEFING: u32 = 342;
/// The small message button of the activity box: history on / off.
pub(super) const A_NEWS: u32 = 343;

/// The eight action icons: first record of the (up, down) pair in `mainbut.imb`, the LNG id of
/// the status-line hint, the English hint and a short caption for when there is no art.
const TOOLS: [(usize, u32, &str, &str); 8] = [
    (20, 1778, "Lay track", "Track"),
    (24, 1780, "Build a new station", "Station"),
    (
        22,
        1779,
        "Bulldoze objects (track, buildings, etc.)",
        "Bulldoze",
    ),
    (42, 1788, "Map Overview", "Maps"),
    (32, 1784, "Purchase a new train", "Trains"),
    (34, 1785, "Go to the stock market", "Stock"),
    (36, 1786, "Control panel", "Panel"),
    (
        38,
        1787,
        "File options (Load, Save, Quit, New Game)",
        "File",
    ),
];

/// Rotate right, rotate left, zoom out, zoom in (records 0, 2, 4, 6 of `mainbut.imb`).
const NAV: [(usize, u32, u32, &str, &str); 4] = [
    (0, A_ROTATE_RIGHT, 1789, "Rotate the map clockwise", "R+"),
    (
        2,
        A_ROTATE_LEFT,
        1790,
        "Rotate the map counter clockwise",
        "R-",
    ),
    (
        4,
        A_ZOOM_OUT,
        1791,
        "Zoom out (view a larger section of the map)",
        "-",
    ),
    (
        6,
        A_ZOOM_IN,
        1792,
        "Zoom in (view a smaller section of the map)",
        "+",
    ),
];

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);

/// State of the center list box.
#[derive(Clone, Copy, Debug)]
pub(super) struct CenterState {
    /// 0 Stations, 1 Trains, 2 Players, 3 Companies.
    pub tab: usize,
    pub sel: Option<usize>,
    pub scroll: usize,
    /// The activity box shows the message history instead of the logo.
    pub news_open: bool,
}

impl Default for CenterState {
    /// The original opens on the Companies tab (the screenshots of the manual and the game).
    fn default() -> CenterState {
        CenterState {
            tab: 3,
            sel: None,
            scroll: 0,
            news_open: false,
        }
    }
}

/// Widgets of the main window the per-frame update writes to.
#[derive(Default, Clone)]
pub(super) struct MainIds {
    pub date: Option<NodeId>,
    pub cash: Option<NodeId>,
    pub speed: Option<NodeId>,
    pub pause: Option<NodeId>,
    pub toast: Option<NodeId>,
    pub status: Option<NodeId>,
    pub radar: Option<NodeId>,
    pub center: Option<NodeId>,
    /// The compass picture and the view rotation it shows.
    pub compass: Option<NodeId>,
    pub compass_rot: u8,
    /// The four tab bars of the tab strip, in tab order.
    pub tab_nodes: Vec<NodeId>,
    /// The floating tooltip over the tabs: its box and its label.
    pub tooltip: Option<(NodeId, NodeId)>,
    /// Hint texts of the widgets, shown on the status line while the mouse is over them.
    pub tips: Vec<(NodeId, String)>,
}

/// An entry of the activity box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NewsItem {
    pub date: String,
    pub text: String,
}

/// Size of the original layout every position below is taken from (`mainwin.win`).
const DESIGN: (f32, f32) = (1024.0, 768.0);

/// Left rail plates of the tool icons (`mwfrme.imb` 0 and 2: 44x34 holes at x 16): design y of
/// the nine plates and the plate of each of the eight tools (the first plate, at the top, stays
/// empty and shows the map, as in the original).
const PLATE_X: f32 = 16.0;
const PLATE_Y: [f32; 9] = [40.0, 85.0, 130.0, 175.0, 220.0, 272.0, 317.0, 362.0, 407.0];
pub(super) const TOOL_PLATE: [usize; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
/// The four tab bars over the tab strip (`mainwin.win` bars 257, 258, 260, 259).
const TAB_X: [f32; 4] = [490.0, 542.0, 594.0, 646.0];
const TAB_W: [f32; 4] = [52.0, 52.0, 51.0, 52.0];
/// Rotate right, rotate left, zoom out, zoom in (`mainwin.win` buttons 216..219).
const NAV_X: [f32; 4] = [788.0, 844.0, 903.0, 959.0];
const TEAL: Color = Color(0x5a, 0xb4, 0xa4, 0xc0);

/// The chrome sits on a canvas that is at least the design size scaled by `dz`.
fn money_k(v: f64) -> String {
    let k = if v.is_finite() {
        (v / 1000.0).round()
    } else {
        0.0
    };
    format!("{}K", crate::info::money(k))
}

fn font(n: u8, dz: f32) -> FontStyle {
    let f = game_font(n);
    f.with_size((f.size * dz).round())
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

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

fn at4(r: [f32; 4]) -> Layout {
    at(r[0], r[1], r[2], r[3])
}

fn panel(fill: Fill) -> Kind {
    Kind::Panel { fill }
}

/// Which compass picture (`compass.imb`: 0 north to the upper right, 1 upper left, 2 lower left,
/// 3 lower right) shows where world north (-y) points on the screen for a view rotation.
pub(super) fn compass_index(rotation: u8) -> usize {
    let mut cam = rt2_render::Camera::fit(1, 1, (1, 1));
    cam.rotation = rotation % 4;
    let (f, r) = (cam.forward(), cam.right());
    // North is (0, -1): its components along the screen's up and right axes.
    let (up, right) = (-f.y, -r.y);
    match (right >= 0.0, up >= 0.0) {
        (true, true) => 0,
        (false, true) => 1,
        (false, false) => 2,
        (true, false) => 3,
    }
}

/// Where everything of the main window goes. The window is the original 1024x768 design scaled
/// by `dz` (the largest scale that fits the canvas) and pinned to the bottom; on a canvas wider
/// than that the extra width `ex` is inserted in the middle of the bottom frame, so the left
/// parts (radar, tools) keep the left edge, the right parts (nav buttons, activity box,
/// compass) keep the right edge and the center box grows.
#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    pub(super) dz: f32,
    /// Top of the design area in logical units.
    pub(super) y0: f32,
    /// Extra width between the left and right parts.
    pub(super) ex: f32,
    /// Height of the strip the bottom frame covers on the map (the camera frames what is above).
    pub(super) ph: f32,
}

pub(super) fn geometry(cw: f32, ch: f32) -> Geometry {
    let dz = (ch / DESIGN.1).min(cw / DESIGN.0).max(0.3);
    let ex = (cw - (DESIGN.0 * dz).round()).max(0.0);
    Geometry {
        dz,
        y0: ch - (DESIGN.1 * dz).round(),
        ex,
        ph: ((DESIGN.1 - 470.0) * dz).round(),
    }
}

impl Geometry {
    /// X of a design position in the left part.
    pub(super) fn x(&self, d: f32) -> f32 {
        (d * self.dz).round()
    }

    /// X of a design position in the right part.
    pub(super) fn xr(&self, d: f32) -> f32 {
        (d * self.dz).round() + self.ex
    }

    pub(super) fn y(&self, d: f32) -> f32 {
        self.y0 + (d * self.dz).round()
    }

    /// A rectangle `[x, y, w, h]` of the left part (design units in, logical out).
    pub(super) fn rl(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let (x0, x1) = (self.x(x), self.x(x + w));
        [x0, self.y(y), x1 - x0, self.y(y + h) - self.y(y)]
    }

    /// A rectangle of the right part.
    pub(super) fn rr(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let (x0, x1) = (self.xr(x), self.xr(x + w));
        [x0, self.y(y), x1 - x0, self.y(y + h) - self.y(y)]
    }

    /// A rectangle from a position of the left part to one of the right part.
    pub(super) fn span(&self, x0: f32, x1: f32, y: f32, h: f32) -> [f32; 4] {
        let (a, b) = (self.x(x0), self.xr(x1));
        [a, self.y(y), b - a, self.y(y + h) - self.y(y)]
    }

    /// The inside of the center box: the list area (`mainwin.win` list 325).
    pub(super) fn center(&self) -> [f32; 4] {
        self.span(287.0, 699.0, 526.0, 158.0)
    }
}

/// The parts of the window the in-game HUD covers, as `[x, y, w, h]` in logical units: the bottom
/// frame, the two rails, the rotate/zoom buttons and the compass with the speed controls (the
/// same geometry as `build_mainwin`).
pub(super) fn hud_blockers(cw: f32, ch: f32) -> Vec<[f32; 4]> {
    let g = geometry(cw, ch);
    let top = g.y(470.0);
    vec![
        [0.0, top, cw, ch - top],
        g.rl(0.0, 6.0, 85.0, 397.0),
        g.rr(954.0, 63.0, 70.0, 340.0),
        g.rr(780.0, 425.0, 244.0, 45.0),
        [
            g.xr(929.0) - 230.0 * g.dz,
            g.y(14.0),
            cw - g.xr(929.0) + 230.0 * g.dz,
            62.0 * g.dz,
        ],
    ]
}

/// Row height, header height and the number of rows the center list shows for `tab`.
fn center_rows(gm: &Geometry, tab: usize) -> (f32, f32, usize) {
    let h = gm.center()[3];
    // The companies tab has no header and taller rows (logo, name, cash and profits).
    let (rowh, head_h) = if tab == 3 {
        ((40.0 * gm.dz).round(), 0.0)
    } else {
        let rowh = (19.0 * gm.dz).round();
        (rowh, rowh + 4.0)
    };
    (
        rowh,
        head_h,
        (((h - head_h) / rowh).floor() as usize).max(1),
    )
}

impl AppState {
    /// Runs `f` with the information tables over the open game (none without game state).
    pub(super) fn with_info<R>(&self, f: impl FnOnce(&Info) -> R) -> Option<R> {
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        let save = g.world.save_ref()?;
        let data = self.data.as_ref()?;
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
        Some(f(&info))
    }

    /// The table of the center list box tab.
    fn center_table(&self, tab: usize) -> Table {
        self.with_info(|i| match tab {
            0 => i.stations_list().table,
            1 => i.trains_list().table,
            2 => i.players_list().table,
            _ => i.stock_market().table,
        })
        .unwrap_or_default()
    }

    pub(super) fn center_tab_names(&self) -> [String; 4] {
        [
            self.texts.a("Stations"),
            self.texts.get(lng::TRAINS, "Trains"),
            self.texts.get(1726, "Players"),
            self.texts.a("Companies"),
        ]
    }

    /// A camera that frames the whole map in the part of the screen the bottom frame leaves
    /// free (the map is drawn under the frame, so the view is shifted up by half of it).
    pub(super) fn fit_camera(&self, w: usize, h: usize) -> rt2_render::Camera {
        let k = self.scale.factor();
        let ph = geometry(self.viewport.0 as f32 / k, self.viewport.1 as f32 / k).ph * k;
        let free = (self.viewport.1 as f32 - ph).max(64.0) as u32;
        let mut cam = rt2_render::Camera::fit(w, h, (self.viewport.0, free));
        cam.target -= cam.forward() * (ph / cam.zoom);
        cam
    }

    /// Builds the main window over the map: the frame of the original art (rails, bottom frame
    /// with the radar, center and activity boxes), the tool icons, the tab strip, the
    /// rotate/zoom buttons and the compass, all placed by [`Geometry`].
    pub(super) fn build_mainwin(&mut self, ui: &mut Ui) {
        let (cw, ch) = ui.logical_size();
        let gm = geometry(cw, ch);
        let a = gm.dz;
        let art = self.dialog_art.main;
        let (center_tab, center_sel, center_scroll, news_open) = match &self.screen {
            Screen::Game(g) => (
                g.center.tab,
                g.center.sel,
                g.center.scroll,
                g.center.news_open,
            ),
            Screen::Menu(_) => return,
        };
        let rotation = match &self.screen {
            Screen::Game(g) => g.camera.rotation,
            Screen::Menu(_) => 0,
        };
        let mut tips: Vec<(NodeId, String)> = Vec::new();
        let tip = |id: u32, en: &str, t: &AppState| t.texts.get(id, en);
        let tile = (a * 100.0).round() as u16;
        let metal = Color(0x14, 0x14, 0x12, 0xff);
        let dark = Color(0x05, 0x08, 0x04, 0xff);

        let root = ui.add_root(panel(Fill::None), Layout::fill());
        // Invisible plates over the frame stop clicks reaching the map.
        for r in hud_blockers(cw, ch) {
            ui.add(root, panel(Fill::Solid(Color(0, 0, 0, 0))), at4(r));
        }
        let solid = |ui: &mut Ui, r: [f32; 4], c: Color| {
            ui.add(root, panel(Fill::Solid(c)), at4(r));
        };
        let image = |ui: &mut Ui, s: Option<SpriteId>, r: [f32; 4], fit: Fit| {
            s.map(|s| ui.add(root, Kind::Image { sprite: s, fit }, at4(r)))
        };

        // ---- what shows through the holes of the frame -------------------------------------
        let radar_rect = gm.rl(21.0, 467.0, 236.0, 244.0);
        let act_rect = gm.rr(766.0, 497.0, 237.0, 214.0);
        let center = gm.center();
        solid(ui, radar_rect, metal);
        solid(ui, act_rect, Color(0x12, 0x14, 0x34, 0xff));
        for y in &PLATE_Y[1..] {
            solid(ui, gm.rl(PLATE_X, *y, 44.0, 34.0), dark);
        }
        // Small gaps of the frame, the tab strip left of the tabs and the strip over the center.
        solid(ui, gm.rl(135.0, 433.0, 34.0, 9.0), dark);
        solid(ui, gm.rr(931.0, 419.0, 34.0, 17.0), dark);
        solid(ui, gm.rr(756.0, 445.0, 24.0, 26.0), dark);
        solid(ui, gm.span(402.0, 625.0, 502.0, 6.0), dark);
        solid(ui, gm.rr(700.0, 692.0, 38.0, 36.0), dark);
        let overlay = self.overview_open() || self.tool_active();
        match art.olive {
            Some(o) => {
                image(ui, Some(o), center, Fit::Tile(tile));
            }
            None => solid(ui, center, Color(0x3c, 0x34, 0x10, 0xff)),
        }
        if overlay {
            solid(ui, center, Color(0, 0, 0, 0x96));
        }

        // ---- the radar -----------------------------------------------------------------------
        let radar_box = ui.add(root, panel(Fill::None), at4(radar_rect));
        let mut radar_node = None;
        if let Screen::Game(g) = &self.screen
            && let Some(r) = &g.radar
        {
            let (cols, rows) = (r.cols as f32, r.rows as f32);
            let fit = (radar_rect[2] / cols).min(radar_rect[3] / rows);
            let (w, h) = ((cols * fit).floor(), (rows * fit).floor());
            let colors = Arc::new(r.colors_with_view(&g.camera, self.viewport));
            let n = ui.add(
                radar_box,
                Kind::Cells {
                    cols: r.cols,
                    rows: r.rows,
                    colors,
                    action: A_RADAR,
                },
                Layout::at(Anchor::Center, w, h),
            );
            tips.push((
                n,
                tip(1772, "Radar area - click to recenter main view", self),
            ));
            radar_node = Some(n);
        }

        // ---- the activity box: logo, messages, history ---------------------------------------
        image(ui, art.title, act_rect, Fit::Stretch);
        let news_font = font(8, a * 0.62);
        let tx = gm.rr(797.0, 623.0, 198.0, 84.0);
        let chars = ((tx[2] - 6.0) / (news_font.size * 0.55)).floor().max(10.0) as usize;
        let mut lines = Vec::new();
        if let Screen::Game(g) = &self.screen {
            let take = if news_open { 50 } else { 3 };
            for item in g.news.iter().take(take) {
                lines.extend(wrap_text(&format!("{}: {}", item.date, item.text), chars));
                lines.push(String::new());
            }
        }
        if !news_open {
            // A click anywhere on the logo box opens the message history; the history has its
            // own Close button.
            let hist = ui.add(
                root,
                Kind::Hotspot {
                    action: A_NEWS,
                    on: false,
                    enabled: true,
                },
                at4(act_rect),
            );
            tips.push((hist, self.texts.a("Message history and scenario briefing")));
        }
        if news_open {
            // The history takes over the whole box.
            solid(ui, act_rect, Color(0x08, 0x0a, 0x1c, 0xe8));
            ui.add(
                root,
                label(
                    &self.texts.a("Messages"),
                    font(2, a * 0.9),
                    HEAD,
                    Align::Left,
                ),
                at(
                    act_rect[0] + 10.0 * a,
                    act_rect[1] + 6.0 * a,
                    act_rect[2] - 20.0 * a,
                    20.0 * a,
                ),
            );
            ui.add(
                root,
                Kind::Text {
                    lines,
                    scroll: 0,
                    row_h: news_font.line_height(),
                    font: news_font,
                    color: TEXT,
                },
                at(
                    act_rect[0] + 10.0 * a,
                    act_rect[1] + 30.0 * a,
                    act_rect[2] - 20.0 * a,
                    act_rect[3] - 30.0 * a - 48.0 * a,
                ),
            );
            let bw = (act_rect[2] - 30.0 * a) / 2.0;
            let by = act_rect[1] + act_rect[3] - 42.0 * a;
            let brief = ui.add(
                root,
                Kind::Button {
                    text: self.texts.get(2642, "Briefing"),
                    font: font(1, a * 0.8),
                    action: A_BRIEFING,
                    enabled: true,
                    hotkey: None,
                },
                at(act_rect[0] + 10.0 * a, by, bw, (24.0 * a).round()),
            );
            tips.push((brief, tip(2643, "View the scenario briefing dialog.", self)));
            ui.add(
                root,
                Kind::Button {
                    text: self.texts.a("Close"),
                    font: font(1, a * 0.8),
                    action: A_NEWS,
                    enabled: true,
                    hotkey: None,
                },
                at(act_rect[0] + 20.0 * a + bw, by, bw, (24.0 * a).round()),
            );
        } else {
            solid(
                ui,
                gm.rr(766.0, 618.0, 237.0, 93.0),
                Color(0x04, 0x06, 0x18, 0xa8),
            );
            ui.add(
                root,
                Kind::Text {
                    lines,
                    scroll: 0,
                    row_h: news_font.line_height(),
                    font: news_font,
                    color: TEXT,
                },
                at4(tx),
            );
        }
        // ---- the center box: list, or the panel of a tool ------------------------------------
        let cbox = ui.add(root, panel(Fill::None), at4(center));
        let mut center_node = None;
        if self.overview_open() {
            // Map Overview (`bpovervw.win`): the layer list takes the place of the list box.
            self.build_overview_panel(ui, cbox, center[2], center[3], a, &mut tips);
        } else if self.tool_active() {
            // A construction tool's palette takes the place of the list (`bp*.win`).
            self.build_tool_panel(ui, cbox, center[2], center[3], a, &mut tips);
        } else {
            self.build_center_table(
                ui,
                cbox,
                center_tab,
                center_sel,
                center_scroll,
                [center[2], center[3]],
                &gm,
                &mut center_node,
            );
        }
        // Inner shadows of the three boxes.
        if let Some([s0, s1, s2]) = art.shade {
            image(ui, Some(s0), radar_rect, Fit::Stretch);
            image(
                ui,
                Some(s1),
                gm.span(287.0, 699.0, 525.0, 159.0),
                Fit::Stretch,
            );
            image(ui, Some(s2), act_rect, Fit::Stretch);
        }

        // ---- the frame -----------------------------------------------------------------------
        image(
            ui,
            art.rail_left,
            gm.rl(0.0, 6.0, 85.0, 397.0),
            Fit::Stretch,
        );
        for (row, y, h, cut) in [
            (art.row0, 403.0, 105.0, crate::artwork::mainwin::ROW0_CUT),
            (art.row1, 508.0, 260.0, crate::artwork::mainwin::ROW1_CUT),
        ] {
            if let Some([l, f, r]) = row {
                let (c0, c1) = (cut.0 as f32, cut.1 as f32);
                image(ui, Some(l), gm.rl(0.0, y, c0, h), Fit::Stretch);
                image(ui, Some(f), gm.span(c0, c1, y, h), Fit::Tile(tile));
                image(ui, Some(r), gm.rr(c1, y, DESIGN.0 - c1, h), Fit::Stretch);
            }
        }

        // ---- the tab strip with cash and date -------------------------------------------------
        let strip = gm.rr(490.0, 687.0, 207.0, 45.0);
        // Widened frames leave a gap between the date box and the tab strip: the box's own strip.
        image(
            ui,
            art.tab_fill,
            gm.span(488.0, 490.0, 687.0, 45.0),
            Fit::Stretch,
        );
        let tab_art = art.tabs.map(|t| t[center_tab.min(3)]);
        image(ui, tab_art, strip, Fit::Stretch);
        let tab_tips = [1800u32, 1801, 1802, 1803];
        let tab_en = [
            "Show stations in this list box.",
            "Show trains in this list box.",
            "Show players in this list box.",
            "Show companies in this list box.",
        ];
        let names = self.center_tab_names();
        let mut tab_nodes = Vec::new();
        for i in 0..4 {
            let r = gm.rr(TAB_X[i], 688.0, TAB_W[i], 45.0);
            let t = ui.add(
                root,
                Kind::Hotspot {
                    action: A_CENTER_TAB + i as u32,
                    on: art.tabs.is_none() && i == center_tab,
                    enabled: true,
                },
                at4(r),
            );
            if art.tabs.is_none() {
                ui.add(
                    t,
                    label(&names[i], font(1, a * 0.7), TEXT, Align::Center),
                    Layout::fill(),
                );
            }
            tips.push((t, tip(tab_tips[i], tab_en[i], self)));
            tab_nodes.push(t);
        }
        let cash = ui.add(
            root,
            label(
                "",
                font(2, a * 0.85),
                Color::rgb(0xff, 0xfd, 0xf0),
                Align::Center,
            ),
            at4(gm.rl(291.0, 697.0, 90.0, 28.0)),
        );
        let date = ui.add(
            root,
            label(
                "",
                font(2, a * 0.85),
                Color::rgb(0xff, 0xfd, 0xf0),
                Align::Center,
            ),
            at4(gm.rl(392.0, 697.0, 90.0, 28.0)),
        );
        let ls = ui.add(
            root,
            Kind::Icon {
                up: self.icons.get(8).map(|i| i.0),
                down: self.icons.get(8).map(|i| i.1),
                fallback: "=".to_owned(),
                font: font(1, a),
                action: A_LIST_SCREEN,
                on: false,
            },
            at4(gm.rr(700.0, 692.0, 38.0, 36.0)),
        );
        tips.push((ls, tip(1799, "Go to the list screen", self)));

        // ---- the tool icons on the left rail ----------------------------------------------------
        for (i, (rec, tip_id, tip_en, caption)) in TOOLS.iter().enumerate() {
            let (up, down) = self
                .icons
                .get(*rec)
                .map_or((None, None), |p| (Some(p.0), Some(p.1)));
            let n = ui.add(
                root,
                Kind::Icon {
                    up,
                    down,
                    fallback: (*caption).to_owned(),
                    font: font(1, a * 0.7),
                    action: A_TOOL + i as u32,
                    on: self.tool_kind() == super::build_ui::ToolKind::from_icon(i),
                },
                at4(gm.rl(PLATE_X, PLATE_Y[TOOL_PLATE[i]], 44.0, 34.0)),
            );
            tips.push((n, tip(*tip_id, tip_en, self)));
        }

        // ---- rotate / zoom on the pipe above the activity box ---------------------------------
        for (i, (rec, action, tip_id, tip_en, caption)) in NAV.into_iter().enumerate() {
            let (up, down) = self
                .icons
                .get(rec)
                .map_or((None, None), |p| (Some(p.0), Some(p.1)));
            let n = ui.add(
                root,
                Kind::Icon {
                    up,
                    down,
                    fallback: caption.to_owned(),
                    font: font(1, a),
                    action,
                    on: false,
                },
                at4(gm.rr(NAV_X[i], 432.0, 51.0, 34.0)),
            );
            tips.push((n, tip(tip_id, tip_en, self)));
        }

        // ---- the compass, top right ---------------------------------------------------------
        let compass = art.compass.map(|c| {
            ui.add(
                root,
                Kind::Image {
                    sprite: c[compass_index(rotation)],
                    fit: Fit::Stretch,
                },
                at4(gm.rr(929.0, 23.0, 80.0, 46.0)),
            )
        });

        // ---- speed controls, left of the compass (the original has none on screen) -------------
        // Never smaller than a readable 0.7 of the design size, however small the canvas is.
        let u = a.max(0.7);
        let bsz = (28.0 * u).round();
        let sp_w = bsz * 3.5 + 12.0 * u + 110.0 * u;
        let sp = ui.add(
            root,
            panel(Fill::None),
            at(gm.xr(929.0) - sp_w - 6.0 * a, gm.y(26.0), sp_w, bsz),
        );
        ui.set_stack(sp, Stack::row(4.0 * u, 0.0));
        ui.add(
            sp,
            Kind::Button {
                text: "-".into(),
                font: font(3, u * 0.75),
                action: super::A_SLOWER,
                enabled: true,
                hotkey: None,
            },
            Layout::at(Anchor::TopLeft, bsz, bsz),
        );
        let pause = ui.add(
            sp,
            Kind::Button {
                text: "II".into(),
                font: font(6, u * 0.75),
                action: super::A_PAUSE,
                enabled: true,
                hotkey: None,
            },
            Layout::at(Anchor::TopLeft, bsz * 1.4, bsz),
        );
        ui.add(
            sp,
            Kind::Button {
                text: "+".into(),
                font: font(3, u * 0.75),
                action: super::A_FASTER,
                enabled: true,
                hotkey: None,
            },
            Layout::at(Anchor::TopLeft, bsz, bsz),
        );
        let speed = ui.add(
            sp,
            label("", font(2, u * 0.9).with_shadow(), TEXT, Align::Left),
            Layout::at(Anchor::TopLeft, 110.0 * u, bsz),
        );

        // ---- the status line on the ground strip ------------------------------------------------
        let status = ui.add(
            root,
            label(
                "",
                font(2, a * 0.8).with_shadow(),
                Color::rgb(0xf4, 0xea, 0xc4),
                Align::Left,
            ),
            at4(gm.span(60.0, 964.0, 735.0, 30.0)),
        );

        // ---- the toast (save confirmations and the like) -----------------------------------
        let toast = ui.add(
            root,
            label(
                "",
                game_font(6).with_shadow(),
                Color::rgb(0xff, 0xe0, 0x80),
                Align::Left,
            ),
            at(
                gm.x(300.0),
                gm.y(470.0) - 34.0,
                gm.xr(740.0) - gm.x(300.0),
                28.0,
            ),
        );
        ui.set_visible(toast, false);

        // The tooltip with the name of the tab under the cursor, on top of everything else.
        let tt_root = ui.add_root(panel(Fill::None), Layout::fill());
        let tt = ui.add(
            tt_root,
            panel(Fill::Solid(Color(0x12, 0x10, 0x08, 0xf0))),
            at(0.0, 0.0, 10.0, 10.0),
        );
        let tt_label = ui.add(
            tt,
            label(
                "",
                font(2, a.max(0.7) * 0.8),
                Color::rgb(0xff, 0xf6, 0xd8),
                Align::Center,
            ),
            Layout::fill(),
        );
        ui.set_visible(tt, false);

        if let Screen::Game(g) = &mut self.screen {
            g.hud = MainIds {
                tooltip: Some((tt, tt_label)),
                date: Some(date),
                cash: Some(cash),
                speed: Some(speed),
                pause: Some(pause),
                toast: Some(toast),
                status: Some(status),
                radar: radar_node,
                center: center_node,
                compass,
                compass_rot: rotation,
                tab_nodes,
                tips,
            };
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_center_table(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        tab: usize,
        sel: Option<usize>,
        scroll: usize,
        [w, h]: [f32; 2],
        gm: &Geometry,
        center_node: &mut Option<NodeId>,
    ) {
        let dz = gm.dz;
        let (rowh, head_h, visible) = center_rows(gm, tab);
        let mut table = self.center_table(tab);
        let n = table.rows.len();
        let first = scroll.min(n.saturating_sub(visible));
        let shown = visible.min(n.saturating_sub(first));
        let size = (rowh * 0.62).max(6.0);
        let bar_w = (14.0 * dz).round();
        let tb_w = w - bar_w - 4.0 * dz;
        if n == 0 {
            ui.add(
                parent,
                label(
                    &self.texts.a("Nothing to show."),
                    info_ui::font(size * 1.1),
                    TEXT,
                    Align::Left,
                ),
                at(12.0 * dz, head_h + 6.0, w - 24.0 * dz, rowh),
            );
        }
        let list = ui.add(
            parent,
            Kind::List {
                items: vec![String::new(); shown],
                selected: sel
                    .and_then(|s| s.checked_sub(first))
                    .filter(|&s| s < shown),
                scroll: 0,
                row_h: rowh,
                font: info_ui::font(size),
                action: A_CENTER_ROW,
            },
            at(0.0, head_h - 1.0, tb_w, rowh * shown as f32 + 2.0),
        );
        *center_node = Some(list);
        if tab == 3 {
            self.draw_company_rows(ui, parent, tb_w, rowh, first, shown, dz);
        } else {
            // As many columns as fit: about 80 design pixels each.
            let max_cols =
                ((tb_w / (80.0 * dz)).floor() as usize).clamp(2, table.cols.len().max(2));
            table.cols.truncate(max_cols);
            for r in &mut table.rows {
                r.truncate(max_cols);
            }
            info_ui::draw_table(
                ui, parent, &table, 0.0, 2.0, tb_w, rowh, size, first, visible, false,
            );
        }
        if n > visible {
            // The scroll bar sits in the dark slot of the frame right of the box.
            let slot = [w + 12.0 * dz, 22.0 * dz, 18.0 * dz, h - 44.0 * dz];
            let frac = visible as f32 / n as f32;
            let th = (slot[3] * frac).max(14.0 * dz);
            let ty = slot[1] + (slot[3] - th) * (first as f32 / (n - visible).max(1) as f32);
            ui.add(
                parent,
                panel(Fill::Solid(Color(0xc8, 0xb4, 0x78, 0xff))),
                at(slot[0] + 2.0, ty, slot[2] - 4.0, th),
            );
        }
    }

    /// The companies tab: one tall row per company (logo, name, cash, profits) like the list of
    /// the original (`mainwin.win` list 325).
    #[allow(clippy::too_many_arguments)]
    fn draw_company_rows(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        w: f32,
        rowh: f32,
        first: usize,
        shown: usize,
        dz: f32,
    ) {
        let rows = self.with_info(|i| i.company_rows()).unwrap_or_default();
        let t = &self.texts;
        let size = (rowh * 0.36).max(6.0);
        let f = info_ui::font(size);
        let pad = 3.0 * dz;
        for (n, c) in rows.iter().skip(first).take(shown).enumerate() {
            let y = n as f32 * rowh;
            ui.add(
                parent,
                panel(Fill::Solid(Color(0xff, 0xf0, 0x8c, 0x1c))),
                at(2.0, y + 2.0, w - 4.0, rowh - 4.0),
            );
            ui.add(
                parent,
                panel(Fill::Frame(if c.human {
                    TEAL
                } else {
                    Color(0, 0, 0, 0x60)
                })),
                at(2.0, y + 2.0, w - 4.0, rowh - 4.0),
            );
            if let Some(Some(s)) = self.icons.brand.logos.get(c.logo) {
                ui.add(
                    parent,
                    Kind::Image {
                        sprite: *s,
                        fit: Fit::Contain,
                    },
                    at(6.0 * dz, y + 5.0 * dz, rowh - 8.0 * dz, rowh - 10.0 * dz),
                );
            }
            let tx = rowh + 4.0 * dz;
            let half = rowh / 2.0;
            ui.add(
                parent,
                label(
                    &c.name,
                    f.with_shadow(),
                    Color::rgb(0xff, 0xf6, 0xd8),
                    Align::Left,
                ),
                at(tx, y + pad, w - tx - 6.0 * dz, half - pad),
            );
            ui.add(
                parent,
                label(
                    &format!("{} {}", t.get(31, "Cash"), money_k(c.cash)),
                    f.with_shadow(),
                    Color::rgb(0xff, 0xf6, 0xd8),
                    Align::Left,
                ),
                at(tx, y + half - pad, w * 0.5 - tx, half),
            );
            ui.add(
                parent,
                label(
                    &format!("{} {}", t.a("Profits"), money_k(c.profit)),
                    f.with_shadow(),
                    if c.profit < 0.0 {
                        Color::rgb(0xff, 0x9a, 0x88)
                    } else {
                        Color::rgb(0xff, 0xf6, 0xd8)
                    },
                    Align::Left,
                ),
                at(w * 0.55, y + half - pad, w * 0.45 - 6.0 * dz, half),
            );
        }
    }

    // ---- actions ----------------------------------------------------------------------------

    /// Handles the main window's buttons; true if `action` was one of them.
    pub(super) fn mainwin_activated(&mut self, action: u32) -> bool {
        match action {
            a if (A_TOOL..A_TOOL + 8).contains(&a) => self.tool_clicked((a - A_TOOL) as usize),
            A_ROTATE_RIGHT => self.camera_op(|c| c.rotate(1)),
            A_ROTATE_LEFT => self.camera_op(|c| c.rotate(-1)),
            A_ZOOM_OUT => self.camera_op(|c| c.zoom_by(1.0 / 1.4)),
            A_ZOOM_IN => self.camera_op(|c| c.zoom_by(1.4)),
            a if (A_CENTER_TAB..A_CENTER_TAB + 4).contains(&a) => {
                self.set_center_tab((a - A_CENTER_TAB) as usize);
            }
            A_CENTER_ROW => self.center_open_row(),
            A_LIST_SCREEN => self.center_list_screen(),
            A_BRIEFING => self.push_dialog(Dialog::Goals),
            A_NEWS => self.toggle_news(),
            _ => return false,
        }
        true
    }

    /// The message button: the activity box shows the history (and the Briefing button) or the
    /// logo with the latest messages.
    fn toggle_news(&mut self) {
        if let Screen::Game(g) = &mut self.screen {
            g.center.news_open = !g.center.news_open;
        }
        self.rebuild_ui();
    }

    fn camera_op(&mut self, f: impl FnOnce(&mut rt2_render::Camera)) {
        if let Screen::Game(g) = &mut self.screen {
            f(&mut g.camera);
            if g.build.tool.is_some() || self.ga.overview.open {
                self.rebuild_ui();
            }
        }
    }

    fn tool_clicked(&mut self, i: usize) {
        if let Some(kind) = super::build_ui::ToolKind::from_icon(i) {
            // Pressing a construction mode leaves the overview.
            self.ga.overview.open = false;
            self.select_tool(kind);
            return;
        }
        match i {
            3 => self.overview_toggle(),
            4 => self.open_train_buy(),
            5 => self.toggle_tab(1),
            7 => self.push_dialog(Dialog::FileOptions),
            n => self.push_dialog(Dialog::Tool(n)),
        }
    }

    /// The name and hint of an action icon (placeholder dialog titles).
    pub(super) fn tool_name(&self, i: usize) -> (String, String) {
        let (_, id, en, caption) = TOOLS[i.min(7)];
        (caption.to_owned(), self.texts.get(id, en))
    }

    pub(super) fn set_center_tab(&mut self, tab: usize) {
        if let Screen::Game(g) = &mut self.screen {
            g.center = CenterState {
                tab: tab.min(3),
                sel: None,
                scroll: 0,
                news_open: g.center.news_open,
            };
        }
        self.rebuild_ui();
    }

    /// Scrolls the center list (wheel over it).
    pub(super) fn center_scroll(&mut self, rows: i32) {
        if let Screen::Game(g) = &mut self.screen {
            g.center.scroll = (g.center.scroll as i32 + rows).max(0) as usize;
        }
        self.rebuild_ui();
    }

    /// A row of the center list was clicked: select it.
    pub(super) fn center_select(&mut self, row: usize) {
        let first = self.center_first();
        if let Screen::Game(g) = &mut self.screen {
            g.center.sel = Some(first + row);
        }
        self.rebuild_ui();
    }

    /// The first visible row, as the builder clamps it.
    fn center_first(&self) -> usize {
        let Screen::Game(g) = &self.screen else {
            return 0;
        };
        let (cw, ch) = self.ui.logical_size();
        let gm = geometry(cw, ch);
        let n = self.center_table(g.center.tab).rows.len();
        let (_, _, visible) = center_rows(&gm, g.center.tab);
        g.center.scroll.min(n.saturating_sub(visible))
    }

    /// Double click / Enter on the selected row: its detail screen.
    fn center_open_row(&mut self) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let (tab, sel) = (g.center.tab, g.center.sel);
        let Some(sel) = sel else {
            return;
        };
        let table = self.center_table(tab);
        let Some(&id) = table.ids.get(sel) else {
            return;
        };
        match tab {
            0 => self.open_station(id as i32),
            1 => self.open_info(InfoScreen::TrainDetail(id)),
            // Players and companies: the company report (no player detail screen yet, `docs/
            // reference/screens.md` H1).
            _ if id > 0 => self.open_info(InfoScreen::Finance(id)),
            _ => {}
        }
    }

    /// The "list screen" button: the full-screen list of the current tab.
    fn center_list_screen(&mut self) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let screen = match g.center.tab {
            0 => InfoScreen::Stations,
            1 => InfoScreen::Trains,
            _ => InfoScreen::Stock,
        };
        self.open_info(screen);
    }

    /// A press or drag on the radar: move the camera there.
    pub(super) fn radar_pointer(&mut self, fx: f32, fy: f32) {
        if let Screen::Game(g) = &mut self.screen
            && let Some(r) = &g.radar
        {
            let (x, y) = r.to_tile(fx, fy);
            g.camera.target = rt2_render::camera::Vec2::new(x, y);
        }
    }

    pub(super) fn mainwin_event(&mut self, e: &UiEvent) -> bool {
        match *e {
            UiEvent::Pointer(A_RADAR, fx, fy) => {
                self.radar_pointer(fx, fy);
                true
            }
            UiEvent::Selected(A_CENTER_ROW, row) => {
                self.center_select(row);
                true
            }
            _ => false,
        }
    }

    // ---- per-frame --------------------------------------------------------------------------

    /// Adds a line to the activity box (newest first), dated now.
    pub(super) fn add_news(&mut self, text: String) {
        let date = match &self.screen {
            Screen::Game(g) => format_date(g.world.date(), &self.texts),
            Screen::Menu(_) => return,
        };
        if let Screen::Game(g) = &mut self.screen {
            g.news.insert(0, NewsItem { date, text });
            g.news.truncate(50);
        }
    }

    /// Rebuilds the main window when the simulated day changes (the center list's numbers
    /// move), unless a screen or dialog covers it.
    pub(super) fn center_refresh(&mut self) {
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        let day = g.world.minutes() / 1440;
        if day != g.center_day {
            g.center_day = day;
            if g.info.is_none() && g.dialogs.is_empty() {
                self.rebuild_ui();
            }
        }
    }

    /// Keeps the texts, radar outline and status line of the main window current.
    pub(super) fn mainwin_refresh(&mut self) {
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        let texts = &self.texts;
        let hud = g.hud.clone();
        let d = g.world.date();
        // The date box of the original shows month and year ("Jan 1830"), the cash box thousands.
        let date = format!("{} {}", texts.month_short(d.month), d.year);
        let speed = if g.world.speed() == 0 {
            texts.get(lng::PAUSED, "Paused")
        } else {
            format!("{} {}", texts.get(lng::SPEED, "Speed:"), g.world.speed())
        };
        let cash = match g.cash {
            Some(c) => money_k(c),
            None => String::new(),
        };
        let paused = g.world.speed() == 0;
        let toast = g.toast.as_ref().map(|t| t.0.clone());
        // The radar outline follows the camera.
        let view = (
            g.camera.target.x,
            g.camera.target.y,
            g.camera.zoom,
            g.camera.rotation,
        );
        let view_changed = g.radar_view != Some((view, self.viewport));
        if view_changed {
            g.radar_view = Some((view, self.viewport));
        }
        let colors = match (&g.radar, hud.radar, view_changed) {
            (Some(r), Some(n), true) => {
                Some((n, Arc::new(r.colors_with_view(&g.camera, self.viewport))))
            }
            _ => None,
        };
        if let (Some(n), Some(c)) = (hud.compass, self.dialog_art.main.compass)
            && let Screen::Game(g) = &mut self.screen
            && g.hud.compass_rot != g.camera.rotation
        {
            g.hud.compass_rot = g.camera.rotation;
            self.ui.set_sprite(n, c[compass_index(g.camera.rotation)]);
        }
        let Screen::Game(_) = &self.screen else {
            return;
        };
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
            self.ui.set_text(n, if paused { ">" } else { "II" });
        }
        if let Some(n) = hud.toast {
            self.ui.set_visible(n, toast.is_some());
            self.ui.set_text(n, toast.unwrap_or_default());
        }
        if let Some((n, c)) = colors {
            self.ui.set_cells(n, c);
        }
        self.update_status();
    }

    /// The status line: the hint of the widget under the mouse, else what is under the cursor on
    /// the map (its coordinates and the page title of the building or place).
    pub(super) fn update_status(&mut self) {
        let hovered = self.ui.hovered();
        let (status, text) = match &self.screen {
            Screen::Game(g) => {
                let Some(status) = g.hud.status else {
                    return;
                };
                let text = if let Some(h) = hovered {
                    g.hud
                        .tips
                        .iter()
                        .find(|(n, _)| *n == h)
                        .map(|(_, t)| t.clone())
                        .unwrap_or_default()
                } else if let Some(p) = self.cursor {
                    self.map_hint(p).unwrap_or_default()
                } else {
                    String::new()
                };
                (status, text)
            }
            Screen::Menu(_) => {
                let Some(status) = self.menu_handles.status else {
                    return;
                };
                let text = hovered
                    .and_then(|h| self.menu_handles.tips.iter().find(|(n, _)| *n == h))
                    .map(|(_, t)| t.clone())
                    .unwrap_or_default();
                (status, text)
            }
        };
        self.ui.set_text(status, text);
        self.update_tooltip(hovered);
    }

    /// Shows the name of the tab under the cursor in a small box above it, hides it elsewhere.
    fn update_tooltip(&mut self, hovered: Option<NodeId>) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let Some((tt, label)) = g.hud.tooltip else {
            return;
        };
        let tab = hovered.and_then(|h| g.hud.tab_nodes.iter().position(|&n| n == h));
        let Some(i) = tab else {
            if self.ui.is_shown(tt) {
                self.ui.set_visible(tt, false);
                self.ui.layout(self.viewport);
            }
            return;
        };
        let name = self.center_tab_names()[i].clone();
        let (cw, ch) = self.ui.logical_size();
        let a = geometry(cw, ch).dz.max(0.7);
        let r = self.ui.rect(g.hud.tab_nodes[i]);
        let size = (game_font(2).size * a * 0.8).round();
        let (w, h) = (
            name.chars().count() as f32 * size * 0.7 + 14.0 * a,
            size * 1.6,
        );
        let x = (r.x + r.w / 2.0 - w / 2.0).clamp(2.0, cw - w - 2.0);
        let y = r.y - h - 4.0 * a;
        let moved = self.ui.rect(tt);
        if self.ui.is_shown(tt) && (moved.x - x).abs() < 0.5 && (moved.y - y).abs() < 0.5 {
            return;
        }
        self.ui.set_text(label, name);
        {
            let node = self.ui.node_mut(tt);
            node.layout = at(x, y, w, h);
            node.visible = true;
        }
        self.ui.layout(self.viewport);
    }

    /// "x, y  Name" for the map tile under physical pixel `pos`.
    fn map_hint(&self, pos: (f32, f32)) -> Option<String> {
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        let save = g.world.save_ref()?;
        let (tx, ty) = crate::pick::screen_to_tile(&g.camera, self.viewport, pos, &save.map)?;
        let name = self.with_info(|i| {
            i.pick(tx, ty).map(|p| match p {
                crate::info::Pick::Industry(s) => i.industry_page(s).title,
                crate::info::Pick::Station(s) => i.station_page(s).title,
                crate::info::Pick::Place(s) => i.place_page(s).title,
                crate::info::Pick::Territory(s) => i.territory_page(s).title,
            })
        });
        let name = name.flatten().unwrap_or_default();
        Some(format!("{tx}, {ty}   {name}").trim_end().to_owned())
    }

    /// Keys of the main window: `Esc` closes the topmost dialog, else opens File Options.
    pub(super) fn mainwin_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Escape if self.has_dialog() => {
                self.close_dialog();
                true
            }
            _ => false,
        }
    }
}
