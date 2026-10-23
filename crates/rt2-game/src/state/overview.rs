//! C6 Map Overview (the globe icon; `bpovervw.win`, docs/reference/screens.md; manual chapter 4
//! "Map Overview"): a list of five layers over the map.
//!
//! * Resource Map: every cargo of the scenario as a toggle (white = supplied, yellow = demanded
//!   on the map); passing the mouse over one lights it up for a moment, clicking keeps it on,
//!   All and Clear change the whole selection. The buildings that supply a selected cargo are
//!   marked white, those that demand it yellow.
//! * Station Statuses: a small box at each station with the lots waiting there.
//! * Track Grade: the track coloured by grade (`+0x12` of the piece).
//! * Traffic Density: the track coloured by accumulated wear (see `overview.rs` for the gap).
//! * Track by Company: the track of the company chosen in the list is highlighted.
//!
//! The panel takes the place of the center list box and Cancel (or Esc, or the icon again)
//! returns to it. The data comes from `crate::overview`; the layer is drawn as translucent
//! squares over the visible tiles, like the construction tools' preview.

use std::cell::RefCell;
use std::collections::BTreeSet;

use rt2_ui::{Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::{AppState, Screen};
use crate::overview::{self as ov, Building, TrackTile};
use crate::text::id as lng;

pub(super) const A_OV_LAYER: u32 = 12000;
pub(super) const A_OV_ALL: u32 = 12010;
pub(super) const A_OV_CLEAR: u32 = 12011;
pub(super) const A_OV_CANCEL: u32 = 12012;
pub(super) const A_OV_CARGO: u32 = 12100;
pub(super) const A_OV_COMPANY: u32 = 12200;

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const SUPPLY: Color = Color::rgb(0xff, 0xff, 0xff);
const DEMAND: Color = Color::rgb(0xff, 0xdc, 0x28);
pub(super) const GRADE_COLORS: [Color; 5] = [
    Color::rgb(0x50, 0xd0, 0x50),
    Color::rgb(0xb4, 0xe0, 0x40),
    Color::rgb(0xf0, 0xe0, 0x40),
    Color::rgb(0xf0, 0x90, 0x30),
    Color::rgb(0xe0, 0x40, 0x40),
];
pub(super) const TRAFFIC_COLORS: [Color; 5] = [
    Color::rgb(0x40, 0x80, 0xf0),
    Color::rgb(0x40, 0xc8, 0xc8),
    Color::rgb(0x60, 0xd8, 0x50),
    Color::rgb(0xf0, 0xd8, 0x40),
    Color::rgb(0xe8, 0x40, 0x30),
];
/// Most overlay squares drawn per layer (the visible ones come first).
const MAX_SQUARES: usize = 3000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Layer {
    Resource,
    Stations,
    Grade,
    Traffic,
    Company,
}

impl Layer {
    pub(super) const ALL: [Layer; 5] = [
        Layer::Resource,
        Layer::Stations,
        Layer::Grade,
        Layer::Traffic,
        Layer::Company,
    ];

    fn caption(self) -> &'static str {
        match self {
            Layer::Resource => "Resource Map",
            Layer::Stations => "Station Statuses",
            Layer::Grade => "Track Grade",
            Layer::Traffic => "Traffic Density",
            Layer::Company => "Track by Company",
        }
    }

    /// LNG id and English text of the hint (`default2.lng` 1936..1940).
    fn tip(self) -> (u32, &'static str) {
        match self {
            Layer::Resource => (1936, "View the resource map."),
            Layer::Stations => (1939, "View station statuses."),
            Layer::Grade => (1937, "View the track grade map."),
            Layer::Traffic => (1938, "View the traffic density map."),
            Layer::Company => (1940, "View track by company."),
        }
    }
}

/// Map Overview state.
#[derive(Debug)]
pub(super) struct OverviewState {
    pub open: bool,
    pub layer: Layer,
    /// Cargos switched on by a click.
    pub cargo_on: BTreeSet<u8>,
    /// Cargo under the mouse (shown while it is there).
    pub hover: Option<u8>,
    /// Company chosen for Track by Company (pool slot).
    pub company: Option<i32>,
    /// Buildings with their cargos, track tiles and the day they were computed for.
    pub blds: Vec<Building>,
    pub tiles: Vec<TrackTile>,
    pub max_wear: f32,
    pub roles: (BTreeSet<u8>, BTreeSet<u8>),
    pub key: Option<(u64, i32)>,
    /// Toggles of the cargo grid, for hover detection (filled while the panel is built).
    pub cargo_nodes: RefCell<Vec<(NodeId, u8)>>,
}

impl Default for OverviewState {
    fn default() -> Self {
        OverviewState {
            open: false,
            layer: Layer::Resource,
            cargo_on: BTreeSet::new(),
            hover: None,
            company: None,
            blds: Vec::new(),
            tiles: Vec::new(),
            max_wear: 0.0,
            roles: Default::default(),
            key: None,
            cargo_nodes: RefCell::default(),
        }
    }
}

fn fnt(n: u8, dz: f32) -> FontStyle {
    let f = game_font(n);
    f.with_size((f.size * dz).round().max(9.0))
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
    pub(super) fn overview_open(&self) -> bool {
        self.ga.overview.open && self.in_game() && !self.editor_active()
    }

    /// The globe icon: opens the overview, or closes it again.
    pub(super) fn overview_toggle(&mut self) {
        if !self.in_game() || self.editor_active() {
            return;
        }
        if self.ga.overview.open {
            self.ga.overview.open = false;
        } else {
            self.ga.overview.open = true;
            if let Screen::Game(g) = &mut self.screen {
                g.build.tool = None;
                g.build.from = None;
                g.build.plan = None;
            }
            if self.ga.overview.company.is_none() {
                self.ga.overview.company = self.player();
            }
            self.ga.overview.key = None;
            self.overview_refresh();
        }
        self.rebuild_ui();
    }

    /// Esc: closes the overview; true if it was open.
    pub(super) fn overview_cancel(&mut self) -> bool {
        if !self.overview_open() {
            return false;
        }
        self.ga.overview.open = false;
        self.rebuild_ui();
        true
    }

    /// Recomputes the data of the layers when the map or the day changed; true if it did.
    pub(super) fn overview_refresh(&mut self) -> bool {
        if !self.overview_open() {
            return false;
        }
        let Screen::Game(g) = &self.screen else {
            return false;
        };
        let (Some(save), Some(data)) = (g.world.save_ref(), self.data.as_ref()) else {
            return false;
        };
        let key = (g.scene_fp, g.world.minutes() / 1440);
        if self.ga.overview.key == Some(key) {
            return false;
        }
        let year = i32::from(g.world.date().year);
        let blds = ov::buildings(save, data, year);
        let tiles = ov::track_tiles(save);
        let max_wear = tiles.iter().map(|t| t.wear).fold(0.0, f32::max);
        let roles = ov::cargo_roles(&blds);
        let o = &mut self.ga.overview;
        o.blds = blds;
        o.tiles = tiles;
        o.max_wear = max_wear;
        o.roles = roles;
        o.key = Some(key);
        true
    }

    /// Hover over a cargo of the grid lights it up on the map; called when the cursor moves.
    pub(super) fn overview_hover(&mut self) {
        if !self.overview_open() {
            return;
        }
        let h = self.ui.hovered();
        let now = h.and_then(|n| {
            self.ga
                .overview
                .cargo_nodes
                .borrow()
                .iter()
                .find(|(id, _)| *id == n)
                .map(|(_, c)| *c)
        });
        if now != self.ga.overview.hover {
            self.ga.overview.hover = now;
            self.rebuild_ui();
        }
    }

    pub(super) fn overview_activated(&mut self, action: u32) -> bool {
        if !self.overview_open() {
            return false;
        }
        let o = &mut self.ga.overview;
        match action {
            a if (A_OV_LAYER..A_OV_LAYER + 5).contains(&a) => {
                o.layer = Layer::ALL[(a - A_OV_LAYER) as usize];
            }
            A_OV_ALL => o.cargo_on = o.roles.0.union(&o.roles.1).copied().collect(),
            A_OV_CLEAR => o.cargo_on.clear(),
            A_OV_CANCEL => o.open = false,
            a if (A_OV_CARGO..A_OV_CARGO + 100).contains(&a) => {
                let c = (a - A_OV_CARGO) as u8;
                if !o.cargo_on.remove(&c) {
                    o.cargo_on.insert(c);
                }
            }
            a if (A_OV_COMPANY..A_OV_COMPANY + 100).contains(&a) => {
                let list = self.overview_companies();
                self.ga.overview.company = list.get((a - A_OV_COMPANY) as usize).map(|c| c.0);
            }
            _ => return false,
        }
        self.rebuild_ui();
        true
    }

    /// Active companies: `(slot, name)`.
    fn overview_companies(&self) -> Vec<(i32, String)> {
        let Some(g) = self.game() else {
            return Vec::new();
        };
        let Some(save) = g.world.save_ref() else {
            return Vec::new();
        };
        let names = self.with_info(|i| {
            save.companies
                .ordered()
                .filter(|c| crate::buildtools::Rec(&c.data).i32(crate::buildtools::CO_ACTIVE) != 0)
                .map(|c| (c.slot as i32, i.company_name(c.slot as u32)))
                .collect::<Vec<_>>()
        });
        names.unwrap_or_default()
    }

    fn cargo_name(&self, c: u8) -> String {
        let name = self.texts.table(
            "CARGO_TYPES",
            u32::from(c),
            &self.texts.af("cargo %1", &[c.to_string()]),
        );
        let mut ch = name.chars();
        match ch.next() {
            Some(f) => f.to_uppercase().chain(ch).collect(),
            None => name,
        }
    }

    /// The panel in place of the center list box.
    pub(super) fn build_overview_panel(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        w: f32,
        h: f32,
        dz: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let t = &self.texts;
        let o = &self.ga.overview;
        o.cargo_nodes.borrow_mut().clear();
        let pad = 8.0 * dz;
        let row_h = (24.0 * dz).round();
        let f1 = fnt(1, dz * 0.92);
        ui.add(
            parent,
            lbl(
                &t.get(1931, "Map Overview").to_uppercase(),
                fnt(2, dz),
                HEAD,
                Align::Left,
            ),
            at(pad, 4.0 * dz, w * 0.4, row_h),
        );
        let cancel = ui.add(
            parent,
            Kind::Button {
                text: t.get(1929, "Cancel"),
                font: f1,
                action: A_OV_CANCEL,
                enabled: true,
                hotkey: None,
            },
            Layout::at(Anchor::TopRight, 90.0 * dz, row_h).offset(pad, 4.0 * dz),
        );
        tips.push((cancel, t.get(1930, "Cancel out of 'Map Overview'")));
        // The five layers down the left side.
        let lw = (w * 0.27).min(190.0 * dz);
        let mut y = 4.0 * dz + row_h + 4.0 * dz;
        let lh = ((h - y - pad) / 5.0).min(row_h * 1.1);
        for (i, l) in Layer::ALL.iter().enumerate() {
            let (id, en) = l.tip();
            let n = ui.add(
                parent,
                Kind::Toggle {
                    text: t.a(l.caption()),
                    font: f1,
                    action: A_OV_LAYER + i as u32,
                    on: o.layer == *l,
                    enabled: true,
                },
                at(pad, y, lw, lh - 2.0),
            );
            tips.push((n, t.get(id, en)));
            y += lh;
        }
        // The right side depends on the layer.
        let rx = pad + lw + pad;
        let rw = (w - rx - pad).max(40.0);
        let top = 4.0 * dz + row_h + 4.0 * dz;
        let area_h = (h - top - pad).max(20.0);
        match o.layer {
            Layer::Resource => self.overview_cargo_grid(ui, parent, rx, top, rw, area_h, dz, tips),
            Layer::Stations => {
                self.overview_note(
                    ui,
                    parent,
                    rx,
                    top,
                    rw,
                    area_h,
                    dz,
                    &t.a("A box at each station shows how many lots are waiting there: green is little, red is a lot."),
                );
            }
            Layer::Grade => {
                let names = ["< 1%", "1 - 2%", "2 - 3%", "3 - 5%", "5% +"];
                self.overview_legend(
                    ui,
                    parent,
                    rx,
                    top,
                    rw,
                    dz,
                    &t.a("Steepness of the track"),
                    &GRADE_COLORS,
                    &names.map(String::from),
                );
            }
            Layer::Traffic => {
                let names = [
                    t.a("Very light"),
                    t.a("Light"),
                    t.a("Moderate"),
                    t.a("Heavy"),
                    t.a("Very heavy"),
                ];
                self.overview_legend(
                    ui,
                    parent,
                    rx,
                    top,
                    rw,
                    dz,
                    &t.a("Traffic on the track"),
                    &TRAFFIC_COLORS,
                    &names,
                );
            }
            Layer::Company => self.overview_company_list(ui, parent, rx, top, rw, area_h, dz),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn overview_note(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        dz: f32,
        text: &str,
    ) {
        ui.add(
            parent,
            Kind::Label {
                text: text.to_owned(),
                font: fnt(1, dz * 0.92),
                color: MUTED,
                align: Align::Left,
                wrap: true,
            },
            at(x, y, w, h),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn overview_legend(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        x: f32,
        y: f32,
        w: f32,
        dz: f32,
        title: &str,
        colors: &[Color; 5],
        names: &[String],
    ) {
        let row = (22.0 * dz).round();
        ui.add(
            parent,
            lbl(title, fnt(1, dz * 0.92), HEAD, Align::Left),
            at(x, y, w, row),
        );
        for (i, (c, n)) in colors.iter().zip(names).enumerate() {
            let yy = y + row * (i as f32 + 1.0);
            ui.add(
                parent,
                Kind::Panel {
                    fill: Fill::Solid(*c),
                },
                at(x, yy + 3.0, 30.0 * dz, row - 6.0),
            );
            ui.add(
                parent,
                lbl(n, fnt(1, dz * 0.92), TEXT, Align::Left),
                at(x + 38.0 * dz, yy, w - 38.0 * dz, row),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn overview_cargo_grid(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        dz: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let t = &self.texts;
        let o = &self.ga.overview;
        let f1 = fnt(1, dz * 0.92);
        // All and Clear on the first line.
        let bh = (22.0 * dz).round();
        let bw = (80.0 * dz).min(w / 2.0 - 4.0);
        let all = ui.add(
            parent,
            Kind::Button {
                text: t.get(1932, "All"),
                font: f1,
                action: A_OV_ALL,
                enabled: true,
                hotkey: None,
            },
            at(x, y, bw, bh),
        );
        tips.push((
            all,
            t.get(
                1933,
                "Select all of the cargos for viewing in the resource map.",
            ),
        ));
        let clear = ui.add(
            parent,
            Kind::Button {
                text: t.get(1934, "Clear"),
                font: f1,
                action: A_OV_CLEAR,
                enabled: true,
                hotkey: None,
            },
            at(x + bw + 6.0 * dz, y, bw, bh),
        );
        tips.push((clear, t.get(1935, "Clear all selected cargos.")));
        let (supply, demand) = &o.roles;
        let cargos: Vec<u8> = supply.union(demand).copied().collect();
        if cargos.is_empty() {
            return;
        }
        let gy = y + bh + 4.0 * dz;
        let gh = (h - bh - 4.0 * dz).max(20.0);
        let min_cell = 15.0 * dz;
        let max_rows = ((gh / min_cell).floor() as usize).max(1);
        let cols = cargos.len().div_ceil(max_rows).max(1);
        let rows = cargos.len().div_ceil(cols);
        let cw = w / cols as f32;
        let ch = (gh / rows as f32).min(row_cap(dz));
        let cf = fnt(1, (dz * 0.8).min(ch / 20.0 * 1.15));
        for (i, &c) in cargos.iter().enumerate() {
            let (col, row) = (i / rows, i % rows);
            let on = o.cargo_on.contains(&c);
            let n = ui.add(
                parent,
                Kind::Toggle {
                    text: String::new(),
                    font: cf,
                    action: A_OV_CARGO + u32::from(c),
                    on,
                    enabled: true,
                },
                at(
                    x + col as f32 * cw,
                    gy + row as f32 * ch,
                    cw - 2.0,
                    ch - 1.0,
                ),
            );
            // White text: some building supplies it; yellow: only demanded.
            let color = if supply.contains(&c) { SUPPLY } else { DEMAND };
            ui.add(
                parent,
                lbl(&self.cargo_name(c), cf, color, Align::Center),
                at(
                    x + col as f32 * cw,
                    gy + row as f32 * ch,
                    cw - 2.0,
                    ch - 1.0,
                ),
            );
            o.cargo_nodes.borrow_mut().push((n, c));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn overview_company_list(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        dz: f32,
    ) {
        let o = &self.ga.overview;
        let counts = ov::track_by_owner(&o.tiles);
        let list = self.overview_companies();
        let row = (22.0 * dz).round();
        let max = ((h / row) as usize).max(1);
        let f = fnt(1, dz * 0.92);
        for (i, (slot, name)) in list.iter().take(max).enumerate() {
            let n = counts.get(&(*slot as u8)).copied().unwrap_or(0);
            ui.add(
                parent,
                Kind::Toggle {
                    text: format!(
                        "{name}   {}",
                        self.texts.af("%1 track tiles", &[n.to_string()])
                    ),
                    font: f,
                    action: A_OV_COMPANY + i as u32,
                    on: o.company == Some(*slot),
                    enabled: true,
                },
                at(x, y + i as f32 * row, w, row - 2.0),
            );
        }
        if list.is_empty() {
            self.overview_note(ui, parent, x, y, w, h, dz, &self.texts.a("No companies."));
        }
    }

    /// The layer over the map: squares on the visible tiles.
    pub(super) fn build_overview_overlay(&self, ui: &mut Ui) {
        if !self.overview_open() {
            return;
        }
        let Some(g) = self.game() else {
            return;
        };
        let Some(save) = g.world.save_ref() else {
            return;
        };
        let o = &self.ga.overview;
        let map = &save.map;
        let (cw, ch) = ui.logical_size();
        let root = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let mut count = 0usize;
        let mut square = |ui: &mut Ui, x: i32, y: i32, c: Color, shrink: f32| {
            if count >= MAX_SQUARES
                || x < 0
                || y < 0
                || x as usize >= map.width
                || y as usize >= map.height
            {
                return;
            }
            let (px, py, w, h) = self.tile_rect(g, map, x, y, shrink);
            if px + w < 0.0 || py + h < 0.0 || px > cw || py > ch {
                return;
            }
            count += 1;
            ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Overlay(c),
                },
                at(px, py, w, h),
            );
        };
        match o.layer {
            Layer::Resource => {
                let mut sel = o.cargo_on.clone();
                sel.extend(o.hover);
                for m in ov::resource_markers(&o.blds, &sel) {
                    for y in m.y0..=m.y1 {
                        for x in m.x0..=m.x1 {
                            if m.supply {
                                square(ui, x, y, Color(0xff, 0xff, 0xff, 0xc8), -0.1);
                            }
                            if m.demand {
                                let shrink = if m.supply { 0.4 } else { -0.1 };
                                square(ui, x, y, Color(0xff, 0xd8, 0x20, 0xd8), shrink);
                            }
                        }
                    }
                }
            }
            Layer::Stations => {
                let cargos = self.data.as_ref().map_or(0, |d| d.cargo.len()).max(1);
                for (x, y, lots) in ov::station_statuses(save, cargos) {
                    if x < 0 || y < 0 || x as usize >= map.width || y as usize >= map.height {
                        continue;
                    }
                    let (px, py, _, _) = self.tile_rect(g, map, x, y, 0.0);
                    let (bw, bh) = (30.0 * self.hud_zoom(ch), 16.0 * self.hud_zoom(ch));
                    let color = match lots {
                        0 => Color(0x50, 0x58, 0x50, 0xe0),
                        1..=5 => Color(0x2a, 0x7a, 0x34, 0xe8),
                        6..=15 => Color(0xa0, 0x8c, 0x1c, 0xe8),
                        _ => Color(0xa8, 0x30, 0x28, 0xe8),
                    };
                    let bx = px - bw / 2.0;
                    let by = py - bh - 6.0;
                    if bx + bw < 0.0 || by + bh < 0.0 || bx > cw || by > ch {
                        continue;
                    }
                    let b = ui.add(
                        root,
                        Kind::Panel {
                            fill: Fill::Solid(color),
                        },
                        at(bx, by, bw, bh),
                    );
                    ui.add(
                        b,
                        lbl(
                            &lots.to_string(),
                            fnt(1, self.hud_zoom(ch) * 0.8).with_shadow(),
                            Color::rgb(0xff, 0xff, 0xff),
                            Align::Center,
                        ),
                        Layout::fill(),
                    );
                }
            }
            Layer::Grade => {
                for t in &o.tiles {
                    let c = GRADE_COLORS[ov::grade_band(t.grade)];
                    square(ui, t.x, t.y, Color(c.0, c.1, c.2, 0xd0), 0.0);
                }
            }
            Layer::Traffic => {
                for t in &o.tiles {
                    if let Some(b) = ov::traffic_band(t.wear, o.max_wear) {
                        let c = TRAFFIC_COLORS[b];
                        square(ui, t.x, t.y, Color(c.0, c.1, c.2, 0xd8), 0.0);
                    }
                }
            }
            Layer::Company => {
                let sel = o.company.and_then(|c| u8::try_from(c).ok());
                for t in &o.tiles {
                    if Some(t.owner) == sel {
                        square(ui, t.x, t.y, Color(0xff, 0xf0, 0x40, 0xe0), 0.0);
                    } else {
                        square(ui, t.x, t.y, Color(0x60, 0x80, 0xc0, 0x60), 0.2);
                    }
                }
            }
        }
    }

    /// Scale of small map labels, by canvas height.
    fn hud_zoom(&self, ch: f32) -> f32 {
        (ch / 700.0).clamp(0.8, 1.6)
    }
}

fn row_cap(dz: f32) -> f32 {
    26.0 * dz
}

#[allow(dead_code)]
const _: u32 = lng::CANCEL;
