//! The data of the Reports screen (docs/reference/screens.md "Relatórios", docs/ui.md): the
//! archived years of every company (income by category and the per-share metrics), the live
//! balance of each company for the session samples, and the rankings of trains and stations.
//! Read-only over the save, like the rest of `info`; nothing here changes the simulation.
//!
//! Sources: `finance.md` 1 (category ledgers `+0x4286`, `+0x42ee` archive, `+0x46fe` lifetime, the
//! balance accounts `+0x4766`), 16.2 (the archived metrics `+0x49b6`: 0 price, 1 book value, 2
//! revenue, 3 earnings, 4 dividend per share, 5 weighted 5-year return), `trains.md` 6.2 (the
//! train ledger `+0xfd` this year, `+0x10d` last year, categories 0 revenue, 1 maintenance, 2 cars,
//! 3 fuel; `train_year_step` shifts them and leaves `+0x13d`, the accumulator since purchase) and
//! `finance.md` 13.4 (the station counters of the year: `+0x4b5` lots generated for the station,
//! `+0x4b7` dispatched and `+0x4b9` received).

use super::*;

/// Number of income-statement categories (finance.md 1).
pub const NCAT: usize = 13;
/// Archived years kept by the original (finance.md 16.2: `k = 1..=10`).
pub const ARCHIVE_YEARS: usize = 10;

const TR_LEDGER_YEAR: usize = 0xfd;
const TR_LEDGER_LAST: usize = 0x10d;
const TR_LEDGER_LIFE: usize = 0x13d;
const ST_GENERATED: usize = 0x4b5;
const ST_DISPATCHED: usize = 0x4b7;
const ST_RECEIVED: usize = 0x4b9;

/// The balance of one company at one moment: the values the session samples keep.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub slot: u32,
    pub cash: f64,
    pub debt: f64,
    pub equity: f64,
    pub price: Option<f64>,
}

/// One archived year of a company: the 13 ledger categories and the first six archived metrics.
#[derive(Clone, Debug, PartialEq)]
pub struct YearRow {
    pub year: i32,
    pub cats: [f64; NCAT],
    /// Price, book value, revenue, earnings and dividend per share, weighted 5-year return.
    pub metrics: [f64; 6],
}

/// Everything the Reports screen shows about one company.
#[derive(Clone, Debug, PartialEq)]
pub struct CompanyReport {
    pub slot: u32,
    pub name: String,
    pub human: bool,
    pub founded: i32,
    /// The year-to-date ledger.
    pub ytd: [f64; NCAT],
    /// The never-reset accumulator.
    pub life: [f64; NCAT],
    /// Archived years, newest first.
    pub years: Vec<YearRow>,
    /// Live price, book value, revenue, earnings and dividend per share (`None` without shares).
    pub live: Option<[f64; 5]>,
    pub snapshot: Snapshot,
}

/// A train of the profit ranking.
#[derive(Clone, Debug, PartialEq)]
pub struct TrainRank {
    pub slot: u32,
    pub name: String,
    pub company: u32,
    pub company_name: String,
    pub engine: String,
    /// Net result this year, last year and since purchase (`None`: the counters hold values no
    /// train can have, see [`train_result`]).
    pub year: Option<i64>,
    pub last: Option<i64>,
    pub life: Option<i64>,
    pub tile: Option<(i32, i32)>,
}

/// A station of the volume ranking.
#[derive(Clone, Debug, PartialEq)]
pub struct StationRank {
    pub slot: u32,
    pub name: String,
    pub owner: u32,
    pub owner_name: String,
    /// Lots dispatched plus lots received this year.
    pub moved: u32,
    /// Lots the industries generated for the station this year.
    pub generated: u32,
    pub tile: (i32, i32),
}

/// Largest believable value of one ledger category of a train in a year, and since purchase. A
/// train of this game moves some hundred thousand dollars a year. The records of the saves the
/// original wrote can carry uninitialised bytes (the shipped Example save has the debug heap fill
/// `0xF00DBAAD` and arbitrary words in the older year slots and in the accumulator since
/// purchase, which the simulation then only adds to); a result with a category beyond these caps
/// is shown as unknown instead of as billions of dollars.
const CAP_YEAR: i64 = 5_000_000;
const CAP_LIFE: i64 = 100_000_000;

/// The net result of a train over one ledger slot: the sum of its four categories (revenue,
/// maintenance, cars, fuel; trains.md 6.2), `None` when one of them is beyond `cap`.
fn train_result(r: &Rec, offset: usize, cap: i64) -> Option<i64> {
    let mut sum = 0;
    for k in 0..4 {
        let v = i64::from(r.i32(offset + 4 * k));
        if v.abs() > cap {
            return None;
        }
        sum += v;
    }
    Some(sum)
}

impl Info<'_> {
    /// Every company that exists, with its archived years and live figures. Empty for a save in
    /// the old record layout (the fields are not decoded there).
    pub fn company_reports(&self) -> Vec<CompanyReport> {
        if !self.detailed() {
            return Vec::new();
        }
        let ctx = self.ctx();
        let mut out = Vec::new();
        for r in self.save.companies.ordered() {
            let rr = Rec(&r.data);
            if rr.i32(CO_ACTIVE) == 0 {
                continue;
            }
            let slot = r.slot as u32;
            let s = self.standing(&rr);
            let co = &s.co;
            let founded = co.founded;
            let mut years = Vec::new();
            for k in 1..=ARCHIVE_YEARS {
                let year = self.year - k as i32;
                if year < founded {
                    break;
                }
                let m = &co.arch_metrics[k];
                years.push(YearRow {
                    year,
                    cats: co.arch_cats[k - 1],
                    metrics: [m[0], m[1], m[2], m[3], m[4], m[5]],
                });
            }
            let live = s.price.map(|p| {
                [
                    f64::from(p),
                    s.book_ps,
                    co.live_metric(&ctx, 2),
                    co.live_metric(&ctx, 3),
                    co.live_metric(&ctx, 4),
                ]
            });
            out.push(CompanyReport {
                slot,
                name: self.company_name(slot),
                human: self.is_human_company(&rr),
                founded,
                ytd: co.cats,
                life: co.year_cats,
                years,
                live,
                snapshot: Snapshot {
                    slot,
                    cash: co.acct[finance::ACCT_CASH],
                    debt: -co.acct[finance::ACCT_DEBT],
                    equity: co.equity(),
                    price: s.price.map(f64::from),
                },
            });
        }
        out
    }

    /// The balances of every company now (what the session samples record each month).
    pub fn report_snapshots(&self) -> Vec<Snapshot> {
        self.company_reports()
            .into_iter()
            .map(|c| c.snapshot)
            .collect()
    }

    /// All trains by profit this year, best first (ties by slot, so the order is stable).
    pub fn train_ranking(&self) -> Vec<TrainRank> {
        if !self.detailed() {
            return Vec::new();
        }
        let mut out: Vec<TrainRank> = self
            .save
            .trains
            .ordered()
            .map(|t| {
                let r = Rec(&t.data);
                let cars = self.cars(&r);
                // The locomotive is the first car: its tile is the train's place (trains.md 1.2).
                let tile = rec(&self.save.pool_70, r.i32(0).max(0) as u32)
                    .map(|c| (i32::from(c.i16(9)), i32::from(c.i16(0xb))));
                let company = r.i32(0x41).max(0) as u32;
                TrainRank {
                    slot: t.slot as u32,
                    name: self.train_name(&r),
                    company,
                    company_name: self.company_name(company),
                    engine: self.train_engine(&cars),
                    year: train_result(&r, TR_LEDGER_YEAR, CAP_YEAR),
                    last: train_result(&r, TR_LEDGER_LAST, CAP_YEAR),
                    life: train_result(&r, TR_LEDGER_LIFE, CAP_LIFE),
                    tile,
                }
            })
            .collect();
        out.sort_by_key(|t| (std::cmp::Reverse(t.year), t.slot));
        out
    }

    /// All stations by lots moved this year, busiest first.
    pub fn station_ranking(&self) -> Vec<StationRank> {
        if !self.detailed() {
            return Vec::new();
        }
        let mut out: Vec<StationRank> = self
            .save
            .stations
            .ordered()
            .map(|s| {
                let st = Rec(&s.data);
                let owner = st.i32(0x39).max(0) as u32;
                StationRank {
                    slot: s.slot as u32,
                    name: self.station_name(s.slot as u32),
                    owner,
                    owner_name: self.company_name(owner),
                    moved: u32::from(st.u16(ST_DISPATCHED)) + u32::from(st.u16(ST_RECEIVED)),
                    generated: u32::from(st.u16(ST_GENERATED)),
                    tile: (st.i32(0xa), st.i32(0xe)),
                }
            })
            .collect();
        out.sort_by_key(|s| (std::cmp::Reverse(s.moved), s.slot));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(words: &[(usize, i32)]) -> Vec<u8> {
        let mut d = vec![0u8; 0x14d];
        for &(o, v) in words {
            d[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        d
    }

    #[test]
    fn a_train_result_is_the_sum_of_its_four_categories() {
        let d = record(&[
            (TR_LEDGER_YEAR, 120_000),
            (TR_LEDGER_YEAR + 4, -3_000),
            (TR_LEDGER_YEAR + 8, -2_000),
            (TR_LEDGER_YEAR + 12, -5_000),
        ]);
        assert_eq!(
            train_result(&Rec(&d), TR_LEDGER_YEAR, CAP_YEAR),
            Some(110_000)
        );
        assert_eq!(train_result(&Rec(&d), TR_LEDGER_LAST, CAP_YEAR), Some(0));
    }

    #[test]
    fn counters_with_heap_fill_are_unknown_not_billions() {
        // The debug heap fill 0xF00DBAAD and the like of the shipped Example save.
        let d = record(&[
            (TR_LEDGER_LIFE + 8, -267_535_699),
            (TR_LEDGER_LAST + 4, 16_819_918),
        ]);
        assert_eq!(train_result(&Rec(&d), TR_LEDGER_LIFE, CAP_LIFE), None);
        assert_eq!(train_result(&Rec(&d), TR_LEDGER_LAST, CAP_YEAR), None);
        // A big but believable lifetime stays.
        let d = record(&[(TR_LEDGER_LIFE, 40_000_000)]);
        assert_eq!(
            train_result(&Rec(&d), TR_LEDGER_LIFE, CAP_LIFE),
            Some(40_000_000)
        );
    }
}
