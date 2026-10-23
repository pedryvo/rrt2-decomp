//! The windows that follow a construction: the station detail (`statndtl.win`) with its Buy
//! dialog (`buildbuy.win`), Purchase Train (`trainbuy.win`), the editable train detail
//! (`traindtl.win`: map router, route list), the industry window with Buy and Sell, and the
//! territory rights window (docs/reference/screens.md E1..E4, F1, F2, C3, D4, G5).
//!
//! Every action is a command of `rt2_sim::build::cmd::BuildCmd` (`0x17`..`0x20`, `0x2a`, `0x2d`)
//! sent with `World::apply_build`; what the original's windows check before sending is checked
//! here (permissions `0x5706b4`..`0x5706b9`, cash, availability: docs/spec/commands.md 3.3 to
//! 3.7). The picture of a station is composed from the original's overlay art
//! (`stationview.rs`, docs/formats/sprite-catalog.md 6.1). Gaps (marked TODO): the stop commands `0x1a`, `0x1b`, `0x1c`, `0x21` and `0x2f`
//! (next stop, wait mode, cargo per stop, class, sell mode) are not in `rt2-sim` yet, so the
//! route list shows the wait mode and cargo but cannot change them.

use rt2_sim::build::cmd::{BuildCmd, CmdError};
use rt2_ui::{Align, Anchor, Color, Fill, Fit, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::dialogs::Dialog;
use super::info_ui;
use super::{AppState, Screen};
use crate::buildtools as bt;
use crate::info::{Block, Table};

pub(super) const A_SD_BUY: u32 = 5100;
pub(super) const A_SB_ITEM: u32 = 5110;
pub(super) const A_SB_OK: u32 = 5140;
pub(super) const A_TB_OK: u32 = 5152;
pub(super) const A_TE_STOPS: u32 = 5160;
pub(super) const A_TE_PICK: u32 = 5161;
pub(super) const A_TE_ADD: u32 = 5162;
pub(super) const A_TE_INSERT: u32 = 5163;
pub(super) const A_TE_DELETE: u32 = 5164;
pub(super) const A_TE_REPLACE: u32 = 5165;
pub(super) const A_TE_SELL: u32 = 5166;
pub(super) const A_TE_MAP: u32 = 5167;
pub(super) const A_RT_BUY: u32 = 5180;
pub(super) const A_CLOSE: u32 = 5199;

pub(super) const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
pub(super) const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
pub(super) const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
pub(super) const BAD: Color = Color::rgb(0xf0, 0x8c, 0x7a);
const GOOD: Color = Color::rgb(0x9a, 0xe0, 0x8a);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd8);

pub(super) fn fnt(n: u8, dz: f32) -> FontStyle {
    let f = game_font(n);
    f.with_size((f.size * dz).round())
}

pub(super) fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

pub(super) fn lbl(text: &str, f: FontStyle, color: Color, align: Align) -> Kind {
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
    pub(super) fn with_game<R>(
        &self,
        f: impl FnOnce(&rt2_formats::map::Save, &rt2_sim::data::GameData, i32) -> R,
    ) -> Option<R> {
        let co = self.player()?;
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        Some(f(g.world.save_ref()?, self.data.as_ref()?, co))
    }

    pub(super) fn engine_name(&self, data: &rt2_sim::data::GameData, e: i32) -> String {
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
            } => self.build_station_buy_art(ui, *station, *mask, *last),
            Dialog::TrainBuy {
                sel,
                scroll,
                tons,
                replace,
            } => self.build_train_buy(ui, *sel, *scroll, *tons, *replace),
            Dialog::TrainEdit {
                train,
                sel,
                pick,
                confirm,
            } => self.build_train_edit(ui, *train, *sel, *pick, *confirm),
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
        // The picture keeps the art's 1024x569 aspect.
        let pic_h = (lw * crate::stationview::VIEW_H as f32 / crate::stationview::VIEW_W as f32)
            .min(body_h * 0.7);
        let pic = ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(pad, top, lw, pic_h),
        );
        // The picture: backdrop and overlays composed by `stationview.rs` into the reserved
        // sprite (docs/formats/sprite-catalog.md 6.1).
        let (style, site, mask) = match &self.screen {
            Screen::Game(g) => g
                .world
                .save_ref()
                .and_then(|s| {
                    let st = bt::rec(&s.stations, station)?;
                    let b = bt::rec(&s.objects, st.i32(4))?;
                    let (double, electrified) = bt::station_track_flags(s, station);
                    Some((
                        b.u8(0x52),
                        st.i32(0x4d),
                        crate::stationview::derive_mask(st.u32(0x49), double, electrified),
                    ))
                })
                .unwrap_or_default(),
            Screen::Menu(_) => Default::default(),
        };
        if let Some(sprite) = self.dialog_art.station_view {
            let key = (style, site, mask);
            if *self.station_view_key.borrow() != Some(key)
                && let Some(rgba) = crate::stationview::compose(&self.assets, site, style, mask)
            {
                self.sprite_uploads.borrow_mut().push((sprite, rgba));
                *self.station_view_key.borrow_mut() = Some(key);
            }
            ui.add(
                pic,
                Kind::Image {
                    sprite,
                    fit: Fit::Contain,
                },
                Layout::fill(),
            );
        }
        // Facts under the picture.
        let left_facts: Vec<Block> = page.left.iter().take(1).cloned().collect();
        self.blocks(ui, root, &left_facts, pad, top + pic_h + 4.0 * dz, lw, dz);
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
    pub(super) fn improvement_total(&self, station: i32, mask: u32) -> i32 {
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
            at(cx, top + 14.0 * dz, cw, body_h * 0.5),
        );
        // The instrument panel and the stop controls (`train_panel.rs`).
        self.draw_train_panel(
            ui,
            root,
            cx,
            top + 18.0 * dz + body_h * 0.5,
            cw,
            dz,
            train,
            sel,
        );
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

    pub(super) fn top_build_dialog(&self) -> Option<Dialog> {
        match &self.screen {
            Screen::Game(g) => g.dialogs.last().cloned(),
            Screen::Menu(_) => None,
        }
    }

    pub(super) fn set_top(&mut self, d: Dialog) {
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
                    last: u32::MAX,
                });
            }
            (
                a,
                Dialog::StationBuy {
                    station,
                    mask,
                    last,
                },
            ) if (A_SB_ITEM..A_SB_ITEM + 30).contains(&a) => {
                let row = a - A_SB_ITEM;
                let mut mask = mask ^ (1 << row);
                // `0x407540`: checking one of a pair clears the other (small/medium station,
                // telephone/telegraph, small/large restaurant, large/small hotel).
                if mask & (1 << row) != 0
                    && let Some(&(_, other)) = [
                        (1, 2),
                        (2, 1),
                        (3, 4),
                        (4, 3),
                        (5, 6),
                        (6, 5),
                        (15, 16),
                        (16, 15),
                    ]
                    .iter()
                    .find(|p| p.0 == row)
                {
                    mask &= !(1 << other);
                }
                self.set_top(Dialog::StationBuy {
                    station,
                    mask,
                    last,
                });
            }
            (a, Dialog::StationBuy { station, mask, .. })
                if (super::buy_ui::A_SB_PICK..super::buy_ui::A_SB_PICK + 30).contains(&a) =>
            {
                self.set_top(Dialog::StationBuy {
                    station,
                    mask,
                    last: a - super::buy_ui::A_SB_PICK,
                });
            }
            (
                A_SB_OK,
                Dialog::StationBuy {
                    station,
                    mask,
                    last,
                },
            ) => {
                // `0x407540`: the row that is selected is bought with the checked ones.
                let mask = if last < 30 { mask | (1 << last) } else { mask };
                if mask == 0 {
                    self.close_dialog();
                } else {
                    self.buy_improvements(station, mask);
                }
            }
            (A_TB_OK, Dialog::TrainBuy { sel, replace, .. }) => self.buy_train(sel, replace),
            (a, Dialog::TrainBuy { .. }) if self.train_buy_action(a) => {}
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
                self.open_train_buy_dialog(Some(train));
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
            // `0x407540`: LNG 0x159.
            let m = self
                .texts
                .get(345, "Your company doesn't have enough money to build that!");
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

    fn buy_train(&mut self, sel: usize, replace: Option<i32>) {
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
        // `0x4d9a50`: the purchase is refused with LNG 0x161 when the price exceeds the cash.
        if cash <= 0.0 || f64::from(row.price) > cash {
            let m = self
                .texts
                .get(353, "Your company doesn't have enough money to build that!");
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
        match self.issue(BuildCmd::BuyTrain {
            company: co,
            station: 0,
            engine: row.index,
            copy_from: 0,
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

    pub(super) fn trade_industry(&mut self, building: i32, buy: bool) {
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
            None => self.open_train_buy_dialog(None),
        }
    }

    /// A click on an industry: the information box of `eagled.win` (C3), with Buy where the
    /// rules allow it (`bldinfo.rs`).
    pub(super) fn open_industry(&mut self, building: i32) {
        self.bldinfo_industry(building, true);
    }

    /// Screenshots: opens one of these windows for the first suitable object; false if there is
    /// none. Names: `station`, `station-buy`, `train-buy`, `train-edit`, `industry`, `rights`.
    pub fn show_build_screen(&mut self, name: &str) -> bool {
        let Some(co) = self.player() else {
            return false;
        };
        let year = self.date_ym().0;
        let pick = |s: &rt2_formats::map::Save, name: &str| -> Option<Dialog> {
            Some(match name {
                "station" | "station-buy" | "station-buy-sel" => {
                    let station = bt::company_stations(s, co).into_iter().max_by_key(|&st| {
                        bt::rec(&s.stations, st).map_or(0, |r| r.u32(0x49).count_ones())
                    })?;
                    if name == "station" {
                        Dialog::Station { station }
                    } else {
                        Dialog::StationBuy {
                            station,
                            mask: 0,
                            last: u32::MAX,
                        }
                    }
                }
                "train-buy" => Dialog::TrainBuy {
                    sel: 0,
                    scroll: 0,
                    tons: super::buy_ui::default_tons(year),
                    replace: None,
                },
                "train-edit" => Dialog::TrainEdit {
                    train: *bt::company_trains(s, co).first()?,
                    sel: 0,
                    pick: 0,
                    confirm: false,
                },
                "car-list" => {
                    let train = *bt::company_trains(s, co).first()?;
                    Dialog::CarList {
                        train,
                        stop: 0,
                        cargo: bt::train_stops(s, train).first()?.cargo.clone(),
                    }
                }
                "brand" => {
                    let r = bt::rec(&s.companies, co)?;
                    Dialog::Brand {
                        company: co,
                        name: (4..0x23)
                            .map(|i| r.u8(i))
                            .take_while(|&b| b != 0)
                            .map(|b| b as char)
                            .collect(),
                        logo: r.u8(0x2b).min(40),
                        error: None,
                    }
                }
                "rights" => Dialog::Rights {
                    territory: s.territories.ordered().next()?.slot as i32,
                },
                _ => return None,
            })
        };
        if name == "industry" {
            let building = match &self.screen {
                Screen::Game(g) => g.world.save_ref().and_then(|s| {
                    s.objects
                        .ordered()
                        .find(|o| bt::Rec(&o.data).u8(7) == 9)
                        .or_else(|| {
                            s.objects.ordered().find(|o| {
                                bt::Rec(&o.data).u8(7) > 0 && bt::Rec(&o.data).u8(7) != 0x12
                            })
                        })
                        .map(|o| o.slot as i32)
                }),
                Screen::Menu(_) => None,
            };
            return match building {
                Some(b) => {
                    self.open_industry(b);
                    true
                }
                None => false,
            };
        }
        let d = match &self.screen {
            Screen::Game(g) => g.world.save_ref().and_then(|s| pick(s, name)),
            Screen::Menu(_) => None,
        };
        match d {
            Some(d) => {
                self.push_dialog(d);
                if name == "station-buy-sel" {
                    self.select_improvements_for_shot();
                }
                true
            }
            None => false,
        }
    }

    /// Screenshots: the Buy dialog with the second listed row checked and the first selected.
    fn select_improvements_for_shot(&mut self) {
        let Some(Dialog::StationBuy { station, .. }) = self.top_build_dialog() else {
            return;
        };
        let (year, month) = self.date_ym();
        let rows: Vec<u32> = self
            .with_game(|s, data, _| bt::improvements(data, s, station, year, month))
            .unwrap_or_default()
            .iter()
            .filter(|i| i.available && !i.owned && !i.excluded)
            .map(|i| i.row)
            .collect();
        if let [a, b, ..] = rows[..] {
            self.set_top(Dialog::StationBuy {
                station,
                mask: 1 << b,
                last: a,
            });
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
