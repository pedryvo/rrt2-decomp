//! The information screens' widgets and input (docs/ui.md "Telas de informacao"): finance,
//! stock market, trains, stations, cities, industries and the detail pages. Read-only: the screens
//! only look at the simulation state through [`crate::info::Info`] and never enqueue a command.
//!
//! The layout follows the original windows' logic (`docs/formats/ui.md`): list screens are a full
//! panel with one big table (`trainlst.win`, `statnlst.win`, `compnlst.win`: a 974x686 list inside
//! a 1024x768 window), detail screens are two panels side by side (`compndtl.win`: 265x454 at x 216
//! and 539; `traindtl.win`: cars on the left, route on the right) under a bottom toolbar. Here the
//! sizes are not fixed: the panel fills the screen below the HUD (up to a maximum width), the row
//! height shrinks to fit a small logical screen (a 2x UI scale on 1080p) and the two columns fall
//! back to one on a narrow one. The panel art is the skin's window panel.

use winit::keyboard::KeyCode;

use rt2_ui::{Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::dialogs::fin::A_FINACT;
use super::{AppState, Screen};
use crate::info::{Block, Info, ListPage, Page, Pick, TOTAL, Table};
use crate::text::id as lng;

pub(super) const A_TAB: u32 = 100;
pub(super) const A_CLOSE: u32 = 110;
pub(super) const A_BACK: u32 = 111;
pub(super) const A_PREV_CO: u32 = 112;
pub(super) const A_NEXT_CO: u32 = 113;
pub(super) const A_ROW: u32 = 120;
pub(super) const A_TOOL: u32 = 130;

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const NEG: Color = Color::rgb(0xf0, 0x8c, 0x7a);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xc8);
const DIM: Color = Color(0, 0, 0, 120);

/// Widest panel in logical units.
const MAX_PANEL_W: f32 = 1400.0;
/// Below this panel width the two columns become one.
const NARROW: f32 = 860.0;
const PAD: f32 = 30.0;
const HUD_H: f32 = 46.0;

/// Names of the six top-level screens, in tab order.
pub(super) const TABS: [InfoScreen; 6] = [
    InfoScreen::Finance(0),
    InfoScreen::Stock,
    InfoScreen::Trains,
    InfoScreen::Stations,
    InfoScreen::Cities,
    InfoScreen::Industries,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InfoScreen {
    /// A company by pool slot (0 = the player's / first company).
    Finance(u32),
    Stock,
    Trains,
    TrainDetail(u32),
    Stations,
    StationDetail(u32),
    Cities,
    CityDetail(u32),
    Industries,
    IndustryDetail(u32),
    Territory(u32),
}

impl InfoScreen {
    fn tab(self) -> usize {
        match self {
            InfoScreen::Finance(_) => 0,
            InfoScreen::Stock => 1,
            InfoScreen::Trains | InfoScreen::TrainDetail(_) => 2,
            InfoScreen::Stations | InfoScreen::StationDetail(_) => 3,
            InfoScreen::Cities | InfoScreen::CityDetail(_) | InfoScreen::Territory(_) => 4,
            InfoScreen::Industries | InfoScreen::IndustryDetail(_) => 5,
        }
    }

    /// The list a detail page returns to.
    fn parent(self) -> Option<InfoScreen> {
        Some(match self {
            InfoScreen::TrainDetail(_) => InfoScreen::Trains,
            InfoScreen::StationDetail(_) => InfoScreen::Stations,
            InfoScreen::CityDetail(_) | InfoScreen::Territory(_) => InfoScreen::Cities,
            InfoScreen::IndustryDetail(_) => InfoScreen::Industries,
            _ => return None,
        })
    }

    /// Parses a `--screen` name of the screenshot command.
    pub(super) fn from_name(name: &str) -> Option<InfoScreen> {
        Some(match name {
            "finance" => InfoScreen::Finance(0),
            "stock" => InfoScreen::Stock,
            "trains" => InfoScreen::Trains,
            "train" => InfoScreen::TrainDetail(0),
            "stations" => InfoScreen::Stations,
            "station" => InfoScreen::StationDetail(0),
            "cities" => InfoScreen::Cities,
            "city" => InfoScreen::CityDetail(0),
            "industries" => InfoScreen::Industries,
            "industry" => InfoScreen::IndustryDetail(0),
            "territory" => InfoScreen::Territory(0),
            _ => return None,
        })
    }
}

/// What is open: the screen and the first visible row of a list.
#[derive(Clone, Copy, Debug)]
pub(super) struct InfoState {
    pub screen: InfoScreen,
    pub scroll: usize,
    /// Day of the last build, to refresh the numbers when the simulated day changes.
    pub day: i32,
}

enum Content {
    List(ListPage),
    Page(Page),
}

pub(super) fn font(size: f32) -> FontStyle {
    game_font(2).with_size(size)
}

fn label(text: &str, font: FontStyle, color: Color, align: Align) -> Kind {
    Kind::Label {
        text: text.to_owned(),
        font,
        color,
        align,
        wrap: false,
    }
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

/// Rows a wrapped note takes at `width` (Cinzel is about 0.75 em per character).
fn note_rows(text: &str, width: f32, size: f32) -> f32 {
    let per_line = (width / (size * 0.72)).max(8.0);
    (text.chars().count() as f32 / per_line).ceil().max(1.0)
}

fn block_rows(b: &Block, width: f32, size: f32) -> f32 {
    match b {
        Block::Facts(f) => f.len() as f32,
        Block::Table { table, .. } => {
            let head = table.cols.iter().any(|c| !c.head.is_empty());
            1.3 + f32::from(head) + table.rows.len() as f32
        }
        Block::Note(t) => note_rows(t, width, size),
    }
}

fn column_rows(blocks: &[Block], width: f32, size: f32) -> f32 {
    let gaps = blocks.len().saturating_sub(1) as f32 * 0.6;
    blocks
        .iter()
        .map(|b| block_rows(b, width, size))
        .sum::<f32>()
        + gaps
}

fn max_cols(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|b| match b {
            Block::Table { table, .. } => table.cols.len(),
            Block::Facts(_) => 2,
            Block::Note(_) => 1,
        })
        .max()
        .unwrap_or(1)
}

/// Draws a header row and the rows of a table at `(x, y)`; returns the y below it. `visible`
/// caps the rows drawn (a last "+N more" row replaces the overflow); `first` skips rows.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_table(
    ui: &mut Ui,
    parent: NodeId,
    table: &Table,
    x: f32,
    y: f32,
    w: f32,
    rowh: f32,
    size: f32,
    first: usize,
    visible: usize,
    ellipsis: bool,
) -> f32 {
    let total: f32 = table.cols.iter().map(|c| c.weight).sum::<f32>().max(0.1);
    let mut xs = Vec::new();
    let mut cx = x;
    for c in &table.cols {
        let cw = w * c.weight / total;
        xs.push((cx, cw));
        cx += cw;
    }
    let f = font(size);
    let headed = table.cols.iter().any(|c| !c.head.is_empty());
    for (c, &(cx, cw)) in table.cols.iter().zip(&xs) {
        let a = if c.right { Align::Right } else { Align::Left };
        ui.add(
            parent,
            label(&c.head, f, HEAD, a),
            at(cx + 8.0, y, (cw - 16.0).max(1.0), rowh),
        );
    }
    let mut yy = y + if headed { rowh + 2.0 } else { 2.0 };
    let n = table.rows.len().saturating_sub(first);
    let (shown, more) = if ellipsis && n > visible && visible > 0 {
        (visible - 1, n - (visible - 1))
    } else {
        (n.min(visible), 0)
    };
    for (ri, row) in table.rows.iter().enumerate().skip(first).take(shown) {
        let total = table.ids.get(ri) == Some(&TOTAL);
        if total {
            ui.add(
                parent,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x6a, 0x52, 0x20, 0x58)),
                },
                at(x + 2.0, yy, w - 4.0, rowh),
            );
        }
        for ((cell, c), &(cx, cw)) in row.iter().zip(&table.cols).zip(&xs) {
            let a = if c.right { Align::Right } else { Align::Left };
            let neg = cell.starts_with("-$");
            let color = if neg {
                NEG
            } else if total {
                HEAD
            } else {
                TEXT
            };
            ui.add(
                parent,
                label(cell, f, color, a),
                at(cx + 8.0, yy, (cw - 16.0).max(1.0), rowh),
            );
        }
        yy += rowh;
    }
    if more > 0 {
        ui.add(
            parent,
            label(&format!("... {more} more"), f, MUTED, Align::Left),
            at(x + 8.0, yy, w - 16.0, rowh),
        );
        yy += rowh;
    }
    yy
}

/// Draws a column of blocks starting at `(x, y)` inside `bottom`; returns nothing (clipped by the
/// row budget of each block).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_blocks(
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
    let f = font(size);
    let mut y = y0;
    for b in blocks {
        let room = ((bottom - y) / rowh).floor().max(0.0) as usize;
        if room == 0 {
            break;
        }
        match b {
            Block::Facts(facts) => {
                ui.add(
                    parent,
                    Kind::Panel {
                        fill: Fill::Solid(INSET),
                    },
                    at(x, y, w, rowh * facts.len().min(room) as f32 + 4.0),
                );
                for (i, (k, v)) in facts.iter().take(room).enumerate() {
                    let yy = y + 2.0 + i as f32 * rowh;
                    ui.add(
                        parent,
                        label(k, f, MUTED, Align::Left),
                        at(x + 10.0, yy, w * 0.55 - 10.0, rowh),
                    );
                    let neg = v.starts_with("-$");
                    ui.add(
                        parent,
                        label(v, f, if neg { NEG } else { TEXT }, Align::Right),
                        at(x + w * 0.45, yy, w * 0.55 - 10.0, rowh),
                    );
                }
                y += rowh * facts.len().min(room) as f32 + 4.0;
            }
            Block::Table { title, table } => {
                ui.add(
                    parent,
                    label(title, font(size * 1.1).with_shadow(), TEXT, Align::Left),
                    at(x + 4.0, y, w - 8.0, rowh * 1.3),
                );
                let top = y + rowh * 1.3;
                let head = table.cols.iter().any(|c| !c.head.is_empty());
                let rows_room = (((bottom - top) / rowh).floor() as usize).saturating_sub(1);
                let rows = rows_room.min(table.rows.len().max(1));
                let h = rowh * (rows as f32 + f32::from(head)) + 6.0;
                ui.add(
                    parent,
                    Kind::Panel {
                        fill: Fill::Solid(INSET),
                    },
                    at(x, top, w, h),
                );
                y = draw_table(
                    ui,
                    parent,
                    table,
                    x,
                    top + 1.0,
                    w,
                    rowh,
                    size,
                    0,
                    rows_room,
                    true,
                ) + 6.0;
            }
            Block::Note(text) => {
                let lines = note_rows(text, w, size);
                ui.add(
                    parent,
                    Kind::Label {
                        text: text.clone(),
                        font: f,
                        color: MUTED,
                        align: Align::Left,
                        wrap: true,
                    },
                    at(x + 4.0, y, w - 8.0, rowh * lines),
                );
                y += rowh * lines;
            }
        }
        y += rowh * 0.6;
    }
}

impl AppState {
    fn info_content(&self, screen: InfoScreen) -> Option<Content> {
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
        Some(match screen {
            InfoScreen::Finance(c) => {
                let slot = if c == 0 {
                    info.player_company().unwrap_or(0)
                } else {
                    c
                };
                Content::Page(info.finance_page(slot))
            }
            InfoScreen::Stock => Content::List(info.stock_market()),
            InfoScreen::Trains => Content::List(info.trains_list()),
            InfoScreen::TrainDetail(s) => Content::Page(info.train_page(first_if_zero(
                s,
                save.trains.ordered().next().map(|r| r.slot as u32),
            ))),
            InfoScreen::Stations => Content::List(info.stations_list()),
            InfoScreen::StationDetail(s) => Content::Page(info.station_page(first_if_zero(
                s,
                save.stations.ordered().next().map(|r| r.slot as u32),
            ))),
            InfoScreen::Cities => Content::List(info.places_list()),
            InfoScreen::CityDetail(s) => Content::Page(info.place_page(first_if_zero(
                s,
                save.places.ordered().next().map(|r| r.slot as u32),
            ))),
            InfoScreen::Industries => Content::List(info.industries_list()),
            InfoScreen::IndustryDetail(s) => Content::Page(
                info.industry_page(first_if_zero(
                    s,
                    save.objects
                        .ordered()
                        .find(|r| !matches!(r.data.get(7), Some(0 | 0x12 | 0x27)))
                        .map(|r| r.slot as u32),
                )),
            ),
            InfoScreen::Territory(s) => Content::Page(info.territory_page(first_if_zero(
                s,
                save.territories.ordered().next().map(|r| r.slot as u32),
            ))),
        })
    }

    /// The company slots in list order (for the finance screen's previous / next).
    fn company_slots(&self) -> Vec<u32> {
        match &self.screen {
            Screen::Game(g) => g
                .world
                .save_ref()
                .map(|s| s.companies.ordered().map(|r| r.slot as u32).collect())
                .unwrap_or_default(),
            Screen::Menu(_) => Vec::new(),
        }
    }

    fn resolved_company(&self, c: u32) -> u32 {
        if c != 0 {
            return c;
        }
        let Screen::Game(g) = &self.screen else {
            return 0;
        };
        let (Some(save), Some(data)) = (g.world.save_ref(), self.data.as_ref()) else {
            return 0;
        };
        let d = g.world.date();
        Info {
            save,
            data,
            texts: &self.texts,
            year: i32::from(d.year),
            month: i32::from(d.month),
            day: i32::from(d.day),
            minutes: g.world.minutes(),
        }
        .player_company()
        .unwrap_or(0)
    }

    pub(super) fn info_screen(&self) -> Option<InfoScreen> {
        match &self.screen {
            Screen::Game(g) => g.info.map(|i| i.screen),
            Screen::Menu(_) => None,
        }
    }

    pub(super) fn open_info(&mut self, screen: InfoScreen) {
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        let day = g.world.minutes() / 1440;
        let keep = g
            .info
            .filter(|i| i.screen == screen)
            .map_or(0, |i| i.scroll);
        g.info = Some(InfoState {
            screen,
            scroll: keep,
            day,
        });
        self.rebuild_ui();
    }

    pub(super) fn close_info(&mut self) {
        if let Screen::Game(g) = &mut self.screen
            && g.info.take().is_some()
        {
            self.rebuild_ui();
        }
    }

    /// Opens (or, if it is the one open, closes) the top-level screen `tab`.
    pub(super) fn toggle_tab(&mut self, tab: usize) {
        let screen = TABS[tab];
        match self.info_screen() {
            Some(s) if s.tab() == tab && s == screen => self.close_info(),
            _ => self.open_info(screen),
        }
    }

    /// Opens the page for what a map click found.
    pub(super) fn open_pick(&mut self, pick: Pick) {
        // With a company the construction windows take over: the station window (Buy), the
        // industry window (Buy / Sell) and the territory rights.
        if self.player().is_some() {
            match pick {
                Pick::Station(s) => return self.open_station(s as i32),
                Pick::Industry(s) => return self.open_industry(s as i32),
                Pick::Territory(s) => {
                    return self.push_dialog(super::dialogs::Dialog::Rights {
                        territory: s as i32,
                    });
                }
                Pick::Place(_) => {}
            }
        }
        self.open_info(match pick {
            Pick::Industry(s) => InfoScreen::IndustryDetail(s),
            Pick::Station(s) => InfoScreen::StationDetail(s),
            Pick::Place(s) => InfoScreen::CityDetail(s),
            Pick::Territory(s) => InfoScreen::Territory(s),
        });
    }

    /// Screenshots: opens a screen by name; returns false for an unknown name.
    pub fn show_info_screen(&mut self, name: &str) -> bool {
        match InfoScreen::from_name(name) {
            Some(s) => {
                self.open_info(s);
                true
            }
            None => false,
        }
    }

    /// Key handling while a screen is open; true if the key was used.
    pub(super) fn info_key(&mut self, code: KeyCode) -> bool {
        let Some(screen) = self.info_screen() else {
            return false;
        };
        let b = self.binds.clone();
        let tabs = [
            &b.finance,
            &b.stock,
            &b.trains,
            &b.stations,
            &b.cities,
            &b.industries,
        ];
        if let Some(i) = tabs.iter().position(|k| k.matches(code, false)) {
            self.toggle_tab(i);
            return true;
        }
        match code {
            KeyCode::Escape => self.close_info(),
            KeyCode::Backspace => self.info_back(screen),
            KeyCode::ArrowLeft => self.cycle_company(-1),
            KeyCode::ArrowRight => self.cycle_company(1),
            KeyCode::PageUp => self.info_scroll(-10),
            KeyCode::PageDown => self.info_scroll(10),
            KeyCode::Home => self.info_scroll(i32::MIN / 2),
            KeyCode::End => self.info_scroll(i32::MAX / 2),
            _ => return false,
        }
        true
    }

    fn info_back(&mut self, screen: InfoScreen) {
        if let Some(p) = screen.parent() {
            self.open_info(p);
        }
    }

    fn cycle_company(&mut self, dir: i32) {
        let Some(InfoScreen::Finance(c)) = self.info_screen() else {
            return;
        };
        let slots = self.company_slots();
        if slots.is_empty() {
            return;
        }
        let cur = self.resolved_company(c);
        let i = slots.iter().position(|&s| s == cur).unwrap_or(0) as i32;
        let n = slots.len() as i32;
        let next = slots[(i + dir).rem_euclid(n) as usize];
        self.open_info(InfoScreen::Finance(next));
    }

    /// Scrolls a list screen by `rows` (clamped when the screen is rebuilt).
    pub(super) fn info_scroll(&mut self, rows: i32) {
        if let Screen::Game(g) = &mut self.screen
            && let Some(i) = &mut g.info
        {
            let next = (i.scroll as i64 + i64::from(rows)).max(0) as usize;
            if next != i.scroll {
                i.scroll = next;
                self.rebuild_ui();
            }
        }
    }

    /// UI action of the info widgets; false if `action` is not one of them.
    pub(super) fn info_activated(&mut self, action: u32) -> bool {
        let Some(screen) = self.info_screen() else {
            return self.tool_activated(action);
        };
        match action {
            a if (A_TAB..A_TAB + 6).contains(&a) => self.toggle_tab((a - A_TAB) as usize),
            A_CLOSE => self.close_info(),
            A_BACK => self.info_back(screen),
            A_PREV_CO => self.cycle_company(-1),
            A_NEXT_CO => self.cycle_company(1),
            a if (A_FINACT..A_FINACT + 12).contains(&a) => {
                let company = match screen {
                    InfoScreen::Finance(c) => self.resolved_company(c),
                    _ => 0,
                };
                return self.fin_book_activated(a, company);
            }
            A_ROW => {
                // Enter on the selected row of a list.
                if let Some(row) = self.ui.focused().and_then(|f| self.ui.selected(f)) {
                    self.info_row(row);
                }
            }
            a => return self.tool_activated(a),
        }
        true
    }

    /// The HUD toolbar buttons (they stay under an open screen, so a tab button also works).
    fn tool_activated(&mut self, action: u32) -> bool {
        if (A_TOOL..A_TOOL + 6).contains(&action) {
            self.toggle_tab((action - A_TOOL) as usize);
            return true;
        }
        false
    }

    /// A list row was clicked (or entered): open its detail page.
    pub(super) fn info_row(&mut self, row: usize) {
        let Some(InfoState { screen, scroll, .. }) = (match &self.screen {
            Screen::Game(g) => g.info,
            Screen::Menu(_) => None,
        }) else {
            return;
        };
        let abs = scroll + row;
        let Some(Content::List(list)) = self.info_content(screen) else {
            return;
        };
        let Some(&id) = list.table.ids.get(abs) else {
            return;
        };
        let next = match screen {
            InfoScreen::Stock => InfoScreen::Finance(id),
            InfoScreen::Trains => InfoScreen::TrainDetail(id),
            InfoScreen::Stations => InfoScreen::StationDetail(id),
            InfoScreen::Cities => InfoScreen::CityDetail(id),
            InfoScreen::Industries => InfoScreen::IndustryDetail(id),
            _ => return,
        };
        self.open_info(next);
    }

    /// Re-renders the open screen when the simulated day changes (the numbers move).
    pub(super) fn info_refresh(&mut self) {
        let stale = match &self.screen {
            Screen::Game(g) => g.info.is_some_and(|i| i.day != g.world.minutes() / 1440),
            Screen::Menu(_) => false,
        };
        if stale {
            if let Screen::Game(g) = &mut self.screen
                && let Some(i) = &mut g.info
            {
                i.day = g.world.minutes() / 1440;
            }
            self.rebuild_ui();
        }
    }

    /// The open screen: a dimmer, the panel with the tabs, and the content.
    pub(super) fn build_info(&mut self, ui: &mut Ui) {
        let Some(screen) = self.info_screen() else {
            return;
        };
        let content = self.info_content(screen);
        let first_scroll = match &self.screen {
            Screen::Game(g) => g.info.map_or(0, |i| i.scroll),
            Screen::Menu(_) => 0,
        };
        let company_nav = matches!(screen, InfoScreen::Finance(_));
        let (cw, ch) = ui.logical_size();
        ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(DIM),
            },
            Layout::fill(),
        );
        let pw = (cw - 16.0).clamp(300.0, MAX_PANEL_W);
        let top = HUD_H + 8.0;
        let ph = (ch - top - 10.0).max(200.0);
        let panel = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Top, pw, ph).offset(0.0, top),
        );
        let inner_w = pw - 2.0 * PAD;

        // Title row: title and subtitle on the left, navigation on the right.
        let (title, subtitle) = match &content {
            Some(Content::List(l)) => (l.title.clone(), String::new()),
            Some(Content::Page(p)) => (p.title.clone(), p.subtitle.clone()),
            None => (self.texts.a("Not available"), String::new()),
        };
        let nav = if company_nav {
            2.0 * 120.0 + 8.0
        } else if screen.parent().is_some() {
            120.0
        } else {
            0.0
        };
        let room = (inner_w - nav - 8.0).max(60.0);
        // The title takes what it needs (Cinzel 20 is about 15 px per character), the subtitle
        // the rest.
        let tw = (title.chars().count() as f32 * 16.0 + 12.0).min(room * 0.7);
        ui.add(
            panel,
            label(&title, game_font(6).with_shadow(), TEXT, Align::Left),
            at(PAD, 18.0, tw, 32.0),
        );
        if !subtitle.is_empty() {
            ui.add(
                panel,
                label(&subtitle, font(15.0), MUTED, Align::Right),
                at(PAD + tw, 18.0, room - tw, 32.0),
            );
        }
        let navx = PAD + inner_w - nav;
        let nbtn = |ui: &mut Ui, text: String, action: u32, x: f32| {
            ui.add(
                panel,
                Kind::Button {
                    text,
                    font: font(14.0),
                    action,
                    enabled: true,
                    hotkey: None,
                },
                at(x, 17.0, 112.0, 30.0),
            );
        };
        if company_nav {
            nbtn(
                ui,
                self.texts.get(lng::PREVIOUS, "Previous"),
                A_PREV_CO,
                navx,
            );
            nbtn(
                ui,
                self.texts.get(lng::NEXT, "Next"),
                A_NEXT_CO,
                navx + 120.0,
            );
        } else if screen.parent().is_some() {
            nbtn(ui, self.texts.a("Back"), A_BACK, navx);
        }

        // Tabs.
        let names = [
            self.texts.get(lng::FINANCES, "Finances"),
            self.texts.a("Market"),
            self.texts.get(lng::TRAINS, "Trains"),
            self.texts.a("Stations"),
            self.texts.get(lng::CITIES, "Cities"),
            self.texts.a("Industries"),
        ];
        let gap = 6.0;
        let tw = ((inner_w - 7.0 * gap) / 7.0).min(170.0);
        for (i, n) in names.iter().enumerate() {
            ui.add(
                panel,
                Kind::Button {
                    text: n.clone(),
                    font: font(13.0),
                    action: A_TAB + i as u32,
                    enabled: screen.tab() != i,
                    hotkey: None,
                },
                at(PAD + i as f32 * (tw + gap), 54.0, tw, 28.0),
            );
        }
        ui.add(
            panel,
            Kind::Button {
                text: self.texts.a("Close"),
                font: font(13.0),
                action: A_CLOSE,
                enabled: true,
                hotkey: None,
            },
            at(PAD + 6.0 * (tw + gap), 54.0, tw, 28.0),
        );

        let body_y = 90.0;
        let mut bottom = ph - 34.0;
        // The finance buttons of the company book (issue, buy back, dividend, ...).
        let acts = match screen {
            InfoScreen::Finance(c) => self.fin_buttons(self.resolved_company(c)),
            _ => Vec::new(),
        };
        if !acts.is_empty() {
            let bw = ((inner_w - 6.0 * (acts.len() as f32 - 1.0)) / acts.len() as f32).min(170.0);
            let per_row = ((inner_w + 6.0) / (bw + 6.0)).floor().max(1.0) as usize;
            let bw = if acts.len() <= per_row {
                bw
            } else {
                ((inner_w - 6.0 * (per_row as f32 - 1.0)) / per_row as f32).min(170.0)
            };
            let rows = acts.len().div_ceil(per_row);
            bottom -= rows as f32 * 34.0 + 4.0;
            for (i, (text, action)) in acts.iter().enumerate() {
                let (c, r) = ((i % per_row) as f32, (i / per_row) as f32);
                ui.add(
                    panel,
                    Kind::Button {
                        text: text.clone(),
                        font: font(
                            (bw / (text.chars().count().max(1) as f32 * 0.78)).clamp(9.0, 12.5),
                        ),
                        action: *action,
                        enabled: true,
                        hotkey: None,
                    },
                    at(
                        PAD + c * (bw + 6.0),
                        ph - 30.0 - (rows as f32 - r) * 34.0,
                        bw,
                        30.0,
                    ),
                );
            }
        }
        let personal = if matches!(screen, InfoScreen::Stock) {
            self.personal_lines()
        } else {
            Vec::new()
        };
        if !personal.is_empty() {
            bottom -= personal.len() as f32 * 20.0 + 6.0;
            for (i, l) in personal.iter().enumerate() {
                ui.add(
                    panel,
                    label(l, font(14.0), TEXT, Align::Left),
                    at(PAD, bottom + 6.0 + i as f32 * 20.0, inner_w, 20.0),
                );
            }
        }
        let Some(content) = content else {
            ui.add(
                panel,
                label(
                    &self.texts.a("No game state to show."),
                    font(16.0),
                    MUTED,
                    Align::Left,
                ),
                at(PAD, body_y, inner_w, 24.0),
            );
            return;
        };
        match content {
            Content::List(list) => self.draw_list(
                ui,
                panel,
                &list,
                screen,
                first_scroll,
                PAD,
                body_y,
                inner_w,
                bottom,
                ph,
            ),
            Content::Page(page) => draw_page(ui, panel, &page, PAD, body_y, inner_w, bottom, pw),
        }
        let hint = match screen {
            InfoScreen::Finance(_) => self.texts.a("Left / Right: company   Esc: close"),
            InfoScreen::Stock => self
                .texts
                .a("* = human chairman   Click a row for the company report   Esc: close"),
            s if s.parent().is_some() => self.texts.a("Backspace: back   Esc: close"),
            _ => self
                .texts
                .a("Click a row for details   Wheel, PgUp, PgDn: scroll   Esc: close"),
        };
        ui.add(
            panel,
            label(&hint, font(12.0), MUTED, Align::Left),
            at(PAD, ph - 26.0, inner_w, 18.0),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_list(
        &mut self,
        ui: &mut Ui,
        panel: NodeId,
        list: &ListPage,
        screen: InfoScreen,
        scroll: usize,
        x: f32,
        y: f32,
        w: f32,
        bottom: f32,
        ph: f32,
    ) {
        let n = list.table.rows.len();
        let rowh = if ph > 560.0 { 26.0 } else { 21.0 };
        let size = rowh * 0.62;
        let head_h = rowh + 4.0;
        let visible = (((bottom - y - head_h) / rowh).floor() as usize).max(3);
        let first = scroll.min(n.saturating_sub(visible));
        if let Screen::Game(g) = &mut self.screen
            && let Some(i) = &mut g.info
            && i.screen == screen
        {
            i.scroll = first;
        }
        let shown = visible.min(n - first);
        // Inset behind the table, header, then the click targets and the cells over them.
        ui.add(
            panel,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(x, y, w, head_h + rowh * visible as f32 + 4.0),
        );
        if list.table.rows.is_empty() {
            ui.add(
                panel,
                label(
                    &self.texts.a("Nothing to show."),
                    font(16.0),
                    MUTED,
                    Align::Left,
                ),
                at(x + 12.0, y + 40.0, w, rowh),
            );
        }
        let sel = ui.add(
            panel,
            Kind::List {
                items: vec![String::new(); shown],
                selected: None,
                scroll: 0,
                row_h: rowh,
                font: font(size),
                action: A_ROW,
            },
            at(x, y + head_h - 1.0, w, rowh * shown as f32 + 2.0),
        );
        let _ = sel;
        draw_table(
            ui,
            panel,
            &list.table,
            x,
            y + 2.0,
            w - if n > visible { 10.0 } else { 0.0 },
            rowh,
            size,
            first,
            visible,
            false,
        );
        // Position text under the table.
        let last = (first + shown).min(n);
        let txt = if n == 0 {
            String::new()
        } else {
            format!("{}-{} of {}", first + 1, last, n)
        };
        ui.add(
            panel,
            label(&txt, font(13.0), HEAD, Align::Right),
            at(x, ph - 28.0, w, 20.0),
        );
    }
}

fn first_if_zero(slot: u32, first: Option<u32>) -> u32 {
    if slot == 0 { first.unwrap_or(0) } else { slot }
}

/// Two columns of blocks (one on a narrow panel), the row height fitted to the space.
#[allow(clippy::too_many_arguments)]
fn draw_page(
    ui: &mut Ui,
    panel: NodeId,
    page: &Page,
    x: f32,
    y: f32,
    w: f32,
    bottom: f32,
    pw: f32,
) {
    let avail = bottom - y;
    let two = pw >= NARROW;
    let (lcols, rcols) = (max_cols(&page.left), max_cols(&page.right));
    let lw_frac = (lcols as f32 / (lcols + rcols) as f32).clamp(0.38, 0.58);
    let (gap, lw) = if two {
        (24.0, (w - 24.0) * lw_frac)
    } else {
        (0.0, w)
    };
    let rw = if two { w - gap - lw } else { w };
    let fit = |rowh: f32| {
        let size = (rowh * 0.64).max(10.0);
        let (l, r) = (
            column_rows(&page.left, lw, size),
            column_rows(&page.right, rw, size),
        );
        let need = if two { l.max(r) } else { l + r + 0.6 };
        need * rowh <= avail
    };
    let mut rowh = 25.0;
    while rowh > 14.0 && !fit(rowh) {
        rowh -= 0.5;
    }
    let size = (rowh * 0.64).max(10.0);
    draw_blocks(ui, panel, &page.left, x, y, lw, bottom, rowh, size);
    if two {
        draw_blocks(
            ui,
            panel,
            &page.right,
            x + lw + gap,
            y,
            rw,
            bottom,
            rowh,
            size,
        );
    } else {
        let used = column_rows(&page.left, lw, size) * rowh + rowh * 0.6;
        draw_blocks(ui, panel, &page.right, x, y + used, rw, bottom, rowh, size);
    }
}
