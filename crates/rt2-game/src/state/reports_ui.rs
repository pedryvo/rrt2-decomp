//! The Reports screen (docs/ui.md "Relatórios e gráficos"; docs/reference/screens.md after I1):
//! line charts over time for the player's company and the competitors, the income statement
//! by category per year, the rankings of trains and stations, and the CSV export of what is on
//! screen.
//!
//! This is a feature of the clone built on figures the simulation already keeps (see
//! `crate::reports` for where each series comes from). The window opens from the Reports button
//! of the company book. It adds no rule and sends no command; a click on a ranking row only
//! moves the camera and opens the existing train or station window.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_ui::{Align, Anchor, Color, Fill, FontStyle, Kind, Layout, Ui, game_font};
use winit::keyboard::KeyCode;

use super::dialogs::Dialog;
use super::info_ui::{InfoScreen, draw_table};
use super::{AppState, Screen};
use crate::info::{CATEGORY_LABEL, CompanyReport, NCAT, StationRank, Table, TrainRank};
use crate::reports::{self, METRICS, Metric, Series};

pub(super) const A_REP_OPEN: u32 = 14000;
const A_REP_TAB: u32 = 14010;
const TABS: usize = 3;
const A_REP_METRIC: u32 = 14020;
const A_REP_LEGEND: u32 = 14040;
const MAX_LEGEND: usize = 24;
const A_REP_CHART: u32 = 14090;
const A_REP_PREV: u32 = 14091;
const A_REP_NEXT: u32 = 14092;
const A_REP_RANK: u32 = 14100;
const RANKS: usize = 4;
const A_REP_ROW: u32 = 14110;
const MAX_ROWS: usize = 40;
const A_REP_OPEN_SEL: u32 = 14160;
const A_REP_PGUP: u32 = 14161;
const A_REP_PGDN: u32 = 14162;
const A_REP_EXPORT: u32 = 14163;
const A_REP_LAST: u32 = 14163;

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd8);
const SEL: Color = Color(0xc8, 0x9a, 0x30, 0x50);

/// Years of the income statement shown next to the lifetime and year-to-date columns.
const INCOME_YEARS: usize = 5;

pub(super) fn owns(a: u32) -> bool {
    (A_REP_OPEN..=A_REP_LAST).contains(&a)
}

/// The window's state: tab, charted figure, hidden companies, chart cursor, the income tab's
/// company, the ranking and its selection, and the last export's message.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ReportsDlg {
    pub tab: usize,
    pub metric: usize,
    /// Bit `i` set: the company at position `i` of the list is hidden in the chart.
    pub hidden: u64,
    /// Horizontal position of the chart cursor (fraction of the plot width).
    pub cursor: Option<f32>,
    /// Slot of the company of the income tab (0 = the player's, else the first).
    pub company: u32,
    pub rank: usize,
    pub sel: Option<usize>,
    pub scroll: usize,
    pub notice: Option<String>,
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
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

/// `s` cut to about `max` characters, with an ellipsis.
fn fit(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push_str("...");
    out
}

/// A train's result, `-` when its counters hold no believable value.
fn money_i(v: Option<i64>) -> String {
    v.map_or_else(|| "-".to_owned(), |v| crate::info::money(v as f64))
}

/// The data every tab draws from, read once per build.
struct Data {
    companies: Vec<CompanyReport>,
    now: f32,
}

impl AppState {
    /// Opens the Reports window (the Reports button of the company book).
    pub(super) fn open_reports(&mut self, company: u32) {
        self.push_dialog(Dialog::Reports(ReportsDlg {
            company,
            ..ReportsDlg::default()
        }));
    }

    fn reports_data(&self) -> Option<Data> {
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        let d = g.world.date();
        let now = reports::time_of(i32::from(d.year), i32::from(d.month), i32::from(d.day));
        let companies = self.with_info(|i| i.company_reports())?;
        Some(Data { companies, now })
    }

    #[cfg(test)]
    pub(super) fn reports_data_for_test(&self) -> Option<(Vec<CompanyReport>, f32)> {
        self.reports_data().map(|d| (d.companies, d.now))
    }

    /// Samples the balances of every company once per game month (cash, debt and book value have
    /// no history in the save). Cheap when the month is already sampled.
    pub(super) fn sample_reports(&mut self) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        if g.editor.is_some() {
            return;
        }
        let d = g.world.date();
        let (y, m, day) = (i32::from(d.year), i32::from(d.month), i32::from(d.day));
        if !g.reports.wants(y, m) {
            return;
        }
        let Some(snaps) = self.with_info(|i| i.report_snapshots()) else {
            return;
        };
        if let Screen::Game(g) = &mut self.screen {
            g.reports.record(y, m, day, snaps);
        }
    }

    fn reports_dlg(&self) -> Option<&ReportsDlg> {
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        match g.dialogs.last() {
            Some(Dialog::Reports(r)) => Some(r),
            _ => None,
        }
    }

    fn reports_edit(&mut self, f: impl FnOnce(&mut ReportsDlg)) {
        if let Screen::Game(g) = &mut self.screen
            && let Some(Dialog::Reports(r)) = g.dialogs.last_mut()
        {
            f(r);
        }
        self.rebuild_ui();
    }

    /// The companies' series of the charted figure, hidden ones left out.
    fn reports_series(&self, data: &Data, r: &ReportsDlg) -> Vec<Series> {
        let Screen::Game(g) = &self.screen else {
            return Vec::new();
        };
        let metric = METRICS[r.metric.min(METRICS.len() - 1)];
        reports::build_series(metric, &data.companies, &g.reports, data.now)
            .into_iter()
            .filter(|s| r.hidden & (1 << s.index.min(63)) == 0)
            .collect()
    }

    /// The company the income tab shows.
    fn reports_company<'a>(&self, data: &'a Data, r: &ReportsDlg) -> Option<&'a CompanyReport> {
        let want = if r.company != 0 {
            r.company
        } else {
            self.player().map_or(0, |c| c as u32)
        };
        data.companies
            .iter()
            .find(|c| c.slot == want)
            .or_else(|| data.companies.first())
    }

    /// The ranking of the selected mode: trains (0..=2 by this year, last year, lifetime) or
    /// stations (3), ties by slot.
    pub(super) fn train_rows(&self, mode: usize) -> Vec<TrainRank> {
        let mut v = self.with_info(|i| i.train_ranking()).unwrap_or_default();
        // Unknown results (`None`) sort last.
        let key = |t: &TrainRank| match mode {
            0 => t.year,
            1 => t.last,
            _ => t.life,
        };
        v.sort_by_key(|t| (std::cmp::Reverse(key(t)), t.slot));
        v
    }

    pub(super) fn station_rows(&self) -> Vec<StationRank> {
        self.with_info(|i| i.station_ranking()).unwrap_or_default()
    }

    fn rank_len(&self, mode: usize) -> usize {
        if mode == 3 {
            self.station_rows().len()
        } else {
            self.train_rows(mode).len()
        }
    }

    // ---- input ------------------------------------------------------------------------------

    pub(super) fn reports_activated(&mut self, a: u32) {
        match a {
            A_REP_OPEN => {
                let c = self
                    .info_screen()
                    .and_then(|s| match s {
                        InfoScreen::Finance(c) | InfoScreen::Overview(c) => Some(c),
                        _ => None,
                    })
                    .unwrap_or(0);
                let company = self.book_company(c);
                self.open_reports(company);
            }
            a if (A_REP_TAB..A_REP_TAB + TABS as u32).contains(&a) => {
                let tab = (a - A_REP_TAB) as usize;
                self.reports_edit(|r| {
                    r.tab = tab;
                    r.notice = None;
                });
            }
            a if (A_REP_METRIC..A_REP_METRIC + METRICS.len() as u32).contains(&a) => {
                let m = (a - A_REP_METRIC) as usize;
                self.reports_edit(|r| {
                    r.metric = m;
                    r.cursor = None;
                    r.notice = None;
                });
            }
            a if (A_REP_LEGEND..A_REP_LEGEND + MAX_LEGEND as u32).contains(&a) => {
                let i = (a - A_REP_LEGEND) as usize;
                self.reports_edit(|r| r.hidden ^= 1 << i);
            }
            A_REP_PREV | A_REP_NEXT => {
                self.reports_step_company(if a == A_REP_NEXT { 1 } else { -1 })
            }
            a if (A_REP_RANK..A_REP_RANK + RANKS as u32).contains(&a) => {
                let m = (a - A_REP_RANK) as usize;
                self.reports_edit(|r| {
                    r.rank = m;
                    r.sel = None;
                    r.scroll = 0;
                    r.notice = None;
                });
            }
            a if (A_REP_ROW..A_REP_ROW + MAX_ROWS as u32).contains(&a) => {
                let Some(r) = self.reports_dlg() else {
                    return;
                };
                let idx = r.scroll + (a - A_REP_ROW) as usize;
                if r.sel == Some(idx) {
                    self.reports_open_selected();
                } else {
                    self.reports_select(idx);
                }
            }
            A_REP_OPEN_SEL => self.reports_open_selected(),
            A_REP_PGUP => self.reports_scroll(-1, true),
            A_REP_PGDN => self.reports_scroll(1, true),
            A_REP_EXPORT => self.reports_export(),
            _ => {}
        }
    }

    /// A press or drag on the chart: the cursor follows the pointer.
    pub(super) fn reports_pointer(&mut self, action: u32, fx: f32) -> bool {
        if action != A_REP_CHART {
            return false;
        }
        self.reports_edit(|r| r.cursor = Some(fx.clamp(0.0, 1.0)));
        true
    }

    fn reports_step_company(&mut self, dir: i32) {
        let Some(data) = self.reports_data() else {
            return;
        };
        let Some(r) = self.reports_dlg().cloned() else {
            return;
        };
        let n = data.companies.len() as i32;
        if n == 0 {
            return;
        }
        let cur = self
            .reports_company(&data, &r)
            .and_then(|c| data.companies.iter().position(|d| d.slot == c.slot))
            .unwrap_or(0) as i32;
        let next = data.companies[(cur + dir).rem_euclid(n) as usize].slot;
        self.reports_edit(|r| r.company = next);
    }

    /// Selects row `idx` of the ranking and moves the camera to it.
    fn reports_select(&mut self, idx: usize) {
        let Some(r) = self.reports_dlg().cloned() else {
            return;
        };
        let tile = if r.rank == 3 {
            self.station_rows().get(idx).map(|s| s.tile)
        } else {
            self.train_rows(r.rank).get(idx).and_then(|t| t.tile)
        };
        if self.rank_len(r.rank) <= idx {
            return;
        }
        self.reports_edit(|r| r.sel = Some(idx));
        if let Some((x, y)) = tile {
            self.center_camera(x, y);
        }
    }

    /// Opens the window of the selected train or station (the dialog closes first).
    fn reports_open_selected(&mut self) {
        let Some(r) = self.reports_dlg().cloned() else {
            return;
        };
        let Some(idx) = r.sel else {
            return;
        };
        if r.rank == 3 {
            let Some(s) = self.station_rows().get(idx).map(|s| s.slot) else {
                return;
            };
            self.close_dialog();
            self.open_station(s as i32);
        } else {
            let Some(t) = self.train_rows(r.rank).get(idx).cloned() else {
                return;
            };
            self.close_dialog();
            if self.player() == Some(t.company as i32) {
                self.push_dialog(Dialog::TrainEdit {
                    train: t.slot as i32,
                    sel: 0,
                    pick: 0,
                    confirm: false,
                });
            } else {
                self.open_info(InfoScreen::TrainDetail(t.slot));
            }
        }
    }

    /// Scrolls the ranking by a row or, with `page`, by a screenful.
    fn reports_scroll(&mut self, dir: i32, page: bool) {
        let Some(r) = self.reports_dlg().cloned() else {
            return;
        };
        if r.tab != 2 {
            return;
        }
        let n = self.rank_len(r.rank);
        let step = if page { self.reports_rows_visible() } else { 1 } as i32;
        let max = n.saturating_sub(1) as i32;
        let to = (r.scroll as i32 + dir * step).clamp(0, max) as usize;
        self.reports_edit(|r| r.scroll = to);
    }

    /// Rows of the ranking that fit the window (what the last layout drew).
    fn reports_rows_visible(&self) -> usize {
        let n = self
            .ui
            .draw_order()
            .into_iter()
            .filter(|(_, id)| {
                matches!(&self.ui.node(*id).kind, Kind::Hotspot { action, .. }
                    if (A_REP_ROW..A_REP_ROW + MAX_ROWS as u32).contains(action))
            })
            .count();
        n.max(1)
    }

    pub(super) fn reports_wheel(&mut self, notches: f32) -> bool {
        let Some(r) = self.reports_dlg() else {
            return false;
        };
        if r.tab == 2 {
            let n = (notches.abs().round().max(1.0)) as i32 * 3;
            self.reports_scroll(if notches > 0.0 { -n } else { n }, false);
        }
        true
    }

    /// Keys of the open window: 1..3 tabs, arrows (cursor, company or selection), Page Up / Down,
    /// Enter opens the selected row. Escape is the main window's.
    pub(super) fn reports_key(&mut self, code: KeyCode) -> bool {
        let Some(r) = self.reports_dlg().cloned() else {
            return false;
        };
        let tab_key = match code {
            KeyCode::Digit1 | KeyCode::Numpad1 => Some(0),
            KeyCode::Digit2 | KeyCode::Numpad2 => Some(1),
            KeyCode::Digit3 | KeyCode::Numpad3 => Some(2),
            _ => None,
        };
        if let Some(t) = tab_key {
            self.reports_edit(|r| {
                r.tab = t;
                r.notice = None;
            });
            return true;
        }
        let dir = match code {
            KeyCode::ArrowLeft | KeyCode::ArrowUp => -1,
            KeyCode::ArrowRight | KeyCode::ArrowDown => 1,
            _ => 0,
        };
        match (r.tab, code) {
            (_, KeyCode::PageUp) => self.reports_scroll(-1, true),
            (_, KeyCode::PageDown) => self.reports_scroll(1, true),
            (0, KeyCode::ArrowLeft | KeyCode::ArrowRight) => self.reports_step_cursor(dir),
            (1, KeyCode::ArrowLeft | KeyCode::ArrowRight) => self.reports_step_company(dir),
            (2, KeyCode::ArrowUp | KeyCode::ArrowDown) => {
                let n = self.rank_len(r.rank);
                if n > 0 {
                    let to = match r.sel {
                        None => 0,
                        Some(s) => (s as i32 + dir).clamp(0, n as i32 - 1) as usize,
                    };
                    let vis = self.reports_rows_visible();
                    self.reports_edit(|r| {
                        if to < r.scroll {
                            r.scroll = to;
                        } else if to >= r.scroll + vis {
                            r.scroll = to + 1 - vis;
                        }
                    });
                    self.reports_select(to);
                }
            }
            (2, KeyCode::Enter | KeyCode::NumpadEnter) => self.reports_open_selected(),
            _ => return false,
        }
        true
    }

    /// Moves the chart cursor to the previous / next point in time.
    fn reports_step_cursor(&mut self, dir: i32) {
        let (Some(data), Some(r)) = (self.reports_data(), self.reports_dlg().cloned()) else {
            return;
        };
        let series = self.reports_series(&data, &r);
        let refs: Vec<&Series> = series.iter().collect();
        let view = reports::view_of(&refs);
        let mut times: Vec<f32> = refs
            .iter()
            .flat_map(|s| s.points.iter().map(|p| p.t))
            .collect();
        times.sort_by(f32::total_cmp);
        times.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
        if times.is_empty() {
            return;
        }
        let cur = r.cursor.map(|fx| {
            let t = reports::time_at(&view, CHART_COLS, fx);
            reports::nearest_time(&refs, t).unwrap_or(t)
        });
        let i = match cur.and_then(|c| times.iter().position(|t| (t - c).abs() < 1e-4)) {
            Some(i) => (i as i32 + dir).clamp(0, times.len() as i32 - 1) as usize,
            None if dir < 0 => times.len() - 1,
            None => 0,
        };
        let (cx, _) = reports::cell_of(&view, CHART_COLS, CHART_ROWS, times[i], 0.0);
        let fx = (cx as f32 + 0.5) / CHART_COLS as f32;
        self.reports_edit(|r| r.cursor = Some(fx));
    }

    // ---- export -----------------------------------------------------------------------------

    /// Writes what the current tab shows to a CSV file in the reports folder and tells the path.
    fn reports_export(&mut self) {
        let Some(data) = self.reports_data() else {
            return;
        };
        let Some(r) = self.reports_dlg().cloned() else {
            return;
        };
        let (name, csv) = match r.tab {
            0 => {
                let series = self.reports_series(&data, &r);
                let refs: Vec<&Series> = series.iter().collect();
                let m = METRICS[r.metric.min(METRICS.len() - 1)];
                (
                    format!("chart-{}", slug(m.label().1)),
                    reports::chart_csv(m, &refs),
                )
            }
            1 => {
                let Some(c) = self.reports_company(&data, &r) else {
                    return;
                };
                let labels: [&str; NCAT] = std::array::from_fn(|i| CATEGORY_LABEL[i].1);
                (
                    format!("income-{}", slug(&c.name)),
                    reports::income_csv(c, &labels),
                )
            }
            _ => {
                if r.rank == 3 {
                    ("stations".to_owned(), self.stations_csv())
                } else {
                    (
                        format!(
                            "trains-{}",
                            ["this-year", "last-year", "lifetime"][r.rank.min(2)]
                        ),
                        self.trains_csv(),
                    )
                }
            }
        };
        let (y, m) = self.date_ym();
        let file = format!("rrt2-{name}-{y}-{m:02}.csv");
        let msg = match self.write_report(&file, &csv) {
            Ok(p) => self.texts.af("Exported to %1", &[p.display().to_string()]),
            Err(e) => self.texts.af("Cannot export: %1", &[e]),
        };
        self.toast(msg.clone());
        self.reports_edit(|r| r.notice = Some(msg));
    }

    fn write_report(&self, file: &str, csv: &str) -> Result<PathBuf, String> {
        let dir = self
            .reports_dir
            .clone()
            .ok_or_else(|| "no data folder".to_owned())?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(file);
        std::fs::write(&path, csv).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    }

    fn trains_csv(&self) -> String {
        let mode = self.reports_dlg().map_or(0, |r| r.rank.min(2));
        let mut out = String::from("rank,train,company,engine,this_year,last_year,lifetime\n");
        for (i, t) in self.train_rows(mode).iter().enumerate() {
            out.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                i + 1,
                reports::csv_field(&t.name),
                reports::csv_field(&t.company_name),
                reports::csv_field(&t.engine),
                t.year.map_or_else(String::new, |v| v.to_string()),
                t.last.map_or_else(String::new, |v| v.to_string()),
                t.life.map_or_else(String::new, |v| v.to_string()),
            ));
        }
        out
    }

    fn stations_csv(&self) -> String {
        let mut out = String::from("rank,station,owner,lots_moved,lots_generated\n");
        for (i, s) in self.station_rows().iter().enumerate() {
            out.push_str(&format!(
                "{},{},{},{},{}\n",
                i + 1,
                reports::csv_field(&s.name),
                reports::csv_field(&s.owner_name),
                s.moved,
                s.generated
            ));
        }
        out
    }

    // ---- drawing ----------------------------------------------------------------------------

    pub(super) fn build_reports(&self, ui: &mut Ui, r: &ReportsDlg) {
        let t = &self.texts;
        let (cw, ch) = ui.logical_size();
        // Design size of the window; everything is laid out in these units times `s`.
        let (dw, dh) = (980.0, 640.0);
        let dz = (ch / 620.0).clamp(1.0, 2.0);
        let s = (dz).min((cw - 16.0) / dw).min((ch - 16.0) / dh);
        let (w, h) = (dw * s, dh * s);
        let root = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, w, h),
        );
        let pad = 14.0 * s;
        let fs = (14.0 * s).round().max(9.0);
        let font = game_font(8).with_size(fs);
        let title_font = game_font(3);
        let title_font = title_font.with_size((title_font.size * s * 0.7).round());
        let bfont = game_font(1).with_size((game_font(1).size * s * 0.8).round().max(9.0));

        ui.add(
            root,
            lbl(
                &t.a("Reports").to_uppercase(),
                title_font,
                HEAD,
                Align::Left,
            ),
            at(pad, 8.0 * s, 360.0 * s, 30.0 * s),
        );
        let data = self.reports_data();
        // The tabs.
        let tab_names = [t.a("Charts"), t.a("Income by category"), t.a("Rankings")];
        for (i, n) in tab_names.iter().enumerate() {
            ui.add(
                root,
                Kind::Toggle {
                    text: n.clone(),
                    font: bfont,
                    action: A_REP_TAB + i as u32,
                    on: i == r.tab,
                    enabled: true,
                },
                at(pad + i as f32 * 186.0 * s, 44.0 * s, 180.0 * s, 28.0 * s),
            );
        }
        let body = (pad, 82.0 * s, w - 2.0 * pad, h - 82.0 * s - 56.0 * s);
        match &data {
            None => {
                ui.add(
                    root,
                    lbl(&t.a("No game state to show."), font, MUTED, Align::Center),
                    at(body.0, body.1, body.2, 30.0 * s),
                );
            }
            Some(d) => match r.tab {
                0 => self.build_charts(ui, root, r, d, body, s),
                1 => self.build_income(ui, root, r, d, body, s),
                _ => self.build_rankings(ui, root, r, body, s),
            },
        }
        // The footer: export, its message, close.
        let fy = h - 44.0 * s;
        ui.add(
            root,
            Kind::Button {
                text: t.a("Export CSV"),
                font: bfont,
                action: A_REP_EXPORT,
                enabled: data.is_some(),
                hotkey: None,
            },
            at(pad, fy, 170.0 * s, 30.0 * s),
        );
        if let Some(n) = &r.notice {
            ui.add(
                root,
                lbl(
                    n,
                    game_font(8).with_size((fs * 0.9).round()),
                    TEXT,
                    Align::Left,
                ),
                at(pad + 180.0 * s, fy, w - 2.0 * pad - 340.0 * s, 30.0 * s),
            );
        }
        ui.add(
            root,
            Kind::Button {
                text: t.a("Close"),
                font: bfont,
                action: super::dialogs::A_DLG_CLOSE,
                enabled: true,
                hotkey: None,
            },
            at(w - pad - 140.0 * s, fy, 140.0 * s, 30.0 * s),
        );
    }

    fn build_charts(
        &self,
        ui: &mut Ui,
        root: rt2_ui::NodeId,
        r: &ReportsDlg,
        d: &Data,
        body: (f32, f32, f32, f32),
        s: f32,
    ) {
        let t = &self.texts;
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let fs = (13.0 * s).round().max(9.0);
        let font = game_font(8).with_size(fs);
        let small = game_font(8).with_size((12.0 * s).round().max(8.0));
        let bfont = game_font(1).with_size((game_font(1).size * s * 0.62).round().max(8.0));
        let lfont = game_font(1).with_size((game_font(1).size * s * 0.85).round().max(8.0));
        let (bx, by, bw, bh) = body;
        // The twelve figures, two rows of six.
        let gap = 4.0 * s;
        let mw = (bw - 5.0 * gap) / 6.0;
        for (i, m) in METRICS.iter().enumerate() {
            let (id, en) = m.label();
            ui.add(
                root,
                Kind::Toggle {
                    text: t.get(id, en),
                    font: bfont,
                    action: A_REP_METRIC + i as u32,
                    on: i == r.metric,
                    enabled: true,
                },
                at(
                    bx + (i % 6) as f32 * (mw + gap),
                    by + (i / 6) as f32 * 30.0 * s,
                    mw,
                    26.0 * s,
                ),
            );
        }
        let metric = METRICS[r.metric.min(METRICS.len() - 1)];
        let unit = metric.unit();
        let all = reports::build_series(metric, &d.companies, &g.reports, d.now);
        let shown: Vec<&Series> = all
            .iter()
            .filter(|x| r.hidden & (1 << x.index.min(63)) == 0)
            .collect();
        let view = reports::view_of(&shown);
        // The plot: a raster of cells, with the axis labels around it.
        let cy = by + 62.0 * s;
        let label_w = 64.0 * s;
        let cx = bx + label_w + 4.0 * s;
        let legend_w = 236.0 * s;
        let plot_w = bw - label_w - 4.0 * s - legend_w - 10.0 * s;
        let plot_h = bh - 62.0 * s - 44.0 * s;
        // The plot is CHART_COLS x CHART_ROWS cells (about 1.6 design units each).
        let (cell_w, cell_h) = (plot_w / CHART_COLS as f32, plot_h / CHART_ROWS as f32);
        let highlight = self.player().map(|c| c as u32);
        let cursor_t = r.cursor.map(|fx| {
            let tc = reports::time_at(&view, CHART_COLS, fx);
            reports::nearest_time(&shown, tc).unwrap_or(tc)
        });
        let colors = reports::rasterize(
            &shown,
            highlight,
            &view,
            CHART_COLS,
            CHART_ROWS,
            cursor_t.filter(|_| !shown.is_empty()),
        );
        ui.add(
            root,
            Kind::Cells {
                cols: CHART_COLS,
                rows: CHART_ROWS,
                colors: Arc::new(colors),
                action: A_REP_CHART,
            },
            at(cx, cy, plot_w, plot_h),
        );
        // Axis labels.
        for &v in &view.y.ticks {
            let (_, row) = reports::cell_of(&view, CHART_COLS, CHART_ROWS, view.t0, v);
            ui.add(
                root,
                lbl(&reports::format_axis(unit, v), small, MUTED, Align::Right),
                at(
                    bx,
                    cy + (row as f32 + 0.5) * cell_h - 8.0 * s,
                    label_w,
                    16.0 * s,
                ),
            );
        }
        for &tick in &view.x_ticks {
            let (col, _) = reports::cell_of(&view, CHART_COLS, CHART_ROWS, tick, 0.0);
            let (y, m) = reports::ym_of(tick);
            let text = if m == 1 {
                y.to_string()
            } else {
                format!("{} {y}", t.month_short(m as u8))
            };
            ui.add(
                root,
                lbl(&text, small, MUTED, Align::Center),
                at(
                    cx + (col as f32 + 0.5) * cell_w - 40.0 * s,
                    cy + plot_h + 2.0 * s,
                    80.0 * s,
                    16.0 * s,
                ),
            );
        }
        if shown.is_empty() {
            ui.add(
                root,
                lbl(&t.a("No data yet."), font, MUTED, Align::Center),
                at(cx, cy, plot_w, plot_h),
            );
        }
        // The note under the plot.
        let note = if metric.sampled() && g.reports.len() < 2 {
            t.a("Collected while you play: a line needs at least two samples.")
        } else if matches!(metric, Metric::Revenue | Metric::Expenses | Metric::Profit) {
            t.a("The last point of each yearly series is the year to date (hollow marker).")
        } else {
            t.a("Click a chart to read the values at that date.")
        };
        ui.add(
            root,
            lbl(&note, small, MUTED, Align::Left),
            at(cx, cy + plot_h + 20.0 * s, plot_w, 16.0 * s),
        );
        // The legend: one row per company with its swatch, a toggle and the value at the cursor
        // (or the latest one).
        let lx = cx + plot_w + 10.0 * s;
        let head = match cursor_t {
            Some(tc) => {
                let (y, m) = reports::ym_of(tc);
                format!("{} {y}", t.month_short(m as u8))
            }
            None => t.get(421, "Click to toggle whether or not this number is graphed"),
        };
        ui.add(
            root,
            Kind::Text {
                lines: crate::scenario_info::wrap_text(
                    &head,
                    (legend_w / (small.size * 0.5)) as usize,
                ),
                scroll: 0,
                row_h: small.size * 1.25,
                font: small,
                color: HEAD,
            },
            at(lx, cy, legend_w, 34.0 * s),
        );
        let row_h = 24.0 * s;
        let first = cy + 38.0 * s;
        let room = (((cy + plot_h) - first) / row_h).floor().max(1.0) as usize;
        let rows = d.companies.len().min(room).min(MAX_LEGEND);
        for (i, c) in d.companies.iter().enumerate().take(rows) {
            let y = first + i as f32 * row_h;
            let on = r.hidden & (1 << i.min(63)) == 0;
            // The swatch shows the line and marker of the company whatever its visibility.
            ui.add(
                root,
                Kind::Cells {
                    cols: 18,
                    rows: 8,
                    colors: Arc::new(reports::swatch(i, 18, 8)),
                    action: 0,
                },
                at(lx, y + 4.0 * s, 36.0 * s, 16.0 * s),
            );
            ui.add(
                root,
                Kind::Toggle {
                    text: fit(&c.name, 14),
                    font: lfont,
                    action: A_REP_LEGEND + i as u32,
                    on,
                    enabled: true,
                },
                at(lx + 40.0 * s, y, 122.0 * s, row_h - 2.0 * s),
            );
            let value = all
                .iter()
                .find(|x| x.index == i)
                .and_then(|x| match cursor_t {
                    Some(tc) => reports::value_at(x, tc),
                    None => x.points.last().copied(),
                });
            if let Some(p) = value.filter(|_| on) {
                ui.add(
                    root,
                    lbl(&reports::format_axis(unit, p.v), small, TEXT, Align::Right),
                    at(lx + 164.0 * s, y, legend_w - 164.0 * s, row_h - 2.0 * s),
                );
            }
        }
        if d.companies.len() > rows {
            ui.add(
                root,
                lbl(
                    &format!("... {}", d.companies.len() - rows),
                    small,
                    MUTED,
                    Align::Left,
                ),
                at(lx, first + rows as f32 * row_h, legend_w, 16.0 * s),
            );
        }
    }

    fn build_income(
        &self,
        ui: &mut Ui,
        root: rt2_ui::NodeId,
        r: &ReportsDlg,
        d: &Data,
        body: (f32, f32, f32, f32),
        s: f32,
    ) {
        let t = &self.texts;
        let (bx, by, bw, bh) = body;
        let fs = (14.0 * s).round().max(9.0);
        let font = game_font(8).with_size(fs);
        let bfont = game_font(1).with_size((game_font(1).size * s * 0.8).round().max(9.0));
        let Some(c) = self.reports_company(d, r) else {
            ui.add(
                root,
                lbl(&t.a("No data yet."), font, MUTED, Align::Center),
                at(bx, by, bw, 30.0 * s),
            );
            return;
        };
        ui.add(
            root,
            Kind::Button {
                text: "<".to_owned(),
                font: bfont,
                action: A_REP_PREV,
                enabled: d.companies.len() > 1,
                hotkey: None,
            },
            at(bx, by, 40.0 * s, 28.0 * s),
        );
        ui.add(
            root,
            lbl(
                &c.name,
                game_font(8).with_size((fs * 1.15).round()),
                HEAD,
                Align::Center,
            ),
            at(bx + 46.0 * s, by, 400.0 * s, 28.0 * s),
        );
        ui.add(
            root,
            Kind::Button {
                text: ">".to_owned(),
                font: bfont,
                action: A_REP_NEXT,
                enabled: d.companies.len() > 1,
                hotkey: None,
            },
            at(bx + 452.0 * s, by, 40.0 * s, 28.0 * s),
        );
        // The table: the categories (revenue ones, their total, the costs, their total, profit).
        let years = c.years.len().min(INCOME_YEARS);
        let mut cols: Vec<(String, f32, bool)> = vec![
            (String::new(), 3.0, false),
            (t.get(2368, "Lifetime"), 1.5, true),
            (t.a("This year"), 1.5, true),
        ];
        cols.extend(
            c.years
                .iter()
                .take(years)
                .map(|y| (y.year.to_string(), 1.4, true)),
        );
        let spec: Vec<(&str, f32, bool)> =
            cols.iter().map(|(h, w, r)| (h.as_str(), *w, *r)).collect();
        let mut table = Table::new(&spec);
        let lab = |i: usize| t.get(CATEGORY_LABEL[i].0, CATEGORY_LABEL[i].1);
        let mut row = |label: String, f: &dyn Fn(&[f64; NCAT]) -> f64, total: bool| {
            let mut cells = vec![label, money_f(f(&c.life)), money_f(f(&c.ytd))];
            cells.extend(c.years.iter().take(years).map(|y| money_f(f(&y.cats))));
            table.push(if total { crate::info::TOTAL } else { 0 }, cells);
        };
        for i in 0..5 {
            row(lab(i), &|v| v[i], false);
        }
        row(t.get(984, "Revenue"), &|v| v[..5].iter().sum(), true);
        for i in 5..NCAT {
            row(lab(i), &|v| v[i], false);
        }
        row(t.get(35, "Expenses"), &|v| v[5..].iter().sum(), true);
        row(t.get(36, "Profit"), &|v| v.iter().sum(), true);
        let ty = by + 40.0 * s;
        let rowh = ((bh - 40.0 * s - 24.0 * s) / (table.rows.len() as f32 + 1.0))
            .min(26.0 * s)
            .floor();
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(bx, ty, bw, rowh * (table.rows.len() as f32 + 1.0) + 8.0 * s),
        );
        draw_table(
            ui,
            root,
            &table,
            bx,
            ty + 2.0 * s,
            bw,
            rowh,
            fs,
            0,
            usize::MAX,
            false,
        );
    }

    fn build_rankings(
        &self,
        ui: &mut Ui,
        root: rt2_ui::NodeId,
        r: &ReportsDlg,
        body: (f32, f32, f32, f32),
        s: f32,
    ) {
        let t = &self.texts;
        let (bx, by, bw, bh) = body;
        let fs = (14.0 * s).round().max(9.0);
        let font = game_font(8).with_size(fs);
        let bfont = game_font(1).with_size((game_font(1).size * s * 0.75).round().max(9.0));
        let names = [
            t.a("Trains: this year"),
            t.a("Trains: last year"),
            t.a("Trains: lifetime"),
            t.a("Stations: volume"),
        ];
        let gap = 6.0 * s;
        let mw = (bw - 3.0 * gap) / 4.0;
        for (i, n) in names.iter().enumerate() {
            ui.add(
                root,
                Kind::Toggle {
                    text: n.clone(),
                    font: bfont,
                    action: A_REP_RANK + i as u32,
                    on: i == r.rank,
                    enabled: true,
                },
                at(bx + i as f32 * (mw + gap), by, mw, 28.0 * s),
            );
        }
        // The table.
        let (table, count, empty) = if r.rank == 3 {
            let rows = self.station_rows();
            let mut tb = Table::new(&[
                ("#", 0.5, true),
                (&t.a("Station"), 2.6, false),
                (&t.a("Owner"), 2.6, false),
                (&t.a("Lots moved"), 1.3, true),
                (&t.a("Lots generated"), 1.5, true),
            ]);
            for (i, x) in rows.iter().enumerate() {
                tb.push(
                    0,
                    vec![
                        (i + 1).to_string(),
                        x.name.clone(),
                        x.owner_name.clone(),
                        x.moved.to_string(),
                        x.generated.to_string(),
                    ],
                );
            }
            (tb, rows.len(), t.a("No stations yet."))
        } else {
            let rows = self.train_rows(r.rank);
            let mut tb = Table::new(&[
                ("#", 0.5, true),
                (&t.a("Train"), 2.0, false),
                (&t.get(178, "Company"), 2.4, false),
                (&t.a("Engine"), 2.0, false),
                (&t.a("This year"), 1.3, true),
                (&t.a("Last year"), 1.3, true),
                (&t.get(2368, "Lifetime"), 1.4, true),
            ]);
            for (i, x) in rows.iter().enumerate() {
                tb.push(
                    0,
                    vec![
                        (i + 1).to_string(),
                        x.name.clone(),
                        x.company_name.clone(),
                        x.engine.clone(),
                        money_i(x.year),
                        money_i(x.last),
                        money_i(x.life),
                    ],
                );
            }
            (tb, rows.len(), t.a("No trains yet."))
        };
        let ty = by + 38.0 * s;
        let rowh = (fs * 1.5).round();
        let foot = 34.0 * s;
        let avail = bh - 38.0 * s - foot - rowh - 6.0 * s;
        let visible = ((avail / rowh).floor().max(1.0) as usize).min(MAX_ROWS);
        let scroll = r.scroll.min(count.saturating_sub(1));
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(bx, ty, bw, rowh * (visible as f32 + 1.0) + 6.0 * s),
        );
        if count == 0 {
            draw_table(
                ui,
                root,
                &table,
                bx,
                ty + 2.0 * s,
                bw,
                rowh,
                fs,
                0,
                0,
                false,
            );
            ui.add(
                root,
                lbl(&empty, font, MUTED, Align::Center),
                at(bx, ty + rowh * 3.0, bw, rowh),
            );
        } else {
            let shown = (count - scroll).min(visible);
            if let Some(sel) = r.sel.filter(|s| (scroll..scroll + shown).contains(s)) {
                ui.add(
                    root,
                    Kind::Panel {
                        fill: Fill::Solid(SEL),
                    },
                    at(
                        bx + 2.0 * s,
                        ty + 2.0 * s + rowh + 2.0 + (sel - scroll) as f32 * rowh,
                        bw - 4.0 * s,
                        rowh,
                    ),
                );
            }
            draw_table(
                ui,
                root,
                &table,
                bx,
                ty + 2.0 * s,
                bw,
                rowh,
                fs,
                scroll,
                visible,
                false,
            );
            for i in 0..shown {
                ui.add(
                    root,
                    Kind::Hotspot {
                        action: A_REP_ROW + i as u32,
                        on: r.sel == Some(scroll + i),
                        enabled: true,
                    },
                    at(
                        bx + 2.0 * s,
                        ty + 2.0 * s + rowh + 2.0 + i as f32 * rowh,
                        bw - 4.0 * s,
                        rowh,
                    ),
                );
            }
        }
        // Paging, the count and the Open button.
        let fy = by + bh - foot + 4.0 * s;
        let bw2 = 120.0 * s;
        ui.add(
            root,
            Kind::Button {
                text: t.a("Page up"),
                font: bfont,
                action: A_REP_PGUP,
                enabled: scroll > 0,
                hotkey: None,
            },
            at(bx, fy, bw2, 28.0 * s),
        );
        ui.add(
            root,
            Kind::Button {
                text: t.a("Page down"),
                font: bfont,
                action: A_REP_PGDN,
                enabled: scroll + visible < count,
                hotkey: None,
            },
            at(bx + bw2 + 6.0 * s, fy, bw2, 28.0 * s),
        );
        let range = if count == 0 {
            String::new()
        } else {
            format!(
                "{}-{} / {}",
                scroll + 1,
                (scroll + visible).min(count),
                count
            )
        };
        ui.add(
            root,
            lbl(&range, font, MUTED, Align::Left),
            at(bx + 2.0 * bw2 + 18.0 * s, fy, 160.0 * s, 28.0 * s),
        );
        ui.add(
            root,
            lbl(
                &t.a("Click a row to center the map on it; click again or press Open to open its window."),
                game_font(8).with_size((11.0 * s).round().max(8.0)),
                MUTED,
                Align::Right,
            ),
            at(bx + 2.0 * bw2 + 180.0 * s, fy, bw - 2.0 * bw2 - 180.0 * s - 156.0 * s, 28.0 * s),
        );
        ui.add(
            root,
            Kind::Button {
                text: t.a("Open"),
                font: bfont,
                action: A_REP_OPEN_SEL,
                enabled: r.sel.is_some(),
                hotkey: None,
            },
            at(bx + bw - 150.0 * s, fy, 150.0 * s, 28.0 * s),
        );
    }
}

fn money_f(v: f64) -> String {
    crate::info::money(v)
}

/// Cells of the chart raster.
pub(super) const CHART_COLS: usize = 400;
pub(super) const CHART_ROWS: usize = 250;

/// A file-name fragment: lower case letters and digits, the rest as dashes.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}
