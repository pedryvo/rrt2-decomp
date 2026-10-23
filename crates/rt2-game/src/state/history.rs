//! The event history (docs/ui.md "Histórico de eventos e notificações"): a window listing the
//! messages and news of the session, newest first, with the game date, a filter by category and
//! a click that centers the map on the place the event is about.
//!
//! This is a feature of the clone. It adds no event source of its own: every entry is a line the
//! activity box already receives (`AppState::add_event`, fed by `World::take_messages()` in
//! `poll_world` and by the player's own actions). The history lives in memory for the session
//! (`GameState::news`, `HISTORY_CAP` entries) and is not saved: the save format is the
//! original's and has no place for it.

use rt2_ui::{Align, Anchor, Color, Fill, Kind, Layout, Ui, game_font};

use super::dialogs::Dialog;
use super::mainwin::NewsItem;
use super::{AppState, Screen};
use crate::notices::Category;
use crate::scenario_info::wrap_text;

pub(super) const A_HIST_OPEN: u32 = 13000;
pub(super) const A_HIST_GOTO: u32 = 13001;
pub(super) const A_HIST_LIST: u32 = 13002;
/// Filter buttons: `A_HIST_FILTER` is "All", then one per [`Category`].
pub(super) const A_HIST_FILTER: u32 = 13010;
const A_HIST_LAST: u32 = A_HIST_FILTER + 4;

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd8);

pub(super) fn owns(a: u32) -> bool {
    (A_HIST_OPEN..=A_HIST_LAST).contains(&a)
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

/// Rough width of a text in units of one Latin character (CJK glyphs are about twice as wide).
fn units(s: &str) -> f32 {
    s.chars()
        .map(|c| if c.is_ascii() { 1.0 } else { 1.9 })
        .sum()
}

/// `s` cut to at most `max` character units, with an ellipsis.
fn fit(s: &str, max: f32) -> String {
    if units(s) <= max {
        return s.to_owned();
    }
    let mut out = String::new();
    let mut w = 0.0;
    for c in s.chars() {
        let cw = if c.is_ascii() { 1.0 } else { 1.9 };
        if w + cw > max - 3.0 {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push_str("...");
    out
}

/// The entries shown for `filter` (0 = all, else category index + 1), newest first.
fn filtered(news: &[NewsItem], filter: usize) -> Vec<&NewsItem> {
    news.iter()
        .filter(|n| filter == 0 || n.category.index() + 1 == filter)
        .collect()
}

impl AppState {
    /// Opens the history window (the "History" button of the activity box).
    pub(super) fn open_history(&mut self) {
        self.push_dialog(Dialog::History {
            filter: 0,
            sel: None,
            scroll: 0,
        });
    }

    /// The tile an event of the simulation is about: the place of a town message, the departure
    /// of a connection.
    pub(super) fn message_pos(&self, m: &rt2_sim::message::GameMessage) -> Option<(i32, i32)> {
        use rt2_sim::message::GameMessage as M;
        let place = match m {
            M::FirstOfType { place, .. } | M::TownBonusOffer { place, .. } => *place,
            M::CompanyConnected { from, to, .. } => {
                if *from > 0 {
                    *from
                } else {
                    *to
                }
            }
            _ => 0,
        };
        if place <= 0 {
            return None;
        }
        self.with_info(|i| i.place_tile(place.cast_unsigned()))
            .flatten()
    }

    /// The first row of the history list as the window scrolled it (the list keeps its own
    /// offset between rebuilds).
    fn history_scroll(&self) -> usize {
        self.ui
            .draw_order()
            .into_iter()
            .find_map(|(_, id)| match &self.ui.node(id).kind {
                Kind::List {
                    action: A_HIST_LIST,
                    scroll,
                    ..
                } => Some(*scroll),
                _ => None,
            })
            .unwrap_or(0)
    }

    /// The entry selected in the open history window.
    fn history_selected(&self) -> Option<&NewsItem> {
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        let Some(Dialog::History { filter, sel, .. }) = g.dialogs.last() else {
            return None;
        };
        filtered(&g.news, *filter).get((*sel)?).copied()
    }

    #[cfg(test)]
    pub(super) fn history_selected_has_no_pos_for_test(&self) -> bool {
        self.history_selected().is_some_and(|n| n.pos.is_none())
    }

    /// A click on a row: select it and, when the event has a position, center the camera there.
    pub(super) fn history_event(&mut self, action: u32, row: usize) -> bool {
        if action != A_HIST_LIST {
            return false;
        }
        let scroll = self.history_scroll();
        if let Screen::Game(g) = &mut self.screen
            && let Some(Dialog::History {
                sel, scroll: sc, ..
            }) = g.dialogs.last_mut()
        {
            *sel = Some(row);
            *sc = scroll;
        }
        if let Some((x, y)) = self.history_selected().and_then(|n| n.pos) {
            self.center_camera(x, y);
        }
        self.rebuild_ui();
        true
    }

    pub(super) fn center_camera(&mut self, x: i32, y: i32) {
        if let Screen::Game(g) = &mut self.screen {
            g.camera.target = rt2_render::camera::Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
        }
    }

    /// The buttons of the history window.
    pub(super) fn history_activated(&mut self, a: u32) {
        match a {
            A_HIST_OPEN => self.open_history(),
            // Enter / second click on a row, or the button: show the place and close.
            A_HIST_GOTO | A_HIST_LIST => {
                if let Some((x, y)) = self.history_selected().and_then(|n| n.pos) {
                    self.center_camera(x, y);
                    self.close_dialog();
                }
            }
            a if (A_HIST_FILTER..=A_HIST_LAST).contains(&a) => {
                if let Screen::Game(g) = &mut self.screen
                    && let Some(Dialog::History {
                        filter,
                        sel,
                        scroll,
                    }) = g.dialogs.last_mut()
                {
                    *filter = (a - A_HIST_FILTER) as usize;
                    *sel = None;
                    *scroll = 0;
                }
                self.rebuild_ui();
            }
            _ => {}
        }
    }

    pub(super) fn build_history(
        &self,
        ui: &mut Ui,
        filter: usize,
        sel: Option<usize>,
        scroll: usize,
    ) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let t = &self.texts;
        let (cw, ch) = ui.logical_size();
        let dz = (ch / 620.0).clamp(1.0, 2.0);
        let w = (860.0 * dz).min(cw - 16.0);
        let h = (560.0 * dz).min(ch - 16.0);
        let root = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, w, h),
        );
        let pad = 14.0 * dz;
        let fs = (15.0 * dz).round();
        let font = game_font(8).with_size(fs);
        let items = filtered(&g.news, filter);
        let title_font = game_font(3);
        let title_font = title_font.with_size((title_font.size * dz * 0.7).round());
        ui.add(
            root,
            Kind::Label {
                text: t.a("Event history").to_uppercase(),
                font: title_font,
                color: HEAD,
                align: Align::Left,
                wrap: false,
            },
            at(pad, pad * 0.6, w * 0.6, 28.0 * dz),
        );
        ui.add(
            root,
            Kind::Label {
                text: t.af("%1 events", &[items.len().to_string()]),
                font,
                color: MUTED,
                align: Align::Right,
                wrap: false,
            },
            at(w * 0.6, pad * 0.6 + 4.0 * dz, w * 0.4 - pad, 24.0 * dz),
        );
        // The filter row: All, then the categories.
        let bf = game_font(1).with_size((game_font(1).size * dz * 0.8).round());
        let fy = 44.0 * dz;
        let fh = 28.0 * dz;
        let gap = 4.0 * dz;
        let fw = (w - 2.0 * pad - 4.0 * gap) / 5.0;
        for i in 0..5usize {
            let text = if i == 0 {
                t.a("All")
            } else {
                t.a(Category::ALL[i - 1].english())
            };
            ui.add(
                root,
                Kind::Toggle {
                    text,
                    font: bf,
                    action: A_HIST_FILTER + i as u32,
                    on: i == filter,
                    enabled: true,
                },
                at(pad + i as f32 * (fw + gap), fy, fw, fh),
            );
        }
        // The list.
        let foot_h = 40.0 * dz;
        let detail_h = 92.0 * dz;
        let ly = fy + fh + 8.0 * dz;
        let lh = h - ly - detail_h - foot_h - 16.0 * dz;
        let lw = w - 2.0 * pad;
        let max_units = (lw - 16.0 * dz) / (fs * 0.5);
        let list_items: Vec<String> = items
            .iter()
            .map(|n| {
                let cat = t.a(n.category.english());
                let short = cat.split_whitespace().next().unwrap_or("").to_owned();
                fit(&format!("{}   [{}]   {}", n.date, short, n.text), max_units)
            })
            .collect();
        if list_items.is_empty() {
            ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Solid(INSET),
                },
                at(pad, ly, lw, lh),
            );
            ui.add(
                root,
                Kind::Label {
                    text: t.a("No events yet."),
                    font,
                    color: MUTED,
                    align: Align::Center,
                    wrap: false,
                },
                at(pad, ly, lw, lh),
            );
        } else {
            let list = ui.add(
                root,
                Kind::List {
                    items: list_items,
                    selected: sel.filter(|s| *s < items.len()),
                    scroll,
                    row_h: (fs * 1.45).round(),
                    font,
                    action: A_HIST_LIST,
                },
                at(pad, ly, lw, lh),
            );
            ui.set_focus(Some(list));
        }
        // The detail of the selected entry.
        let dy = ly + lh + 8.0 * dz;
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(pad, dy, lw, detail_h),
        );
        let selected = sel.and_then(|s| items.get(s)).copied();
        let mut lines = Vec::new();
        if let Some(n) = selected {
            lines.push(format!("{}  -  {}", n.date, t.a(n.category.english())));
            let chars = ((lw - 16.0 * dz) / (fs * 0.5)).floor().max(10.0) as usize;
            lines.extend(wrap_text(&n.detail.replace("\n\n", " "), chars));
            if n.pos.is_none() {
                lines.push(t.a("No location for this event."));
            }
        } else {
            lines.push(t.a("Click an event to center the map on it."));
        }
        ui.add(
            root,
            Kind::Text {
                lines,
                scroll: 0,
                row_h: (fs * 1.3).round(),
                font,
                color: TEXT,
            },
            at(
                pad + 8.0 * dz,
                dy + 4.0 * dz,
                lw - 16.0 * dz,
                detail_h - 8.0 * dz,
            ),
        );
        // The buttons.
        let by = h - foot_h - 4.0 * dz + 8.0 * dz;
        let bw = 190.0 * dz;
        let bh = 30.0 * dz;
        let bfont = game_font(1).with_size((game_font(1).size * dz * 0.85).round());
        ui.add(
            root,
            Kind::Button {
                text: t.a("Show on map"),
                font: bfont,
                action: A_HIST_GOTO,
                enabled: selected.is_some_and(|n| n.pos.is_some()),
                hotkey: None,
            },
            at(w - 2.0 * pad - 2.0 * bw - gap, by - 4.0 * dz, bw, bh),
        );
        ui.add(
            root,
            Kind::Button {
                text: t.a("Close"),
                font: bfont,
                action: super::dialogs::A_DLG_CLOSE,
                enabled: true,
                hotkey: None,
            },
            at(w - pad - bw, by - 4.0 * dz, bw, bh),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_cuts_long_text_with_an_ellipsis() {
        assert_eq!(fit("short", 20.0), "short");
        let cut = fit("a long line of text that does not fit", 12.0);
        assert!(cut.ends_with("..."));
        assert!(units(&cut) <= 12.0);
        // Wide glyphs count double.
        assert!(units(&fit("场景与经济的消息一二三四五六七", 10.0)) <= 10.0);
    }
}
