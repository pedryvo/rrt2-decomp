//! The company book's extra pages (docs/reference/screens.md G2, G5, G13): the section strip
//! (Overview, Finances, Territories, all companies), the Territories tab with its three map
//! views (Political, Access Rights, Goodwill; manual chapter 7) and the clicks that lead to
//! the territory rights window, and the full-screen company list `compnlst.win`.
//!
//! Data comes from `info::groupb` (read-only); the only things that change the game are the
//! existing dialogs (`Dialog::Rights`, `Dialog::Fin`, `Dialog::Brand`).

use std::sync::Arc;

use rt2_ui::{Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::dialogs::Dialog;
use super::info_ui::{InfoScreen, draw_table};
use super::{AppState, Screen};
use crate::info::{Table, TerritoryRow};

pub(super) const A_BOOK: u32 = 11000;
pub(super) const A_TERR_MODE: u32 = 11010;
pub(super) const A_TERR_MAP: u32 = 11020;
pub(super) const A_BOOK_BRAND: u32 = 11030;
pub(super) const A_TERR_ROW: u32 = 11040;

const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xc8);
const NO_LAND: Color = Color::rgb(0x18, 0x1c, 0x20);

/// The three views of the Territories tab, in button order.
pub(super) const TERR_MODES: usize = 3;

fn fnt(size: f32) -> FontStyle {
    game_font(2).with_size(size)
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

/// Distinct colour of territory `i` for the political view.
fn political(i: usize) -> Color {
    const P: [(u8, u8, u8); 12] = [
        (0xc8, 0x5a, 0x4a),
        (0x4a, 0x8a, 0xc8),
        (0x6a, 0xb0, 0x5a),
        (0xd0, 0xb0, 0x4a),
        (0x9a, 0x6a, 0xc0),
        (0x4a, 0xb0, 0xa8),
        (0xc8, 0x8a, 0x4a),
        (0xb0, 0x5a, 0x8a),
        (0x8a, 0xa0, 0x4a),
        (0x5a, 0x6a, 0xb8),
        (0xb8, 0x9a, 0x7a),
        (0x7a, 0xb8, 0xd8),
    ];
    let (r, g, b) = P[i % P.len()];
    Color::rgb(r, g, b)
}

/// Colour of a territory in a view: 0 political, 1 access rights (green all four, yellow some,
/// red none), 2 goodwill (red at -100 through grey to green at +100).
pub(super) fn territory_color(mode: usize, i: usize, row: &TerritoryRow) -> Color {
    match mode {
        0 => political(i),
        1 => match row.rights & 0xf {
            0xf => Color::rgb(0x5a, 0xc0, 0x5a),
            0 => Color::rgb(0xc8, 0x4a, 0x4a),
            _ => Color::rgb(0xd8, 0xc0, 0x4a),
        },
        _ => {
            let g = (row.goodwill as f32 / 10_000.0).clamp(-1.0, 1.0);
            let (r, gr) = if g < 0.0 {
                (0xc0 as f32, 0x80 as f32 * (1.0 + g))
            } else {
                (0xc0 as f32 * (1.0 - g), 0x80 as f32 + 0x60 as f32 * g)
            };
            Color::rgb(r as u8, gr as u8, 0x50)
        }
    }
}

/// Cells of the territory map: `cols` x `rows` samples of the tiles' regions.
pub(super) struct TerrMap {
    pub cols: usize,
    pub rows: usize,
    /// Territory row index per cell, `None` outside every territory.
    pub cells: Vec<Option<usize>>,
}

pub(super) fn terr_map(save: &rt2_formats::map::Save, rows: &[TerritoryRow]) -> TerrMap {
    let (w, h) = (save.map.width.max(1), save.map.height.max(1));
    let step = w.max(h).div_ceil(120).max(1);
    let (cols, nrows) = (w.div_ceil(step), h.div_ceil(step));
    let mut cells = Vec::with_capacity(cols * nrows);
    for cy in 0..nrows {
        for cx in 0..cols {
            let (x, y) = (
                (cx * step + step / 2).min(w - 1),
                (cy * step + step / 2).min(h - 1),
            );
            let region = save.map.tile(x, y).region;
            cells.push(
                rows.iter()
                    .position(|r| r.region == region && r.region != 0),
            );
        }
    }
    TerrMap {
        cols,
        rows: nrows,
        cells,
    }
}

impl AppState {
    /// The company the book is showing (the player's when the screen says 0).
    pub(super) fn book_company(&self, c: u32) -> u32 {
        if c != 0 {
            return c;
        }
        self.with_info(|i| i.player_company().unwrap_or(0))
            .unwrap_or(0)
    }

    /// The buttons of the Overview tab: Resign / View-Hire Manager (own company) or the
    /// shareholder's buttons, plus Change Company Name/Logo for the player's company.
    pub(super) fn overview_buttons(&self, slot: u32) -> Vec<(String, u32)> {
        use super::dialogs::fin::A_FINACT;
        // Indexes of `FinAct`: 7 Manager, 8 Resign, 9 Assume, 10 Election, 11 Trade.
        let mut v: Vec<(String, u32)> = self
            .fin_buttons(slot)
            .into_iter()
            .filter(|(_, id)| (7..=11).contains(&(id - A_FINACT)))
            .collect();
        if self.player() == Some(slot as i32) {
            v.push((self.texts.a("Change Company Name/Logo"), A_BOOK_BRAND));
        }
        v
    }

    /// The four section buttons under the tabs; returns nothing, the caller shifts the body.
    pub(super) fn draw_book_strip(
        &self,
        ui: &mut Ui,
        panel: NodeId,
        screen: InfoScreen,
        x: f32,
        y: f32,
        w: f32,
    ) {
        let t = &self.texts;
        let names = [
            t.a("Overview"),
            t.get(lng_finances(), "Finances"),
            t.get(2369, "Territories"),
            t.a("Companies"),
        ];
        let cur = match screen {
            InfoScreen::Overview(_) => 0,
            InfoScreen::Finance(_) => 1,
            InfoScreen::Territories(..) => 2,
            _ => 3,
        };
        let gap = 6.0;
        let bw = ((w - 3.0 * gap) / 4.0).min(190.0);
        for (i, n) in names.iter().enumerate() {
            ui.add(
                panel,
                Kind::Toggle {
                    text: n.clone(),
                    font: fnt(13.0),
                    action: A_BOOK + i as u32,
                    on: i == cur,
                    enabled: true,
                },
                at(x + i as f32 * (bw + gap), y, bw, 28.0),
            );
        }
    }

    /// Section buttons, mode buttons and the name / logo button.
    pub(super) fn book_activated(&mut self, action: u32, screen: Option<InfoScreen>) -> bool {
        let company = match screen {
            Some(
                InfoScreen::Finance(c) | InfoScreen::Overview(c) | InfoScreen::Territories(c, _),
            ) => self.book_company(c),
            _ => self.book_company(0),
        };
        match action {
            a if (A_BOOK..A_BOOK + 4).contains(&a) => {
                let next = match a - A_BOOK {
                    0 => InfoScreen::Overview(company),
                    1 => InfoScreen::Finance(company),
                    2 => InfoScreen::Territories(company, 0),
                    _ => InfoScreen::Companies,
                };
                self.open_info(next);
            }
            a if (A_TERR_MODE..A_TERR_MODE + TERR_MODES as u32).contains(&a) => {
                self.open_info(InfoScreen::Territories(company, (a - A_TERR_MODE) as u8));
            }
            A_BOOK_BRAND => self.open_brand(company as i32),
            _ => return false,
        }
        true
    }

    /// A press on the territory map: the territory under it opens its rights window.
    pub(super) fn book_pointer(&mut self, action: u32, fx: f32, fy: f32) -> bool {
        if action != A_TERR_MAP {
            return false;
        }
        let Some(InfoScreen::Territories(c, _)) = self.info_screen() else {
            return true;
        };
        let company = self.book_company(c);
        let hit = self.with_info(|i| {
            let rows = i.territory_rows(company);
            let map = terr_map(i.save, &rows);
            let cx = ((fx * map.cols as f32) as usize).min(map.cols - 1);
            let cy = ((fy * map.rows as f32) as usize).min(map.rows - 1);
            map.cells[cy * map.cols + cx].map(|k| rows[k].slot)
        });
        if let Some(Some(slot)) = hit {
            self.open_territory(slot);
        }
        true
    }

    /// A click on a row of the territories table.
    pub(super) fn territory_row(&mut self, company: u32, row: usize) {
        let company = self.book_company(company);
        if let Some(Some(r)) = self.with_info(|i| i.territory_rows(company).get(row).cloned()) {
            self.open_territory(r.slot);
        }
    }

    /// The rights window of the player's company, or the information page without one.
    fn open_territory(&mut self, slot: u32) {
        if self.player().is_some() {
            self.push_dialog(Dialog::Rights {
                territory: slot as i32,
            });
        } else {
            self.open_info(InfoScreen::Territory(slot));
        }
    }

    /// The Territories tab body: mode buttons, the map and the table (click either).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_territories(
        &self,
        ui: &mut Ui,
        panel: NodeId,
        company: u32,
        mode: u8,
        x: f32,
        y: f32,
        w: f32,
        bottom: f32,
    ) {
        let t = &self.texts;
        let Some((rows, map, table)) = self.with_info(|i| {
            let rows = i.territory_rows(company);
            let map = terr_map(i.save, &rows);
            let table = match i.territories_page(company).right.into_iter().next() {
                Some(crate::info::Block::Table { table, .. }) => Some(table),
                _ => None,
            };
            (rows, map, table)
        }) else {
            return;
        };
        let mode = usize::from(mode).min(TERR_MODES - 1);
        let names = [
            t.a("Political view"),
            t.a("Access rights view"),
            t.a("Goodwill view"),
        ];
        let two = w >= 700.0;
        let lw = if two { w * 0.46 } else { w };
        let bw = (lw - 12.0) / 3.0;
        for (i, n) in names.iter().enumerate() {
            ui.add(
                panel,
                Kind::Toggle {
                    text: n.clone(),
                    font: fnt(12.0),
                    action: A_TERR_MODE + i as u32,
                    on: i == mode,
                    enabled: true,
                },
                at(x + i as f32 * (bw + 6.0), y, bw, 28.0),
            );
        }
        // The map keeps the aspect of the cells; its press opens the rights window.
        let avail_h = (bottom - y - 40.0).max(80.0);
        let side = (avail_h * 0.7).min(lw);
        let fit = (side / map.cols.max(map.rows) as f32).max(1.0);
        let (mw, mh) = (map.cols as f32 * fit, map.rows as f32 * fit);
        let frame = ui.add(
            panel,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(x, y + 34.0, mw + 8.0, mh + 8.0),
        );
        // The territory colour over the radar's terrain, so coasts stay visible.
        let radar = self.game().and_then(|g| g.radar.as_ref());
        let colors: Vec<Color> = map
            .cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let base = radar.map_or(NO_LAND, |r| {
                    let (cx, cy) = (i % map.cols, i / map.cols);
                    let rx = (cx * r.cols / map.cols).min(r.cols - 1);
                    let ry = (cy * r.rows / map.rows).min(r.rows - 1);
                    r.colors()[ry * r.cols + rx]
                });
                match c {
                    None => base,
                    Some(k) => {
                        let t = territory_color(mode, *k, &rows[*k]);
                        Color::rgb(
                            ((u16::from(t.0) * 3 + u16::from(base.0)) / 4) as u8,
                            ((u16::from(t.1) * 3 + u16::from(base.1)) / 4) as u8,
                            ((u16::from(t.2) * 3 + u16::from(base.2)) / 4) as u8,
                        )
                    }
                }
            })
            .collect();
        ui.add(
            frame,
            Kind::Cells {
                cols: map.cols,
                rows: map.rows,
                colors: Arc::new(colors),
                action: A_TERR_MAP,
            },
            at(4.0, 4.0, mw, mh),
        );
        // Legend of the view.
        let legend: Vec<(Color, String)> = match mode {
            0 => Vec::new(),
            1 => vec![
                (territory_color(1, 0, &row_with(0xf, 0)), t.a("All rights")),
                (territory_color(1, 0, &row_with(1, 0)), t.a("Some rights")),
                (territory_color(1, 0, &row_with(0, 0)), t.a("No rights")),
            ],
            _ => vec![
                (territory_color(2, 0, &row_with(0, -10_000)), "-100".into()),
                (territory_color(2, 0, &row_with(0, 0)), "0".into()),
                (territory_color(2, 0, &row_with(0, 10_000)), "+100".into()),
            ],
        };
        let mut lx = x;
        let ly = y + 34.0 + mh + 14.0;
        for (c, text) in legend {
            ui.add(
                panel,
                Kind::Panel {
                    fill: Fill::Solid(c),
                },
                at(lx, ly + 3.0, 14.0, 14.0),
            );
            ui.add(
                panel,
                lbl(&text, fnt(12.0), MUTED, Align::Left),
                at(lx + 20.0, ly, 120.0, 20.0),
            );
            lx += 140.0;
        }
        ui.add(
            panel,
            Kind::Label {
                text: t.a("Click a territory to see its government and buy access rights."),
                font: fnt(12.0),
                color: MUTED,
                align: Align::Left,
                wrap: true,
            },
            at(x, ly + 24.0, lw, 40.0),
        );
        // The table, with an invisible list over it for the clicks.
        let Some(table) = table else {
            ui.add(
                panel,
                lbl(
                    &t.a("This map has no political divisions."),
                    fnt(15.0),
                    MUTED,
                    Align::Left,
                ),
                at(x + lw + 24.0, y + 40.0, w - lw - 24.0, 24.0),
            );
            return;
        };
        let (tx, tw) = if two {
            (x + lw + 20.0, w - lw - 20.0)
        } else {
            (x, w)
        };
        let ty = if two { y } else { y + 34.0 + mh + 70.0 };
        let rowh = 22.0;
        let visible = (((bottom - ty - rowh - 6.0) / rowh).floor() as usize).max(3);
        let shown = table.rows.len().min(visible);
        ui.add(
            panel,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(tx, ty, tw, rowh * (shown as f32 + 1.0) + 8.0),
        );
        ui.add(
            panel,
            Kind::List {
                items: vec![String::new(); shown],
                selected: None,
                scroll: 0,
                row_h: rowh,
                font: fnt(13.0),
                action: super::info_ui::A_ROW,
            },
            at(tx, ty + rowh + 3.0, tw, rowh * shown as f32),
        );
        let t2: Table = table;
        draw_table(
            ui,
            panel,
            &t2,
            tx,
            ty + 2.0,
            tw,
            rowh,
            13.0,
            0,
            shown,
            false,
        );
        let _ = A_TERR_ROW;
    }
}

fn row_with(rights: u8, goodwill: i32) -> TerritoryRow {
    TerritoryRow {
        slot: 0,
        region: 0,
        name: String::new(),
        rights,
        goodwill,
    }
}

/// LNG id of "Finances" (the tab label of the company book).
fn lng_finances() -> u32 {
    crate::text::id::FINANCES
}

/// Whether the screen is one of the company book's pages.
pub(super) fn is_book(screen: InfoScreen) -> bool {
    matches!(
        screen,
        InfoScreen::Finance(_)
            | InfoScreen::Overview(_)
            | InfoScreen::Territories(..)
            | InfoScreen::Companies
    )
}

#[allow(dead_code)]
fn _unused(_: &Screen) {}
