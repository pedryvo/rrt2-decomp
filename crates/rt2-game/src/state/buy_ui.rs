//! Purchase Train (`trainbuy.win`, docs/reference/screens.md E1; `0x4d9d20` window, `0x4e3f90`
//! the right panel, `0x4d9bd0` a list row, `0x4d9a50` the buttons, docs/ui.md "Janelas de
//! compra"). The frame is the window's own art (`artwork/buywin.rs`), the picture of the train
//! is composed by `trainview.rs`, and every position below is a design coordinate of the 774x578
//! window scaled by one factor, so the window fits any canvas.

use rt2_render::atlas::scale2x;
use rt2_ui::{Align, Anchor, Color, Fill, Fit, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::AppState;
use super::build_dialogs::{A_CLOSE, A_SB_ITEM, A_SB_OK, A_TB_OK, HEAD, MUTED, TEXT};
use crate::artwork::buywin::{BUY_PIC, BUY_ROW_H, LIST, PIC, ROW_H, SCROLL_X, WIN_H, WIN_W};
use crate::buildtools as bt;

pub(super) const A_SB_PICK: u32 = 5400;
pub(super) const A_TB_ROW: u32 = 5300;
pub(super) const A_TB_TONS_UP: u32 = 5290;
pub(super) const A_TB_TONS_DOWN: u32 = 5291;
pub(super) const A_TB_SCROLL_UP: u32 = 5292;
pub(super) const A_TB_SCROLL_DOWN: u32 = 5293;
pub(super) const A_TB_PAGE_UP: u32 = 5294;
pub(super) const A_TB_PAGE_DOWN: u32 = 5295;

/// Rows of the list that are visible at once (450 / 54).
pub(super) const VISIBLE_ROWS: usize = 8;

/// The weight per car at which the window opens (`0x4e57c0`, by the date: before 1830 10 tons,
/// 1850 20, 1880 30, 1910 40, then 50).
pub(crate) fn default_tons(year: i32) -> i32 {
    match year {
        ..1830 => 10,
        1830..1850 => 20,
        1850..1880 => 30,
        1880..1910 => 40,
        _ => 50,
    }
}

/// Text of the plates of the original: dark lettering on the cream metal.
const PLATE_INK: Color = Color::rgb(0x2a, 0x1e, 0x10);

/// Design -> logical scale and origin of a window placed in the middle of the canvas.
struct Frame {
    root: NodeId,
    k: f32,
}

impl Frame {
    fn at(&self, x: f32, y: f32, w: f32, h: f32) -> Layout {
        Layout::at(
            Anchor::TopLeft,
            (w * self.k).max(1.0),
            (h * self.k).max(1.0),
        )
        .offset((x * self.k).round(), (y * self.k).round())
    }

    fn font(&self, n: u8, px: f32) -> FontStyle {
        game_font(n).with_size((px * self.k).round().max(8.0))
    }
}

impl AppState {
    /// The first visible row that keeps `sel` in view.
    pub(super) fn keep_in_view(scroll: usize, sel: usize) -> usize {
        if sel < scroll {
            sel
        } else if sel >= scroll + VISIBLE_ROWS {
            sel + 1 - VISIBLE_ROWS
        } else {
            scroll
        }
    }

    pub(super) fn open_train_buy_dialog(&mut self, replace: Option<i32>) {
        let tons = default_tons(self.date_ym().0);
        self.push_dialog(super::dialogs::Dialog::TrainBuy {
            sel: 0,
            scroll: 0,
            tons,
            replace,
        });
    }

    fn train_buy_rows(&self) -> usize {
        let (year, month) = self.date_ym();
        self.with_game(|s, data, co| bt::engine_rows(data, s, co, year, month).len())
            .unwrap_or(0)
    }

    /// Clicks of the Purchase Train window other than the two buttons; true if consumed.
    pub(super) fn train_buy_action(&mut self, action: u32) -> bool {
        let Some(super::dialogs::Dialog::TrainBuy {
            sel,
            scroll,
            tons,
            replace,
        }) = self.top_build_dialog()
        else {
            return false;
        };
        let n = self.train_buy_rows();
        let max_scroll = n.saturating_sub(VISIBLE_ROWS);
        let (mut sel, mut scroll, mut tons) = (sel, scroll, tons);
        match action {
            a if (A_TB_ROW..A_TB_ROW + 200).contains(&a) => {
                sel = ((a - A_TB_ROW) as usize).min(n.saturating_sub(1));
            }
            // `0x4d9a50`: the arrows add or take 5 tons, between 5 and 100.
            A_TB_TONS_UP => tons = (tons + 5).min(100),
            A_TB_TONS_DOWN => tons = (tons - 5).max(5),
            A_TB_SCROLL_UP => scroll = scroll.saturating_sub(1),
            A_TB_SCROLL_DOWN => scroll = (scroll + 1).min(max_scroll),
            A_TB_PAGE_UP => scroll = scroll.saturating_sub(VISIBLE_ROWS),
            A_TB_PAGE_DOWN => scroll = (scroll + VISIBLE_ROWS).min(max_scroll),
            _ => return false,
        }
        let scroll = if (A_TB_ROW..A_TB_ROW + 200).contains(&action) {
            Self::keep_in_view(scroll, sel)
        } else {
            scroll.min(max_scroll)
        };
        self.set_top(super::dialogs::Dialog::TrainBuy {
            sel,
            scroll,
            tons,
            replace,
        });
        true
    }

    /// Mouse wheel over the Purchase Train window scrolls the list; true if consumed.
    pub(super) fn train_buy_wheel(&mut self, notches: f32) -> bool {
        if !matches!(
            self.top_build_dialog(),
            Some(super::dialogs::Dialog::TrainBuy { .. })
        ) {
            return false;
        }
        let a = if notches > 0.0 {
            A_TB_SCROLL_UP
        } else {
            A_TB_SCROLL_DOWN
        };
        for _ in 0..notches.abs().round().max(1.0) as u32 {
            self.train_buy_action(a);
        }
        true
    }

    pub(super) fn build_train_buy(
        &self,
        ui: &mut Ui,
        sel: usize,
        scroll: usize,
        tons: i32,
        replace: Option<i32>,
    ) {
        let (cw, ch) = ui.logical_size();
        let k = ((cw - 16.0) / WIN_W).min((ch - 16.0) / WIN_H).max(0.2);
        let art = &self.dialog_art.buy;
        let root = ui.add_root(
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Center, (WIN_W * k).round(), (WIN_H * k).round()),
        );
        let f = Frame { root, k };
        let t = &self.texts;
        match art.frame {
            Some(sprite) => {
                ui.add(
                    root,
                    Kind::Image {
                        sprite,
                        fit: Fit::Stretch,
                    },
                    Layout::fill(),
                );
            }
            None => {
                ui.add(root, Kind::Panel { fill: Fill::Panel }, Layout::fill());
            }
        }
        let (year, month) = self.date_ym();
        let rows = self
            .with_game(|s, data, co| bt::engine_rows(data, s, co, year, month))
            .unwrap_or_default();
        let sel = sel.min(rows.len().saturating_sub(1));
        let scroll = scroll.min(rows.len().saturating_sub(VISIBLE_ROWS));
        if rows.is_empty() {
            ui.add(
                root,
                Kind::Label {
                    text: t.get(355, "There are currently no engines available for purchase"),
                    font: f.font(1, 12.0),
                    color: TEXT,
                    align: Align::Center,
                    wrap: true,
                },
                f.at(LIST.0 + 6.0, LIST.1 + 6.0, LIST.2 - 12.0, 80.0),
            );
        }
        // ---- the list: side view over the name, one row of 54 px each ------------------------
        for (i, r) in rows.iter().enumerate().skip(scroll).take(VISIBLE_ROWS) {
            let y = LIST.1 + (i - scroll) as f32 * ROW_H;
            let name = self
                .with_game(|_, data, _| self.engine_name(data, r.index))
                .unwrap_or_default();
            let rec = self
                .with_game(|_, data, _| {
                    data.image
                        .u16(0x0053_ddb8 + u32::from(bt_vehicle(data, r.index)) * 0x39 + 0x27)
                })
                .unwrap_or(u16::MAX);
            if i == sel {
                ui.add(
                    root,
                    Kind::Panel {
                        fill: Fill::Solid(Color(0, 0, 0, 0x6e)),
                    },
                    f.at(LIST.0, y, LIST.2, ROW_H - 1.0),
                );
            }
            if let Some(Some(p)) = art.side.get(usize::from(rec)) {
                // `0x4d9bd0` draws the sprite at (row x + 7, row y - 39) plus its own offset.
                ui.add(
                    root,
                    Kind::Image {
                        sprite: p.sprite,
                        fit: Fit::Stretch,
                    },
                    f.at(LIST.0 + 7.0 + p.x as f32, y - 39.0 + p.y as f32, p.w, p.h),
                );
            }
            ui.add(
                root,
                Kind::Label {
                    text: name,
                    font: f.font(1, 11.5),
                    color: TEXT,
                    align: Align::Left,
                    wrap: false,
                },
                f.at(LIST.0 + 10.0, y + 38.0, LIST.2 - 12.0, 15.0),
            );
            ui.add(
                root,
                Kind::Hotspot {
                    action: A_TB_ROW + i as u32,
                    on: false,
                    enabled: true,
                },
                f.at(LIST.0, y, LIST.2, ROW_H - 1.0),
            );
        }
        // ---- the scroll bar (`trainblb.imb`) -------------------------------------------------
        if let Some(sc) = art.scroll {
            let (up, down, track, thumb) = (sc[0], sc[1], sc[2], sc[3]);
            let place = |ui: &mut Ui, p: &crate::artwork::buywin::Placed, x: f32, y: f32| {
                ui.add(
                    root,
                    Kind::Image {
                        sprite: p.sprite,
                        fit: Fit::Stretch,
                    },
                    f.at(x, y, p.w, p.h),
                );
            };
            place(ui, &track, SCROLL_X, LIST.1 + up.h);
            place(ui, &up, SCROLL_X, LIST.1);
            let down_y = LIST.1 + 450.0 - down.h;
            place(ui, &down, SCROLL_X, down_y);
            let span = (down_y - LIST.1 - up.h - thumb.h).max(1.0);
            let max_scroll = rows.len().saturating_sub(VISIBLE_ROWS);
            let frac = if max_scroll == 0 {
                0.0
            } else {
                scroll as f32 / max_scroll as f32
            };
            let thumb_y = LIST.1 + up.h + span * frac;
            place(ui, &thumb, SCROLL_X + 6.0, thumb_y);
            let hot = |ui: &mut Ui, action: u32, x: f32, y: f32, w: f32, h: f32| {
                ui.add(
                    root,
                    Kind::Hotspot {
                        action,
                        on: false,
                        enabled: rows.len() > VISIBLE_ROWS,
                    },
                    f.at(x, y, w, h),
                );
            };
            hot(ui, A_TB_SCROLL_UP, SCROLL_X, LIST.1, up.w, up.h);
            hot(ui, A_TB_SCROLL_DOWN, SCROLL_X, down_y, down.w, down.h);
            let (t0, t1) = (LIST.1 + up.h, down_y);
            hot(
                ui,
                A_TB_PAGE_UP,
                SCROLL_X,
                t0,
                track.w,
                (thumb_y - t0).max(0.0),
            );
            hot(
                ui,
                A_TB_PAGE_DOWN,
                SCROLL_X,
                thumb_y + thumb.h,
                track.w,
                (t1 - thumb_y - thumb.h).max(0.0),
            );
        }
        // ---- the right panel -----------------------------------------------------------------
        let detail = self
            .with_game(|s, data, co| {
                rows.get(sel)
                    .and_then(|r| bt::engine_detail(data, s, co, r.index, tons))
            })
            .flatten();
        if let Some(d) = &detail {
            self.paint_train_picture(ui, &f, d, rows[sel].index);
            self.paint_train_facts(ui, &f, d, &rows[sel], tons, sel);
        }
        // ---- buttons -------------------------------------------------------------------------
        let labels = [
            if replace.is_some() {
                t.get(429, "Replace")
            } else {
                t.get(1912, "Purchase")
            },
            t.get(crate::text::id::CANCEL, "Cancel"),
        ];
        let plates = [(336.0, 472.0, 18.0, A_TB_OK), (534.0, 472.0, 15.0, A_CLOSE)];
        for (i, (x, y, lx, action)) in plates.into_iter().enumerate() {
            let sprites = art.buttons.map(|b| b[i]);
            ui.add(
                root,
                Kind::Icon {
                    up: sprites.map(|s| s.0),
                    down: sprites.map(|s| s.1),
                    fallback: labels[i].clone(),
                    font: f.font(5, 14.0),
                    action,
                    on: false,
                },
                f.at(x, y, 138.0, 63.0),
            );
            if sprites.is_some() {
                ui.add(
                    root,
                    Kind::Label {
                        text: labels[i].clone(),
                        font: f.font(5, 14.0),
                        color: PLATE_INK,
                        align: Align::Center,
                        wrap: false,
                    },
                    f.at(x + lx, y + 23.0, 108.0, 26.0),
                );
            }
        }
    }

    /// The picture box: terrain, locomotive and tender (`trainview.rs`) and the first year.
    fn paint_train_picture(&self, ui: &mut Ui, f: &Frame, d: &bt::EngineDetail, engine: i32) {
        let art = &self.dialog_art.buy;
        if let Some(sprite) = art.preview {
            let key = engine;
            if *self.buy_view_key.borrow() != Some(key) {
                let names = self.with_game(|_, data, _| {
                    let name = |v: u8| {
                        (0..5u32)
                            .map(|k| data.image.u8(0x0053_ddb8 + u32::from(v) * 0x39 + k))
                            .take_while(|&b| b != 0)
                            .map(|b| b as char)
                            .collect::<String>()
                    };
                    (name(d.vehicle), d.tender.map(name))
                });
                if let Some((en, te)) = names
                    && let Some(img) =
                        crate::trainview::compose(&self.assets, &en, te.as_deref(), d.lengths)
                {
                    self.sprite_uploads
                        .borrow_mut()
                        .push((sprite, scale2x(&img).rgba));
                    *self.buy_view_key.borrow_mut() = Some(key);
                }
            }
            ui.add(
                f.root,
                Kind::Image {
                    sprite,
                    fit: Fit::Stretch,
                },
                f.at(PIC.0, PIC.1, PIC.2, PIC.3),
            );
        }
        // The first year of the engine, at the foot of the picture (`0x4e3f90`, last lines).
        let cx = PIC.0 + PIC.2 / 2.0;
        ui.add(
            f.root,
            Kind::Panel {
                fill: Fill::Solid(Color(0x10, 0x10, 0x08, 0x90)),
            },
            f.at(cx - 20.0, PIC.1 + PIC.3 - 17.0, 40.0, 13.0),
        );
        ui.add(
            f.root,
            Kind::Label {
                text: d.first_year.to_string(),
                font: f.font(1, 9.5),
                color: TEXT,
                align: Align::Center,
                wrap: false,
            },
            f.at(cx - 20.0, PIC.1 + PIC.3 - 17.0, 40.0, 13.0),
        );
    }

    /// The facts and the speed matrix (`0x4e3f90`, docs/ui.md "Janelas de compra").
    fn paint_train_facts(
        &self,
        ui: &mut Ui,
        f: &Frame,
        d: &bt::EngineDetail,
        row: &bt::EngineRow,
        tons: i32,
        _sel: usize,
    ) {
        let t = &self.texts;
        let root = f.root;
        let money = |v: f64| crate::info::money(v);
        let per_year = |v: f64| t.get(209, "%1 / year").replace("%1", &money(v));
        let name = self
            .with_game(|_, data, _| self.engine_name(data, row.index))
            .unwrap_or_default();
        let (lx, rw) = (369.0, 280.0);
        let label = |ui: &mut Ui, x: f32, y: f32, w: f32, text: String, a: Align, size: f32| {
            ui.add(
                root,
                Kind::Label {
                    text,
                    font: f.font(1, size),
                    color: TEXT,
                    align: a,
                    wrap: false,
                },
                f.at(x, y, w, 14.0),
            );
        };
        // The name, then Cost, Maintenance, Fuel, Acceleration and Reliability: label left,
        // value right (`{[HJL]%s:}{[HJR]%s}`).
        ui.add(
            root,
            Kind::Label {
                text: name,
                font: f.font(2, 13.0),
                color: HEAD,
                align: Align::Center,
                wrap: false,
            },
            f.at(PIC.0, 249.0, PIC.2, 16.0),
        );
        let kinds = [
            t.get(2502, "Steam"),
            t.get(2503, "Diesel"),
            t.get(2504, "Electric"),
        ];
        let fuel_label = format!(
            "{} ({})",
            t.get(157, "Fuel"),
            kinds[usize::from(d.kind).min(2)]
        );
        let accel = t.get(1251 + d.accel_class as u32, "");
        let reliab = t.get(2475 + d.reliability as u32, "");
        let lines = [
            (t.get(154, "Cost"), money(f64::from(d.price))),
            (t.get(155, "Maintenance"), per_year(d.maintenance_year)),
            (fuel_label, per_year(d.fuel_year)),
            (t.get(156, "Acceleration"), accel),
            (t.get(2340, "Reliability"), reliab),
        ];
        for (i, (l, v)) in lines.into_iter().enumerate() {
            let y = 264.0 + 13.2 * i as f32;
            label(ui, lx, y, rw * 0.6, format!("{l}:"), Align::Left, 10.5);
            label(ui, lx, y, rw, v, Align::Right, 10.5);
        }
        // Top speed (mph) and the tons per car with its arrows.
        let cx = PIC.0;
        label(
            ui,
            cx,
            344.0,
            PIC.2,
            t.get(441, "Top speed (mph)"),
            Align::Center,
            10.5,
        );
        label(
            ui,
            cx,
            366.0,
            PIC.2,
            t.get(2747, "Cars @ %1 tons each")
                .replace("%1", &tons.to_string()),
            Align::Center,
            10.5,
        );
        if let Some(a) = self.dialog_art.buy.small_arrows {
            for (i, (sp, action, y)) in [(a[0], A_TB_TONS_UP, 358.0), (a[2], A_TB_TONS_DOWN, 369.0)]
                .into_iter()
                .enumerate()
            {
                let _ = i;
                ui.add(
                    root,
                    Kind::Image {
                        sprite: sp,
                        fit: Fit::Stretch,
                    },
                    f.at(616.0, y, 17.0, 9.0),
                );
                ui.add(
                    root,
                    Kind::Hotspot {
                        action,
                        on: false,
                        enabled: true,
                    },
                    f.at(614.0, y - 1.0, 21.0, 11.0),
                );
            }
        }
        // The matrix: one column per car count, one row per grade (0, 2, 4, 6 percent).
        let col = |c: usize| 428.0 + 39.8 * c as f32;
        label(
            ui,
            362.0,
            381.0,
            40.0,
            t.get(443, "Grade"),
            Align::Left,
            8.5,
        );
        for c in 0..6 {
            label(
                ui,
                col(c) - 19.0,
                381.0,
                38.0,
                (c + 1).to_string(),
                Align::Center,
                10.5,
            );
        }
        for g in 0..4 {
            let y = 381.0 + 13.75 * (g + 1) as f32;
            label(ui, 380.0, y, 36.0, format!("{}%", g * 2), Align::Left, 10.5);
            for c in 0..6 {
                label(
                    ui,
                    col(c) - 19.0,
                    y,
                    38.0,
                    d.speeds[g][c].to_string(),
                    Align::Center,
                    10.5,
                );
            }
        }
    }
}

/// The vehicle type of the locomotive of engine `e` (`engine +0x2c`).
fn bt_vehicle(data: &rt2_sim::data::GameData, e: i32) -> u8 {
    data.image.u8(0x0053_cec0 + 0x3a * e as u32 + 0x2c)
}

impl AppState {
    /// The Buy dialog of a station (`buildbuy.win`, F2; `0x407e80`, `0x407540`, `0x407910`): the
    /// improvements that can be bought now, each with a check box; clicking a name shows its
    /// picture (`bm<name>.imb`), its text and its price; Purchase buys the checked rows and the
    /// selected one.
    pub(super) fn build_station_buy_art(&self, ui: &mut Ui, station: i32, mask: u32, last: u32) {
        let (cw, ch) = ui.logical_size();
        let k = ((cw - 16.0) / WIN_W).min((ch - 16.0) / WIN_H).max(0.2);
        let art = &self.dialog_art.buy;
        let root = ui.add_root(
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Center, (WIN_W * k).round(), (WIN_H * k).round()),
        );
        let f = Frame { root, k };
        let t = &self.texts;
        match art.frame_build {
            Some(sprite) => {
                ui.add(
                    root,
                    Kind::Image {
                        sprite,
                        fit: Fit::Stretch,
                    },
                    Layout::fill(),
                );
            }
            None => {
                ui.add(root, Kind::Panel { fill: Fill::Panel }, Layout::fill());
            }
        }
        let (year, month) = self.date_ym();
        let items = self
            .with_game(|s, data, _| bt::improvements(data, s, station, year, month))
            .unwrap_or_default();
        // `0x407e80`: only the rows that are available and not yet bought or excluded are listed.
        let rows: Vec<&bt::Improvement> = items
            .iter()
            .filter(|i| i.available && !i.owned && !i.excluded)
            .collect();
        let cash = self
            .with_game(|s, _, co| bt::cash(s, co))
            .unwrap_or_default();
        // ---- the list ------------------------------------------------------------------------
        for (n, it) in rows.iter().enumerate().take(20) {
            let y = LIST.1 + n as f32 * BUY_ROW_H;
            if it.row == last {
                ui.add(
                    root,
                    Kind::Panel {
                        fill: Fill::Solid(Color(0, 0, 0, 0x6e)),
                    },
                    f.at(LIST.0, y, LIST.2, BUY_ROW_H),
                );
            }
            let checked = mask & (1 << it.row) != 0;
            if let Some((off, on)) = art.checks {
                ui.add(
                    root,
                    Kind::Image {
                        sprite: if checked { on } else { off },
                        fit: Fit::Stretch,
                    },
                    f.at(LIST.0 + 3.0, y + 3.5, 15.0, 15.0),
                );
            } else {
                ui.add(
                    root,
                    Kind::Label {
                        text: if checked { "[x]" } else { "[ ]" }.to_owned(),
                        font: f.font(1, 10.0),
                        color: TEXT,
                        align: Align::Left,
                        wrap: false,
                    },
                    f.at(LIST.0 + 2.0, y + 4.0, 20.0, 14.0),
                );
            }
            ui.add(
                root,
                Kind::Label {
                    text: t.get(1043 + it.row, &format!("Improvement {}", it.row)),
                    font: f.font(1, 11.0),
                    color: TEXT,
                    align: Align::Left,
                    wrap: false,
                },
                f.at(LIST.0 + 22.0, y + 4.0, LIST.2 - 26.0, 14.0),
            );
            ui.add(
                root,
                Kind::Hotspot {
                    action: A_SB_PICK + it.row,
                    on: false,
                    enabled: true,
                },
                f.at(LIST.0 + 20.0, y, LIST.2 - 20.0, BUY_ROW_H),
            );
            ui.add(
                root,
                Kind::Hotspot {
                    action: A_SB_ITEM + it.row,
                    on: false,
                    enabled: true,
                },
                f.at(LIST.0, y, 20.0, BUY_ROW_H),
            );
        }
        // ---- the picture and the text of the selected row ------------------------------------
        let shown = if last < 30 { last as usize } else { 0 };
        if let Some(Some(sprite)) = art.pictures.get(shown) {
            ui.add(
                root,
                Kind::Image {
                    sprite: *sprite,
                    fit: Fit::Stretch,
                },
                f.at(BUY_PIC.0, BUY_PIC.1, BUY_PIC.2, BUY_PIC.3),
            );
        }
        let round_k = |v: i32| ((v + 500) / 1000) * 1000;
        if last < 30
            && let Some(sel) = items.iter().find(|i| i.row == last)
        {
            let name = t.get(1043 + last, "");
            ui.add(
                root,
                Kind::Label {
                    text: name,
                    font: f.font(2, 13.0),
                    color: HEAD,
                    align: Align::Center,
                    wrap: false,
                },
                f.at(PIC.0, 248.0, PIC.2, 16.0),
            );
            ui.add(
                root,
                Kind::Label {
                    text: t.get(1076 + last, ""),
                    font: f.font(1, 10.5),
                    color: TEXT,
                    align: Align::Left,
                    wrap: true,
                },
                f.at(PIC.0 + 8.0, 268.0, PIC.2 - 16.0, 110.0),
            );
            let money = |v: f64| crate::info::money(v);
            let avail = money(((cash * 0.001).trunc() * 1000.0).max(0.0));
            let others = mask & !(1 << last) != 0;
            let cost = money(f64::from(round_k(sel.cost)));
            let text = if others {
                let total = self.improvement_total(station, mask);
                t.get(2704, "Cost %1\n(Checked buildings cost %2)\n(Available %3)")
                    .replace("%1", &cost)
                    .replace("%2", &money(f64::from(round_k(total))))
                    .replace("%3", &avail)
            } else {
                t.get(137, "Cost %1\n(Available %2)")
                    .replace("%1", &cost)
                    .replace("%2", &avail)
            };
            for (i, line) in text.split('\n').enumerate() {
                ui.add(
                    root,
                    Kind::Label {
                        text: line.to_owned(),
                        font: f.font(1, 10.5),
                        color: if i == 0 { TEXT } else { MUTED },
                        align: Align::Center,
                        wrap: false,
                    },
                    f.at(PIC.0, 392.0 + 14.0 * i as f32, PIC.2, 14.0),
                );
            }
        }
        // ---- buttons -------------------------------------------------------------------------
        let labels = [
            t.get(1912, "Purchase"),
            t.get(crate::text::id::CANCEL, "Cancel"),
        ];
        let plates = [(336.0, 472.0, 18.0, A_SB_OK), (534.0, 472.0, 15.0, A_CLOSE)];
        for (i, (x, y, lx, action)) in plates.into_iter().enumerate() {
            let sprites = art.buttons.map(|b| b[i]);
            ui.add(
                root,
                Kind::Icon {
                    up: sprites.map(|s| s.0),
                    down: sprites.map(|s| s.1),
                    fallback: labels[i].clone(),
                    font: f.font(5, 14.0),
                    action,
                    on: false,
                },
                f.at(x, y, 138.0, 63.0),
            );
            if sprites.is_some() {
                ui.add(
                    root,
                    Kind::Label {
                        text: labels[i].clone(),
                        font: f.font(5, 14.0),
                        color: PLATE_INK,
                        align: Align::Center,
                        wrap: false,
                    },
                    f.at(x + lx, y + 23.0, 108.0, 26.0),
                );
            }
        }
    }
}

/// Cells of the style grid of `bpaddstn.win` (ids 2001..2007): the position of each of the six
/// styles of [`super::build_ui::STYLES`] (Default, Victorian, Colonial, Mission, Tudor, Kyoto).
const STYLE_CELLS: [(f32, f32); 6] = [
    (340.0, 4.0),
    (340.0, 31.0),
    (340.0, 57.0),
    (397.0, 4.0),
    (397.0, 31.0),
    (397.0, 57.0),
];
/// Cells of the direction grid (ids 2030..2038): N, NE, E, SE, S, SW, W, NW, then the middle
/// one (Auto).
const ORIENT_CELLS: [(f32, f32); 9] = [
    (379.0, 86.0),
    (417.0, 86.0),
    (417.0, 112.0),
    (417.0, 138.0),
    (379.0, 138.0),
    (341.0, 138.0),
    (341.0, 112.0),
    (341.0, 86.0),
    (379.0, 112.0),
];

impl AppState {
    /// The Add Station palette drawn with the art of `bpaddstn.win` (D2; `0x4941c0` case 1):
    /// three cards with the station of each size in the chosen architecture and its cost, the
    /// six architecture cells and the nine direction cells, over the message strip. False
    /// without the install art (the caller falls back to the flat rows).
    pub(super) fn build_station_art_panel(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        area: (f32, f32),
        _dz: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) -> bool {
        use super::build_ui::{A_BT_CANCEL, A_BT_ORIENT, A_BT_SIZE, A_BT_STYLE, STYLES};
        let art = &self.dialog_art.buy;
        let (Some(bg), Some(misc)) = (art.station_bg, self.dialog_art.editor.misc) else {
            return false;
        };
        let Some(g) = self.game() else {
            return false;
        };
        let (cw, ch) = (458.0f32, 216.0f32);
        let k = (area.0 / cw).min(area.1 / ch).max(0.1);
        let (w, h) = (cw * k, ch * k);
        let root = ui.add(
            parent,
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::TopLeft, w, h)
                .offset(((area.0 - w) / 2.0).round(), ((area.1 - h) / 2.0).round()),
        );
        let f = Frame { root, k };
        let img = |ui: &mut Ui, sprite, x: f32, y: f32, w: f32, h: f32| {
            ui.add(
                root,
                Kind::Image {
                    sprite,
                    fit: Fit::Stretch,
                },
                f.at(x, y, w, h),
            );
        };
        img(ui, bg, 0.0, 0.0, 458.0, 166.0);
        // The style strip: record = style value + 1 (`0x4941c0`: message 0x6e to control 1816);
        // the Default style shows the Victorian art here (the original uses the nearest town's).
        let b = &g.build;
        let value = STYLES[b.style.min(5)].1 + 1;
        let rec = if value == 0 { 1 } else { value as usize };
        if let Some(Some(p)) = art.station_strips.get(rec) {
            img(ui, p.sprite, p.x as f32, p.y as f32, p.w, p.h);
        }
        img(ui, misc, 0.0, 166.0, 458.0, 50.0);
        let t = &self.texts;
        let Some((save, data, co)) = g
            .world
            .save_ref()
            .zip(self.data.as_ref())
            .zip(self.player())
            .map(|((s, d), c)| (s, d, c))
        else {
            return true;
        };
        let cell = |ui: &mut Ui,
                    tips: &mut Vec<(NodeId, String)>,
                    r: [f32; 4],
                    action: u32,
                    on: bool,
                    tip: String| {
            let n = ui.add(
                root,
                Kind::Hotspot {
                    action,
                    on,
                    enabled: true,
                },
                f.at(r[0], r[1], r[2], r[3]),
            );
            tips.push((n, tip));
        };
        // Size cards with the cost under the picture (`"%s: %s"` of LNG 0x9a and the price).
        for (i, s) in bt::StationSize::ALL.iter().enumerate() {
            let x = 5.0 + 110.0 * i as f32;
            let price = bt::station_price(data, save, co, s.shape(0), 0, 0);
            ui.add(
                root,
                Kind::Label {
                    text: format!("{}: ${}K", t.get(154, "Cost"), price / 1000),
                    font: f.font(1, 10.5),
                    color: TEXT,
                    align: Align::Center,
                    wrap: false,
                },
                f.at(x, 130.0, 106.0, 16.0),
            );
            cell(
                ui,
                tips,
                [x, 3.0, 106.0, 159.0],
                A_BT_SIZE + i as u32,
                b.size == *s,
                t.get(1076 + i as u32, ""),
            );
        }
        let style_tip = t.a("The architecture of the station building.");
        for (i, (x, y)) in STYLE_CELLS.into_iter().enumerate() {
            cell(
                ui,
                tips,
                [x, y, 56.0, 26.0],
                A_BT_STYLE + i as u32,
                b.style == i,
                style_tip.clone(),
            );
        }
        for (o, (x, y)) in ORIENT_CELLS.into_iter().enumerate() {
            let tip = if o == 8 {
                t.get(
                    2556,
                    "Change the station direction dynamically, to try to best fit nearby track.",
                )
            } else {
                t.a("The direction of the station's track.")
            };
            let on = if o == 8 {
                b.orient.is_none()
            } else {
                b.orient == Some(o as i32)
            };
            cell(
                ui,
                tips,
                [x, y, 37.0, 25.0],
                A_BT_ORIENT + o as u32,
                on,
                tip,
            );
        }
        // The message field of the strip: the cost, then why the command is refused.
        let plan = b.plan.as_ref();
        let (msg, color) = match (plan.and_then(|p| p.error.clone()), &b.message) {
            (Some(e), _) => (e, super::build_dialogs::BAD),
            (None, Some((m, good))) => (
                m.clone(),
                if *good {
                    Color::rgb(0x9a, 0xe0, 0x8a)
                } else {
                    super::build_dialogs::BAD
                },
            ),
            (None, None) => (
                match plan {
                    Some(p) if p.cost > 0.0 => {
                        t.af("Cost: %1", &[crate::info::money(p.cost)])
                    }
                    _ => t.a("Click next to existing track to place the station. The shaded area is its catchment."),
                },
                TEXT,
            ),
        };
        ui.add(
            root,
            Kind::Label {
                text: msg,
                font: f.font(1, 8.0),
                color,
                align: Align::Left,
                wrap: true,
            },
            f.at(116.0, 175.0, 210.0, 28.0),
        );
        // The plate of the strip (control 2015).
        let n = ui.add(
            root,
            Kind::Hotspot {
                action: A_BT_CANCEL,
                on: false,
                enabled: true,
            },
            f.at(354.0, 175.0, 84.0, 29.0),
        );
        tips.push((n, t.get(crate::text::id::CANCEL, "Cancel")));
        ui.add(
            root,
            Kind::Label {
                text: t.get(crate::text::id::CANCEL, "Cancel"),
                font: f.font(1, 11.0),
                color: TEXT,
                align: Align::Center,
                wrap: false,
            },
            f.at(354.0, 175.0, 84.0, 29.0),
        );
        true
    }
}
