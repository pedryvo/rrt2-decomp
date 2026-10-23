//! The charts and the CSV export of the Reports screen (docs/ui.md "Relatórios e gráficos";
//! docs/reference/screens.md G15 and the extras after I1). A feature of the clone over the
//! original: it reads the figures the simulation already keeps and adds nothing to the rules.
//!
//! Where the series come from:
//!
//! * revenue, expenses and profit: the 13 ledger categories of each archived year (finance.md 1,
//!   16.2) plus the year to date as a last, partial point;
//! * share price, book value, revenue, earnings and dividend per share and the weighted 5-year
//!   return: the archived metrics of finance.md 16.2 plus the live value;
//! * cash, debt and book value of the company: the save keeps no history of them, so the game
//!   samples every company once a game month ([`Samples`]; kept in memory for the session, not
//!   saved, like the event history).
//!
//! This module is pure (no UI, no clock): the drawing is a raster of coloured cells that the
//! UI shows with `Kind::Cells`.

use rt2_ui::Color;

use crate::info::{CompanyReport, NCAT, Snapshot};

/// The twelve charted figures, in button order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    Revenue,
    Expenses,
    Profit,
    Cash,
    Debt,
    Equity,
    Price,
    BookPs,
    RevPs,
    Eps,
    DivPs,
    Return5,
}

pub const METRICS: [Metric; 12] = [
    Metric::Revenue,
    Metric::Expenses,
    Metric::Profit,
    Metric::Cash,
    Metric::Debt,
    Metric::Equity,
    Metric::Price,
    Metric::BookPs,
    Metric::RevPs,
    Metric::Eps,
    Metric::DivPs,
    Metric::Return5,
];

/// How a metric's values are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Money,
    PerShare,
    Percent,
}

impl Metric {
    /// The LNG id and English fallback of the label (the original's own words where it has them).
    pub fn label(self) -> (u32, &'static str) {
        match self {
            Metric::Revenue => (984, "Revenue"),
            Metric::Expenses => (35, "Expenses"),
            Metric::Profit => (36, "Profit"),
            Metric::Cash => (974, "Cash"),
            Metric::Debt => (979, "Debt"),
            Metric::Equity => (980, "Equity (Book Value)"),
            Metric::Price => (1005, "Share Price"),
            Metric::BookPs => (1006, "Book Value Per Share"),
            Metric::RevPs => (1007, "Revenue Per Share"),
            Metric::Eps => (1008, "Earnings Per Share"),
            Metric::DivPs => (1009, "Dividend Per Share"),
            Metric::Return5 => (1010, "5 Year Weighted Return"),
        }
    }

    pub fn unit(self) -> Unit {
        match self {
            Metric::Revenue
            | Metric::Expenses
            | Metric::Profit
            | Metric::Cash
            | Metric::Debt
            | Metric::Equity => Unit::Money,
            Metric::Return5 => Unit::Percent,
            _ => Unit::PerShare,
        }
    }

    /// Whether the series needs the monthly samples of the session.
    pub fn sampled(self) -> bool {
        matches!(
            self,
            Metric::Cash | Metric::Debt | Metric::Equity | Metric::Price
        )
    }
}

/// A short form for axis labels and the legend: `$1.5M`, `$340K`, `$12`, `15%`.
pub fn format_axis(unit: Unit, v: f64) -> String {
    match unit {
        Unit::Percent => format!("{:.0}%", v * 100.0),
        Unit::PerShare => {
            if v.abs() >= 100.0 {
                format!("${v:.0}")
            } else {
                format!("${v:.1}")
            }
        }
        Unit::Money => {
            let a = v.abs();
            let sign = if v < 0.0 { "-" } else { "" };
            if a >= 1e9 {
                format!("{sign}${:.1}B", a / 1e9)
            } else if a >= 1e6 {
                format!("{sign}${:.1}M", a / 1e6)
            } else if a >= 1e3 {
                format!("{sign}${:.0}K", a / 1e3)
            } else {
                format!("{sign}${a:.0}")
            }
        }
    }
}

// ---- time and samples -------------------------------------------------------------------------

/// The chart's time axis: years with the month and day as a fraction (12 months of 28 days).
pub fn time_of(year: i32, month: i32, day: i32) -> f32 {
    year as f32 + (((month - 1) * 28 + (day - 1)) as f32) / 336.0
}

/// Year and month (1..=12) of a point of the axis.
pub fn ym_of(t: f32) -> (i32, i32) {
    let y = (t + 1e-4).floor();
    let m = (((t - y) * 12.0 + 1e-3).floor() as i32 + 1).clamp(1, 12);
    (y as i32, m)
}

/// The monthly samples of the session: one row per game month with the balances of every
/// company then.
#[derive(Clone, Debug, Default)]
pub struct Samples {
    rows: Vec<SampleRow>,
}

#[derive(Clone, Debug)]
struct SampleRow {
    ym: (i32, i32),
    t: f32,
    companies: Vec<Snapshot>,
}

impl Samples {
    /// Records the balances of `(year, month)`; true when the row is new. A month already
    /// sampled is ignored; a date before the last sample (a loaded game) restarts the history.
    pub fn record(&mut self, year: i32, month: i32, day: i32, companies: Vec<Snapshot>) -> bool {
        let ym = (year, month);
        if let Some(last) = self.rows.last() {
            if ym == last.ym {
                return false;
            }
            if ym < last.ym {
                self.rows.clear();
            }
        }
        self.rows.push(SampleRow {
            ym,
            t: time_of(year, month, day),
            companies,
        });
        true
    }

    /// Whether the month `(year, month)` still has to be sampled.
    pub fn wants(&self, year: i32, month: i32) -> bool {
        self.rows.last().is_none_or(|l| l.ym != (year, month))
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
}

// ---- series ------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub t: f32,
    pub v: f64,
    /// The year to date (drawn with a hollow marker).
    pub partial: bool,
}

/// One company's line.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub slot: u32,
    /// Position of the company in the screen's list (picks colour, marker and dash).
    pub index: usize,
    pub name: String,
    pub human: bool,
    pub points: Vec<Point>,
}

fn pt(t: f32, v: f64, partial: bool) -> Option<Point> {
    v.is_finite().then_some(Point { t, v, partial })
}

/// The series of `metric` for every company that has at least one point.
pub fn build_series(
    metric: Metric,
    companies: &[CompanyReport],
    samples: &Samples,
    now: f32,
) -> Vec<Series> {
    let mut out = Vec::new();
    for (index, c) in companies.iter().enumerate() {
        let mut pts: Vec<Point> = Vec::new();
        let ledger = |cats: &[f64; NCAT]| -> f64 {
            match metric {
                Metric::Revenue => cats[..5].iter().sum(),
                Metric::Expenses => -cats[5..].iter().sum::<f64>(),
                _ => cats.iter().sum(),
            }
        };
        let sampled = |f: &dyn Fn(&Snapshot) -> Option<f64>, pts: &mut Vec<Point>| {
            for row in &samples.rows {
                if let Some(s) = row.companies.iter().find(|s| s.slot == c.slot)
                    && let Some(v) = f(s)
                {
                    pts.extend(pt(row.t, v, false));
                }
            }
        };
        match metric {
            Metric::Revenue | Metric::Expenses | Metric::Profit => {
                for y in c.years.iter().rev() {
                    pts.extend(pt(y.year as f32 + 1.0, ledger(&y.cats), false));
                }
                pts.extend(pt(now, ledger(&c.ytd), true));
            }
            Metric::Cash => {
                sampled(&|s| Some(s.cash), &mut pts);
                pts.extend(pt(now, c.snapshot.cash, false));
            }
            Metric::Debt => {
                sampled(&|s| Some(s.debt), &mut pts);
                pts.extend(pt(now, c.snapshot.debt, false));
            }
            Metric::Equity => {
                sampled(&|s| Some(s.equity), &mut pts);
                pts.extend(pt(now, c.snapshot.equity, false));
            }
            Metric::Price => {
                for y in c.years.iter().rev() {
                    pts.extend(pt(y.year as f32 + 1.0, y.metrics[0], false));
                }
                sampled(&|s| s.price, &mut pts);
                if let Some(p) = c.snapshot.price {
                    pts.extend(pt(now, p, false));
                }
            }
            Metric::BookPs | Metric::RevPs | Metric::Eps | Metric::DivPs | Metric::Return5 => {
                let m = match metric {
                    Metric::BookPs => 1,
                    Metric::RevPs => 2,
                    Metric::Eps => 3,
                    Metric::DivPs => 4,
                    _ => 5,
                };
                for y in c.years.iter().rev() {
                    pts.extend(pt(y.year as f32 + 1.0, y.metrics[m], false));
                }
                if m < 5
                    && let Some(live) = c.live
                {
                    pts.extend(pt(now, live[m], false));
                }
            }
        }
        // The same instant twice (a sample on the day of the live point): keep the later one.
        pts.sort_by(|a, b| a.t.total_cmp(&b.t));
        let mut dedup: Vec<Point> = Vec::with_capacity(pts.len());
        for p in pts {
            match dedup.last_mut() {
                Some(l) if (l.t - p.t).abs() < 1e-4 => *l = p,
                _ => dedup.push(p),
            }
        }
        if !dedup.is_empty() {
            out.push(Series {
                slot: c.slot,
                index,
                name: c.name.clone(),
                human: c.human,
                points: dedup,
            });
        }
    }
    out
}

/// The time of the point nearest to `t` among the shown series.
pub fn nearest_time(series: &[&Series], t: f32) -> Option<f32> {
    series
        .iter()
        .flat_map(|s| s.points.iter())
        .map(|p| p.t)
        .min_by(|a, b| (a - t).abs().total_cmp(&(b - t).abs()))
}

/// The value of `s` at exactly the time `t` (the points the cursor snapped to).
pub fn value_at(s: &Series, t: f32) -> Option<Point> {
    s.points.iter().copied().find(|p| (p.t - t).abs() < 1e-4)
}

// ---- axes --------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub min: f64,
    pub max: f64,
    pub ticks: Vec<f64>,
}

/// A value axis that includes zero, with round ticks (about four intervals).
pub fn nice_axis(lo: f64, hi: f64) -> Axis {
    let (mut lo, mut hi) = (lo.min(0.0), hi.max(0.0));
    if !(lo.is_finite() && hi.is_finite()) || (hi - lo).abs() < 1e-12 {
        (lo, hi) = (0.0, 1.0);
    }
    let raw = (hi - lo) / 4.0;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = mag
        * if norm <= 1.0 {
            1.0
        } else if norm <= 2.0 {
            2.0
        } else if norm <= 2.5 {
            2.5
        } else if norm <= 5.0 {
            5.0
        } else {
            10.0
        };
    let min = (lo / step).floor() * step;
    let max = (hi / step).ceil() * step;
    let n = ((max - min) / step).round() as i32;
    let ticks = (0..=n).map(|i| min + f64::from(i) * step).collect();
    Axis { min, max, ticks }
}

/// What the chart shows: the time range, the ticks of both axes.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartView {
    pub t0: f32,
    pub t1: f32,
    /// Times of the labelled ticks on the x axis (year starts, or month starts for a short span).
    pub x_ticks: Vec<f32>,
    pub y: Axis,
}

pub fn view_of(series: &[&Series]) -> ChartView {
    let mut t0 = f32::MAX;
    let mut t1 = f32::MIN;
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for p in series.iter().flat_map(|s| s.points.iter()) {
        t0 = t0.min(p.t);
        t1 = t1.max(p.t);
        lo = lo.min(p.v);
        hi = hi.max(p.v);
    }
    if t0 > t1 {
        (t0, t1) = (0.0, 1.0);
        (lo, hi) = (0.0, 1.0);
    }
    if t1 - t0 < 1.0 / 12.0 {
        // One instant: show a month around it.
        t0 -= 1.0 / 24.0;
        t1 += 1.0 / 24.0;
    }
    let span = t1 - t0;
    let mut x_ticks = Vec::new();
    if span >= 1.5 {
        let step = (span / 7.0).ceil().max(1.0) as i32;
        let first = t0.ceil() as i32;
        let mut y = first;
        while y as f32 <= t1 + 1e-4 {
            x_ticks.push(y as f32);
            y += step;
        }
    } else {
        let step = ((span * 12.0) / 7.0).ceil().max(1.0) as i32;
        let mut m = (t0 * 12.0).ceil() as i32;
        while m as f32 / 12.0 <= t1 + 1e-4 {
            x_ticks.push(m as f32 / 12.0);
            m += step;
        }
    }
    ChartView {
        t0,
        t1,
        x_ticks,
        y: nice_axis(lo, hi),
    }
}

// ---- the raster --------------------------------------------------------------------------------

/// Okabe-Ito colours (distinguishable with the common colour-vision deficiencies), slightly
/// lifted for the dark chart background.
pub const PALETTE: [Color; 8] = [
    Color::rgb(0xe6, 0x9f, 0x00),
    Color::rgb(0x56, 0xb4, 0xe9),
    Color::rgb(0x00, 0x9e, 0x73),
    Color::rgb(0xf0, 0xe4, 0x42),
    Color::rgb(0x3c, 0x8c, 0xe0),
    Color::rgb(0xd5, 0x5e, 0x00),
    Color::rgb(0xcc, 0x79, 0xa7),
    Color::rgb(0xd0, 0xd0, 0xd0),
];

/// Marker shapes: circle, square, triangle, diamond, cross, plus.
pub const MARKERS: usize = 6;
/// Line styles: solid, dashed, dotted.
pub const DASHES: usize = 3;

/// The look of the series of the company at `index`: colour, marker and dash pattern. Colour,
/// marker and dash repeat with different periods (8, 6, 3), so two companies of a game of up to
/// 24 never share all three, and the shape tells them apart where the colour cannot.
pub fn style_of(index: usize) -> (Color, usize, usize) {
    (PALETTE[index % 8], index % MARKERS, index % DASHES)
}

pub const BG: Color = Color(0x0c, 0x0f, 0x0c, 0xe0);
const GRID: Color = Color(0x58, 0x52, 0x40, 0xff);
const ZERO: Color = Color(0xa0, 0x96, 0x78, 0xff);
const CURSOR: Color = Color(0xf0, 0xe8, 0xd0, 0xff);

struct Canvas {
    cols: usize,
    rows: usize,
    cells: Vec<Color>,
}

impl Canvas {
    fn put(&mut self, x: i32, y: i32, c: Color) {
        if x >= 0 && y >= 0 && (x as usize) < self.cols && (y as usize) < self.rows {
            self.cells[y as usize * self.cols + x as usize] = c;
        }
    }

    fn line(&mut self, a: (i32, i32), b: (i32, i32), c: Color, dash: usize, width: i32) {
        // Bresenham with a step counter for the dash pattern.
        let (mut x, mut y) = a;
        let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
        let (sx, sy) = (
            if a.0 < b.0 { 1 } else { -1 },
            if a.1 < b.1 { 1 } else { -1 },
        );
        let mut err = dx + dy;
        let mut n = 0usize;
        loop {
            let on = match dash {
                0 => true,
                1 => n % 9 < 6,
                _ => n % 4 < 2,
            };
            if on {
                for ox in 0..width {
                    for oy in 0..width {
                        self.put(x + ox, y + oy, c);
                    }
                }
            }
            if (x, y) == b {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
            n += 1;
        }
    }

    fn marker(&mut self, at: (i32, i32), shape: usize, c: Color, hollow: bool) {
        let r = 3i32;
        for oy in -r..=r {
            for ox in -r..=r {
                if !shape_has(shape, ox, oy) {
                    continue;
                }
                // A hollow marker (the year to date) keeps only the cells on the shape's edge;
                // the thin shapes (cross, plus) are all edge already.
                if hollow
                    && shape < 4
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .all(|(dx, dy)| shape_has(shape, ox + dx, oy + dy))
                {
                    continue;
                }
                self.put(at.0 + ox, at.1 + oy, c);
            }
        }
    }
}

/// Whether the cell `(ox, oy)` around a marker's centre belongs to the shape (radius 3).
fn shape_has(shape: usize, ox: i32, oy: i32) -> bool {
    let r = 3i32;
    if ox.abs() > r || oy.abs() > r {
        return false;
    }
    match shape {
        0 => ox * ox + oy * oy <= r * r + 1,
        1 => true,
        2 => ox.abs() <= (oy + r + 1) / 2,
        3 => ox.abs() + oy.abs() <= r,
        4 => ox.abs() == oy.abs(),
        _ => ox == 0 || oy == 0,
    }
}

/// The cell of a point inside the plot area of a raster of `cols` x `rows`.
pub fn cell_of(view: &ChartView, cols: usize, rows: usize, t: f32, v: f64) -> (i32, i32) {
    let m = 5.0;
    let w = (cols as f32 - 1.0 - 2.0 * m).max(1.0);
    let h = (rows as f32 - 1.0 - 2.0 * m).max(1.0);
    let fx = ((t - view.t0) / (view.t1 - view.t0)).clamp(0.0, 1.0);
    let fy = ((v - view.y.min) / (view.y.max - view.y.min)).clamp(0.0, 1.0) as f32;
    (
        (m + fx * w).round() as i32,
        (m + (1.0 - fy) * h).round() as i32,
    )
}

/// The time under a horizontal position `fx` (0..1) of the raster.
pub fn time_at(view: &ChartView, cols: usize, fx: f32) -> f32 {
    let m = 5.0;
    let w = (cols as f32 - 1.0 - 2.0 * m).max(1.0);
    let f = ((fx * cols as f32 - m) / w).clamp(0.0, 1.0);
    view.t0 + f * (view.t1 - view.t0)
}

/// Draws the chart: background, grid, the series (the highlighted company last and thicker) and
/// the cursor line.
pub fn rasterize(
    series: &[&Series],
    highlight: Option<u32>,
    view: &ChartView,
    cols: usize,
    rows: usize,
    cursor: Option<f32>,
) -> Vec<Color> {
    let mut cv = Canvas {
        cols,
        rows,
        cells: vec![BG; cols * rows],
    };
    for &v in &view.y.ticks {
        let (_, y) = cell_of(view, cols, rows, view.t0, v);
        let c = if v == 0.0 { ZERO } else { GRID };
        for x in 0..cols as i32 {
            // The grid is dotted so the lines stay in front.
            if v == 0.0 || x % 3 == 0 {
                cv.put(x, y, c);
            }
        }
    }
    for &t in &view.x_ticks {
        let (x, _) = cell_of(view, cols, rows, t, 0.0);
        for y in 0..rows as i32 {
            if y % 3 == 0 {
                cv.put(x, y, GRID);
            }
        }
    }
    if let Some(t) = cursor {
        let (x, _) = cell_of(view, cols, rows, t, 0.0);
        for y in 0..rows as i32 {
            cv.put(x, y, CURSOR);
        }
    }
    let mut order: Vec<&&Series> = series.iter().collect();
    order.sort_by_key(|s| (Some(s.slot) == highlight, s.index));
    for s in order {
        let (color, marker, dash) = style_of(s.index);
        let width = if Some(s.slot) == highlight { 2 } else { 1 };
        let cells: Vec<(i32, i32)> = s
            .points
            .iter()
            .map(|p| cell_of(view, cols, rows, p.t, p.v))
            .collect();
        for w in cells.windows(2) {
            cv.line(w[0], w[1], color, dash, width);
        }
        for (p, &c) in s.points.iter().zip(&cells) {
            cv.marker(c, marker, color, p.partial);
        }
    }
    cv.cells
}

/// A small swatch of a series' look (line and marker) for the legend.
pub fn swatch(index: usize, cols: usize, rows: usize) -> Vec<Color> {
    let (color, marker, dash) = style_of(index);
    let mut cv = Canvas {
        cols,
        rows,
        cells: vec![Color(0, 0, 0, 0); cols * rows],
    };
    let y = rows as i32 / 2;
    cv.line((0, y), (cols as i32 - 1, y), color, dash, 1);
    cv.marker((cols as i32 / 2, y), marker, color, false);
    cv.cells
}

// ---- CSV ---------------------------------------------------------------------------------------

/// A CSV field: quoted when it holds a comma, a quote or a line break.
pub fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

fn csv_row(fields: &[String]) -> String {
    let mut s = fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",");
    s.push('\n');
    s
}

fn num(v: f64, unit: Unit) -> String {
    match unit {
        Unit::Money => format!("{v:.0}"),
        Unit::PerShare => format!("{v:.2}"),
        Unit::Percent => format!("{v:.4}"),
    }
}

/// The chart as rows `year,month,company,metric,value,year_to_date`.
pub fn chart_csv(metric: Metric, series: &[&Series]) -> String {
    let mut out = csv_row(
        &[
            "year",
            "month",
            "company",
            "metric",
            "value",
            "year_to_date",
        ]
        .map(str::to_owned),
    );
    let name = metric.label().1;
    for s in series {
        for p in &s.points {
            let (y, m) = ym_of(p.t);
            out.push_str(&csv_row(&[
                y.to_string(),
                m.to_string(),
                s.name.clone(),
                name.to_owned(),
                num(p.v, metric.unit()),
                u8::from(p.partial).to_string(),
            ]));
        }
    }
    out
}

/// The income statement by category: lifetime, year to date and the archived years (newest
/// first), with the revenue, expense and profit totals.
pub fn income_csv(c: &CompanyReport, labels: &[&str; NCAT]) -> String {
    let mut head = vec![
        "company".to_owned(),
        "category".to_owned(),
        "lifetime".to_owned(),
        "year_to_date".to_owned(),
    ];
    head.extend(c.years.iter().map(|y| y.year.to_string()));
    let mut out = csv_row(&head);
    let mut line = |label: &str, f: &dyn Fn(&[f64; NCAT]) -> f64| {
        let mut row = vec![
            c.name.clone(),
            label.to_owned(),
            num(f(&c.life), Unit::Money),
            num(f(&c.ytd), Unit::Money),
        ];
        row.extend(c.years.iter().map(|y| num(f(&y.cats), Unit::Money)));
        out.push_str(&csv_row(&row));
    };
    for (i, l) in labels.iter().enumerate().take(5) {
        line(l, &|v| v[i]);
    }
    line("Revenue", &|v| v[..5].iter().sum());
    for (i, l) in labels.iter().enumerate().skip(5) {
        line(l, &|v| v[i]);
    }
    line("Expenses", &|v| v[5..].iter().sum());
    line("Profit", &|v| v.iter().sum());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::info::reports::YearRow;

    fn report(slot: u32, name: &str, rev_per_year: &[(i32, f64)]) -> CompanyReport {
        let years = rev_per_year
            .iter()
            .rev()
            .map(|&(year, rev)| {
                let mut cats = [0.0; NCAT];
                cats[1] = rev;
                cats[8] = -rev / 4.0;
                YearRow {
                    year,
                    cats,
                    metrics: [rev / 1000.0, 5.0, 2.0, 1.0, 0.5, 0.1],
                }
            })
            .collect();
        CompanyReport {
            slot,
            name: name.to_owned(),
            human: slot == 1,
            founded: 1850,
            ytd: {
                let mut c = [0.0; NCAT];
                c[1] = 100.0;
                c
            },
            life: [0.0; NCAT],
            years,
            live: Some([30.0, 5.0, 2.0, 1.0, 0.5]),
            snapshot: Snapshot {
                slot,
                cash: 1000.0,
                debt: 200.0,
                equity: 5000.0,
                price: Some(30.0),
            },
        }
    }

    fn snap(slot: u32, cash: f64) -> Snapshot {
        Snapshot {
            slot,
            cash,
            debt: 0.0,
            equity: cash * 2.0,
            price: Some(cash / 10.0),
        }
    }

    #[test]
    fn axis_ticks_are_round_and_include_zero() {
        let a = nice_axis(120.0, 870.0);
        assert_eq!(a.min, 0.0);
        assert!(a.max >= 870.0);
        assert!(
            a.ticks
                .windows(2)
                .all(|w| (w[1] - w[0] - 250.0).abs() < 1e-9 || (w[1] - w[0] - 200.0).abs() < 1e-9)
        );
        let n = nice_axis(-5.0, 3.0);
        assert!(n.min <= -5.0 && n.max >= 3.0 && n.ticks.contains(&0.0));
        let flat = nice_axis(0.0, 0.0);
        assert_eq!((flat.min, flat.max), (0.0, 1.0));
    }

    #[test]
    fn money_axis_labels_are_short() {
        assert_eq!(format_axis(Unit::Money, 1_500_000.0), "$1.5M");
        assert_eq!(format_axis(Unit::Money, -340_000.0), "-$340K");
        assert_eq!(format_axis(Unit::Money, 12.0), "$12");
        assert_eq!(format_axis(Unit::Percent, 0.15), "15%");
    }

    #[test]
    fn income_series_use_the_archive_and_end_with_the_year_to_date() {
        let c = report(1, "A", &[(1875, 400.0), (1876, 600.0)]);
        let now = time_of(1877, 3, 1);
        let rev = build_series(
            Metric::Revenue,
            std::slice::from_ref(&c),
            &Samples::default(),
            now,
        );
        assert_eq!(rev.len(), 1);
        let p = &rev[0].points;
        assert_eq!(p.len(), 3);
        assert_eq!((p[0].t, p[0].v), (1876.0, 400.0));
        assert_eq!((p[1].t, p[1].v), (1877.0, 600.0));
        assert!(p[2].partial && p[2].v == 100.0);
        let exp = build_series(
            Metric::Expenses,
            std::slice::from_ref(&c),
            &Samples::default(),
            now,
        );
        assert_eq!(
            exp[0].points[0].v, 100.0,
            "expenses are charted as a positive cost"
        );
        let profit = build_series(
            Metric::Profit,
            std::slice::from_ref(&c),
            &Samples::default(),
            now,
        );
        assert_eq!(profit[0].points[0].v, 300.0);
    }

    #[test]
    fn balance_series_come_from_the_samples_and_the_live_point() {
        let c = report(2, "B", &[]);
        let mut s = Samples::default();
        assert!(s.record(1877, 1, 5, vec![snap(2, 100.0)]));
        assert!(
            !s.record(1877, 1, 20, vec![snap(2, 999.0)]),
            "one row per month"
        );
        assert!(s.record(1877, 2, 3, vec![snap(2, 200.0)]));
        let now = time_of(1877, 2, 10);
        let cash = build_series(Metric::Cash, std::slice::from_ref(&c), &s, now);
        let v: Vec<f64> = cash[0].points.iter().map(|p| p.v).collect();
        assert_eq!(v, vec![100.0, 200.0, 1000.0]);
        // A date before the last sample (a load) starts over.
        assert!(s.record(1870, 6, 1, vec![snap(2, 5.0)]));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn the_return_series_has_no_live_point() {
        let c = report(1, "A", &[(1876, 600.0)]);
        let now = time_of(1877, 3, 1);
        let r = build_series(
            Metric::Return5,
            std::slice::from_ref(&c),
            &Samples::default(),
            now,
        );
        assert_eq!(r[0].points.len(), 1);
        let b = build_series(
            Metric::BookPs,
            std::slice::from_ref(&c),
            &Samples::default(),
            now,
        );
        assert_eq!(b[0].points.len(), 2);
    }

    #[test]
    fn companies_have_distinct_looks() {
        let looks: Vec<_> = (0..24).map(style_of).collect();
        for i in 0..24 {
            for j in (i + 1)..24 {
                assert_ne!(looks[i], looks[j], "{i} and {j}");
            }
        }
    }

    #[test]
    fn the_raster_draws_lines_and_markers_in_the_series_colour() {
        let a = report(1, "A", &[(1875, 400.0), (1876, 600.0), (1877, 500.0)]);
        let b = report(2, "B", &[(1875, 100.0), (1876, 200.0), (1877, 300.0)]);
        let all = [a, b];
        let now = time_of(1878, 1, 1);
        let series = build_series(Metric::Revenue, &all, &Samples::default(), now);
        let refs: Vec<&Series> = series.iter().collect();
        let view = view_of(&refs);
        assert_eq!(view.t0, 1876.0);
        assert!(view.x_ticks.contains(&1877.0));
        let (cols, rows) = (200, 100);
        let cells = rasterize(&refs, Some(1), &view, cols, rows, None);
        assert_eq!(cells.len(), cols * rows);
        for s in &series {
            let (color, ..) = style_of(s.index);
            assert!(cells.contains(&color), "series {} is drawn", s.name);
        }
        // The first point of A sits at its cell and carries a marker (a filled circle).
        let (x, y) = cell_of(
            &view,
            cols,
            rows,
            series[0].points[0].t,
            series[0].points[0].v,
        );
        assert_eq!(cells[y as usize * cols + x as usize], style_of(0).0);
        // The cursor draws a full-height line.
        let with_cursor = rasterize(&refs, None, &view, cols, rows, Some(1877.0));
        let (cx, _) = cell_of(&view, cols, rows, 1877.0, 0.0);
        assert!((0..rows).all(|r| with_cursor[r * cols + cx as usize] != BG));
    }

    #[test]
    fn a_hollow_marker_keeps_only_its_edge() {
        for shape in 0..4 {
            let paint = |hollow: bool| {
                let mut cv = Canvas {
                    cols: 9,
                    rows: 9,
                    cells: vec![BG; 81],
                };
                cv.marker((4, 4), shape, PALETTE[0], hollow);
                cv.cells
            };
            let (full, hollow) = (paint(false), paint(true));
            let count = |c: &[Color]| c.iter().filter(|x| **x != BG).count();
            assert!(
                count(&hollow) > 0 && count(&hollow) < count(&full),
                "shape {shape}"
            );
            assert_eq!(
                hollow[4 * 9 + 4],
                BG,
                "the centre of shape {shape} is empty"
            );
            assert_ne!(full[4 * 9 + 4], BG);
        }
    }

    #[test]
    fn the_year_to_date_point_is_marked_partial_in_the_series() {
        let a = report(1, "A", &[(1876, 600.0)]);
        let all = [a];
        let series = build_series(
            Metric::Revenue,
            &all,
            &Samples::default(),
            time_of(1877, 6, 1),
        );
        assert!(series[0].points.last().unwrap().partial);
        assert!(!series[0].points[0].partial);
    }

    #[test]
    fn csv_quotes_what_needs_it() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("Baltimore, Ohio"), "\"Baltimore, Ohio\"");
        assert_eq!(csv_field("the \"Big\" one"), "\"the \"\"Big\"\" one\"");
    }

    #[test]
    fn the_chart_csv_has_one_row_per_point() {
        let a = report(1, "A, Inc", &[(1876, 600.0)]);
        let all = [a];
        let series = build_series(
            Metric::Revenue,
            &all,
            &Samples::default(),
            time_of(1877, 3, 1),
        );
        let refs: Vec<&Series> = series.iter().collect();
        let csv = chart_csv(Metric::Revenue, &refs);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "year,month,company,metric,value,year_to_date");
        assert_eq!(lines[1], "1877,1,\"A, Inc\",Revenue,600,0");
        assert_eq!(lines[2], "1877,3,\"A, Inc\",Revenue,100,1");
    }

    #[test]
    fn the_income_csv_totals_match_the_categories() {
        let c = report(1, "A", &[(1875, 400.0), (1876, 600.0)]);
        let labels = [
            "Passengers",
            "Freight",
            "Station",
            "Industry Profits",
            "Miscellaneous",
            "Track Maintenance",
            "Engine Maintenance",
            "Car Maintenance",
            "Fuel",
            "Interest",
            "Overhead",
            "Salaries",
            "Miscellaneous",
        ];
        let csv = income_csv(&c, &labels);
        let rows: Vec<Vec<&str>> = csv.lines().map(|l| l.split(',').collect()).collect();
        assert_eq!(
            rows[0],
            [
                "company",
                "category",
                "lifetime",
                "year_to_date",
                "1876",
                "1875"
            ]
        );
        let find = |name: &str| rows.iter().find(|r| r[1] == name).unwrap().clone();
        assert_eq!(find("Freight")[4], "600");
        assert_eq!(find("Revenue")[4], "600");
        assert_eq!(find("Expenses")[4], "-150");
        assert_eq!(find("Profit")[4], "450");
        assert_eq!(find("Profit")[3], "100");
    }
}
