//! Stock Market (`InfoScreen::Stock`) rendered over the original `stockdtl.win` artwork
//! (`stockdtl.imb` + `odbottom.imb`). Falls back to the flat info-panel layout when install art
//! is absent.

use rt2_ui::{Align, Anchor, Color, Fill, Fit, Kind, Layout, Ui, game_font};

use super::info_ui::{
    A_CLOSE, A_ROW, A_STOCK_BUY, A_STOCK_SELL, Content, InfoScreen, draw_table, font,
};
use super::{AppState, Screen};
use crate::info::TOTAL;
use crate::text::id as lng;

pub(super) const A_STOCK_TAB_PERSONAL: u32 = 6320;
pub(super) const A_STOCK_TAB_CORPORATE: u32 = 6321;
pub(super) const A_STOCK_VIEW: u32 = 6330;

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const INSET: Color = Color(0x0c, 0x10, 0x18, 0xe8);
const DIM: Color = Color(0, 0, 0, 120);

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

impl AppState {
    /// Renders `InfoScreen::Stock` over the `stockdtl.win` 1024x768 artwork.
    pub(super) fn build_stock_art(
        &mut self,
        ui: &mut Ui,
        content: Option<Content>,
        first_scroll: usize,
    ) {
        let Some(frame) = self.dialog_art.stock.frame else {
            return;
        };
        let art = self.dialog_art.stock.clone();
        let (cw, ch) = ui.logical_size();
        let s = (cw / 1024.0).min(ch / 768.0);
        let (w, h) = (1024.0 * s, 768.0 * s);

        ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(DIM),
            },
            Layout::fill(),
        );
        let root = ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(Color::rgb(0x10, 0x10, 0x10)),
            },
            Layout::at(Anchor::Center, w, h),
        );
        ui.add(
            root,
            Kind::Image {
                sprite: frame,
                fit: Fit::Stretch,
            },
            at(0.0, 0.0, w, h),
        );

        // Glass ticker dome readout on the left desk area (C8).
        self.draw_ticker_sized(
            ui,
            root,
            16.0 * s,
            274.0 * s,
            360.0 * s,
            28.0 * s,
            (15.0 * s).max(10.0),
        );

        // Clipboard / notepad inset (`Image 1900`: 409, 82, 577, 446).
        let [pad_x, pad_y, pad_w, pad_h] = art.r_pad;
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(pad_x * s, pad_y * s, pad_w * s, pad_h * s),
        );

        let px = pad_x * s + 10.0 * s;
        let py = pad_y * s + 8.0 * s;
        let pw = pad_w * s - 20.0 * s;
        let p_bot = (pad_y + pad_h) * s - 8.0 * s;

        let is_personal = self.stock_tab == 0;
        let list = match content {
            Some(Content::List(l)) => Some(l),
            _ => None,
        };

        if let Some(list) = list {
            let tab_title = if is_personal {
                list.title.clone()
            } else {
                format!("{} — {}", list.title, self.texts.a("Corporate"))
            };
            ui.add(
                root,
                Kind::Label {
                    text: tab_title,
                    font: game_font(6).with_size((15.0 * s).max(10.0)).with_shadow(),
                    color: HEAD,
                    align: Align::Left,
                    wrap: false,
                },
                at(px, py, pw, 22.0 * s),
            );

            let mut table_y = py + 24.0 * s;
            if is_personal {
                let personal = self.personal_lines();
                let line_h = 17.0 * s;
                let line_sz = (11.0 * s).max(9.0);
                for line in personal.iter().take(2) {
                    ui.add(
                        root,
                        Kind::Label {
                            text: line.clone(),
                            font: font(line_sz),
                            color: TEXT,
                            align: Align::Left,
                            wrap: false,
                        },
                        at(px, table_y, pw, line_h),
                    );
                    table_y += line_h;
                }
                if !personal.is_empty() {
                    table_y += 4.0 * s;
                }
            } else {
                table_y += 2.0 * s;
            }

            let n = list.table.rows.len();
            let rowh = if is_personal { 22.0 * s } else { 24.0 * s };
            let head_h = rowh + 4.0 * s;
            let fsize = (if is_personal { 11.0 } else { 12.0 } * s).max(9.0);
            let visible = (((p_bot - table_y - head_h) / rowh).floor() as usize).max(3);
            let first = first_scroll.min(n.saturating_sub(visible));
            if let Screen::Game(g) = &mut self.screen
                && let Some(i) = &mut g.info
                && i.screen == InfoScreen::Stock
            {
                i.scroll = first;
            }
            let shown = visible.min(n - first);

            let bw = if is_personal { 38.0 * s } else { 44.0 * s };
            let bgap = 3.0 * s;
            let btns_w = if is_personal {
                3.0 * bw + 2.0 * bgap + 6.0 * s
            } else {
                bw + 6.0 * s
            };
            let table_w = pw - btns_w;

            if list.table.rows.is_empty() {
                ui.add(
                    root,
                    Kind::Label {
                        text: self.texts.a("Nothing to show."),
                        font: font((13.0 * s).max(9.0)),
                        color: MUTED,
                        align: Align::Left,
                        wrap: false,
                    },
                    at(px + 8.0 * s, table_y + 32.0 * s, table_w, rowh),
                );
            } else {
                ui.add(
                    root,
                    Kind::List {
                        items: vec![String::new(); shown],
                        selected: None,
                        scroll: 0,
                        row_h: rowh,
                        font: font(fsize),
                        action: A_ROW,
                    },
                    at(
                        px,
                        table_y + head_h - 1.0,
                        table_w,
                        rowh * shown as f32 + 2.0,
                    ),
                );
                draw_table(
                    ui,
                    root,
                    &list.table,
                    px,
                    table_y + 2.0 * s,
                    table_w - if n > visible { 10.0 * s } else { 0.0 },
                    rowh,
                    fsize,
                    first,
                    visible,
                    false,
                );

                let trade = self.human_person();
                let buy_t = self.texts.get(305, "Buy").replace('&', "");
                let sell_t = self.texts.get(307, "Sell").replace('&', "");
                let view_t = self.texts.a("View");
                let btn_font = font((10.0 * s).max(8.5));

                for r in 0..shown {
                    let Some(&id) = list.table.ids.get(first + r) else {
                        continue;
                    };
                    if id == TOTAL {
                        continue;
                    }
                    let ry = table_y + 2.0 * s + (rowh + 2.0 * s) + r as f32 * rowh + 1.0;
                    let bx0 = px + table_w + 6.0 * s;
                    if is_personal {
                        let (can_buy, can_sell) = match (&self.screen, trade) {
                            (Screen::Game(g), Some(person)) => g
                                .world
                                .fin_trade_quote(person, id as i32)
                                .map_or((false, false), |q| {
                                    (q.can_buy || q.can_buy_margin, q.can_sell || q.can_short)
                                }),
                            _ => (false, false),
                        };
                        for (k, (label_t, action, enabled)) in [
                            (&buy_t, A_STOCK_BUY + r as u32, can_buy),
                            (&sell_t, A_STOCK_SELL + r as u32, can_sell),
                            (&view_t, A_STOCK_VIEW + r as u32, true),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            ui.add(
                                root,
                                Kind::Button {
                                    text: label_t.clone(),
                                    font: btn_font,
                                    action,
                                    enabled,
                                    hotkey: None,
                                },
                                at(bx0 + k as f32 * (bw + bgap), ry, bw, rowh - 2.0),
                            );
                        }
                    } else {
                        ui.add(
                            root,
                            Kind::Button {
                                text: view_t.clone(),
                                font: btn_font,
                                action: A_STOCK_VIEW + r as u32,
                                enabled: true,
                                hotkey: None,
                            },
                            at(bx0, ry, bw, rowh - 2.0),
                        );
                    }
                }
            }
        } else {
            ui.add(
                root,
                Kind::Label {
                    text: self.texts.a("No game state to show."),
                    font: font((14.0 * s).max(10.0)),
                    color: MUTED,
                    align: Align::Left,
                    wrap: false,
                },
                at(px, py, pw, 24.0 * s),
            );
        }

        // Personal / Corporate tab buttons below the notepad (`Bar 1904` & `Bar 1905`).
        let tab_font = font((11.5 * s).max(9.0));
        for (rect, label_t, action, enabled) in [
            (
                art.r_tab_personal,
                self.texts.a("Personal"),
                A_STOCK_TAB_PERSONAL,
                self.stock_tab != 0,
            ),
            (
                art.r_tab_corporate,
                self.texts.a("Corporate"),
                A_STOCK_TAB_CORPORATE,
                self.stock_tab != 1,
            ),
        ] {
            ui.add(
                root,
                Kind::Button {
                    text: label_t,
                    font: tab_font,
                    action,
                    enabled,
                    hotkey: None,
                },
                at(rect[0] * s, rect[1] * s, rect[2] * s, rect[3] * s),
            );
        }

        // Bottom `odbottom.imb` bar.
        if let Some(p) = self.dialog_art.detail.odbottom_bg {
            ui.add(
                root,
                Kind::Image {
                    sprite: p.sprite,
                    fit: Fit::Stretch,
                },
                at(p.x as f32 * s, p.y as f32 * s, p.w * s, p.h * s),
            );
        }
        let [sx, sy, sw, sh] = art.r_status;
        ui.add(
            root,
            Kind::Label {
                text: self
                    .texts
                    .a("* = human chairman   Click a row for the company report   Esc: close"),
                font: font((12.0 * s).max(9.0)),
                color: TEXT,
                align: Align::Left,
                wrap: false,
            },
            at(sx * s, sy * s, sw * s, sh * s),
        );

        let [ox, oy, ow, oh] = art.r_ok;
        let ok_rect = at(ox * s, oy * s, ow * s, oh * s);
        let ok_str = self.texts.get(lng::OK, "OK");
        if let Some((up, down)) = self.dialog_art.detail.odbottom_ok {
            ui.add(
                root,
                Kind::Icon {
                    up: Some(up),
                    down: Some(down),
                    fallback: ok_str.clone(),
                    font: font((12.0 * s).max(9.0)),
                    action: A_CLOSE,
                    on: false,
                },
                ok_rect,
            );
            ui.add(
                root,
                Kind::Label {
                    text: ok_str,
                    font: game_font(1).with_size((13.0 * s).max(9.0)),
                    color: Color::rgb(0x1e, 0x16, 0x0a),
                    align: Align::Center,
                    wrap: false,
                },
                ok_rect,
            );
        } else {
            ui.add(
                root,
                Kind::Button {
                    text: ok_str,
                    font: font((12.0 * s).max(9.0)),
                    action: A_CLOSE,
                    enabled: true,
                    hotkey: None,
                },
                ok_rect,
            );
        }
    }
}
