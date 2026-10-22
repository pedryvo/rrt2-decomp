//! Read-only information screens over the simulation state: finance, stock market, trains,
//! stations, cities, industries and territories (docs/ui.md "Telas de informacao").
//!
//! This module only turns the save's records into text tables ([`Page`], [`Table`]); the widget
//! layout lives in `state/info_ui.rs`. Nothing here writes to the world. Field offsets come from
//! the specs of the in-memory (0x2348) record layout: `finance.md` 1 (company), `trains.md` 1.1 and
//! 10.1 (trains, cars, stops), `terrain-track.md` 3.4 (station improvement bits), `cargo.md` 2
//! (station and industry records), `world-growth.md` 2.1/2.2 (places, buildings). A file in the old
//! 0x233C layout keeps only names and positions: the loader's record conversion is not decoded, so
//! every other field is shown as "-" (TODO: convert on load) and the pages carry a banner.

use rt2_formats::map::{MAGIC_2348, Pool, Save};
use rt2_sim::cargo::{IndustryType, NO_CARGO};
use rt2_sim::data::GameData;
use rt2_sim::finance::{self, Bond, Ctx};

use crate::text::Texts;

// ---- text tables ------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Col {
    pub head: String,
    /// Relative width.
    pub weight: f32,
    pub right: bool,
}

/// A grid of strings; `ids[r]` is the record (pool slot) of row `r` for selectable tables.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub cols: Vec<Col>,
    pub rows: Vec<Vec<String>>,
    pub ids: Vec<u32>,
}

impl Table {
    fn new(cols: &[(&str, f32, bool)]) -> Table {
        Table {
            cols: cols
                .iter()
                .map(|&(h, w, r)| Col {
                    head: h.to_owned(),
                    weight: w,
                    right: r,
                })
                .collect(),
            rows: Vec::new(),
            ids: Vec::new(),
        }
    }

    fn push(&mut self, id: u32, row: Vec<String>) {
        debug_assert_eq!(row.len(), self.cols.len());
        self.ids.push(id);
        self.rows.push(row);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Label / value lines.
    Facts(Vec<(String, String)>),
    Table {
        title: String,
        table: Table,
    },
    /// A wrapped remark (TODO markers for what is not decoded).
    Note(String),
}

/// A detail page: two columns of blocks (stacked into one on a narrow screen).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub title: String,
    pub subtitle: String,
    pub left: Vec<Block>,
    pub right: Vec<Block>,
}

/// A list screen: a selectable table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListPage {
    pub title: String,
    pub table: Table,
}

/// Row id of a total / subtotal row of a non-selectable table (drawn emphasised).
pub const TOTAL: u32 = u32::MAX;

/// What a map click found (`Info::pick`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Industry(u32),
    Station(u32),
    Place(u32),
    Territory(u32),
}

// ---- formatting -------------------------------------------------------------------------------

/// `$1,234,567` with a leading minus, like the original's money fields.
pub fn money(v: f64) -> String {
    let n = if v.is_finite() { v.round() as i64 } else { 0 };
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("{}${out}", if n < 0 { "-" } else { "" })
}

fn int(v: i64) -> String {
    money(v as f64).replace('$', "")
}

fn pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

const DASH: &str = "-";

// ---- raw record access ------------------------------------------------------------------------

struct Rec<'a>(&'a [u8]);

impl Rec<'_> {
    fn u8(&self, o: usize) -> u8 {
        self.0.get(o).copied().unwrap_or(0)
    }
    fn i8(&self, o: usize) -> i8 {
        self.u8(o) as i8
    }
    fn u16(&self, o: usize) -> u16 {
        u16::from_le_bytes([self.u8(o), self.u8(o + 1)])
    }
    fn i16(&self, o: usize) -> i16 {
        self.u16(o) as i16
    }
    fn i32(&self, o: usize) -> i32 {
        i32::from_le_bytes([self.u8(o), self.u8(o + 1), self.u8(o + 2), self.u8(o + 3)])
    }
    fn u32(&self, o: usize) -> u32 {
        self.i32(o) as u32
    }
    fn f32(&self, o: usize) -> f32 {
        f32::from_bits(self.u32(o))
    }
    fn f64(&self, o: usize) -> f64 {
        let mut b = [0u8; 8];
        for (i, v) in b.iter_mut().enumerate() {
            *v = self.u8(o + i);
        }
        f64::from_le_bytes(b)
    }
    /// A NUL-terminated Latin-1 string in `from..to`.
    fn text(&self, from: usize, to: usize) -> String {
        self.0
            .get(from..to.min(self.0.len()))
            .unwrap_or(&[])
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect()
    }
}

fn rec(pool: &Pool, slot: u32) -> Option<Rec<'_>> {
    pool.get(slot as usize).map(|r| Rec(&r.data))
}

// Company record offsets (finance.md 1).
const CO_NAME: usize = 4;
const CO_CHAIRMAN: usize = 0x23;
const CO_SHARES: usize = 0x2c;
const CO_DIVIDEND: usize = 0x30;
const CO_BOND_COUNT: usize = 0x40;
const CO_BONDS: usize = 0x44;
const CO_FOUNDED: usize = 0x148;
const CO_ACTIVE: usize = 0x16e;
const CO_CATS: usize = 0x4286;
const CO_ARCH_CATS: usize = 0x42ee;
const CO_YEAR_CATS: usize = 0x46fe;
const CO_ACCT: usize = 0x4766;
const CO_ARCH_METRICS: usize = 0x49b6;
/// Person record: human flag (finance.md 1, trains.md "Em aberto" 0x43c030).
const PE_HUMAN: usize = 0x122;

/// The finance rules' view of a raw company record (finance.md 1; same fields as the simulation).
fn company_from_record(r: &Rec) -> finance::Company {
    let mut c = finance::Company {
        shares: r.i32(CO_SHARES),
        dividend: r.f32(CO_DIVIDEND),
        pressure: r.f32(0x34),
        dividends_paid: r.f32(0x38),
        init_price: r.f32(0x3c),
        overhead: r.i32(0x138),
        chairman_salary: r.i32(0x134),
        founded: r.i32(CO_FOUNDED),
        bankrupt_year: r.i32(0x158),
        price_stamp: 0, // forces a recompute: the cached stamp may be stale
        price_cache: r.f64(0x4254),
        created: r.i32(0x425c),
        investor_mood: r.f32(0x4276),
        no_div_price: r.f32(0x427a),
        last_price: r.f32(0x427e),
        month_end_price: r.f32(0x4282),
        ..finance::Company::default()
    };
    for (i, v) in c.cats.iter_mut().enumerate() {
        *v = r.f64(CO_CATS + 8 * i);
    }
    for (i, v) in c.year_cats.iter_mut().enumerate() {
        *v = r.f64(CO_YEAR_CATS + 8 * i);
    }
    for (k, row) in c.arch_cats.iter_mut().enumerate() {
        for (i, v) in row.iter_mut().enumerate() {
            *v = r.f64(CO_ARCH_CATS + 0x68 * k + 8 * i);
        }
    }
    for (i, v) in c.acct.iter_mut().enumerate() {
        *v = r.f64(CO_ACCT + 8 * i);
    }
    for (k, row) in c.arch_metrics.iter_mut().enumerate() {
        for (i, v) in row.iter_mut().enumerate() {
            *v = r.f64(CO_ARCH_METRICS + 0x70 * k + 8 * i);
        }
    }
    let n = (r.i32(CO_BOND_COUNT).max(0) as usize).min(finance::MAX_BONDS);
    for k in 0..n {
        let o = CO_BONDS + 12 * k;
        c.bonds.push(Bond {
            principal: r.i32(o),
            maturity_year: r.i32(o + 4),
            rate: r.f32(o + 8),
        });
    }
    c
}

/// Income-statement line `i` (finance.md 1, categories 0..12) to its LNG label id: 0 Passengers
/// 961, 1 Freight 962, 2 Station 963, 3 Industry investments 971 ("Industry Profits"), 4 Misc 964,
/// 5 Track 965, 6 Engine 966, 7 Car 967, 8 Fuel 968, 9 Interest 969, 10 Overhead 970, 11 Salaries
/// 972, 12 Misc 973.
// TODO(finance.md 1): the LNG order of 971..973 vs the category index is inferred from the labels
// and the sign of each category in the Example save; the original's table was not read.
const CATEGORY_LABEL: [(u32, &str); 13] = [
    (961, "Passengers"),
    (962, "Freight"),
    (963, "Station"),
    (971, "Industry Profits"),
    (964, "Miscellaneous"),
    (965, "Track Maintenance"),
    (966, "Engine Maintenance"),
    (967, "Car Maintenance"),
    (968, "Fuel"),
    (969, "Interest"),
    (970, "Overhead"),
    (972, "Salaries"),
    (973, "Miscellaneous"),
];

/// Balance accounts 0..5 (finance.md 1): cash, buildings, land rights/stations, track, trains,
/// industry investments.
const ACCOUNT_LABEL: [(u32, &str); 6] = [
    (974, "Cash"),
    (975, "Buildings"),
    (976, "Land Rights"),
    (977, "Track"),
    (978, "Trains"),
    (981, "Industry Investments"),
];

/// Improvement lines that can be bought (terrain-track.md 3.4: 0..11 and 15..21); bit 12 and
/// 22+ are derived or disabled and not listed as buildings.
const PURCHASABLE: u32 = 0x0000_0fff | 0x003f_8000;

/// The simulation date and the tables the pages need.
pub struct Info<'a> {
    pub save: &'a Save,
    pub data: &'a GameData,
    pub texts: &'a Texts,
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub minutes: i32,
}

/// One company's derived finance numbers.
struct Standing {
    co: finance::Company,
    price: Option<i32>,
    rating: f64,
    book_ps: f64,
}

impl<'a> Info<'a> {
    pub fn detailed(&self) -> bool {
        self.save.layout.version >= MAGIC_2348
    }

    fn t(&self, id: u32, fb: &str) -> String {
        self.texts.get(id, fb)
    }

    /// One of the clone's own texts, translated (`docs/i18n.md`, `[app]`).
    fn a(&self, english: &str) -> String {
        self.texts.a(english)
    }

    /// Like [`Info::a`] with `%1`.. replaced by `args`.
    fn af(&self, english: &str, args: &[String]) -> String {
        self.texts.af(english, args)
    }

    fn banner(&self) -> Option<Block> {
        (!self.detailed()).then(|| {
            Block::Note(
                self.a("This save uses the old 0x233C record layout: only names and positions are decoded, the other fields show \"-\". TODO: record conversion on load."),
            )
        })
    }

    fn with_banner(&self, mut left: Vec<Block>) -> Vec<Block> {
        if let Some(b) = self.banner() {
            left.insert(0, b);
        }
        left
    }

    fn cargo_name(&self, c: u8) -> String {
        capitalize(&self.texts.table(
            "CARGO_TYPES",
            u32::from(c),
            &self.af("cargo %1", &[c.to_string()]),
        ))
    }

    // ---- companies ------------------------------------------------------------------------

    fn ctx(&self) -> Ctx<'_> {
        // Economic state 0..4 is a game global (`0x570717`) that is not in the decoded part of
        // the save: use the initial state 2 (cargo.md 9). TODO: read it from the state block.
        let state = 2usize;
        Ctx {
            tables: &self.data.finance,
            year: self.year,
            month: self.month,
            day: self.day,
            minutes: self.minutes,
            econ_state: state,
            prime_rate: finance::prime_rate_for_state(state),
            liquidity: 100.0,
            price_effect: 1.0,
            prime_effect_pct: 0,
            rating_effect: 0,
        }
    }

    fn standing(&self, r: &Rec) -> Standing {
        let mut co = company_from_record(r);
        let ctx = self.ctx();
        let (price, rating, book_ps) = if self.detailed() && co.shares > 0 {
            // Share price: quote without trading (finance.md 3.2); rating: finance.md 4.2.
            let price = co.trade_price(&ctx, 0, false);
            (
                Some(price),
                co.credit_rating(&ctx, 0),
                co.live_metric(&ctx, 1),
            )
        } else {
            (None, 0.0, 0.0)
        };
        Standing {
            co,
            price,
            rating,
            book_ps,
        }
    }

    fn grade(&self, rating: f64) -> String {
        let i = (rating.round().clamp(0.0, 10.0)) as u32;
        let fb = [
            "F", "D", "C", "CC", "CCC", "B", "BB", "BBB", "A", "AA", "AAA",
        ][i as usize];
        self.t(1020 + i, fb)
    }

    pub fn person_name(&self, slot: u32) -> String {
        rec(&self.save.persons, slot)
            .map(|p| p.text(8, 0x27))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DASH.to_owned())
    }

    fn is_human_company(&self, r: &Rec) -> bool {
        let chair = r.i32(CO_CHAIRMAN);
        chair > 0
            && rec(&self.save.persons, chair as u32).is_some_and(|p| p.u8(PE_HUMAN) != 0)
            && self.detailed()
    }

    /// The company the screens open on: the first with a human chairman, else the first.
    // TODO(finance.md 2, docs/ui.md): which company is the player's is not decoded.
    pub fn player_company(&self) -> Option<u32> {
        let detailed = self.detailed();
        let active =
            |r: &&rt2_formats::map::PoolRecord| !detailed || Rec(&r.data).i32(CO_ACTIVE) != 0;
        let first = |human: bool| {
            self.save
                .companies
                .ordered()
                .filter(active)
                .find(|r| !human || self.is_human_company(&Rec(&r.data)))
                .map(|r| r.slot as u32)
        };
        first(true)
            .or_else(|| first(false))
            .or_else(|| self.save.companies.ordered().next().map(|r| r.slot as u32))
    }

    pub fn company_name(&self, slot: u32) -> String {
        rec(&self.save.companies, slot)
            .map(|r| r.text(CO_NAME, CO_NAME + 0x1f))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DASH.to_owned())
    }

    /// Stock market overview (`stockdtl.win`, LNG 56 "Market Overview"): one row per company.
    pub fn stock_market(&self) -> ListPage {
        // Short headings: the full LNG labels (1005, 1009, 1006, 1011) do not fit a column.
        let mut t = Table::new(&[
            (&self.t(178, "Company"), 3.2, false),
            (&self.t(43, "Price"), 1.1, true),
            (&self.t(280, "Shares"), 1.3, true),
            (&self.t(45, "Share Value"), 1.9, true),
            (&self.a("Dividend"), 1.3, true),
            (&self.a("Book Value"), 1.4, true),
            (&self.a("Rating"), 1.0, true),
        ]);
        for r in self.save.companies.ordered() {
            let rr = Rec(&r.data);
            let name = self.company_name(r.slot as u32);
            let s = self.standing(&rr);
            let mark = if self.is_human_company(&rr) { " *" } else { "" };
            let shares = i64::from(s.co.shares);
            t.push(
                r.slot as u32,
                match s.price {
                    Some(p) => vec![
                        format!("{name}{mark}"),
                        money(f64::from(p)),
                        int(shares),
                        money(f64::from(p) * shares as f64),
                        format!("${:.2}", s.co.dividend),
                        money(s.book_ps),
                        self.grade(s.rating),
                    ],
                    None => vec![
                        name,
                        DASH.into(),
                        DASH.into(),
                        DASH.into(),
                        DASH.into(),
                        DASH.into(),
                        DASH.into(),
                    ],
                },
            );
        }
        ListPage {
            title: self.t(56, "Market Overview"),
            table: t,
        }
    }

    /// Company report: income statement (this year, last year, lifetime), balance sheet, stock
    /// figures and bonds (`compndtl.win` two-panel layout; finance.md 1, 2, 3, 4).
    pub fn finance_page(&self, slot: u32) -> Page {
        let Some(r) = rec(&self.save.companies, slot) else {
            return Page {
                title: self.t(111, "Finances"),
                left: self.with_banner(vec![Block::Note(self.a("No such company."))]),
                ..Page::default()
            };
        };
        let name = self.company_name(slot);
        let s = self.standing(&r);
        let co = &s.co;
        let d = self.detailed();
        let chair = {
            let c = r.i32(CO_CHAIRMAN);
            if c > 0 && d {
                self.person_name(c as u32)
            } else {
                DASH.to_owned()
            }
        };

        // Income statement. "This year" is the year-to-date ledger (`+0x4286`), "last year" the
        // newest archive row (`+0x42ee`), "lifetime" the never-reset accumulator (`+0x46fe`).
        let mut inc = Table::new(&[
            ("", 3.2, false),
            (&self.a("This year"), 1.5, true),
            (&self.a("Last year"), 1.5, true),
            (&self.t(2368, "Lifetime"), 1.5, true),
        ]);
        let sum = |c: &[f64; 13], a: usize, b: usize| c[a..b].iter().sum::<f64>();
        let cell = |v: f64| if d { money(v) } else { DASH.to_owned() };
        let mut row = |label: String, life: f64, now: f64, last: f64, id: u32| {
            inc.push(id, vec![label, cell(now), cell(last), cell(life)]);
        };
        let (now, last, life) = (&co.cats, &co.arch_cats[0], &co.year_cats);
        let lab = |i: usize| self.t(CATEGORY_LABEL[i].0, CATEGORY_LABEL[i].1);
        for i in 0..5 {
            row(lab(i), life[i], now[i], last[i], 0);
        }
        row(
            self.t(984, "Revenue"),
            sum(life, 0, 5),
            sum(now, 0, 5),
            sum(last, 0, 5),
            TOTAL,
        );
        for i in 5..13 {
            row(lab(i), life[i], now[i], last[i], 0);
        }
        row(
            self.t(35, "Expenses"),
            sum(life, 5, 13),
            sum(now, 5, 13),
            sum(last, 5, 13),
            TOTAL,
        );
        row(
            self.t(36, "Profit"),
            sum(life, 0, 13),
            sum(now, 0, 13),
            sum(last, 0, 13),
            TOTAL,
        );

        // Balance sheet: accounts 0..5, debt (-acct 6) and book value (-acct 7).
        let mut bal = Table::new(&[("", 3.0, false), ("", 1.6, true)]);
        for (i, (id, fb)) in ACCOUNT_LABEL.iter().enumerate() {
            bal.push(0, vec![self.t(*id, fb), cell(co.acct[i])]);
        }
        bal.push(TOTAL, vec![self.t(38, "Assets"), cell(co.total_assets())]);
        bal.push(
            0,
            vec![self.t(979, "Debt"), cell(-co.acct[finance::ACCT_DEBT])],
        );
        bal.push(
            TOTAL,
            vec![self.t(980, "Equity (Book Value)"), cell(co.equity())],
        );

        // Stock figures (finance.md 3: price; 1: book value, revenue and profit per share).
        let ctx = self.ctx();
        let per_share = |m: usize| {
            if d && co.shares > 0 {
                format!("${:.2}", co.live_metric(&ctx, m))
            } else {
                DASH.to_owned()
            }
        };
        let facts = vec![
            (
                self.t(1005, "Share Price"),
                s.price.map_or(DASH.into(), |p| money(f64::from(p))),
            ),
            (
                self.t(1006, "Book Value Per Share"),
                s.price.map_or(DASH.into(), |_| money(s.book_ps)),
            ),
            (self.t(1007, "Revenue Per Share"), per_share(2)),
            (self.t(1008, "Earnings Per Share"), per_share(3)),
            (
                self.t(1009, "Dividend Per Share"),
                if d {
                    format!("${:.2}", co.dividend)
                } else {
                    DASH.into()
                },
            ),
            (
                self.t(280, "Shares"),
                if d {
                    int(i64::from(co.shares))
                } else {
                    DASH.into()
                },
            ),
            (
                self.t(45, "Share Value"),
                s.price
                    .map_or(DASH.into(), |p| money(f64::from(p) * f64::from(co.shares))),
            ),
            (
                self.t(1011, "Credit Rating"),
                s.price.map_or(DASH.into(), |_| self.grade(s.rating)),
            ),
            (
                self.a("Founded"),
                if d {
                    co.founded.to_string()
                } else {
                    DASH.into()
                },
            ),
        ];

        let mut bonds = Table::new(&[
            (&self.t(59, "Amount"), 1.6, true),
            (&self.t(58, "Year Matures"), 1.4, true),
            (&self.t(60, "Interest Rate"), 1.4, true),
        ]);
        for b in &co.bonds {
            bonds.push(
                0,
                vec![
                    money(f64::from(b.principal)),
                    b.maturity_year.to_string(),
                    pct(f64::from(b.rate)),
                ],
            );
        }
        let mut right = vec![
            Block::Table {
                title: self.t(117, "Balance Sheet"),
                table: bal,
            },
            Block::Facts(facts),
        ];
        if d {
            right.push(if co.bonds.is_empty() {
                Block::Facts(vec![(self.t(57, "Bonds outstanding"), "0".into())])
            } else {
                Block::Table {
                    title: self.t(57, "Bonds outstanding"),
                    table: bonds,
                }
            });
        }
        Page {
            title: name,
            subtitle: format!(
                "{}: {}{}",
                self.t(112, "Chairman"),
                chair,
                if r.i32(CO_ACTIVE) == 0 && d {
                    "   (inactive)"
                } else {
                    ""
                }
            ),
            left: self.with_banner(vec![Block::Table {
                title: self.t(118, "Income Statement"),
                table: inc,
            }]),
            right,
        }
    }

    // ---- trains ---------------------------------------------------------------------------

    fn station_name(&self, slot: u32) -> String {
        rec(&self.save.stations, slot)
            .map(|r| r.text(26, 0x39))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DASH.to_owned())
    }

    fn train_name(&self, r: &Rec) -> String {
        let n = r.text(12, 0x37);
        // Unnamed trains (the sample save names only one) show their id.
        if n.is_empty() {
            self.af("Train %1", &[r.i32(0x3d).to_string()])
        } else {
            n
        }
    }

    /// The train's cars in consist order (trains.md 1.1: `train+0` first car, car `+2` the next
    /// toward the back).
    fn cars(&self, train: &Rec) -> Vec<Rec<'_>> {
        let mut out = Vec::new();
        let mut id = train.i32(0);
        while id > 0 && out.len() < 64 {
            let Some(c) = rec(&self.save.pool_70, id as u32) else {
                break;
            };
            let next = i32::from(c.i16(2));
            out.push(c);
            if next == id {
                break;
            }
            id = next;
        }
        out
    }

    /// Engine name (`engine_name_ids` `0x5534f4`, u16 per motor: an LNG id; trains.md 2.1).
    fn engine_name(&self, engine: i8) -> String {
        if engine < 0 {
            return DASH.into();
        }
        let id = u32::from(self.data.image.u16(0x0055_34f4 + 2 * engine as u32));
        self.texts
            .get(id, &self.af("Engine %1", &[engine.to_string()]))
    }

    fn vehicle_label(&self, car: &Rec) -> String {
        let Some(v) = self.data.vehicles.get(usize::from(car.u8(8))) else {
            return self.af("Vehicle %1", &[car.u8(8).to_string()]);
        };
        match v.species {
            0 => self.engine_name(v.engine),
            1 => self.a("Tender"),
            _ => match v.cargo {
                0x2c => self.a("Restaurant car"),
                0x2d => self.a("Caboose"),
                c if c == NO_CARGO => self.a("Car"),
                c => self.af("%1 car", &[self.cargo_name(c)]),
            },
        }
    }

    /// The train's locomotive name (first species-0 vehicle).
    fn train_engine(&self, cars: &[Rec]) -> String {
        cars.iter()
            .find_map(|c| {
                let v = self.data.vehicles.get(usize::from(c.u8(8)))?;
                (v.species == 0).then(|| self.engine_name(v.engine))
            })
            .unwrap_or_else(|| DASH.into())
    }

    /// The train's stops: station id and flag byte of each 14-byte entry (trains.md 10.1; the
    /// count is `+0x3b`, the entries are the "cars" extras of the save, map-gam.md 5.5).
    fn stops(&self, train: &Rec, extras: &[u8]) -> Vec<(u32, u8)> {
        let n = usize::from(train.u8(0x3b));
        extras
            .as_chunks::<14>()
            .0
            .iter()
            .take(n)
            .map(|c| (u32::from(u16::from_le_bytes([c[0], c[1]])), c[3]))
            .collect()
    }

    fn extras_of(&self, slot: u32) -> &[u8] {
        self.save
            .train_extras
            .iter()
            .find(|e| e.slot == slot as usize)
            .map_or(&[][..], |e| &e.cars_raw)
    }

    fn mph(&self, train: &Rec) -> String {
        // `+0x69`: current speed, mph in 16.16 fixed point (trains.md 1.1).
        self.af("%1 mph", &[(train.i32(0x69) >> 16).to_string()])
    }

    pub fn trains_list(&self) -> ListPage {
        let mut t = Table::new(&[
            (&self.t(216, "Name"), 2.4, false),
            (&self.t(178, "Company"), 3.2, false),
            (&self.a("Engine"), 2.2, false),
            (&self.a("Cars"), 0.8, true),
            (&self.a("Route"), 3.6, false),
            (&self.a("Speed"), 1.0, true),
            (&self.a("Age"), 0.7, true),
            (&self.t(36, "Profit"), 1.2, true),
        ]);
        let d = self.detailed();
        for r in self.save.trains.ordered() {
            let rr = Rec(&r.data);
            let cars = self.cars(&rr);
            let stops = self.stops(&rr, self.extras_of(r.slot as u32));
            let route = match (stops.first(), stops.last()) {
                (Some(a), Some(b)) if d => {
                    format!("{} - {}", self.station_name(a.0), self.station_name(b.0))
                }
                _ => DASH.into(),
            };
            t.push(
                r.slot as u32,
                vec![
                    self.train_name(&rr),
                    if d {
                        self.company_name(rr.i32(0x41).max(0) as u32)
                    } else {
                        DASH.into()
                    },
                    if d {
                        self.train_engine(&cars)
                    } else {
                        DASH.into()
                    },
                    if d {
                        cars.len().to_string()
                    } else {
                        DASH.into()
                    },
                    route,
                    if d { self.mph(&rr) } else { DASH.into() },
                    // `+0xbd`: purchase year (trains.md 1.1).
                    if d {
                        (self.year - i32::from(rr.u16(0xbd))).max(0).to_string()
                    } else {
                        DASH.into()
                    },
                    // TODO: the train's profit counter is not identified in trains.md.
                    DASH.into(),
                ],
            );
        }
        ListPage {
            title: self.t(978, "Trains"),
            table: t,
        }
    }

    pub fn train_page(&self, slot: u32) -> Page {
        let Some(rr) = rec(&self.save.trains, slot) else {
            return Page {
                title: self.t(978, "Trains"),
                ..Page::default()
            };
        };
        let d = self.detailed();
        let cars = self.cars(&rr);
        let stops = self.stops(&rr, self.extras_of(slot));
        let cur = i32::from(rr.i8(0x3c));
        let mut facts = vec![
            (
                self.t(178, "Company"),
                if d {
                    self.company_name(rr.i32(0x41).max(0) as u32)
                } else {
                    DASH.into()
                },
            ),
            (
                self.a("Engine"),
                if d {
                    self.train_engine(&cars)
                } else {
                    DASH.into()
                },
            ),
            (
                self.a("Purchased"),
                if d {
                    rr.u16(0xbd).to_string()
                } else {
                    DASH.into()
                },
            ),
            (
                self.a("Age"),
                if d {
                    self.af(
                        "%1 years",
                        &[(self.year - i32::from(rr.u16(0xbd))).max(0).to_string()],
                    )
                } else {
                    DASH.into()
                },
            ),
            (self.a("Speed"), if d { self.mph(&rr) } else { DASH.into() }),
            (
                self.a("Status"),
                if !d {
                    DASH.into()
                } else if rr.u8(0x99) != 0 {
                    self.a("stopped")
                } else {
                    self.a("running")
                },
            ),
            (
                self.a("Weight"),
                if d {
                    self.af("%1 t", &[rr.i32(0xaf).to_string()])
                } else {
                    DASH.into()
                },
            ),
            (self.t(36, "Profit"), DASH.to_owned()),
        ];
        if d && rr.u8(0xed) != 0 {
            facts.push((self.a("Condition"), self.a("broken down")));
        }
        let mut car_t = Table::new(&[
            ("#", 0.5, true),
            (&self.a("Vehicle"), 3.0, false),
            (&self.a("Cargo"), 2.0, false),
        ]);
        for (i, c) in cars.iter().enumerate() {
            // Car `+0x2d`: cargo lot id, 0 = empty; the lot's byte 0 is the cargo (cargo.md 2).
            let lot = c.u16(0x2d);
            let load = if lot == 0 {
                self.a("empty")
            } else {
                rec(&self.save.pool_7c, u32::from(lot))
                    .map_or(self.a("loaded"), |l| self.cargo_name(l.u8(0)))
            };
            car_t.push(
                i as u32,
                vec![(i + 1).to_string(), self.vehicle_label(c), load],
            );
        }
        let mut stop_t = Table::new(&[
            ("#", 0.5, true),
            (&self.a("Station"), 3.0, false),
            (&self.a("Wait"), 2.0, false),
        ]);
        for (i, (st, bits)) in stops.iter().enumerate() {
            // Stop bits (trains.md 10.1): 0x08 waypoint, 0xc0 wait (0x40 half full, 0x80 full).
            let wait = match bits & 0xc0 {
                0x40 => self.a("half full"),
                0x80 => self.a("full"),
                _ => self.a("no wait"),
            };
            let here = if i as i32 == cur { ">" } else { "" };
            stop_t.push(
                i as u32,
                vec![
                    format!("{}{}", i + 1, here),
                    if bits & 0x08 != 0 {
                        self.a("waypoint")
                    } else {
                        self.station_name(*st)
                    },
                    wait,
                ],
            );
        }
        Page {
            title: self.train_name(&rr),
            subtitle: self.t(978, "Trains"),
            left: self.with_banner(vec![
                Block::Facts(facts),
                Block::Table {
                    title: self.a("Consist"),
                    table: car_t,
                },
            ]),
            right: vec![
                Block::Table {
                    title: self.a("Route"),
                    table: stop_t,
                },
                Block::Note(
                    self.a("TODO: profit, cargo carried per stop and the schedule status are not decoded in the save yet."),
                ),
            ],
        }
    }

    // ---- stations -------------------------------------------------------------------------

    /// Improvement bit names (terrain-track.md 3.4: LNG 1043 + bit; bits 12 and 22+ are editor
    /// leftovers or disabled lines).
    fn improvement_names(&self, mask: u32) -> Vec<String> {
        (0..30u32)
            .filter(|b| PURCHASABLE & (1 << b) != 0 && mask & (1 << b) != 0)
            .map(|b| {
                self.texts.table(
                    "STATION_BUILDINGS",
                    b,
                    &self.af("Improvement bit %1", &[b.to_string()]),
                )
            })
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn station_size(&self, mask: u32) -> String {
        let b = if mask & 4 != 0 {
            2
        } else if mask & 2 != 0 {
            1
        } else {
            0
        };
        self.texts.table("STATION_BUILDINGS", b, &self.a("Station"))
    }

    /// Lots waiting at a station for a cargo: the chain head is `+0x61 + 2*c`, each lot links the
    /// next in its `+0x13` (cargo.md 2).
    fn waiting_lots(&self, st: &Rec, c: usize) -> usize {
        let mut id = u32::from(st.u16(0x61 + 2 * c));
        let mut n = 0;
        while id != 0 && n < 5000 {
            n += 1;
            id = rec(&self.save.pool_7c, id).map_or(0, |l| u32::from(l.u16(0x13)));
        }
        n
    }

    pub fn stations_list(&self) -> ListPage {
        let mut t = Table::new(&[
            (&self.t(216, "Name"), 2.6, false),
            (&self.a("Owner"), 2.4, false),
            (&self.t(545, "Size"), 1.6, false),
            (&self.a("Improvements"), 1.4, true),
            (&self.a("Cargo waiting"), 1.4, true),
            (&self.t(191, "Supply"), 1.1, true),
            (&self.t(192, "Demand"), 1.1, true),
        ]);
        let d = self.detailed();
        for r in self.save.stations.ordered() {
            let st = Rec(&r.data);
            let name = self.station_name(r.slot as u32);
            if !d {
                t.push(r.slot as u32, {
                    let mut v = vec![name];
                    v.extend((0..6).map(|_| DASH.to_owned()));
                    v
                });
                continue;
            }
            let mask = st.u32(0x49);
            let waiting: usize = (0..44).map(|c| self.waiting_lots(&st, c)).sum();
            let supply = (0..44).filter(|&c| st.u8(0x169 + c) != 0).count();
            let demand = (0..44).filter(|&c| st.u8(0x195 + c) != 0).count();
            t.push(
                r.slot as u32,
                vec![
                    name,
                    self.company_name(st.i32(0x39).max(0) as u32),
                    self.station_size(mask),
                    (mask & !7).count_ones().to_string(),
                    waiting.to_string(),
                    supply.to_string(),
                    demand.to_string(),
                ],
            );
        }
        ListPage {
            title: self.a("Stations"),
            table: t,
        }
    }

    pub fn station_page(&self, slot: u32) -> Page {
        let Some(st) = rec(&self.save.stations, slot) else {
            return Page {
                title: self.a("Stations"),
                ..Page::default()
            };
        };
        let d = self.detailed();
        let mask = st.u32(0x49);
        let facts = vec![
            (
                self.a("Owner"),
                if d {
                    self.company_name(st.i32(0x39).max(0) as u32)
                } else {
                    DASH.into()
                },
            ),
            (
                self.a("Location"),
                format!("{}, {}", st.i32(0xa), st.i32(0xe)),
            ),
            (
                self.t(545, "Size"),
                if d {
                    self.station_size(mask)
                } else {
                    DASH.into()
                },
            ),
            (
                self.a("Linked buildings"),
                if d {
                    st.u8(0x41).to_string()
                } else {
                    DASH.into()
                },
            ),
        ];
        let imps = if d {
            self.improvement_names(mask)
        } else {
            Vec::new()
        };
        let imp_facts: Vec<(String, String)> = if imps.is_empty() {
            vec![(
                self.a("Improvements"),
                if d { self.a("none") } else { DASH.into() },
            )]
        } else {
            imps.into_iter().map(|n| (n, self.a("built"))).collect()
        };
        // Cargo rows: any cargo produced, demanded or waiting (cargo.md 2: `+0x169` produced by
        // an industry (1) or a house (2), `+0x195` demanded, `+0x321` price level 0..9, `+0x271`
        // delivery counter).
        let mut cargo = Table::new(&[
            (&self.a("Cargo"), 2.0, false),
            (&self.t(191, "Supply"), 1.5, false),
            (&self.a("Waiting"), 1.3, true),
            (&self.t(192, "Demand"), 1.3, true),
            (&self.a("Level"), 1.0, true),
            (&self.a("Delivered"), 1.4, true),
        ]);
        if d {
            for c in 0..44usize {
                let (prod, dem, lots) = (
                    st.u8(0x169 + c),
                    st.u8(0x195 + c),
                    self.waiting_lots(&st, c),
                );
                if prod == 0 && dem == 0 && lots == 0 {
                    continue;
                }
                cargo.push(
                    c as u32,
                    vec![
                        self.cargo_name(c as u8),
                        match prod {
                            0 => DASH.to_owned(),
                            1 => self.a("industry"),
                            2 => self.a("houses"),
                            _ => self.af("code %1", &[prod.to_string()]),
                        },
                        lots.to_string(),
                        if dem != 0 {
                            format!("{:.1}", st.f32(0x1c1 + 4 * c))
                        } else {
                            DASH.to_owned()
                        },
                        if dem != 0 {
                            st.u8(0x321 + c).to_string()
                        } else {
                            DASH.into()
                        },
                        format!("{:.1}", st.f32(0x271 + 4 * c)),
                    ],
                );
            }
        }
        let mut right = Vec::new();
        if cargo.rows.is_empty() {
            right.push(Block::Note(if d {
                "No cargo is supplied, demanded or waiting at this station yet (the weekly \
                 supply and demand pass has not run)."
                    .to_owned()
            } else {
                self.a("Supply and demand are not decoded for the old 0x233C layout.")
            }));
        } else {
            right.push(Block::Table {
                title: format!("{} / {}", self.t(191, "Supply"), self.t(192, "Demand")),
                table: cargo,
            });
        }
        Page {
            title: self.station_name(slot),
            subtitle: self.a("Station"),
            left: self.with_banner(vec![
                Block::Facts(facts),
                Block::Facts(imp_facts),
                Block::Note(format!(
                    "Improvement mask {mask:#010x} (bits 12-14 and 22+ are derived or unused \
                     lines, terrain-track.md 3.4)"
                )),
            ]),
            right,
        }
    }

    // ---- cities and territories ------------------------------------------------------------

    fn size_name(&self, class: u8) -> String {
        self.texts.table(
            "CITY_SIZE",
            u32::from(class),
            &self.af("Class %1", &[class.to_string()]),
        )
    }

    fn place_kind(&self, kind: u8) -> String {
        self.texts.table(
            "CITY_TYPE",
            u32::from(kind),
            &self.af("Type %1", &[kind.to_string()]),
        )
    }

    fn place_name(&self, slot: u32) -> String {
        rec(&self.save.places, slot)
            .map(|r| r.text(0x16, 0x36))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DASH.to_owned())
    }

    pub fn places_list(&self) -> ListPage {
        let mut t = Table::new(&[
            (&self.t(216, "Name"), 2.6, false),
            (&self.a("Kind"), 1.2, false),
            (&self.t(545, "Size"), 1.4, false),
            (&self.a("Houses"), 0.9, true),
            (&self.a("Growth"), 0.9, true),
            (&self.a("Delivered"), 1.0, true),
            (&self.a("Shipped"), 1.0, true),
            (&self.a("Location"), 1.3, true),
        ]);
        let d = self.detailed();
        for r in self.save.places.ordered() {
            let p = Rec(&r.data);
            let kind = p.u8(0x10);
            let name = self.place_name(r.slot as u32);
            let loc = format!("{}, {}", p.i16(4), p.i16(6));
            if !d {
                let mut v = vec![name];
                v.extend((0..6).map(|_| DASH.to_owned()));
                v.push(loc);
                t.push(r.slot as u32, v);
                continue;
            }
            t.push(
                r.slot as u32,
                vec![
                    name,
                    self.place_kind(kind),
                    if kind <= 1 {
                        self.size_name(p.u8(0x2c))
                    } else {
                        DASH.into()
                    },
                    p.u8(0x48).to_string(),
                    format!("{}%", p.i32(0x2e)),
                    p.u16(0x3e).to_string(),
                    p.u16(0x40).to_string(),
                    loc,
                ],
            );
        }
        ListPage {
            title: self.t(103, "Cities"),
            table: t,
        }
    }

    pub fn place_page(&self, slot: u32) -> Page {
        let Some(p) = rec(&self.save.places, slot) else {
            return Page {
                title: self.t(103, "Cities"),
                ..Page::default()
            };
        };
        let d = self.detailed();
        let kind = p.u8(0x10);
        let mut facts = vec![
            (
                self.a("Kind"),
                if d {
                    self.place_kind(kind)
                } else {
                    DASH.into()
                },
            ),
            (
                self.t(545, "Size"),
                if d && kind <= 1 {
                    self.size_name(p.u8(0x2c))
                } else {
                    DASH.into()
                },
            ),
            (self.a("Location"), format!("{}, {}", p.i16(4), p.i16(6))),
            (
                self.a("Extent"),
                format!("{},{} to {},{}", p.i16(8), p.i16(10), p.i16(12), p.i16(14)),
            ),
        ];
        if d {
            facts.extend([
                (self.a("Houses"), p.u8(0x48).to_string()),
                (self.a("Growth index"), format!("{}%", p.i32(0x2e))),
                (self.a("Intensity"), format!("{}%", p.u16(0x46))),
                (self.a("Trend"), p.i8(0x49).to_string()),
                (self.a("Land tiles"), p.i32(0x32).to_string()),
                (
                    self.a("Connection bonus"),
                    if p.i32(0x36) == 0 {
                        self.a("none")
                    } else {
                        money(f64::from(p.i32(0x36)))
                    },
                ),
                (self.a("Lots delivered (yr)"), p.u16(0x3e).to_string()),
                (self.a("Lots shipped (yr)"), p.u16(0x40).to_string()),
            ]);
        }
        // Industries that belong to the place (building `+2` = place id).
        let mut inds = Table::new(&[
            (&self.a("Industry"), 3.0, false),
            (&self.a("Built"), 1.0, true),
            (&self.a("Rating"), 1.0, true),
        ]);
        for o in self.save.objects.ordered() {
            let b = Rec(&o.data);
            if u32::from(b.u16(2)) != slot || is_house(b.u8(7)) || b.u8(7) == 0 {
                continue;
            }
            inds.push(
                o.slot as u32,
                vec![
                    self.building_name(b.u8(7)),
                    b.u16(0x3c).to_string(),
                    b.u16(0x4b).to_string(),
                ],
            );
        }
        let mut right = Vec::new();
        if inds.rows.is_empty() {
            right.push(Block::Note(self.a("No industries belong to this place.")));
        } else {
            right.push(Block::Table {
                title: self.a("Industries"),
                table: inds,
            });
        }
        right.push(Block::Note(self.a(
            "TODO: population and cargo demand per place are not decoded in the save.",
        )));
        Page {
            title: self.place_name(slot),
            subtitle: self.t(103, "Cities"),
            left: self.with_banner(vec![Block::Facts(facts)]),
            right,
        }
    }

    pub fn territory_page(&self, slot: u32) -> Page {
        let Some(t) = rec(&self.save.territories, slot) else {
            return Page {
                title: self.t(2369, "Territories"),
                ..Page::default()
            };
        };
        let region = t.u8(0x2a);
        let in_region = |tile_region: u8| tile_region == region;
        let (w, h) = (self.save.map.width, self.save.map.height);
        let tiles = self
            .save
            .map
            .tiles
            .iter()
            .filter(|x| in_region(x.region))
            .count();
        let places = self
            .save
            .places
            .ordered()
            .filter(|p| {
                let (x, y) = (
                    usize::from(Rec(&p.data).u16(4)),
                    usize::from(Rec(&p.data).u16(6)),
                );
                x < w && y < h && self.save.map.tile(x, y).region == region
            })
            .count();
        let facts = vec![
            (self.a("Region"), region.to_string()),
            (self.a("Tiles"), int(tiles as i64)),
            (self.a("Cities and regions"), places.to_string()),
            (
                self.a("Industries"),
                self.save
                    .objects
                    .ordered()
                    .filter(|o| Rec(&o.data).u8(0x9d) == region && !is_house(Rec(&o.data).u8(7)))
                    .count()
                    .to_string(),
            ),
        ];
        Page {
            title: t.text(4, 0x24),
            subtitle: self.t(2369, "Territories"),
            left: self.with_banner(vec![Block::Facts(facts)]),
            right: vec![Block::Note(self.a(
                "TODO: territory effects, company rights and goodwill are not decoded here.",
            ))],
        }
    }

    // ---- industries -----------------------------------------------------------------------

    fn building_name(&self, ty: u8) -> String {
        // world-growth.md 2.3: LNG 920 + building type (0 = Station).
        self.texts.get(
            920 + u32::from(ty),
            &self.af("Building %1", &[ty.to_string()]),
        )
    }

    fn industry_type(&self, ty: u8) -> Option<IndustryType> {
        // TODO: the industry set (era) is read from the scenario header (`0x5704ac`), not
        // decoded; set 0 is the one the simulation uses too (fidelity-status.md).
        let mut t = self.data.industries.first()?.get(usize::from(ty))?.clone();
        t.rebuild_inputs(self.year);
        Some(t)
    }

    pub fn industries_list(&self) -> ListPage {
        let mut t = Table::new(&[
            (&self.a("Industry"), 2.6, false),
            (&self.a("Place"), 2.0, false),
            (&self.a("Owner"), 2.2, false),
            (&self.a("Built"), 0.8, true),
            (&self.a("Rating"), 0.9, true),
            (&self.a("Stations"), 0.9, true),
            (&self.a("Received"), 1.0, true),
            (&self.a("Location"), 1.3, true),
        ]);
        for o in self.save.objects.ordered() {
            let b = Rec(&o.data);
            let ty = b.u8(7);
            if is_house(ty) || ty == 0 {
                continue;
            }
            let owner = b.u16(0x97);
            t.push(
                o.slot as u32,
                vec![
                    self.building_name(ty),
                    self.place_name(u32::from(b.u16(2))),
                    if owner == 0 {
                        "-".into()
                    } else {
                        self.company_name(u32::from(owner))
                    },
                    b.u16(0x3c).to_string(),
                    b.u16(0x4b).to_string(),
                    b.i8(0x33).to_string(),
                    b.u16(0x34).to_string(),
                    format!("{}, {}", b.i16(0x26), b.i16(0x28)),
                ],
            );
        }
        ListPage {
            title: self.a("Industries"),
            table: t,
        }
    }

    pub fn industry_page(&self, slot: u32) -> Page {
        let Some(b) = rec(&self.save.objects, slot) else {
            return Page {
                title: self.t(190, "Industry"),
                ..Page::default()
            };
        };
        let ty = b.u8(7);
        let owner = b.u16(0x97);
        let facts = vec![
            (self.a("Type"), self.building_name(ty)),
            (self.a("Place"), self.place_name(u32::from(b.u16(2)))),
            (
                self.a("Location"),
                format!("{}, {}", b.i16(0x26), b.i16(0x28)),
            ),
            (
                self.a("Owner"),
                if owner == 0 {
                    self.a("none")
                } else {
                    self.company_name(u32::from(owner))
                },
            ),
            (
                self.a("Price paid"),
                if owner == 0 {
                    DASH.into()
                } else {
                    money(f64::from(b.i32(0x99)))
                },
            ),
            (self.a("Built"), b.u16(0x3c).to_string()),
            (
                self.a("Profitability"),
                format!("{}  (100 = neutral)", b.u16(0x4b)),
            ),
            (
                self.a("Profit last year"),
                money(f64::from(b.i16(0x36)) * 1000.0),
            ),
            (self.a("Linked stations"), b.i8(0x33).to_string()),
            (self.a("Lots received (yr)"), b.u16(0x34).to_string()),
        ];
        let mut out = Table::new(&[
            (&self.a("Output"), 2.4, false),
            (&self.a("Per year"), 1.2, true),
            (&self.a("Produced"), 1.1, true),
            (&self.a("Delivered"), 1.1, true),
        ]);
        let mut inp = Table::new(&[
            (&self.a("Input"), 2.6, false),
            (&self.a("Stock"), 1.2, true),
        ]);
        if let Some(it) = self.industry_type(ty) {
            let stock0 = std::array::from_fn::<f32, 8, _>(|i| b.f32(0x6b + 4 * i));
            // One row per output cargo (several slots can feed the same cargo): rate per year
            // and this year's produced / delivered lot counters (`+0x67` / `+0x63`, world-growth
            // 2.2), summed over the slots.
            let mut rows: Vec<(u8, f64, i32, i32)> = Vec::new();
            for (s, sl) in it.slots.iter().enumerate() {
                let valid =
                    i32::from(sl.year_from) <= self.year && self.year <= i32::from(sl.year_to);
                if !valid || sl.output == NO_CARGO {
                    continue;
                }
                let rate = it.production_rate(s, ty, self.year, false, false, &stock0);
                let (made, sent) = (i32::from(b.i8(0x67 + s)), i32::from(b.i8(0x63 + s)));
                match rows.iter_mut().find(|r| r.0 == sl.output) {
                    Some(r) => {
                        r.1 += rate;
                        r.2 += made;
                        r.3 += sent;
                    }
                    None => rows.push((sl.output, rate, made, sent)),
                }
            }
            for (i, (c, rate, made, sent)) in rows.into_iter().enumerate() {
                out.push(
                    i as u32,
                    vec![
                        self.cargo_name(c),
                        format!("{rate:.1}"),
                        made.to_string(),
                        sent.to_string(),
                    ],
                );
            }
            for (i, &c) in it.inputs.iter().enumerate() {
                if c == NO_CARGO {
                    continue;
                }
                inp.push(
                    i as u32,
                    vec![self.cargo_name(c), format!("{:.1}", stock0[i])],
                );
            }
        }
        let mut right = Vec::new();
        if out.rows.is_empty() {
            right.push(Block::Note(
                self.a("This building produces no cargo this year."),
            ));
        } else {
            right.push(Block::Table {
                title: self.a("Production"),
                table: out,
            });
        }
        if !inp.rows.is_empty() {
            right.push(Block::Table {
                title: self.a("Inputs"),
                table: inp,
            });
        }
        Page {
            title: self.building_name(ty),
            subtitle: self.place_name(u32::from(b.u16(2))),
            left: vec![Block::Facts(facts)],
            right,
        }
    }

    // ---- picking --------------------------------------------------------------------------

    /// What is under map tile `(x, y)`: an industry or a station building (objects' boxes at
    /// `+0x26..+0x2d`), else a city or region (place box), else the territory of the tile.
    pub fn pick(&self, x: i32, y: i32) -> Option<Pick> {
        for o in self.save.objects.ordered() {
            let b = Rec(&o.data);
            let (x0, y0, x1, y1) = (
                i32::from(b.i16(0x26)),
                i32::from(b.i16(0x28)),
                i32::from(b.i16(0x2a)),
                i32::from(b.i16(0x2c)),
            );
            if x < x0 || x > x1 || y < y0 || y > y1 {
                continue;
            }
            let ty = b.u8(7);
            if ty == 0 {
                let st = u32::from(b.u16(8));
                if rec(&self.save.stations, st).is_some() {
                    return Some(Pick::Station(st));
                }
            } else if is_house(ty) {
                let p = u32::from(b.u16(2));
                if rec(&self.save.places, p).is_some() {
                    return Some(Pick::Place(p));
                }
            } else {
                return Some(Pick::Industry(o.slot as u32));
            }
        }
        for p in self.save.places.ordered() {
            let r = Rec(&p.data);
            if r.u8(0x10) <= 1
                && x >= i32::from(r.u16(8))
                && y >= i32::from(r.u16(10))
                && x <= i32::from(r.u16(12))
                && y <= i32::from(r.u16(14))
            {
                return Some(Pick::Place(p.slot as u32));
            }
        }
        if x < 0 || y < 0 || x as usize >= self.save.map.width || y as usize >= self.save.map.height
        {
            return None;
        }
        let region = self.save.map.tile(x as usize, y as usize).region;
        self.save
            .territories
            .ordered()
            .find(|t| Rec(&t.data).u8(0x2a) == region && region != 0)
            .map(|t| Pick::Territory(t.slot as u32))
    }
}

/// Houses (type 0x12) and large houses (0x27) are buildings of a place, not industries
/// (cargo.md 7.5).
fn is_house(ty: u8) -> bool {
    ty == 0x12 || ty == 0x27
}

#[cfg(test)]
mod tests {
    use super::*;
    use rt2_sim::exe::Exe;
    use std::path::PathBuf;

    #[test]
    fn money_has_thousands_separators_and_a_leading_minus() {
        assert_eq!(money(0.0), "$0");
        assert_eq!(money(1234567.4), "$1,234,567");
        assert_eq!(money(-9999.6), "-$10,000");
        assert_eq!(money(f64::NAN), "$0");
    }

    #[test]
    fn records_read_zero_past_the_end() {
        let r = Rec(&[1, 0, 0, 0]);
        assert_eq!(r.i32(0), 1);
        assert_eq!(r.i32(2), 0);
        assert_eq!(r.text(0, 40), "\u{1}");
    }

    #[test]
    fn capitalize_and_percent() {
        assert_eq!(capitalize("coal"), "Coal");
        assert_eq!(pct(0.075), "7.5%");
    }

    #[test]
    fn table_rows_follow_the_columns() {
        let mut t = Table::new(&[("a", 1.0, false), ("b", 1.0, true)]);
        t.push(7, vec!["x".into(), "y".into()]);
        assert_eq!(t.ids, vec![7]);
        assert_eq!(t.rows[0].len(), t.cols.len());
    }

    #[test]
    fn purchasable_improvements_are_the_19_of_the_spec() {
        // terrain-track.md 3.4: 0..11 and 15..21.
        assert_eq!(PURCHASABLE.count_ones(), 19);
    }

    /// Loads the Example save with the game tables; skipped without `RT2_DIR`.
    fn example() -> Option<(Save, GameData)> {
        let dir = PathBuf::from(std::env::var_os("RT2_DIR")?);
        let exe = Exe::from_bytes(std::fs::read(dir.join("RT2_PLAT.EXE")).ok()?).ok()?;
        let data = GameData::load(&exe).ok()?;
        let raw = std::fs::read(dir.join("games/Example save game.GAM")).ok()?;
        Some((Save::parse(&raw).ok()?, data))
    }

    #[test]
    fn every_page_builds_for_the_example_and_flags_the_old_layout() {
        let Some((save, data)) = example() else {
            return;
        };
        let texts = Texts::none();
        let info = Info {
            save: &save,
            data: &data,
            texts: &texts,
            year: 1875,
            month: 12,
            day: 10,
            minutes: 0,
        };
        assert!(!info.detailed());
        let banner = |p: &Page| {
            p.left
                .iter()
                .any(|b| matches!(b, Block::Note(n) if n.contains("0x233C")))
        };
        for r in save.companies.ordered() {
            assert!(banner(&info.finance_page(r.slot as u32)));
        }
        assert_eq!(info.stock_market().table.rows.len(), 5);
        assert_eq!(info.trains_list().table.rows.len(), 9);
        assert_eq!(info.stations_list().table.rows.len(), 10);
        assert_eq!(info.places_list().table.rows.len(), 103);
        for r in save.trains.ordered() {
            assert!(banner(&info.train_page(r.slot as u32)));
        }
        for r in save.stations.ordered() {
            assert!(banner(&info.station_page(r.slot as u32)));
        }
        for r in save.places.ordered() {
            assert!(banner(&info.place_page(r.slot as u32)));
        }
        for r in save.territories.ordered() {
            assert!(banner(&info.territory_page(r.slot as u32)));
        }
        for r in save.objects.ordered() {
            let _ = info.industry_page(r.slot as u32);
        }
        // Names survive the old layout; the numbers are not shown.
        let stations = info.stations_list().table;
        assert_eq!(stations.rows[0][0], "Oxford");
        assert!(stations.rows[0][1..].iter().all(|c| c == "-"));
        assert_eq!(info.finance_page(1).title, "Great Western");
    }

    #[test]
    fn picking_finds_the_industry_before_the_city_and_the_territory_last() {
        let Some((save, data)) = example() else {
            return;
        };
        let texts = Texts::none();
        let info = Info {
            save: &save,
            data: &data,
            texts: &texts,
            year: 1875,
            month: 12,
            day: 10,
            minutes: 0,
        };
        // London's textile mill (building 1, box 256,240 to 257,241) lies inside London.
        assert_eq!(info.pick(256, 240), Some(Pick::Industry(1)));
        // A tile of London away from its buildings: the city.
        let london = rec(&save.places, 1).expect("London");
        let (x0, y0) = (i32::from(london.u16(8)), i32::from(london.u16(10)));
        let hit = info.pick(x0, y0);
        assert!(
            matches!(
                hit,
                Some(Pick::Place(1) | Pick::Industry(_) | Pick::Station(_))
            ),
            "{hit:?}"
        );
        // Open country of Britain: its territory (region 1) or nothing, never a building.
        assert!(!matches!(
            info.pick(0, 0),
            Some(Pick::Industry(_) | Pick::Station(_))
        ));
        assert_eq!(info.pick(-1, 5), None);
    }
}
