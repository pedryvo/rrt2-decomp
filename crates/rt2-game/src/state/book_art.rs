//! Original `.win`/`.imb` artwork rendering for the company book (`compndtl.win`) and
//! player detail book (`playrdtl.win`).

use std::sync::Arc;

use rt2_ui::{Align, Anchor, Color, Fill, Fit, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::AppState;
use super::companybook::{A_BOOK, A_TERR_MAP, A_TERR_MODE, TERR_MODES, terr_map, territory_color};
use super::info_ui::{A_BACK, A_CLOSE, A_NEXT_CO, A_PREV_CO, A_ROW, InfoScreen};
use crate::info::{Block, Page, TOTAL, Table};

pub(super) const A_OD_TAB: u32 = 11050;

const INK: Color = Color::rgb(0x1c, 0x18, 0x12);
const INK_MUTED: Color = Color::rgb(0x4a, 0x3e, 0x30);
const INK_HEAD: Color = Color::rgb(0x3a, 0x1e, 0x10);
const INK_NEG: Color = Color::rgb(0x98, 0x1c, 0x18);
const TOTAL_BG: Color = Color(0x88, 0x6c, 0x3a, 0x38);
const DESK_TEXT: Color = Color::rgb(0xf0, 0xe4, 0xc8);
const DESK_MUTED: Color = Color::rgb(0xc8, 0xb8, 0x98);
const NO_LAND: Color = Color::rgb(0x18, 0x1c, 0x20);

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

fn fnt(id: u8, size: f32) -> FontStyle {
    game_font(id).with_size(size)
}

fn lbl(text: &str, font: FontStyle, color: Color, align: Align) -> Kind {
    Kind::Label {
        text: text.to_owned(),
        font,
        color,
        align,
        wrap: false,
    }
}

fn book_col_rows(blocks: &[Block], w: f32, size: f32) -> f32 {
    let mut rows = 0.0;
    let cpl = (w / (size * 0.56)).max(12.0);
    for b in blocks {
        match b {
            Block::Facts(f) => rows += f.len() as f32 + 0.3,
            Block::Table { title, table } => {
                if !title.is_empty() {
                    rows += 1.2;
                }
                if table.cols.iter().any(|c| !c.head.is_empty()) {
                    rows += 1.05;
                }
                rows += table.rows.len() as f32 + 0.4;
            }
            Block::Note(t) => {
                let lines = (t.chars().count() as f32 / cpl).ceil().max(1.0);
                rows += lines + 0.3;
            }
        }
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn draw_book_table(
    ui: &mut Ui,
    parent: NodeId,
    table: &Table,
    x: f32,
    y: f32,
    w: f32,
    rowh: f32,
    size: f32,
    visible: usize,
) -> f32 {
    let total_w: f32 = table.cols.iter().map(|c| c.weight).sum::<f32>().max(0.1);
    let mut xs = Vec::with_capacity(table.cols.len());
    let mut cx = x;
    for c in &table.cols {
        let cw = w * c.weight / total_w;
        xs.push((cx, cw));
        cx += cw;
    }
    let f = fnt(2, size);
    let headed = table.cols.iter().any(|c| !c.head.is_empty());
    if headed {
        for (c, &(cx, cw)) in table.cols.iter().zip(&xs) {
            let a = if c.right { Align::Right } else { Align::Left };
            ui.add(
                parent,
                lbl(&c.head, f, INK_HEAD, a),
                at(cx + 2.0, y, (cw - 4.0).max(1.0), rowh),
            );
        }
    }
    let mut yy = y + if headed { rowh + 1.0 } else { 1.0 };
    let n = table.rows.len();
    let (shown, more) = if n > visible && visible > 0 {
        (visible - 1, n - (visible - 1))
    } else {
        (n.min(visible), 0)
    };
    for (ri, row) in table.rows.iter().enumerate().take(shown) {
        let is_total = table.ids.get(ri) == Some(&TOTAL);
        if is_total {
            ui.add(
                parent,
                Kind::Panel {
                    fill: Fill::Solid(TOTAL_BG),
                },
                at(x, yy, w, rowh),
            );
        }
        for ((cell, c), &(cx, cw)) in row.iter().zip(&table.cols).zip(&xs) {
            let a = if c.right { Align::Right } else { Align::Left };
            let neg = cell.starts_with("-$");
            let color = if neg {
                INK_NEG
            } else if is_total {
                INK_HEAD
            } else {
                INK
            };
            ui.add(
                parent,
                lbl(cell, f, color, a),
                at(cx + 2.0, yy, (cw - 4.0).max(1.0), rowh),
            );
        }
        yy += rowh;
    }
    if more > 0 {
        ui.add(
            parent,
            lbl(&format!("... {more} more"), f, INK_MUTED, Align::Left),
            at(x + 2.0, yy, w - 4.0, rowh),
        );
        yy += rowh;
    }
    yy
}

#[allow(clippy::too_many_arguments)]
fn draw_book_blocks(
    ui: &mut Ui,
    parent: NodeId,
    blocks: &[Block],
    x: f32,
    y0: f32,
    w: f32,
    bottom: f32,
    rowh: f32,
    size: f32,
) {
    let f = fnt(2, size);
    let mut y = y0;
    for b in blocks {
        let room = ((bottom - y) / rowh).floor().max(0.0) as usize;
        if room == 0 {
            break;
        }
        match b {
            Block::Facts(facts) => {
                for (i, (k, v)) in facts.iter().take(room).enumerate() {
                    let yy = y + i as f32 * rowh;
                    ui.add(
                        parent,
                        lbl(k, f, INK_MUTED, Align::Left),
                        at(x + 2.0, yy, w * 0.56 - 2.0, rowh),
                    );
                    let neg = v.starts_with("-$");
                    ui.add(
                        parent,
                        lbl(v, f, if neg { INK_NEG } else { INK }, Align::Right),
                        at(x + w * 0.44, yy, w * 0.56 - 2.0, rowh),
                    );
                }
                y += rowh * facts.len().min(room) as f32 + rowh * 0.25;
            }
            Block::Table { title, table } => {
                if !title.is_empty() {
                    ui.add(
                        parent,
                        lbl(title, fnt(1, size * 1.1), INK_HEAD, Align::Left),
                        at(x + 2.0, y, w - 4.0, rowh * 1.15),
                    );
                    y += rowh * 1.2;
                }
                let headed = usize::from(table.cols.iter().any(|c| !c.head.is_empty()));
                let vis = ((bottom - y) / rowh).floor().max(0.0) as usize;
                let vis = vis.saturating_sub(headed);
                y = draw_book_table(ui, parent, table, x, y, w, rowh, size, vis) + rowh * 0.3;
            }
            Block::Note(t) => {
                let h = (bottom - y).max(rowh);
                ui.add(
                    parent,
                    Kind::Label {
                        text: t.clone(),
                        font: f,
                        color: INK,
                        align: Align::Left,
                        wrap: true,
                    },
                    at(x + 2.0, y, w - 4.0, h),
                );
                let cpl = (w / (size * 0.56)).max(12.0);
                let lines = (t.chars().count() as f32 / cpl).ceil().max(1.0);
                y += lines * rowh + rowh * 0.25;
            }
        }
    }
}

fn fit_book_column(blocks: &[Block], w: f32, h: f32, s: f32) -> (f32, f32) {
    let mut rowh = 18.0 * s;
    let min_rowh = 10.5 * s;
    while rowh > min_rowh {
        let size = (rowh * 0.68).max(8.5 * s);
        if book_col_rows(blocks, w, size) * rowh <= h {
            return (rowh, size);
        }
        rowh -= 0.5 * s;
    }
    (min_rowh, (min_rowh * 0.68).max(8.5 * s))
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    fn draw_odbottom_book(
        &self,
        ui: &mut Ui,
        root: NodeId,
        s: f32,
        strip: Option<crate::artwork::buywin::Placed>,
        active_tab: u32,
        hint: &str,
        list_action: u32,
        list_fallback: String,
    ) {
        let det = &self.dialog_art.detail;
        if let Some(p) = det.odbottom_bg {
            ui.add(
                root,
                Kind::Image {
                    sprite: p.sprite,
                    fit: Fit::Stretch,
                },
                at(p.x as f32 * s, p.y as f32 * s, p.w * s, p.h * s),
            );
        }
        if let Some(p) = strip {
            ui.add(
                root,
                Kind::Image {
                    sprite: p.sprite,
                    fit: Fit::Stretch,
                },
                at(p.x as f32 * s, p.y as f32 * s, p.w * s, p.h * s),
            );
        }
        for i in 0..4u32 {
            let _ = active_tab;
            ui.add(
                root,
                Kind::Hotspot {
                    action: A_OD_TAB + i,
                    on: false,
                    enabled: true,
                },
                at((2.0 + 45.0 * i as f32) * s, 730.0 * s, 45.0 * s, 35.0 * s),
            );
        }
        ui.add(
            root,
            lbl(hint, fnt(2, 13.0 * s), DESK_MUTED, Align::Left),
            at(190.0 * s, 735.0 * s, 645.0 * s, 30.0 * s),
        );
        let mut add_od_btn =
            |rect: Layout, sprites: Option<(rt2_ui::SpriteId, rt2_ui::SpriteId)>, action: u32, fallback: String| {
                if let Some((up, down)) = sprites {
                    ui.add(
                        root,
                        Kind::Icon {
                            up: Some(up),
                            down: Some(down),
                            fallback,
                            font: fnt(1, 12.0 * s),
                            action,
                            on: false,
                        },
                        rect,
                    );
                } else {
                    ui.add(
                        root,
                        Kind::Button {
                            text: fallback,
                            font: fnt(1, 12.0 * s),
                            action,
                            enabled: true,
                            hotkey: None,
                        },
                        rect,
                    );
                }
            };
        add_od_btn(
            at(845.0 * s, 735.0 * s, 32.0 * s, 29.0 * s),
            det.odbottom_prev,
            A_PREV_CO,
            "<".into(),
        );
        add_od_btn(
            at(880.0 * s, 735.0 * s, 32.0 * s, 29.0 * s),
            det.odbottom_next,
            A_NEXT_CO,
            ">".into(),
        );
        add_od_btn(
            at(916.0 * s, 735.0 * s, 40.0 * s, 31.0 * s),
            det.odbottom_list,
            list_action,
            list_fallback,
        );
        let ok_rect = at(962.0 * s, 734.0 * s, 60.0 * s, 32.0 * s);
        let ok_str = self.texts.get(crate::text::id::OK, "OK");
        add_od_btn(ok_rect, det.odbottom_ok, A_CLOSE, ok_str.clone());
        ui.add(
            root,
            lbl(&ok_str, fnt(1, 12.0 * s), DESK_TEXT, Align::Center),
            ok_rect,
        );
    }

    /// Company book (`compndtl.win`) laid out over the original desk/book artwork.
    pub(super) fn build_company_book_art(&self, ui: &mut Ui, screen: InfoScreen, page: Option<&Page>) {
        let book = &self.dialog_art.book;
        let Some(bg) = book.compndtl_bg else {
            return;
        };
        let (cw, ch) = ui.logical_size();
        let s = (cw / 1024.0).min(ch / 768.0);
        let (w, h) = (1024.0 * s, 768.0 * s);
        let root = ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(Color::rgb(0x10, 0x10, 0x10)),
            },
            Layout::at(Anchor::Center, w, h),
        );
        let r_at = |r: [f32; 4]| at(r[0] * s, r[1] * s, r[2] * s, r[3] * s);

        ui.add(
            root,
            Kind::Image {
                sprite: bg,
                fit: Fit::Stretch,
            },
            at(0.0, 0.0, w, 728.0 * s),
        );

        let tab_idx = match screen {
            InfoScreen::Overview(_) => 0,
            InfoScreen::Income(_) => 1,
            InfoScreen::Balance(_) => 2,
            InfoScreen::Territories(..) => 3,
            _ => 4,
        };
        if let Some(spread) = book.compndtl_spreads[tab_idx] {
            ui.add(
                root,
                Kind::Image {
                    sprite: spread,
                    fit: Fit::Stretch,
                },
                at(152.0 * s, 83.0 * s, 708.0 * s, 538.0 * s),
            );
        }

        // 5 book tab hotspots on the right edge of the book.
        let tab_actions = [A_BOOK, A_BOOK + 4, A_BOOK + 5, A_BOOK + 2, A_BOOK + 1];
        for (i, &action) in tab_actions.iter().enumerate() {
            ui.add(
                root,
                Kind::Hotspot {
                    action,
                    on: false,
                    enabled: true,
                },
                r_at(book.comp_tabs[i]),
            );
        }

        // Company title & subtitle on the dark wood desk above the book.
        let (title, subtitle) = match page {
            Some(p) => (p.title.clone(), p.subtitle.clone()),
            None => (self.texts.a("Not available"), String::new()),
        };
        ui.add(
            root,
            lbl(
                &title,
                game_font(6).with_size(22.0 * s).with_shadow(),
                DESK_TEXT,
                Align::Left,
            ),
            at(160.0 * s, 24.0 * s, 430.0 * s, 32.0 * s),
        );
        if !subtitle.is_empty() {
            ui.add(
                root,
                lbl(&subtitle, fnt(2, 14.0 * s), DESK_MUTED, Align::Right),
                at(590.0 * s, 28.0 * s, 260.0 * s, 26.0 * s),
            );
        }

        // Current section label strip on the top-right of the desk or left margin, plus company logo at desk slot.
        let company = match screen {
            InfoScreen::Overview(c)
            | InfoScreen::Income(c)
            | InfoScreen::Balance(c)
            | InfoScreen::Territories(c, _)
            | InfoScreen::Finance(c) => self.book_company(c),
            _ => self.book_company(0),
        };
        if let Some(logo) = self
            .with_info(|i| {
                i.save
                    .companies
                    .get(company as usize)
                    .and_then(|r| r.data.get(0x2b).copied())
                    .map(usize::from)
            })
            .flatten()
            .and_then(|k| self.icons.brand.logos.get(k).copied().flatten())
        {
            ui.add(
                root,
                Kind::Image {
                    sprite: logo,
                    fit: Fit::Contain,
                },
                at(26.0 * s, 361.0 * s, 92.0 * s, 67.0 * s),
            );
        }

        // Section name buttons on the top edge of the desk (compact, matching the 5 book tabs + Reports).
        let t = &self.texts;
        let sec_names = [
            t.a("Overview"),
            t.a("Income"),
            t.a("Balance"),
            t.get(2369, "Territories"),
            t.get(crate::text::id::FINANCES, "Finances"),
        ];
        for (i, (name, &action)) in sec_names.iter().zip(&tab_actions).enumerate() {
            let rect = at((160.0 + i as f32 * 116.0) * s, 58.0 * s, 110.0 * s, 22.0 * s);
            ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Solid(if i == tab_idx {
                        Color(0xe6, 0xda, 0xbc, 0xf0)
                    } else {
                        Color(0xc2, 0xb2, 0x92, 0xdc)
                    }),
                },
                rect,
            );
            ui.add(
                root,
                Kind::Toggle {
                    text: name.clone(),
                    font: fnt(2, 11.0 * s),
                    action,
                    on: i == tab_idx,
                    enabled: true,
                },
                rect,
            );
        }
        let rep_rect = at(740.0 * s, 58.0 * s, 110.0 * s, 22.0 * s);
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(Color(0xc2, 0xb2, 0x92, 0xdc)),
            },
            rep_rect,
        );
        ui.add(
            root,
            Kind::Button {
                text: t.a("Reports"),
                font: fnt(2, 11.0 * s),
                action: super::reports_ui::A_REP_OPEN,
                enabled: true,
                hotkey: None,
            },
            rep_rect,
        );

        // Book pages content.
        if let Some(page) = page {
            match screen {
                InfoScreen::Territories(_, mode) => {
                    self.draw_territories_book(ui, root, s, company, mode, book.comp_left, book.comp_right);
                }
                InfoScreen::Overview(_) => {
                    self.draw_overview_book(ui, root, s, company, page, book);
                }
                _ => {
                    let (lx, ly, lw, lh) = (
                        book.comp_left[0] * s,
                        book.comp_left[1] * s,
                        book.comp_left[2] * s,
                        book.comp_left[3] * s,
                    );
                    let (rx, ry, rw, rh) = (
                        book.comp_right[0] * s,
                        book.comp_right[1] * s,
                        book.comp_right[2] * s,
                        book.comp_right[3] * s,
                    );
                    let (lrowh, lsize) = fit_book_column(&page.left, lw, lh, s);
                    draw_book_blocks(ui, root, &page.left, lx, ly, lw, ly + lh, lrowh, lsize);
                    let (rrowh, rsize) = fit_book_column(&page.right, rw, rh, s);
                    draw_book_blocks(ui, root, &page.right, rx, ry, rw, ry + rh, rrowh, rsize);
                }
            }
        }

        // Action buttons on the right side of the desk below the book (avoids the pocket watch).
        let acts = match screen {
            InfoScreen::Finance(_) | InfoScreen::Income(_) | InfoScreen::Balance(_) => {
                self.fin_buttons(company)
            }
            InfoScreen::Overview(_) => self.overview_buttons(company),
            _ => Vec::new(),
        };
        if !acts.is_empty() {
            let per_row = acts.len().clamp(1, 3);
            let gap = 6.0 * s;
            let max_w = 496.0 * s;
            let bw = ((max_w - gap * (per_row as f32 - 1.0)) / per_row as f32).min(162.0 * s);
            let bh = 25.0 * s;
            for (i, (text, action)) in acts.iter().enumerate() {
                let c = (i % per_row) as f32;
                let r = (i / per_row) as f32;
                let font_sz = (bw / (text.chars().count().max(1) as f32 * 0.72)).clamp(8.0 * s, 10.5 * s);
                let rect = at(504.0 * s + c * (bw + gap), 636.0 * s + r * (bh + 4.0 * s), bw, bh);
                ui.add(
                    root,
                    Kind::Panel {
                        fill: Fill::Solid(Color(0xc8, 0xb8, 0x98, 0xe4)),
                    },
                    rect,
                );
                ui.add(
                    root,
                    Kind::Button {
                        text: text.clone(),
                        font: fnt(2, font_sz),
                        action: *action,
                        enabled: true,
                        hotkey: None,
                    },
                    rect,
                );
            }
        }

        self.draw_odbottom_book(
            ui,
            root,
            s,
            book.odbottom_companies,
            3,
            &self.texts.a("Left / Right: company   Esc: close"),
            A_BOOK + 3,
            self.texts.a("Companies"),
        );
    }

    fn draw_overview_book(
        &self,
        ui: &mut Ui,
        root: NodeId,
        s: f32,
        company: u32,
        page: &Page,
        book: &crate::artwork::bookwin::BookArt,
    ) {
        let portrait = self
            .with_info(|i| i.chairman_portrait(company))
            .flatten()
            .and_then(|k| self.icons.brand.portraits.get(k).copied().flatten());
        let (lx, ly, lw, lh) = (
            book.comp_left[0] * s,
            book.comp_left[1] * s,
            book.comp_left[2] * s,
            book.comp_left[3] * s,
        );
        let body_y = if let Some(sprite) = portrait {
            ui.add(
                root,
                Kind::Image {
                    sprite,
                    fit: Fit::Contain,
                },
                at(lx + 72.0 * s, ly, 120.0 * s, 138.0 * s),
            );
            ly + 146.0 * s
        } else {
            ly
        };
        let avail_h = (ly + lh - body_y).max(60.0 * s);
        let (lrowh, lsize) = fit_book_column(&page.left, lw, avail_h, s);
        draw_book_blocks(ui, root, &page.left, lx, body_y, lw, ly + lh, lrowh, lsize);

        let (rx, ry, rw, rh) = (
            book.comp_right[0] * s,
            book.comp_right[1] * s,
            book.comp_right[2] * s,
            book.comp_right[3] * s,
        );
        let (rrowh, rsize) = fit_book_column(&page.right, rw, rh, s);
        draw_book_blocks(ui, root, &page.right, rx, ry, rw, ry + rh, rrowh, rsize);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_territories_book(
        &self,
        ui: &mut Ui,
        root: NodeId,
        s: f32,
        company: u32,
        mode: u8,
        r_left: [f32; 4],
        r_right: [f32; 4],
    ) {
        let t = &self.texts;
        let Some((rows, map, table)) = self.with_info(|i| {
            let rows = i.territory_rows(company);
            let map = terr_map(i.save, &rows);
            let table = match i.territories_page(company).right.into_iter().next() {
                Some(Block::Table { table, .. }) => Some(table),
                _ => None,
            };
            (rows, map, table)
        }) else {
            return;
        };
        let mode = usize::from(mode).min(TERR_MODES - 1);
        let (lx, ly, lw, lh) = (r_left[0] * s, r_left[1] * s, r_left[2] * s, r_left[3] * s);
        let names = [
            t.a("Political view"),
            t.a("Access rights view"),
            t.a("Goodwill view"),
        ];
        let bw = (lw - 8.0 * s) / 3.0;
        for (i, n) in names.iter().enumerate() {
            ui.add(
                root,
                Kind::Toggle {
                    text: n.clone(),
                    font: fnt(2, 9.5 * s),
                    action: A_TERR_MODE + i as u32,
                    on: i == mode,
                    enabled: true,
                },
                at(lx + i as f32 * (bw + 4.0 * s), ly, bw, 24.0 * s),
            );
        }

        let side = (lw - 8.0 * s).min(lh - 100.0 * s).max(80.0 * s);
        let fit = (side / map.cols.max(map.rows) as f32).max(1.0);
        let (mw, mh) = (map.cols as f32 * fit, map.rows as f32 * fit);
        let frame = ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(Color::rgb(0x28, 0x22, 0x1a)),
            },
            at(lx, ly + 30.0 * s, mw + 6.0 * s, mh + 6.0 * s),
        );
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
                        let tc = territory_color(mode, *k, &rows[*k]);
                        Color::rgb(
                            ((u16::from(tc.0) * 3 + u16::from(base.0)) / 4) as u8,
                            ((u16::from(tc.1) * 3 + u16::from(base.1)) / 4) as u8,
                            ((u16::from(tc.2) * 3 + u16::from(base.2)) / 4) as u8,
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
            at(3.0 * s, 3.0 * s, mw, mh),
        );
        ui.add(
            root,
            Kind::Label {
                text: t.a("Click a territory to see its government and buy access rights."),
                font: fnt(2, 10.5 * s),
                color: INK_MUTED,
                align: Align::Left,
                wrap: true,
            },
            at(lx, ly + 42.0 * s + mh, lw, 42.0 * s),
        );

        let (rx, ry, rw, rh) = (r_right[0] * s, r_right[1] * s, r_right[2] * s, r_right[3] * s);
        let Some(table) = table else {
            ui.add(
                root,
                lbl(
                    &t.a("This map has no political divisions."),
                    fnt(2, 12.0 * s),
                    INK_MUTED,
                    Align::Left,
                ),
                at(rx, ry + 10.0 * s, rw, 22.0 * s),
            );
            return;
        };
        let rowh = 18.0 * s;
        let visible = (((rh - rowh - 4.0 * s) / rowh).floor() as usize).max(3);
        let shown = table.rows.len().min(visible);
        ui.add(
            root,
            Kind::List {
                items: vec![String::new(); shown],
                selected: None,
                scroll: 0,
                row_h: rowh,
                font: fnt(2, 11.0 * s),
                action: A_ROW,
            },
            at(rx, ry + rowh + 2.0 * s, rw, rowh * shown as f32),
        );
        draw_book_table(ui, root, &table, rx, ry, rw, rowh, 9.0 * s, shown);
    }

    /// Player detail book (`playrdtl.win`) laid out over the original desk/book artwork.
    pub(super) fn build_player_book_art(&self, ui: &mut Ui, person: u32, page: Option<&Page>) {
        let book = &self.dialog_art.book;
        let Some(bg) = book.playrdtl_bg else {
            return;
        };
        let (cw, ch) = ui.logical_size();
        let s = (cw / 1024.0).min(ch / 768.0);
        let (w, h) = (1024.0 * s, 768.0 * s);
        let root = ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(Color::rgb(0x10, 0x10, 0x10)),
            },
            Layout::at(Anchor::Center, w, h),
        );

        ui.add(
            root,
            Kind::Image {
                sprite: bg,
                fit: Fit::Stretch,
            },
            at(0.0, 0.0, w, 728.0 * s),
        );

        let person = if person == 0 {
            self.with_info(|i| i.save.persons.ordered().next().map(|r| r.slot as u32))
                .flatten()
                .unwrap_or(0)
        } else {
            person
        };
        let portrait = self
            .with_info(|i| i.player_portrait(person))
            .flatten()
            .and_then(|k| self.icons.brand.portraits.get(k).copied().flatten());
        if let Some(sprite) = portrait {
            ui.add(
                root,
                Kind::Image {
                    sprite,
                    fit: Fit::Stretch,
                },
                at(22.0 * s, 12.0 * s, 226.0 * s, 236.0 * s),
            );
        }
        if let Some(frame) = book.playrdtl_frame {
            ui.add(
                root,
                Kind::Image {
                    sprite: frame.sprite,
                    fit: Fit::Stretch,
                },
                at(frame.x as f32 * s, frame.y as f32 * s, frame.w * s, frame.h * s),
            );
        }

        if let Some(page) = page {
            let title_sz = (186.0 * s / (page.title.chars().count().max(1) as f32 * 0.7))
                .clamp(9.0 * s, 13.0 * s);
            ui.add(
                root,
                lbl(&page.title, fnt(1, title_sz), INK_HEAD, Align::Center),
                at(30.0 * s, 260.0 * s, 210.0 * s, 24.0 * s),
            );
            // Left page: avoid the oval portrait frame + ribbon on the top-left (x < 276, y < 314).
            let lx = 276.0 * s;
            let ly = book.playr_left_body[1] * s;
            let lw = (book.playr_left[0] + book.playr_left[2] - 276.0).max(180.0) * s;
            let lh = book.playr_left_body[3] * s;
            let (lrowh, lsize) = fit_book_column(&page.left, lw, lh, s);
            draw_book_blocks(ui, root, &page.left, lx, ly, lw, ly + lh, lrowh, lsize);

            let (rx, ry, rw, rh) = (
                book.playr_right[0] * s,
                book.playr_right[1] * s,
                book.playr_right[2] * s,
                book.playr_right[3] * s,
            );
            let (rrowh, rsize) = fit_book_column(&page.right, rw, rh, s);
            draw_book_blocks(ui, root, &page.right, rx, ry, rw, ry + rh, rrowh, rsize);
        }

        self.draw_odbottom_book(
            ui,
            root,
            s,
            book.odbottom_players,
            2,
            &self.texts.a("Left / Right: player   Esc: close"),
            A_BACK,
            self.texts.a("Back"),
        );
    }
}
