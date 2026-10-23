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
    Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Stack, Ui, UiEvent, game_font,
};
use winit::keyboard::KeyCode;

use super::dialogs::Dialog;
use super::info_ui::{self, InfoScreen};
use super::{AppState, Screen, format_date, format_money};
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
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd8);

/// State of the center list box.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CenterState {
    /// 0 Stations, 1 Trains, 2 Players, 3 Companies.
    pub tab: usize,
    pub sel: Option<usize>,
    pub scroll: usize,
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
    /// Hint texts of the widgets, shown on the status line while the mouse is over them.
    pub tips: Vec<(NodeId, String)>,
}

/// An entry of the activity box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NewsItem {
    pub date: String,
    pub text: String,
}

/// How big everything is: `dz` is 1 on a small canvas and grows with the canvas height.
fn design_scale(ch: f32) -> f32 {
    (ch / 620.0).clamp(1.0, 2.0)
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

fn panel(fill: Fill) -> Kind {
    Kind::Panel { fill }
}

/// Geometry of the bottom panel, shared by the builder and the camera code.
#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    pub(super) dz: f32,
    /// Height of the whole bottom panel (content plus status line).
    pub(super) ph: f32,
    /// Height of the status line.
    pub(super) sl: f32,
    pub(super) inset: f32,
    /// Side of the square radar and activity boxes.
    pub(super) side: f32,
}

pub(super) fn geometry(cw: f32, ch: f32) -> Geometry {
    let dz = design_scale(ch);
    let sl = (22.0 * dz).round();
    let ph = (215.0 * dz).min(ch * 0.40).max(150.0).round();
    let inset = (8.0 * dz).round();
    let mut side = ph - sl - 2.0 * inset;
    // On a narrow canvas the side boxes shrink so the center box keeps some room.
    let min_center = 300.0;
    if cw - 2.0 * (side + 2.0 * inset) < min_center {
        side = ((cw - min_center) / 2.0 - 2.0 * inset).max(60.0);
    }
    Geometry {
        dz,
        ph,
        sl,
        inset,
        side,
    }
}

/// The parts of the window the in-game HUD covers, as `[x, y, w, h]` in logical units: the bottom
/// panel, the eight action icons down the left edge, the rotate/zoom buttons above the activity
/// box and the speed controls at the top right (the same geometry as `build_mainwin`).
pub(super) fn hud_blockers(cw: f32, ch: f32) -> Vec<[f32; 4]> {
    let Geometry {
        dz,
        ph,
        inset,
        side,
        ..
    } = geometry(cw, ch);
    let per = (ch - ph - 16.0) / 8.0;
    let bh = (per - 4.0).clamp(20.0, 34.0 * dz).round();
    let bw = (bh * 50.0 / 34.0).round();
    let nav_h = (34.0 * dz).round().min(side / 4.0 * 0.68);
    let nav_w = (nav_h * 50.0 / 34.0).round();
    let nav_total = 4.0 * nav_w + 12.0 * dz;
    vec![
        [0.0, ch - ph, cw, ph],
        [6.0 * dz, 8.0 * dz, bw, per * 8.0],
        [
            cw - inset - nav_total,
            ch - ph - 4.0 - nav_h,
            nav_total,
            nav_h,
        ],
        [cw - inset - 250.0 * dz, inset, 250.0 * dz, 30.0 * dz],
    ]
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

    /// A camera that frames the whole map in the part of the screen the bottom panel leaves
    /// free (the map is drawn under the panel, so the view is shifted up by half of it).
    pub(super) fn fit_camera(&self, w: usize, h: usize) -> rt2_render::Camera {
        let k = self.scale.factor();
        let ph = geometry(self.viewport.0 as f32 / k, self.viewport.1 as f32 / k).ph * k;
        let free = (self.viewport.1 as f32 - ph).max(64.0) as u32;
        let mut cam = rt2_render::Camera::fit(w, h, (self.viewport.0, free));
        cam.target -= cam.forward() * (ph / cam.zoom);
        cam
    }

    /// Builds the main window over the map.
    pub(super) fn build_mainwin(&mut self, ui: &mut Ui) {
        let (cw, ch) = ui.logical_size();
        let gm = geometry(cw, ch);
        let Geometry {
            dz,
            ph,
            sl,
            inset,
            side,
        } = gm;
        let (center_tab, center_sel, center_scroll) = match &self.screen {
            Screen::Game(g) => (g.center.tab, g.center.sel, g.center.scroll),
            Screen::Menu(_) => return,
        };
        let mut tips: Vec<(NodeId, String)> = Vec::new();
        let tip = |id: u32, en: &str, t: &AppState| t.texts.get(id, en);

        // ---- the bottom panel -------------------------------------------------------------
        let bottom = ui.add_root(
            panel(Fill::Panel),
            Layout::at(Anchor::Bottom, 0.0, ph).offset(0.0, 0.0),
        );
        // Status line.
        let status_bar = ui.add(
            bottom,
            panel(Fill::Solid(Color(0x10, 0x20, 0x14, 0xe0))),
            Layout::at(Anchor::Bottom, 0.0, sl).offset(inset, 4.0),
        );
        let status = ui.add(
            status_bar,
            label("", font(2, dz * 0.9), TEXT, Align::Left),
            Layout::fill().offset(8.0 * dz, 0.0),
        );

        // Radar (B).
        let body_h = ph - sl - 2.0 * inset - 4.0;
        let side = side.min(body_h);
        let radar_box = ui.add(
            bottom,
            panel(Fill::Solid(INSET)),
            at(inset, inset, side, side),
        );
        let mut radar_node = None;
        if let Screen::Game(g) = &self.screen
            && let Some(r) = &g.radar
        {
            let (cols, rows) = (r.cols as f32, r.rows as f32);
            let fit = (side - 4.0) / cols.max(rows);
            let (w, h) = (cols * fit, rows * fit);
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

        // Activity box (D).
        let act_x = cw - inset - side;
        let act = ui.add(
            bottom,
            panel(Fill::Solid(INSET)),
            at(act_x, inset, side, side),
        );
        let news_font = font(8, dz * 0.62);
        let chars = ((side - 14.0 * dz) / (news_font.size * 0.55))
            .floor()
            .max(10.0) as usize;
        let mut lines = Vec::new();
        if let Screen::Game(g) = &self.screen {
            for item in g.news.iter().take(12) {
                lines.extend(wrap_text(&format!("{}: {}", item.date, item.text), chars));
                lines.push(String::new());
            }
        }
        ui.add(
            act,
            label(
                &self.texts.a("Messages"),
                font(2, dz * 0.9),
                HEAD,
                Align::Left,
            ),
            at(8.0 * dz, 4.0 * dz, side - 16.0 * dz, 20.0 * dz),
        );
        let btn_h = (24.0 * dz).round();
        ui.add(
            act,
            Kind::Text {
                lines,
                scroll: 0,
                row_h: news_font.line_height(),
                font: news_font,
                color: TEXT,
            },
            at(
                8.0 * dz,
                26.0 * dz,
                side - 16.0 * dz,
                side - 26.0 * dz - btn_h - 8.0 * dz,
            ),
        );
        let brief = ui.add(
            act,
            Kind::Button {
                text: self.texts.get(2642, "Briefing"),
                font: font(1, dz),
                action: A_BRIEFING,
                enabled: true,
                hotkey: None,
            },
            Layout::at(Anchor::Bottom, side - 16.0 * dz, btn_h).offset(0.0, 4.0 * dz),
        );
        tips.push((brief, tip(2643, "View the scenario briefing dialog.", self)));

        // Center list box (C).
        let left_edge = inset + side + inset;
        let right_edge = act_x - inset;
        let avail_w = (right_edge - left_edge).max(120.0);
        let center_w = avail_w.min(760.0 * dz);
        let center_x = left_edge + (avail_w - center_w) / 2.0;
        let center_h = ph - sl - 2.0 * inset - 4.0;
        let cbox = ui.add(
            bottom,
            panel(Fill::Solid(INSET)),
            at(center_x, inset, center_w, center_h),
        );
        let mut center_node = None;
        let mut date_cash: Option<(NodeId, NodeId)> = None;
        if self.tool_active() {
            // A construction tool's palette takes the place of the list (`bp*.win`).
            self.build_tool_panel(ui, cbox, center_w, center_h, dz, &mut tips);
        } else {
            let strip_h = (28.0 * dz).round();
            self.build_center_table(
                ui,
                cbox,
                center_tab,
                center_sel,
                center_scroll,
                center_w,
                center_h - strip_h,
                dz,
                &mut center_node,
            );
            // The strip under the table: date, cash, the four tabs and the list-screen button.
            let strip_y = center_h - strip_h;
            ui.add(
                cbox,
                panel(Fill::Solid(Color(0x16, 0x1c, 0x16, 0xf0))),
                at(0.0, strip_y, center_w, strip_h),
            );
            let date_w = (150.0 * dz).min(center_w * 0.30);
            let cash_w = (120.0 * dz).min(center_w * 0.24);
            let date = ui.add(
                cbox,
                label("", font(1, dz), TEXT, Align::Left),
                at(6.0 * dz, strip_y, date_w, strip_h),
            );
            let cash = ui.add(
                cbox,
                label("", font(1, dz), HEAD, Align::Left),
                at(6.0 * dz + date_w, strip_y, cash_w, strip_h),
            );
            let tabs_x = 6.0 * dz + date_w + cash_w + 4.0 * dz;
            let list_w = (30.0 * dz).round();
            let tabs_w = (center_w - tabs_x - list_w - 6.0 * dz).max(80.0);
            let tab_w = tabs_w / 4.0;
            let tab_tips = [1800u32, 1801, 1802, 1803];
            let tab_en = [
                "Show stations in this list box.",
                "Show trains in this list box.",
                "Show players in this list box.",
                "Show companies in this list box.",
            ];
            for (i, name) in self.center_tab_names().iter().enumerate() {
                let t = ui.add(
                    cbox,
                    Kind::Toggle {
                        text: name.clone(),
                        font: font(1, dz * 0.92),
                        action: A_CENTER_TAB + i as u32,
                        on: i == center_tab,
                        enabled: true,
                    },
                    at(
                        tabs_x + i as f32 * tab_w,
                        strip_y + 2.0,
                        tab_w - 2.0,
                        strip_h - 4.0,
                    ),
                );
                tips.push((t, tip(tab_tips[i], tab_en[i], self)));
            }
            let ls = ui.add(
                cbox,
                Kind::Icon {
                    up: self.icons.get(8).map(|i| i.0),
                    down: self.icons.get(8).map(|i| i.1),
                    fallback: "=".to_owned(),
                    font: font(1, dz),
                    action: A_LIST_SCREEN,
                    on: false,
                },
                at(
                    center_w - list_w - 4.0 * dz,
                    strip_y + 2.0,
                    list_w,
                    strip_h - 4.0,
                ),
            );
            tips.push((ls, tip(1799, "Go to the list screen", self)));
            date_cash = Some((date, cash));
        }

        // ---- the eight action icons, down the left edge -----------------------------------
        let avail = ch - ph - 16.0;
        let per = avail / 8.0;
        let bh = (per - 4.0).clamp(20.0, 34.0 * dz).round();
        let bw = (bh * 50.0 / 34.0).round();
        let tools = ui.add_root(panel(Fill::None), at(6.0 * dz, 8.0 * dz, bw, per * 8.0));
        ui.set_stack(tools, Stack::column(per - bh, 0.0));
        for (i, (rec, tip_id, tip_en, caption)) in TOOLS.iter().enumerate() {
            let (up, down) = self
                .icons
                .get(*rec)
                .map_or((None, None), |p| (Some(p.0), Some(p.1)));
            let n = ui.add(
                tools,
                Kind::Icon {
                    up,
                    down,
                    fallback: (*caption).to_owned(),
                    font: font(1, dz * 0.9),
                    action: A_TOOL + i as u32,
                    on: self.tool_kind() == super::build_ui::ToolKind::from_icon(i),
                },
                Layout::at(Anchor::TopLeft, bw, bh),
            );
            tips.push((n, tip(*tip_id, tip_en, self)));
        }

        // ---- rotate / zoom above the activity box -----------------------------------------
        let nav_h = (34.0 * dz).round().min(side / 4.0 * 0.68);
        let nav_w = (nav_h * 50.0 / 34.0).round();
        let nav = ui.add_root(
            panel(Fill::None),
            Layout::at(Anchor::BottomRight, 4.0 * nav_w + 12.0 * dz, nav_h).offset(inset, ph + 4.0),
        );
        ui.set_stack(nav, Stack::row(4.0 * dz, 0.0));
        for (rec, action, tip_id, tip_en, caption) in NAV {
            let (up, down) = self
                .icons
                .get(rec)
                .map_or((None, None), |p| (Some(p.0), Some(p.1)));
            let n = ui.add(
                nav,
                Kind::Icon {
                    up,
                    down,
                    fallback: caption.to_owned(),
                    font: font(1, dz),
                    action,
                    on: false,
                },
                Layout::at(Anchor::TopLeft, nav_w, nav_h),
            );
            tips.push((n, tip(tip_id, tip_en, self)));
        }

        // ---- speed controls, top right (the original has none on screen: `+` / `-` keys) ----
        let sp = ui.add_root(
            panel(Fill::None),
            Layout::at(Anchor::TopRight, 250.0 * dz, 30.0 * dz).offset(inset, inset),
        );
        ui.set_stack(sp, Stack::row(4.0 * dz, 0.0));
        let bsz = (30.0 * dz).round();
        ui.add(
            sp,
            Kind::Button {
                text: "-".into(),
                font: font(3, dz * 0.8),
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
                font: font(6, dz * 0.8),
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
                font: font(3, dz * 0.8),
                action: super::A_FASTER,
                enabled: true,
                hotkey: None,
            },
            Layout::at(Anchor::TopLeft, bsz, bsz),
        );
        let speed = ui.add(
            sp,
            label("", font(2, dz).with_shadow(), TEXT, Align::Left),
            Layout::at(Anchor::TopLeft, 120.0 * dz, bsz),
        );

        // ---- the toast (save confirmations and the like) -----------------------------------
        let toast_root = ui.add_root(panel(Fill::None), Layout::fill());
        let toast = ui.add(
            toast_root,
            label(
                "",
                game_font(6).with_shadow(),
                Color::rgb(0xff, 0xe0, 0x80),
                Align::Left,
            ),
            Layout::at(Anchor::BottomLeft, 0.0, 28.0).offset(inset + 4.0, ph + 8.0),
        );
        ui.set_visible(toast, false);

        if let Screen::Game(g) = &mut self.screen {
            g.hud = MainIds {
                date: date_cash.map(|d| d.0),
                cash: date_cash.map(|d| d.1),
                speed: Some(speed),
                pause: Some(pause),
                toast: Some(toast),
                status: Some(status),
                radar: radar_node,
                center: center_node,
                tips,
            };
        }
        let _ = (HEAD, MUTED);
    }

    #[allow(clippy::too_many_arguments)]
    fn build_center_table(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        tab: usize,
        sel: Option<usize>,
        scroll: usize,
        w: f32,
        h: f32,
        dz: f32,
        center_node: &mut Option<NodeId>,
    ) {
        let mut table = self.center_table(tab);
        // As many columns as fit: about 70 logical pixels each, the first one wider.
        let max_cols = ((w / (80.0 * dz)).floor() as usize).clamp(2, table.cols.len().max(2));
        table.cols.truncate(max_cols);
        for r in &mut table.rows {
            r.truncate(max_cols);
        }
        let rowh = (19.0 * dz).round();
        let size = (rowh * 0.62).max(10.0);
        let head_h = rowh + 4.0;
        let visible = (((h - head_h) / rowh).floor() as usize).max(1);
        let n = table.rows.len();
        let first = scroll.min(n.saturating_sub(visible));
        let shown = visible.min(n.saturating_sub(first));
        let tb_w = w - if n > visible { 8.0 } else { 0.0 };
        if n == 0 {
            ui.add(
                parent,
                label(
                    &self.texts.a("Nothing to show."),
                    info_ui::font(size * 1.1),
                    MUTED,
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
            at(0.0, head_h - 1.0, w, rowh * shown as f32 + 2.0),
        );
        *center_node = Some(list);
        info_ui::draw_table(
            ui, parent, &table, 0.0, 2.0, tb_w, rowh, size, first, visible, false,
        );
        if n > visible {
            // A slim scroll bar.
            let track_h = h - head_h;
            ui.add(
                parent,
                panel(Fill::Solid(Color(255, 255, 255, 0x18))),
                at(w - 6.0, head_h, 6.0, track_h),
            );
            let frac = visible as f32 / n as f32;
            let th = (track_h * frac).max(12.0);
            let ty = head_h + (track_h - th) * (first as f32 / (n - visible).max(1) as f32);
            ui.add(
                parent,
                panel(Fill::Solid(Color::rgb(0x8a, 0x7c, 0x55))),
                at(w - 6.0, ty, 6.0, th),
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
            _ => return false,
        }
        true
    }

    fn camera_op(&mut self, f: impl FnOnce(&mut rt2_render::Camera)) {
        if let Screen::Game(g) = &mut self.screen {
            f(&mut g.camera);
            if g.build.tool.is_some() {
                self.rebuild_ui();
            }
        }
    }

    fn tool_clicked(&mut self, i: usize) {
        if let Some(kind) = super::build_ui::ToolKind::from_icon(i) {
            self.select_tool(kind);
            return;
        }
        match i {
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
        let rowh = (19.0 * gm.dz).round();
        let h = gm.ph - gm.sl - 2.0 * gm.inset - 4.0 - (28.0 * gm.dz).round();
        let visible = (((h - rowh - 4.0) / rowh).floor() as usize).max(1);
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
        let date = format_date(g.world.date(), texts);
        let speed = if g.world.speed() == 0 {
            texts.get(lng::PAUSED, "Paused")
        } else {
            format!("{} {}", texts.get(lng::SPEED, "Speed:"), g.world.speed())
        };
        let cash = match g.cash {
            Some(c) => format_money(c),
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
