//! Original `.win`/`.imb` artwork rendering for the 4 full-screen list windows:
//! `statnlst.win` (Stations), `trainlst.win` (Trains), `playrlst.win` (Players), and
//! `compnlst.win` (Companies).

use rt2_ui::{Align, Anchor, Color, Fill, Fit, Kind, Layout, Ui, game_font};

use super::info_ui::{
    A_CLOSE, A_INFO_FLEET, A_LIST_TAB, A_ROW, Content, InfoScreen, draw_table, font,
};
use super::{AppState, Screen};
use crate::text::id as lng;

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const INK: Color = Color::rgb(0x1e, 0x16, 0x0a);
const BG: Color = Color::rgb(0x10, 0x10, 0x10);

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

impl AppState {
    /// Renders `InfoScreen::{Stations, Trains, Players, Companies}` over the original `*lst.win`
    /// and `odbottom.imb` artwork, scaling `1024x768` uniformly to `ui.logical_size()`.
    pub(super) fn build_list_art(&mut self, ui: &mut Ui, screen: InfoScreen) {
        let Some(frame) = self.dialog_art.list.frame else {
            return;
        };
        let list_art = self.dialog_art.list.clone();
        let tab_idx = match screen {
            InfoScreen::Stations => 0,
            InfoScreen::Trains => 1,
            InfoScreen::Players => 2,
            InfoScreen::Companies => 3,
            _ => return,
        };

        let content = self.info_content(screen);
        let first_scroll = match &self.screen {
            Screen::Game(g) => g.info.map_or(0, |i| i.scroll),
            Screen::Menu(_) => 0,
        };

        let (cw, ch) = ui.logical_size();
        let s = (cw / 1024.0).min(ch / 768.0);
        let (w, h) = (1024.0 * s, 768.0 * s);

        ui.add_root(Kind::Panel { fill: Fill::Solid(BG) }, Layout::fill());
        let root = ui.add_root(
            Kind::Panel { fill: Fill::Solid(BG) },
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

        // Table inside the `r_list` control rectangle (`26, 26, 974, 686`).
        let [lx, ly, lw, lh] = list_art.r_list;
        let px = (lx + 10.0) * s;
        let py = (ly + 8.0) * s;
        let pw = (lw - 52.0) * s;
        let avail_h = (lh - 16.0) * s;

        let mut range_text = String::new();
        let mut title = String::new();
        if let Some(Content::List(list)) = content {
            title = list.title.clone();
            let n = list.table.rows.len();
            let rowh = (25.0 * s).max(16.0);
            let size = (rowh * 0.62).max(10.0);
            let head_h = rowh + 4.0 * s;
            let visible = (((avail_h - head_h) / rowh).floor() as usize).max(3);
            let first = first_scroll.min(n.saturating_sub(visible));
            if let Screen::Game(g) = &mut self.screen
                && let Some(i) = &mut g.info
                && i.screen == screen
            {
                i.scroll = first;
            }
            let shown = visible.min(n - first);

            if list.table.rows.is_empty() {
                ui.add(
                    root,
                    Kind::Label {
                        text: self.texts.a("Nothing to show."),
                        font: font(16.0 * s),
                        color: MUTED,
                        align: Align::Left,
                        wrap: false,
                    },
                    at(px + 12.0 * s, py + 40.0 * s, pw, rowh),
                );
            }

            ui.add(
                root,
                Kind::List {
                    items: vec![String::new(); shown],
                    selected: None,
                    scroll: 0,
                    row_h: rowh,
                    font: font(size),
                    action: A_ROW,
                },
                at(px, py + head_h - 1.0 * s, pw, rowh * shown as f32 + 2.0 * s),
            );
            draw_table(
                ui,
                root,
                &list.table,
                px,
                py + 2.0 * s,
                pw,
                rowh,
                size,
                first,
                visible,
                false,
            );

            if n > 0 {
                let last = (first + shown).min(n);
                range_text = format!("{}-{} / {}", first + 1, last, n);
            }
        }

        // Bottom `odbottom.imb` active tab strip (`(0, 729)`).
        let tab_sprite = match tab_idx {
            0 => self.dialog_art.detail.odbottom_tabs,
            1 => self.dialog_art.detail.odbottom_tabs_trains,
            _ => list_art.tabs[tab_idx],
        };
        if let Some(p) = tab_sprite {
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
            ui.add(
                root,
                Kind::Hotspot {
                    action: A_LIST_TAB + i,
                    on: i == tab_idx as u32,
                    enabled: true,
                },
                at((2.0 + 45.0 * i as f32) * s, 730.0 * s, 45.0 * s, 35.0 * s),
            );
        }

        // Status label (`185, 733, 653, 35`) + optional Fleet Manager button on Trains.
        let has_fleet = matches!(screen, InfoScreen::Trains);
        let [sx, sy, sw, sh] = list_art.r_status;
        let status_w = if has_fleet { sw - 180.0 } else { sw - 12.0 };
        let status = if range_text.is_empty() {
            title
        } else {
            format!("{title}  ({range_text})")
        };
        if !status.is_empty() {
            ui.add(
                root,
                Kind::Label {
                    text: status,
                    font: game_font(8).with_size(14.0 * s),
                    color: TEXT,
                    align: Align::Left,
                    wrap: false,
                },
                at((sx + 6.0) * s, (sy + 4.0) * s, status_w * s, (sh - 6.0) * s),
            );
        }
        if has_fleet {
            ui.add(
                root,
                Kind::Button {
                    text: self.texts.a("Fleet Manager..."),
                    font: font(12.0 * s),
                    action: A_INFO_FLEET,
                    enabled: true,
                    hotkey: None,
                },
                at((sx + sw - 172.0) * s, (sy + 2.0) * s, 166.0 * s, (sh - 5.0) * s),
            );
        }

        // Bottom-right map/close (`916, 735, 40, 31`) and OK (`962, 734, 60, 32`) buttons.
        let mut add_btn = |r: [f32; 4],
                           sprites: Option<(rt2_ui::SpriteId, rt2_ui::SpriteId)>,
                           action: u32,
                           fallback: String| {
            let rect = at(r[0] * s, r[1] * s, r[2] * s, r[3] * s);
            if let Some((up, down)) = sprites {
                ui.add(
                    root,
                    Kind::Icon {
                        up: Some(up),
                        down: Some(down),
                        fallback,
                        font: game_font(1).with_size(13.0 * s),
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
                        font: game_font(1).with_size(13.0 * s),
                        action,
                        enabled: true,
                        hotkey: None,
                    },
                    rect,
                );
            }
        };
        add_btn(
            list_art.r_btn_map,
            list_art.btn_map,
            A_CLOSE,
            String::new(),
        );
        let ok_str = self.texts.get(lng::OK, "OK");
        let ok_sprites = list_art.btn_ok.or(self.dialog_art.detail.odbottom_ok);
        add_btn(list_art.r_btn_ok, ok_sprites, A_CLOSE, ok_str.clone());
        let [ox, oy, ow, oh] = list_art.r_btn_ok;
        ui.add(
            root,
            Kind::Label {
                text: ok_str,
                font: game_font(1).with_size(14.0 * s),
                color: INK,
                align: Align::Center,
                wrap: false,
            },
            at(ox * s, oy * s, ow * s, oh * s),
        );
    }
}
