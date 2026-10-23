//! The windows that follow a construction: the station detail (`statndtl.win`) with its Buy
//! dialog (`buildbuy.win`), Purchase Train (`trainbuy.win`), the editable train detail
//! (`traindtl.win`: map router, route list), the industry window with Buy and Sell, and the
//! territory rights window (docs/reference/screens.md E1..E4, F1, F2, C3, D4, G5).
//!
//! Every action is a command of `rt2_sim::build::cmd::BuildCmd` (`0x17`..`0x20`, `0x2a`, `0x2d`)
//! sent with `World::apply_build`; what the original's windows check before sending is checked
//! here (permissions `0x5706b4`..`0x5706b9`, cash, availability: docs/spec/commands.md 3.3 to
//! 3.7). Gaps (marked TODO): the picture of a station shows the purchased improvements as
//! labelled chips because the overlay art of the improvements is not catalogued
//! (docs/formats/sprite-catalog.md); the stop commands `0x1a`, `0x1b`, `0x1c`, `0x21` and `0x2f`
//! (next stop, wait mode, cargo per stop, class, sell mode) are not in `rt2-sim` yet, so the
//! route list shows the wait mode and cargo but cannot change them.

use rt2_sim::build::cmd::{BuildCmd, CmdError};
use rt2_ui::{Align, Anchor, Color, Fill, Fit, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::dialogs::Dialog;
use super::info_ui;
use super::{AppState, Screen};
use crate::buildtools::{self as bt, Trade};
use crate::info::{Block, Table};
use crate::text::id as lng;

pub(super) const A_SD_BUY: u32 = 5100;
pub(super) const A_SB_ITEM: u32 = 5110;
pub(super) const A_SB_OK: u32 = 5140;
pub(super) const A_TB_LIST: u32 = 5150;
pub(super) const A_TB_COPY: u32 = 5151;
pub(super) const A_TB_OK: u32 = 5152;
pub(super) const A_TE_STOPS: u32 = 5160;
pub(super) const A_TE_PICK: u32 = 5161;
pub(super) const A_TE_ADD: u32 = 5162;
pub(super) const A_TE_INSERT: u32 = 5163;
pub(super) const A_TE_DELETE: u32 = 5164;
pub(super) const A_TE_REPLACE: u32 = 5165;
pub(super) const A_TE_SELL: u32 = 5166;
pub(super) const A_TE_MAP: u32 = 5167;
pub(super) const A_IN_BUY: u32 = 5170;
pub(super) const A_IN_SELL: u32 = 5171;
pub(super) const A_IN_DETAIL: u32 = 5172;
pub(super) const A_RT_BUY: u32 = 5180;
pub(super) const A_CLOSE: u32 = 5199;

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const BAD: Color = Color::rgb(0xf0, 0x8c, 0x7a);
const GOOD: Color = Color::rgb(0x9a, 0xe0, 0x8a);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd8);

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

fn button(text: String, f: FontStyle, action: u32, enabled: bool) -> Kind {
    Kind::Button {
        text,
        font: f,
        action,
        enabled,
        hotkey: None,
    }
}

/// A dialog frame: the panel, centred, in logical units with the design scale of the canvas.
struct Win {
    root: NodeId,
    w: f32,
    h: f32,
    dz: f32,
    pad: f32,
}

impl AppState {
    fn win(&self, ui: &mut Ui, want_w: f32, want_h: f32) -> Win {
        let (cw, ch) = ui.logical_size();
        let dz = (ch / 620.0).clamp(1.0, 2.0);
        let w = (want_w * dz).min(cw - 16.0);
        let h = (want_h * dz).min(ch - 16.0);
        let root = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, w, h),
        );
        Win {
            root,
            w,
            h,
            dz,
            pad: 14.0 * dz,
        }
    }

    fn player_station(&self, station: i32) -> bool {
        let (Some(co), Screen::Game(g)) = (self.player(), &self.screen) else {
            return false;
        };
        g.world
            .save_ref()
            .and_then(|s| bt::rec(&s.stations, station))
            .is_some_and(|st| st.i32(0x39) == co)
    }

    /// Opens a station: the composite detail for the player's own, the information page for
    /// the others (`0x4c2050`: only the local company's stations open the Buy window).
    pub(super) fn open_station(&mut self, station: i32) {
        if self.player_station(station) {
            self.push_dialog(Dialog::Station { station });
        } else {
            self.open_info(info_ui::InfoScreen::StationDetail(station as u32));
        }
    }

    // ---- the data of the windows ------------------------------------------------------------

    pub(super) fn date_ym(&self) -> (i32, i32) {
        match &self.screen {
            Screen::Game(g) => {
                let d = g.world.date();
                (i32::from(d.year), i32::from(d.month))
            }
            Screen::Menu(_) => (1830, 1),
        }
    }

    /// Runs `f` with the save, the data tables and the player's company.
    fn with_game<R>(
        &self,
        f: impl FnOnce(&rt2_formats::map::Save, &rt2_sim::data::GameData, i32) -> R,
    ) -> Option<R> {
        let co = self.player()?;
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        Some(f(g.world.save_ref()?, self.data.as_ref()?, co))
    }

    fn engine_name(&self, data: &rt2_sim::data::GameData, e: i32) -> String {
        self.texts
            .get(bt::engine_name_id(data, e), &format!("Engine {e}"))
    }

    // ---- building the dialogs ---------------------------------------------------------------

    /// Builds the dialogs of this module; false for the others.
    pub(super) fn build_build_dialog(&self, ui: &mut Ui, d: &Dialog) -> bool {
        match d {
            Dialog::Station { station } => self.build_station_detail(ui, *station),
            Dialog::StationBuy {
                station,
                mask,
                last,
            } => self.build_station_buy(ui, *station, *mask, *last),
            Dialog::TrainBuy { sel, copy, replace } => {
                self.build_train_buy(ui, *sel, *copy, *replace)
            }
            Dialog::TrainEdit {
                train,
                sel,
                pick,
                confirm,
            } => self.build_train_edit(ui, *train, *sel, *pick, *confirm),
            Dialog::Industry { building } => self.build_industry(ui, *building),
            Dialog::Rights { territory } => self.build_rights(ui, *territory),
            _ => return false,
        }
        true
    }

    /// Facts and tables of an information page stacked in a column; returns the height used.
    #[allow(clippy::too_many_arguments)]
    fn blocks(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        blocks: &[Block],
        x: f32,
        y: f32,
        w: f32,
        dz: f32,
    ) -> f32 {
        let row = (18.0 * dz).round();
        let f = fnt(1, dz * 0.85);
        let mut cy = y;
        for b in blocks {
            match b {
                Block::Facts(v) => {
                    for (k, val) in v {
                        ui.add(
                            parent,
                            lbl(k, f, MUTED, Align::Left),
                            at(x, cy, w * 0.45, row),
                        );
                        ui.add(
                            parent,
                            lbl(val, f, TEXT, Align::Left),
                            at(x + w * 0.45, cy, w * 0.55, row),
                        );
                        cy += row;
                    }
                }
                Block::Table { title, table } => {
                    ui.add(parent, lbl(title, f, HEAD, Align::Left), at(x, cy, w, row));
                    cy += row;
                    let rows = table.rows.len().min(14);
                    cy += info_ui::draw_table(
                        ui,
                        parent,
                        table,
                        x,
                        cy,
                        w,
                        row,
                        13.0 * dz.max(1.0) * 0.9,
                        0,
                        rows,
                        true,
                    )
                    .max(row);
                }
                Block::Note(text) => {
                    let lines = crate::scenario_info::wrap_text(
                        text,
                        ((w / (f.size * 0.5)) as usize).max(10),
                    );
                    let h = lines.len() as f32 * row * 0.8;
                    ui.add(
                        parent,
                        Kind::Text {
                            lines,
                            scroll: 0,
                            row_h: row * 0.8,
                            font: fnt(8, dz * 0.55),
                            color: MUTED,
                        },
                        at(x, cy, w, h),
                    );
                    cy += h;
                }
            }
            cy += row * 0.4;
        }
        cy - y
    }

    // ---- station detail ---------------------------------------------------------------------

    fn build_station_detail(&self, ui: &mut Ui, station: i32) {
        let win = self.win(ui, 900.0, 560.0);
        let Win {
            root,
            w,
            h,
            dz,
            pad,
        } = win;
        let Some(page) = self.with_info(|i| i.station_page(station as u32)) else {
            return;
        };
        let t = &self.texts;
        ui.add(
            root,
            lbl(
                &page.title.to_uppercase(),
                fnt(3, dz * 0.8),
                HEAD,
                Align::Left,
            ),
            at(pad, pad * 0.6, w * 0.6, 30.0 * dz),
        );
        ui.add(
            root,
            lbl(&page.subtitle, fnt(1, dz * 0.9), MUTED, Align::Right),
            at(w * 0.6, pad * 0.6, w * 0.4 - pad, 30.0 * dz),
        );
        let top = 40.0 * dz;
        let foot = 44.0 * dz;
        let (lw, body_h) = (w * 0.44, h - top - foot - pad);
        // The composite picture: the building, then one chip per purchased improvement.
        let pic = ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(pad, top, lw, body_h * 0.6),
        );
        let (shape, mask, owner, place) = match &self.screen {
            Screen::Game(g) => g
                .world
                .save_ref()
                .and_then(|s| {
                    let st = bt::rec(&s.stations, station)?;
                    let b = bt::rec(&s.objects, st.i32(4))?;
                    Some((
                        usize::from(b.u8(6)),
                        st.u32(0x49),
                        st.i32(0x39),
                        (st.i32(0xa), st.i32(0xe)),
                    ))
                })
                .unwrap_or_default(),
            Screen::Menu(_) => Default::default(),
        };
        let _ = (owner, place);
        if let Some(Some(sprite)) = self.dialog_art.stations.get(shape) {
            ui.add(
                pic,
                Kind::Image {
                    sprite: *sprite,
                    fit: Fit::Contain,
                },
                Layout::fill().offset(6.0, 6.0),
            );
        }
        // Improvement chips along the bottom of the picture box.
        let chip_h = 18.0 * dz;
        let mut cx = 6.0 * dz;
        let mut cy = body_h * 0.6 - chip_h - 4.0 * dz;
        for row in 3..30u32 {
            if mask & (1 << row) == 0 || matches!(row, 12..=14) {
                continue;
            }
            let name = t.get(1043 + row, &format!("Improvement {row}"));
            let cw = (name.chars().count() as f32 * 7.0 * dz * 0.85 + 12.0 * dz).min(lw - 12.0);
            if cx + cw > lw - 6.0 {
                cx = 6.0 * dz;
                cy -= chip_h + 3.0 * dz;
            }
            let chip = ui.add(
                pic,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x2c, 0x44, 0x2c, 0xe0)),
                },
                at(cx, cy, cw, chip_h),
            );
            ui.add(
                chip,
                lbl(&name, fnt(1, dz * 0.7), TEXT, Align::Center),
                Layout::fill(),
            );
            cx += cw + 4.0 * dz;
        }
        // Facts under the picture.
        let left_facts: Vec<Block> = page.left.iter().take(1).cloned().collect();
        self.blocks(
            ui,
            root,
            &left_facts,
            pad,
            top + body_h * 0.6 + 4.0 * dz,
            lw,
            dz,
        );
        // Supply / demand on the right.
        let rx = pad * 2.0 + lw;
        let rw = w - rx - pad;
        let right = page.right.clone();
        self.blocks(ui, root, &right, rx, top, rw, dz);
        // Buttons.
        let bw = 130.0 * dz;
        let bh = 30.0 * dz;
        let by = h - foot;
        let own = self.player_station(station);
        let allowed = self
            .with_game(|s, _, _| bt::permitted(s, bt::PERM_BUILD_SUPPORT))
            .unwrap_or(false);
        ui.add(
            root,
            button(t.a("Buy buildings"), fnt(1, dz), A_SD_BUY, own && allowed),
            at(pad, by, bw * 1.3, bh),
        );
        ui.add(
            root,
            button(t.a("Close"), fnt(1, dz), A_CLOSE, true),
            at(w - pad - bw, by, bw, bh),
        );
    }

    // ---- buy improvements -------------------------------------------------------------------

    /// The price of the checked improvements: `ftol(m * cost + total)` per bit
    /// (terrain-track.md 3.4).
    fn improvement_total(&self, station: i32, mask: u32) -> i32 {
        self.with_game(|s, data, _| {
            let Some(st) = bt::rec(&s.stations, station) else {
                return 0;
            };
            let m = bt::effect(s, 8, st.i32(0x39), st.i32(0x4ad));
            let mut total = 0i32;
            for i in 0..bt::IMP_ROWS {
                if mask & (1 << i) != 0 {
                    total = rt2_sim::fp::ftol(
                        f64::from(m) * f64::from(bt::improvement_base_cost(data, i))
                            + f64::from(total),
                    );
                }
            }
            total
        })
        .unwrap_or(0)
    }

    fn build_station_buy(&self, ui: &mut Ui, station: i32, mask: u32, last: u32) {
        let Win {
            root,
            w,
            h,
            dz,
            pad,
        } = self.win(ui, 640.0, 560.0);
        let t = &self.texts;
        ui.add(
            root,
            lbl(
                &t.a("Buy buildings").to_uppercase(),
                fnt(3, dz * 0.8),
                HEAD,
                Align::Left,
            ),
            at(pad, pad * 0.6, w - 2.0 * pad, 30.0 * dz),
        );
        let (year, month) = self.date_ym();
        let items = self
            .with_game(|s, data, _| bt::improvements(data, s, station, year, month))
            .unwrap_or_default();
        let row_h = ((h - 150.0 * dz) / items.len().max(1) as f32).clamp(14.0, 26.0 * dz);
        let f = fnt(1, dz * 0.85);
        let mut y = 40.0 * dz;
        for it in &items {
            let usable = it.available && !it.owned && !it.excluded;
            let name = t.get(1043 + it.row, &format!("Improvement {}", it.row));
            let note = if it.owned {
                t.a("built")
            } else if it.excluded {
                t.a("replaced")
            } else if !it.available {
                t.a("not yet available")
            } else {
                crate::info::money(f64::from(it.cost))
            };
            ui.add(
                root,
                Kind::Toggle {
                    text: name,
                    font: f,
                    action: A_SB_ITEM + it.row,
                    on: mask & (1 << it.row) != 0 || it.owned,
                    enabled: usable,
                },
                at(pad, y, w * 0.58, row_h - 2.0),
            );
            ui.add(
                root,
                lbl(&note, f, if usable { HEAD } else { MUTED }, Align::Right),
                at(pad + w * 0.58, y, w * 0.42 - 2.0 * pad, row_h),
            );
            y += row_h;
        }
        // Description of the last row touched, the total and the buttons.
        let desc = t.get(1076 + last, "");
        ui.add(
            root,
            Kind::Label {
                text: desc,
                font: fnt(8, dz * 0.6),
                color: MUTED,
                align: Align::Left,
                wrap: true,
            },
            at(pad, y + 4.0, w - 2.0 * pad, 40.0 * dz),
        );
        let total = self.improvement_total(station, mask);
        let cash = self
            .with_game(|s, _, co| bt::cash(s, co))
            .unwrap_or_default();
        let over = f64::from(total) > cash;
        let by = h - 44.0 * dz;
        ui.add(
            root,
            lbl(
                &t.af("Cost: %1", &[crate::info::money(f64::from(total))]),
                fnt(1, dz),
                if over { BAD } else { HEAD },
                Align::Left,
            ),
            at(pad, by, w * 0.4, 30.0 * dz),
        );
        let bw = 120.0 * dz;
        ui.add(
            root,
            button(
                t.get(1912, "Purchase"),
                fnt(1, dz),
                A_SB_OK,
                mask != 0 && !over,
            ),
            at(w - pad - 2.0 * bw - 8.0, by, bw, 30.0 * dz),
        );
        ui.add(
            root,
            button(t.get(lng::CANCEL, "Cancel"), fnt(1, dz), A_CLOSE, true),
            at(w - pad - bw, by, bw, 30.0 * dz),
        );
    }

    // ---- train purchase ---------------------------------------------------------------------

    fn build_train_buy(&self, ui: &mut Ui, sel: usize, copy: usize, replace: Option<i32>) {
        let Win {
            root,
            w,
            h,
            dz,
            pad,
        } = self.win(ui, 860.0, 540.0);
        let t = &self.texts;
        let (year, month) = self.date_ym();
        let rows = self
            .with_game(|s, data, co| bt::engine_rows(data, s, co, year, month))
            .unwrap_or_default();
        let title = if replace.is_some() {
            t.get(429, "Replace")
        } else {
            t.get(1784, "Purchase a new train")
        };
        ui.add(
            root,
            lbl(&title.to_uppercase(), fnt(3, dz * 0.8), HEAD, Align::Left),
            at(pad, pad * 0.6, w - 2.0 * pad, 30.0 * dz),
        );
        let top = 40.0 * dz;
        let foot = 44.0 * dz;
        let lw = w * 0.5;
        let items: Vec<String> = self
            .with_game(|_, data, _| {
                rows.iter()
                    .map(|r| {
                        format!(
                            "{}  {}  {}",
                            self.engine_name(data, r.index),
                            r.first_year,
                            crate::info::money(f64::from(r.price))
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let list_h = h - top - foot - pad;
        ui.add(
            root,
            Kind::List {
                items: if items.is_empty() {
                    vec![t.get(355, "There are currently no engines available for purchase")]
                } else {
                    items
                },
                selected: Some(sel.min(rows.len().saturating_sub(1))),
                scroll: 0,
                row_h: (22.0 * dz).round(),
                font: fnt(1, dz * 0.85),
                action: A_TB_LIST,
            },
            at(pad, top, lw - pad, list_h * 0.62),
        );
        // The route to copy.
        let (trains, names) = self.copyable_trains();
        let mut copies = vec![t.a("First two stations (default route)")];
        copies.extend(names);
        let copy = copy.min(copies.len() - 1);
        if replace.is_none() {
            ui.add(
                root,
                lbl(
                    &t.a("Route of the new train"),
                    fnt(1, dz * 0.8),
                    MUTED,
                    Align::Left,
                ),
                at(pad, top + list_h * 0.62 + 2.0, lw, 18.0 * dz),
            );
            ui.add(
                root,
                Kind::List {
                    items: copies,
                    selected: Some(copy),
                    scroll: 0,
                    row_h: (20.0 * dz).round(),
                    font: fnt(1, dz * 0.8),
                    action: A_TB_COPY,
                },
                at(
                    pad,
                    top + list_h * 0.62 + 20.0 * dz,
                    lw - pad,
                    list_h * 0.38 - 20.0 * dz,
                ),
            );
        }
        let _ = trains;
        // Stats of the selected engine.
        let rx = lw + pad;
        let rw = w - rx - pad;
        let f = fnt(1, dz * 0.9);
        if let Some(r) = rows.get(sel) {
            let name = self
                .with_game(|_, data, _| self.engine_name(data, r.index))
                .unwrap_or_default();
            ui.add(
                root,
                lbl(&name, fnt(2, dz), HEAD, Align::Left),
                at(rx, top, rw, 26.0 * dz),
            );
            let kinds = [t.a("Steam"), t.a("Diesel"), t.get(2504, "Electric")];
            let facts = [
                (t.a("Type"), kinds[usize::from(r.kind).min(2)].clone()),
                (
                    t.get(2334, "Purchase Price"),
                    crate::info::money(f64::from(r.price)),
                ),
                (
                    t.get(155, "Maintenance"),
                    crate::info::money(f64::from(r.maintenance)),
                ),
                (t.get(157, "Fuel"), r.fuel.to_string()),
                (t.get(441, "Top speed (mph)"), r.top_mph.to_string()),
                (t.a("Power"), r.power.to_string()),
                (t.get(2340, "Reliability"), r.reliability.to_string()),
                (t.a("Available from"), r.first_year.to_string()),
            ];
            let mut y = top + 30.0 * dz;
            for (k, v) in facts {
                ui.add(
                    root,
                    lbl(&k, f, MUTED, Align::Left),
                    at(rx, y, rw * 0.5, 22.0 * dz),
                );
                ui.add(
                    root,
                    lbl(&v, f, TEXT, Align::Left),
                    at(rx + rw * 0.5, y, rw * 0.5, 22.0 * dz),
                );
                y += 22.0 * dz;
            }
            let _ = y;
        }
        let by = h - foot;
        let bw = 140.0 * dz;
        ui.add(
            root,
            button(
                if replace.is_some() {
                    t.get(429, "Replace")
                } else {
                    t.get(1912, "Purchase")
                },
                fnt(1, dz),
                A_TB_OK,
                !rows.is_empty(),
            ),
            at(w - pad - 2.0 * bw - 8.0, by, bw, 30.0 * dz),
        );
        ui.add(
            root,
            button(t.get(lng::CANCEL, "Cancel"), fnt(1, dz), A_CLOSE, true),
            at(w - pad - bw, by, bw, 30.0 * dz),
        );
    }

    /// The player's trains whose route a new train can copy: (slots, labels).
    fn copyable_trains(&self) -> (Vec<i32>, Vec<String>) {
        let slots = self
            .with_game(|s, _, co| bt::company_trains(s, co))
            .unwrap_or_default();
        let names = slots
            .iter()
            .map(|&s| {
                self.with_info(|i| i.train_page(s as u32).title)
                    .unwrap_or_default()
            })
            .collect();
        (slots, names)
    }

    // ---- train detail (editable) ------------------------------------------------------------

    fn build_train_edit(&self, ui: &mut Ui, train: i32, sel: usize, pick: usize, confirm: bool) {
        let Win {
            root,
            w,
            h,
            dz,
            pad,
        } = self.win(ui, 1000.0, 580.0);
        let t = &self.texts;
        let Some(page) = self.with_info(|i| i.train_page(train as u32)) else {
            return;
        };
        ui.add(
            root,
            lbl(
                &page.title.to_uppercase(),
                fnt(3, dz * 0.8),
                HEAD,
                Align::Left,
            ),
            at(pad, pad * 0.6, w * 0.6, 30.0 * dz),
        );
        let top = 40.0 * dz;
        let foot = 44.0 * dz;
        let body_h = h - top - foot - pad;
        let f = fnt(1, dz * 0.85);
        // Column 1: the map router and the facts.
        let side = (w * 0.31).min(body_h * 0.6);
        let (stops, stations, co) = self
            .with_game(|s, _, co| (bt::train_stops(s, train), bt::company_stations(s, co), co))
            .unwrap_or_default();
        let _ = co;
        ui.add(
            root,
            lbl(
                &t.a("Map router: click a station to add a stop"),
                fnt(8, dz * 0.55),
                MUTED,
                Align::Left,
            ),
            at(pad, top - 4.0 * dz, side, 14.0 * dz),
        );
        let box_ = ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(pad, top + 12.0 * dz, side, side),
        );
        if let Screen::Game(g) = &self.screen
            && let Some(r) = &g.radar
            && let Some(save) = g.world.save_ref()
        {
            let colors = self.router_colors(r, save, &stops, &stations);
            let (cols, rows) = (r.cols as f32, r.rows as f32);
            let fit = (side - 4.0) / cols.max(rows);
            ui.add(
                box_,
                Kind::Cells {
                    cols: r.cols,
                    rows: r.rows,
                    colors: std::sync::Arc::new(colors),
                    action: A_TE_MAP,
                },
                Layout::at(Anchor::Center, cols * fit, rows * fit),
            );
        }
        let facts: Vec<Block> = page.left.iter().take(1).cloned().collect();
        self.blocks(
            ui,
            root,
            &facts,
            pad,
            top + side + 20.0 * dz,
            side,
            dz * 0.9,
        );
        // Column 2: the route.
        let cx = pad * 2.0 + side;
        let cw = w * 0.32;
        ui.add(
            root,
            lbl(&t.a("Route"), fnt(1, dz * 0.9), HEAD, Align::Left),
            at(cx, top - 4.0 * dz, cw, 18.0 * dz),
        );
        let cur = self
            .with_game(|s, _, _| {
                bt::rec(&s.trains, train).map_or(-1, |r| i32::from(r.u8(0x3c) as i8))
            })
            .unwrap_or(-1);
        let items: Vec<String> = stops
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let name = if s.waypoint {
                    t.a("waypoint")
                } else {
                    self.with_info(|inf| inf.station_page(s.station as u32).title)
                        .unwrap_or_default()
                };
                let wait = [t.a("no wait"), t.a("half full"), t.a("full")]
                    [usize::from(s.wait.min(2))]
                .clone();
                format!(
                    "{}{} {}  [{} cargo]  {}",
                    i + 1,
                    if i as i32 == cur { ">" } else { "" },
                    name,
                    s.cargo.len(),
                    wait
                )
            })
            .collect();
        ui.add(
            root,
            Kind::List {
                items,
                selected: (!stops.is_empty()).then(|| sel.min(stops.len() - 1)),
                scroll: 0,
                row_h: (22.0 * dz).round(),
                font: f,
                action: A_TE_STOPS,
            },
            at(cx, top + 14.0 * dz, cw, body_h * 0.62),
        );
        // Wait modes, as the original words them, for the selected stop.
        if let Some(s) = stops.get(sel) {
            let msg = t.get(1114 + u32::from(s.wait.min(2)), "");
            ui.add(
                root,
                Kind::Label {
                    text: msg,
                    font: fnt(8, dz * 0.55),
                    color: MUTED,
                    align: Align::Left,
                    wrap: true,
                },
                at(cx, top + 18.0 * dz + body_h * 0.62, cw, body_h * 0.34),
            );
        }
        // Column 3: the station chooser and the buttons.
        let rx = cx + cw + pad;
        let rw = w - rx - pad;
        ui.add(
            root,
            lbl(&t.a("Add a stop at"), fnt(1, dz * 0.9), HEAD, Align::Left),
            at(rx, top - 4.0 * dz, rw, 18.0 * dz),
        );
        let names: Vec<String> = stations
            .iter()
            .map(|&s| {
                self.with_info(|inf| inf.station_page(s as u32).title)
                    .unwrap_or_default()
            })
            .collect();
        ui.add(
            root,
            Kind::List {
                items: names,
                selected: (!stations.is_empty()).then(|| pick.min(stations.len() - 1)),
                scroll: 0,
                row_h: (22.0 * dz).round(),
                font: f,
                action: A_TE_PICK,
            },
            at(rx, top + 14.0 * dz, rw, body_h * 0.45),
        );
        let bh = 26.0 * dz;
        let mut by = top + 18.0 * dz + body_h * 0.45;
        let allowed_sell = self
            .with_game(|s, _, _| bt::permitted(s, bt::PERM_RETIRE_TRAIN))
            .unwrap_or(false);
        let can_edit = !stations.is_empty();
        for (text, action, enabled) in [
            (t.a("Add stop"), A_TE_ADD, can_edit),
            (
                t.a("Insert before"),
                A_TE_INSERT,
                can_edit && !stops.is_empty(),
            ),
            (t.a("Delete stop"), A_TE_DELETE, stops.len() > 1),
            (t.get(429, "Replace engine"), A_TE_REPLACE, true),
            (
                if confirm {
                    t.a("Click again to sell")
                } else {
                    t.get(427, "Retire")
                },
                A_TE_SELL,
                allowed_sell,
            ),
        ] {
            ui.add(
                root,
                button(text, fnt(1, dz * 0.85), action, enabled),
                at(rx, by, rw, bh),
            );
            by += bh + 3.0 * dz;
        }
        ui.add(
            root,
            button(t.a("Close"), fnt(1, dz), A_CLOSE, true),
            at(w - pad - 120.0 * dz, h - foot, 120.0 * dz, 30.0 * dz),
        );
    }

    /// The router's cells: the radar's terrain, the route as a white line between the stops,
    /// the player's stations green and the others yellow.
    fn router_colors(
        &self,
        r: &crate::radar::Radar,
        save: &rt2_formats::map::Save,
        stops: &[bt::Stop],
        own: &[i32],
    ) -> Vec<Color> {
        let mut c: Vec<Color> = r.colors().to_vec();
        let (w, h) = (save.map.width as f32, save.map.height as f32);
        let cell = |x: i32, y: i32| -> Option<usize> {
            let cx = (x as f32 / w * r.cols as f32) as usize;
            let cy = (y as f32 / h * r.rows as f32) as usize;
            (cx < r.cols && cy < r.rows).then_some(cy * r.cols + cx)
        };
        let pos = |s: &bt::Stop| {
            if s.waypoint {
                Some((s.x, s.y))
            } else {
                bt::station_tile(save, s.station)
            }
        };
        let pts: Vec<(i32, i32)> = stops.iter().filter_map(pos).collect();
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            let n = 40;
            for i in 0..=n {
                let x = a.0 + (b.0 - a.0) * i / n;
                let y = a.1 + (b.1 - a.1) * i / n;
                if let Some(ix) = cell(x, y) {
                    c[ix] = Color::rgb(0xff, 0xff, 0xff);
                }
            }
        }
        for s in save.stations.ordered() {
            let (x, y) = (bt::Rec(&s.data).i32(0xa), bt::Rec(&s.data).i32(0xe));
            if let Some(ix) = cell(x, y) {
                c[ix] = if own.contains(&(s.slot as i32)) {
                    Color::rgb(0x40, 0xe0, 0x40)
                } else {
                    Color::rgb(0xf0, 0xe0, 0x40)
                };
            }
        }
        c
    }

    // ---- industry ---------------------------------------------------------------------------

    fn build_industry(&self, ui: &mut Ui, building: i32) {
        let Win {
            root,
            w,
            h,
            dz,
            pad,
        } = self.win(ui, 760.0, 520.0);
        let t = &self.texts;
        let Some(page) = self.with_info(|i| i.industry_page(building as u32)) else {
            return;
        };
        ui.add(
            root,
            lbl(
                &page.title.to_uppercase(),
                fnt(3, dz * 0.8),
                HEAD,
                Align::Left,
            ),
            at(pad, pad * 0.6, w * 0.7, 30.0 * dz),
        );
        let top = 40.0 * dz;
        let colw = (w - 3.0 * pad) / 2.0;
        self.blocks(ui, root, &page.left, pad, top, colw, dz);
        self.blocks(ui, root, &page.right, pad * 2.0 + colw, top, colw, dz);
        // Buy / Sell (commands.md 3.6).
        let (trade, price, cash) = self
            .with_game(|s, data, co| {
                (
                    bt::industry_trade(data, s, co, building),
                    bt::building_price(data, s, co, building),
                    bt::cash(s, co),
                )
            })
            .unwrap_or((Trade::None, 0, 0.0));
        let by = h - 44.0 * dz;
        let bw = 130.0 * dz;
        let f = fnt(1, dz);
        match trade {
            Trade::Buy => {
                ui.add(
                    root,
                    lbl(
                        &format!(
                            "{} {}",
                            t.get(2334, "Purchase Price"),
                            crate::info::money(f64::from(price))
                        ),
                        f,
                        if f64::from(price) > cash { BAD } else { HEAD },
                        Align::Left,
                    ),
                    at(pad, by, w * 0.4, 30.0 * dz),
                );
                ui.add(
                    root,
                    button(t.get(1912, "Purchase"), f, A_IN_BUY, true),
                    at(w - pad - 3.0 * bw - 16.0, by, bw, 30.0 * dz),
                );
            }
            Trade::Sell => {
                ui.add(
                    root,
                    lbl(&t.get(2338, "Sell this building.  You will only receive half the purchase price if you sell."), fnt(8, dz * 0.55), MUTED, Align::Left),
                    at(pad, by, w * 0.42, 30.0 * dz),
                );
                ui.add(
                    root,
                    button(t.get(2337, "Sell"), f, A_IN_SELL, true),
                    at(w - pad - 3.0 * bw - 16.0, by, bw, 30.0 * dz),
                );
            }
            Trade::None => {}
        }
        ui.add(
            root,
            button(t.a("Details"), f, A_IN_DETAIL, true),
            at(w - pad - 2.0 * bw - 8.0, by, bw, 30.0 * dz),
        );
        ui.add(
            root,
            button(t.a("Close"), f, A_CLOSE, true),
            at(w - pad - bw, by, bw, 30.0 * dz),
        );
    }

    // ---- territory rights -------------------------------------------------------------------

    fn build_rights(&self, ui: &mut Ui, territory: i32) {
        let Win {
            root,
            w,
            h,
            dz,
            pad,
        } = self.win(ui, 640.0, 360.0);
        let t = &self.texts;
        let name = self
            .with_info(|i| i.territory_page(territory as u32).title)
            .unwrap_or_default();
        ui.add(
            root,
            lbl(&name.to_uppercase(), fnt(3, dz * 0.8), HEAD, Align::Left),
            at(pad, pad * 0.6, w - 2.0 * pad, 30.0 * dz),
        );
        let have = self
            .with_game(|s, _, co| {
                let region =
                    bt::rec(&s.territories, territory).map_or(0, |r| i32::from(r.u8(0x2a)));
                (bt::rights(s, co, region), region)
            })
            .unwrap_or((0, 0));
        let f = fnt(1, dz * 0.9);
        let names = [
            (
                2459u32,
                "Rights to run trains in this territory will cost %1",
            ),
            (2460, "Rights to lay track in this territory will cost %1"),
            (
                2461,
                "Rights to buy buildings in this territory will cost %1",
            ),
            (
                2462,
                "Rights to build stations in this territory will cost %1",
            ),
        ];
        let mut y = 44.0 * dz;
        for (bit, (id, en)) in names.iter().enumerate() {
            let price = self
                .with_game(|s, _, co| bt::rights_price(s, co, territory, bit))
                .unwrap_or(0);
            let owned = have.0 & (1 << bit) != 0;
            let kind = [
                t.a("Run trains"),
                t.a("Lay track"),
                t.a("Buy buildings"),
                t.a("Build stations"),
            ][bit]
                .clone();
            let text = if owned {
                format!("{kind}: {}", t.a("You have these rights."))
            } else {
                format!(
                    "{kind}: {}",
                    t.get(*id, en)
                        .replace("%1", &crate::info::money(f64::from(price)))
                )
            };
            ui.add(
                root,
                lbl(&text, f, if owned { GOOD } else { TEXT }, Align::Left),
                at(pad, y, w - 2.0 * pad - 150.0 * dz, 28.0 * dz),
            );
            ui.add(
                root,
                button(
                    t.get(2350, "Purchase rights"),
                    fnt(1, dz * 0.8),
                    A_RT_BUY + bit as u32,
                    !owned,
                ),
                at(w - pad - 140.0 * dz, y, 140.0 * dz, 28.0 * dz),
            );
            y += 36.0 * dz;
        }
        ui.add(
            root,
            button(t.a("Close"), fnt(1, dz), A_CLOSE, true),
            at(w - pad - 120.0 * dz, h - 44.0 * dz, 120.0 * dz, 30.0 * dz),
        );
    }

    // ---- actions ----------------------------------------------------------------------------

    fn top_build_dialog(&self) -> Option<Dialog> {
        match &self.screen {
            Screen::Game(g) => g.dialogs.last().cloned(),
            Screen::Menu(_) => None,
        }
    }

    fn set_top(&mut self, d: Dialog) {
        if let Screen::Game(g) = &mut self.screen
            && let Some(top) = g.dialogs.last_mut()
        {
            *top = d;
        }
        self.rebuild_ui();
    }

    fn toast_text(&mut self, text: String) {
        self.toast(text);
    }

    /// List selections of the dialogs of this module; true if consumed.
    pub(super) fn build_row_event(&mut self, action: u32, row: usize) -> bool {
        let Some(d) = self.top_build_dialog() else {
            return false;
        };
        match (action, d) {
            (A_TB_LIST, Dialog::TrainBuy { copy, replace, .. }) => {
                self.set_top(Dialog::TrainBuy {
                    sel: row,
                    copy,
                    replace,
                });
            }
            (A_TB_COPY, Dialog::TrainBuy { sel, replace, .. }) => {
                self.set_top(Dialog::TrainBuy {
                    sel,
                    copy: row,
                    replace,
                });
            }
            (A_TE_STOPS, Dialog::TrainEdit { train, pick, .. }) => {
                self.set_top(Dialog::TrainEdit {
                    train,
                    sel: row,
                    pick,
                    confirm: false,
                });
            }
            (A_TE_PICK, Dialog::TrainEdit { train, sel, .. }) => {
                self.set_top(Dialog::TrainEdit {
                    train,
                    sel,
                    pick: row,
                    confirm: false,
                });
            }
            _ => return false,
        }
        true
    }

    /// The router was clicked at a fraction of its map: add a stop at the nearest own station.
    pub(super) fn build_pointer(&mut self, action: u32, fx: f32, fy: f32) -> bool {
        if action != A_TE_MAP {
            return false;
        }
        let Some(Dialog::TrainEdit { train, .. }) = self.top_build_dialog() else {
            return false;
        };
        let Screen::Game(g) = &self.screen else {
            return true;
        };
        let (Some(r), Some(co)) = (&g.radar, self.player()) else {
            return true;
        };
        let (tx, ty) = r.to_tile(fx, fy);
        let Some(save) = g.world.save_ref() else {
            return true;
        };
        let best = bt::company_stations(save, co)
            .into_iter()
            .filter_map(|s| bt::station_tile(save, s).map(|p| (s, p)))
            .map(|(s, p)| {
                let d = (p.0 as f32 - tx).powi(2) + (p.1 as f32 - ty).powi(2);
                (s, d)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let n = bt::train_stops(save, train).len() as i32;
        if let Some((st, _)) = best {
            self.insert_stop(train, n, st);
        }
        true
    }

    fn insert_stop(&mut self, train: i32, k: i32, station: i32) {
        match self.issue(BuildCmd::InsertStop {
            train,
            k,
            station,
            waypoint: false,
            x: 0,
            y: 0,
        }) {
            Ok(_) => self.rebuild_ui(),
            Err(e) => {
                let m = self.cmd_error_text(e, 394);
                self.toast_text(m);
            }
        }
    }

    /// Buttons of the dialogs of this module; true if `action` was one of them.
    pub(super) fn build_dialog_activated(&mut self, action: u32) -> bool {
        let Some(d) = self.top_build_dialog() else {
            return false;
        };
        if action == A_CLOSE {
            self.close_dialog();
            return true;
        }
        match (action, d) {
            (A_SD_BUY, Dialog::Station { station }) => {
                self.set_top(Dialog::StationBuy {
                    station,
                    mask: 0,
                    last: 3,
                });
            }
            (a, Dialog::StationBuy { station, mask, .. })
                if (A_SB_ITEM..A_SB_ITEM + 30).contains(&a) =>
            {
                let row = a - A_SB_ITEM;
                self.set_top(Dialog::StationBuy {
                    station,
                    mask: mask ^ (1 << row),
                    last: row,
                });
            }
            (A_SB_OK, Dialog::StationBuy { station, mask, .. }) => {
                self.buy_improvements(station, mask)
            }
            (A_TB_OK, Dialog::TrainBuy { sel, copy, replace }) => {
                self.buy_train(sel, copy, replace)
            }
            (
                A_TE_ADD | A_TE_INSERT,
                Dialog::TrainEdit {
                    train, sel, pick, ..
                },
            ) => {
                let st = self
                    .with_game(|s, _, co| bt::company_stations(s, co).get(pick).copied())
                    .flatten();
                let n = self
                    .with_game(|s, _, _| bt::train_stops(s, train).len() as i32)
                    .unwrap_or(0);
                if let Some(st) = st {
                    let k = if action == A_TE_ADD { n } else { sel as i32 };
                    self.insert_stop(train, k, st);
                }
            }
            (
                A_TE_DELETE,
                Dialog::TrainEdit {
                    train, sel, pick, ..
                },
            ) => {
                match self.issue(BuildCmd::DeleteStop {
                    train,
                    k: sel as i32,
                }) {
                    Ok(_) => self.set_top(Dialog::TrainEdit {
                        train,
                        sel: sel.saturating_sub(1),
                        pick,
                        confirm: false,
                    }),
                    Err(e) => {
                        let m = self.cmd_error_text(e, 394);
                        self.toast_text(m);
                    }
                }
            }
            (A_TE_REPLACE, Dialog::TrainEdit { train, .. }) => {
                self.push_dialog(Dialog::TrainBuy {
                    sel: 0,
                    copy: 0,
                    replace: Some(train),
                });
            }
            (
                A_TE_SELL,
                Dialog::TrainEdit {
                    train,
                    sel,
                    pick,
                    confirm,
                },
            ) => {
                if !confirm {
                    self.set_top(Dialog::TrainEdit {
                        train,
                        sel,
                        pick,
                        confirm: true,
                    });
                } else {
                    match self.issue(BuildCmd::DeleteTrain { train }) {
                        Ok(_) => {
                            self.close_dialog();
                            let m = self.texts.a("The train was sold (there is no refund).");
                            self.toast_text(m);
                        }
                        Err(e) => {
                            let m = self.cmd_error_text(e, 394);
                            self.toast_text(m);
                        }
                    }
                }
            }
            (A_IN_BUY, Dialog::Industry { building }) => self.trade_industry(building, true),
            (A_IN_SELL, Dialog::Industry { building }) => self.trade_industry(building, false),
            (A_IN_DETAIL, Dialog::Industry { building }) => {
                self.close_dialog();
                self.open_info(info_ui::InfoScreen::IndustryDetail(building as u32));
            }
            (a, Dialog::Rights { territory }) if (A_RT_BUY..A_RT_BUY + 4).contains(&a) => {
                self.buy_rights(territory, (a - A_RT_BUY) as usize);
            }
            _ => return false,
        }
        true
    }

    fn buy_improvements(&mut self, station: i32, mask: u32) {
        let total = self.improvement_total(station, mask);
        let ok = self
            .with_game(|s, _, co| {
                bt::permitted(s, bt::PERM_BUILD_SUPPORT) && bt::can_afford(s, co, f64::from(total))
            })
            .unwrap_or(false);
        if !ok {
            let m = self
                .texts
                .get(394, "Your company doesn't have enough money to build that!");
            self.toast_text(m);
            return;
        }
        match self.issue(BuildCmd::Improve { station, mask }) {
            Ok(_) => {
                self.close_dialog();
                self.rebuild_ui();
            }
            Err(e) => {
                let m = self.cmd_error_text(e, 394);
                self.toast_text(m);
            }
        }
    }

    fn buy_train(&mut self, sel: usize, copy: usize, replace: Option<i32>) {
        let (year, month) = self.date_ym();
        let Some((rows, co, permitted, cash)) = self.with_game(|s, data, co| {
            (
                bt::engine_rows(data, s, co, year, month),
                co,
                bt::permitted(s, bt::PERM_BUY_TRAIN),
                bt::cash(s, co),
            )
        }) else {
            return;
        };
        let Some(row) = rows.get(sel) else {
            return;
        };
        if !permitted {
            let m = self
                .texts
                .get(23, "That action is not allowed in this scenario.");
            self.toast_text(m);
            return;
        }
        if cash <= 0.0 || f64::from(row.price) > cash {
            let m = self.texts.get(
                354,
                "Your company doesn't have enough money to purchase another train!",
            );
            self.toast_text(m);
            return;
        }
        if let Some(train) = replace {
            match self.issue(BuildCmd::ReplaceEngine {
                train,
                engine: row.index,
            }) {
                Ok(_) => self.close_dialog(),
                Err(e) => {
                    let m = self.cmd_error_text(e, 354);
                    self.toast_text(m);
                }
            }
            return;
        }
        let (slots, _) = self.copyable_trains();
        let copy_from = if copy == 0 {
            0
        } else {
            slots.get(copy - 1).copied().unwrap_or(0)
        };
        match self.issue(BuildCmd::BuyTrain {
            company: co,
            station: 0,
            engine: row.index,
            copy_from,
        }) {
            Ok(train) => {
                // A train only leaves once 0x2c marks it ready (commands.md). The original's window
                // sends it after the stops are edited; we mark right away so a train bought with
                // its default stops runs, and later stop edits still apply through their commands.
                if let Err(e) = self.issue(BuildCmd::TrainMark { train }) {
                    let m = self.cmd_error_text(e, 354);
                    self.toast_text(m);
                }
                self.close_dialog();
                self.push_dialog(Dialog::TrainEdit {
                    train,
                    sel: 0,
                    pick: 0,
                    confirm: false,
                });
            }
            Err(e) => {
                let m = self.cmd_error_text(e, 354);
                self.toast_text(m);
            }
        }
    }

    fn trade_industry(&mut self, building: i32, buy: bool) {
        let Some((co, price, cash, rights_ok)) = self.with_game(|s, data, co| {
            let terr = bt::rec(&s.objects, building).map_or(0, |b| i32::from(b.u8(0x9d)));
            (
                co,
                bt::building_price(data, s, co, building),
                bt::cash(s, co),
                bt::rights(s, co, terr) & 2 != 0,
            )
        }) else {
            return;
        };
        if buy && !rights_ok {
            let m = self.texts.get(
                2288,
                "Your company doesn't have access rights to buy buildings in this territory.",
            );
            self.toast_text(m);
            return;
        }
        if buy && f64::from(price) > cash {
            let m = self
                .texts
                .get(2289, "Your company doesn't have enough money to buy this.");
            self.toast_text(m);
            return;
        }
        match self.issue(BuildCmd::TradeIndustry {
            company: co,
            building,
            buy,
        }) {
            Ok(_) => self.rebuild_ui(),
            Err(e) => {
                let m = self.cmd_error_text(e, 2289);
                self.toast_text(m);
            }
        }
    }

    fn buy_rights(&mut self, territory: i32, bit: usize) {
        let Some((co, region, price, mask, cash)) = self.with_game(|s, _, co| {
            let region = bt::rec(&s.territories, territory).map_or(0, |r| i32::from(r.u8(0x2a)));
            let _ = region;
            (
                co,
                territory,
                bt::rights_price(s, co, territory, bit),
                bt::rights_mask(s, territory, bit),
                bt::cash(s, co),
            )
        }) else {
            return;
        };
        if f64::from(price) > cash {
            let m = self
                .texts
                .get(2287, "Those rights will cost you %1, but your company only has a cash balance of %2.")
                .replace("%1", &crate::info::money(f64::from(price)))
                .replace("%2", &crate::info::money(cash));
            self.toast_text(m);
            return;
        }
        match self.issue(BuildCmd::TerritoryRights {
            company: co,
            territory: region,
            mask,
            price,
        }) {
            Ok(_) => self.rebuild_ui(),
            Err(e) => {
                let m = self.cmd_error_text(e, 394);
                self.toast_text(m);
            }
        }
    }

    // ---- entry points -----------------------------------------------------------------------

    /// The Purchase Train window (action icon 5; `0x4d9d20`), with the refusals of
    /// commands.md 3.5.
    pub(super) fn open_train_buy(&mut self) {
        let (year, month) = self.date_ym();
        let Some((n_eng, n_st, permitted, cash)) = self.with_game(|s, data, co| {
            (
                bt::engine_rows(data, s, co, year, month).len(),
                s.stations.ordered().count(),
                bt::permitted(s, bt::PERM_BUY_TRAIN),
                bt::cash(s, co),
            )
        }) else {
            let m = self
                .texts
                .a("Construction needs a company: start one first (Start New Company).");
            self.toast_text(m);
            return;
        };
        let refusal = if !permitted {
            Some(
                self.texts
                    .get(23, "That action is not allowed in this scenario."),
            )
        } else if cash <= 0.0 {
            Some(self.texts.get(
                354,
                "Your company doesn't have enough money to purchase another train!",
            ))
        } else if n_st < 2 {
            Some(self.texts.get(
                352,
                "You can't add a train until you have at least 2 stations",
            ))
        } else if n_eng == 0 {
            Some(
                self.texts
                    .get(355, "There are currently no engines available for purchase"),
            )
        } else {
            None
        };
        match refusal {
            Some(m) => self.toast_text(m),
            None => self.push_dialog(Dialog::TrainBuy {
                sel: 0,
                copy: 0,
                replace: None,
            }),
        }
    }

    /// A click on an industry: its window with Buy / Sell where the rules allow them.
    pub(super) fn open_industry(&mut self, building: i32) {
        self.push_dialog(Dialog::Industry { building });
    }

    /// Screenshots: opens one of these windows for the first suitable object; false if there is
    /// none. Names: `station`, `station-buy`, `train-buy`, `train-edit`, `industry`, `rights`.
    pub fn show_build_screen(&mut self, name: &str) -> bool {
        let Some(co) = self.player() else {
            return false;
        };
        let pick = |s: &rt2_formats::map::Save, name: &str| -> Option<Dialog> {
            Some(match name {
                "station" | "station-buy" => {
                    let station = bt::company_stations(s, co).into_iter().max_by_key(|&st| {
                        bt::rec(&s.stations, st).map_or(0, |r| r.u32(0x49).count_ones())
                    })?;
                    if name == "station" {
                        Dialog::Station { station }
                    } else {
                        Dialog::StationBuy {
                            station,
                            mask: 0,
                            last: 3,
                        }
                    }
                }
                "train-buy" => Dialog::TrainBuy {
                    sel: 0,
                    copy: 1,
                    replace: None,
                },
                "train-edit" => Dialog::TrainEdit {
                    train: *bt::company_trains(s, co).first()?,
                    sel: 0,
                    pick: 0,
                    confirm: false,
                },
                "industry" => Dialog::Industry {
                    building: s
                        .objects
                        .ordered()
                        .find(|o| bt::Rec(&o.data).u8(7) == 9)
                        .or_else(|| {
                            s.objects.ordered().find(|o| {
                                bt::Rec(&o.data).u8(7) > 0 && bt::Rec(&o.data).u8(7) != 0x12
                            })
                        })?
                        .slot as i32,
                },
                "rights" => Dialog::Rights {
                    territory: s.territories.ordered().next()?.slot as i32,
                },
                _ => return None,
            })
        };
        let d = match &self.screen {
            Screen::Game(g) => g.world.save_ref().and_then(|s| pick(s, name)),
            Screen::Menu(_) => None,
        };
        match d {
            Some(d) => {
                self.push_dialog(d);
                true
            }
            None => false,
        }
    }

    /// Screenshots: selects a tool and puts the cursor on tiles (`from` = the start of a track
    /// drag): the preview over the map as the player would see it.
    pub fn show_tool(
        &mut self,
        name: &str,
        from: Option<(i32, i32)>,
        hover: Option<(i32, i32)>,
    ) -> bool {
        use super::build_ui::ToolKind;
        let (name, size) = name.split_once(':').unwrap_or((name, "small"));
        let kind = match name {
            "track" => ToolKind::Track,
            "station" => ToolKind::Station,
            "bulldoze" => ToolKind::Bulldoze,
            _ => return false,
        };
        self.select_tool(kind);
        if !self.tool_active() {
            return false;
        }
        if let Screen::Game(g) = &mut self.screen {
            g.build.from = from;
            g.build.hover = hover;
            g.build.size = match size {
                "medium" => bt::StationSize::Medium,
                "large" => bt::StationSize::Large,
                _ => bt::StationSize::Small,
            };
        }
        self.recompute_plan();
        self.rebuild_ui();
        true
    }

    /// Screenshots and tests: sends the command of the current plan, as a click would.
    pub fn apply_tool(&mut self) {
        self.recompute_plan();
        self.tool_commit();
        if let Screen::Game(g) = &mut self.screen {
            g.build.from = None;
        }
        self.rebuild_ui();
    }
}

/// Texts of the refusal for a failed [`BuildCmd`] in the test-only error path.
#[allow(dead_code)]
fn _unused(_: CmdError, _: &Table) {}
