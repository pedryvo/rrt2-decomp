//! Company finance (docs/spec/finance.md): share prices, credit rating and bond rate, interest,
//! dividends, split, bankruptcy and the monthly step.
//!
//! Everything is plain data and pure arithmetic: no I/O, no wall clock. The company record is the
//! [`Company`] struct (finance.md section 1); the date, economy state and effect percentages that
//! the original reads from globals come in a [`Ctx`] resolved by the caller. The two game tables
//! used here (stock economy multiplier `0x5504d0`, company-age weights `0x550074`) are read from
//! the executable ([`FinanceTables::load`], also part of [`crate::data::GameData`]).
//!
//! Proof status follows the spec: the items marked `[V]` are checked bit-exact against the
//! original by `crates/rt2-oracle/tests/finance.rs`; `[S]` were read from code and not tested.
//! All arithmetic is done in the order of the original at 53-bit x87 precision ([`crate::fp`]).

use crate::exe::{Exe, ExeError};
use crate::fp::{f32r, ftol, pow};

// ---- game tables ------------------------------------------------------------------------------

pub const ADDR_ECON_STOCK: u32 = 0x0055_04d0;
pub const ADDR_AGE_WEIGHTS: u32 = 0x0055_0074;

/// Tables the finance code reads from the executable.
#[derive(Clone, Debug, PartialEq)]
pub struct FinanceTables {
    /// `0x5504d0` f32[5]: stock price and rating multiplier by economic state (finance.md 3.1).
    pub econ_stock: [f32; 5],
    /// `0x550074` i32[5][4]: weight (percent) of each price criterion by company age 0..4
    /// (finance.md 3.1).
    pub age_weights: [[i32; 4]; 5],
}

impl FinanceTables {
    pub fn load(exe: &Exe) -> Result<FinanceTables, ExeError> {
        let econ = exe.f32s(ADDR_ECON_STOCK, 5)?;
        let mut age_weights = [[0i32; 4]; 5];
        for (a, row) in age_weights.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = exe.u32_at(ADDR_AGE_WEIGHTS + 4 * (a * 4 + j) as u32)? as i32;
            }
        }
        Ok(FinanceTables {
            econ_stock: econ.try_into().expect("5 values"),
            age_weights,
        })
    }
}

// ---- constants --------------------------------------------------------------------------------

/// Account indices of [`Company::acct`] (finance.md section 1).
pub const ACCT_CASH: usize = 0;
pub const ACCT_TRACK: usize = 3;
pub const ACCT_DEBT: usize = 6;
pub const ACCT_EQUITY: usize = 7;

/// Revenue/expense categories of [`Company::cats`] (finance.md section 1).
pub const CAT_TRACK: usize = 5;
pub const CAT_INTEREST: usize = 9;
pub const CAT_OVERHEAD: usize = 10;
pub const CAT_SALARY: usize = 11;
pub const CAT_FEES: usize = 12;

/// Maximum number of bonds per company.
pub const MAX_BONDS: usize = 20;
/// Minimum rating to issue bonds (`0x5706d5`, "B"). The runtime value is 5.0 [V in spec 4.2].
pub const MIN_BOND_RATING: f64 = 5.0;
/// Brokerage on share purchases/sales and buy-backs (`0x5706c9`, runtime f32 0.03) [S].
pub const BROKERAGE: f32 = 0.03;

/// Calendar, economy and effect values the finance code reads from globals.
#[derive(Clone, Copy, Debug)]
pub struct Ctx<'a> {
    pub tables: &'a FinanceTables,
    pub year: i32,
    pub month: i32,
    pub day: i32,
    /// `game_time_minutes`.
    pub minutes: i32,
    /// Economic state 0..=4 (`0x570717`).
    pub econ_state: usize,
    /// `prime_rate` (`0x5706d1`), f32 = `0.09 - state*0.01`.
    pub prime_rate: f32,
    /// "Stock Liquidity" (`0x5706cd`, default 100).
    pub liquidity: f32,
    /// `effect_multiplier(0x1e, company)` ("Stock Prices"), already resolved.
    pub price_effect: f64,
    /// Additive effect percentage `0x20` (prime rate), already resolved.
    pub prime_effect_pct: i16,
    /// Integer effect `0x1f` (rating levels), already resolved.
    pub rating_effect: i16,
}

impl Ctx<'_> {
    fn econ(&self) -> f64 {
        f64::from(self.tables.econ_stock[self.econ_state.min(4)])
    }
}

/// A bond: `{principal, maturity year, rate}` (finance.md section 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bond {
    pub principal: i32,
    pub maturity_year: i32,
    pub rate: f32,
}

/// The company record fields used by the finance rules (finance.md section 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Company {
    /// `+0x2c`
    pub shares: i32,
    /// `+0x30`: annual dividend per share.
    pub dividend: f32,
    /// `+0x34`: accumulated trade pressure (shares).
    pub pressure: f32,
    /// `+0x38`: dividends paid this year per share.
    pub dividends_paid: f32,
    /// `+0x3c`
    pub init_price: f32,
    /// `+0x138`: yearly overhead.
    pub overhead: i32,
    /// `+0x134`: chairman salary per year.
    pub chairman_salary: i32,
    /// `+0x148`
    pub founded: i32,
    /// `+0x158`: year of the last bankruptcy.
    pub bankrupt_year: i32,
    /// `+0x4250`: minute stamp of the fundamental price cache (0 = invalid).
    pub price_stamp: i32,
    /// `+0x4254`
    pub price_cache: f64,
    /// `+0x425c`
    pub created: i32,
    /// `+0x4276`
    pub investor_mood: f32,
    /// `+0x427a`: "no dividend" price.
    pub no_div_price: f32,
    /// `+0x427e`: last traded price.
    pub last_price: f32,
    /// `+0x4282`: price at the end of the last month.
    pub month_end_price: f32,
    /// `+0x4286`: current-year categories.
    pub cats: [f64; 13],
    /// `+0x46fe`: year accumulator of the categories (only read for the 9001 marker year).
    pub year_cats: [f64; 13],
    /// `+0x42ee`: archive, `arch_cats[k-1]` = k years ago.
    pub arch_cats: [[f64; 13]; 10],
    /// `+0x4766`: accounts (cash, ..., debt (negative), equity (negative)).
    pub acct: [f64; 8],
    /// `+0x49b6`: archived metrics, `arch_metrics[k]` = k years ago (k = 1..=10; row 0 unused).
    pub arch_metrics: [[f64; 14]; 11],
    pub bonds: Vec<Bond>,
}

impl Default for Company {
    fn default() -> Company {
        Company {
            shares: 0,
            dividend: 0.0,
            pressure: 0.0,
            dividends_paid: 0.0,
            init_price: 0.0,
            overhead: 0,
            chairman_salary: 0,
            founded: 0,
            bankrupt_year: 0,
            price_stamp: 0,
            price_cache: 0.0,
            created: 0,
            investor_mood: 0.0,
            no_div_price: 0.0,
            last_price: 0.0,
            month_end_price: 0.0,
            cats: [0.0; 13],
            year_cats: [0.0; 13],
            arch_cats: [[0.0; 13]; 10],
            acct: [0.0; 8],
            arch_metrics: [[0.0; 14]; 11],
            bonds: Vec::new(),
        }
    }
}

fn sum_cats(c: &[f64; 13], n: usize) -> f64 {
    c.iter().take(n).fold(0.0, |a, b| a + b)
}

/// Prime rate for a company (`0x46bac0`) [V]: always a whole percentage plus the effect.
pub fn prime_rate_for(prime_rate: f32, effect_pct: i16) -> f64 {
    let v = f64::from(effect_pct) * 0.01 + f64::from(prime_rate) + 0.001;
    f64::from(ftol(v * 100.0)) * 0.01
}

/// `prime_rate` global for an economic state (finance.md 4.1): `0.09 - state*0.01`, as f32.
pub fn prime_rate_for_state(state: usize) -> f32 {
    (0.09 - state as f64 * 0.01) as f32
}

/// `0x535540`: 1/12 as the f64 the original multiplies by.
pub const MONTH: f64 = 0.083_333_333_333_333_33;

/// Interest paid on a cash balance per month (`0x4399d0` step 4, finance.md 5) [S]. Positive cash
/// earns `prime - 4%`; a negative balance (overdraft) pays `prime + (10 - rating)*1% + 2%`. The
/// result has the sign of `cash` (negative = expense).
pub fn cash_interest(cash: f64, prime: f64, rating: f64) -> f64 {
    // `0x535540` = 1/12: the original multiplies (verified against the original, month 1 of
    // Alaska: dividing by 12 differs by one ulp in the salary sum).
    if cash >= 0.0 {
        (prime - 0.04) * cash * MONTH
    } else {
        (prime + (10.0 - rating) * 0.01 + 0.02) * cash * MONTH
    }
}

/// Personal net worth (`0x4a7620`, finance.md 7.1) [S]: cash plus shares times quote; short
/// positions are negative shares.
pub fn personal_net_worth(cash: f64, positions: &[(i32, f64)]) -> f64 {
    positions
        .iter()
        .fold(cash, |a, (n, price)| a + f64::from(*n) * price)
}

/// Margin buying power (`0x4a7a40`, finance.md 7.3) [S]: `cash + sum(position * last price *
/// (0.5 long, 1.5 short))`; negative means a margin call.
pub fn margin_power(cash: f64, positions: &[(i32, f32)]) -> f64 {
    positions.iter().fold(cash, |a, (n, price)| {
        let k = if *n > 0 { 0.5 } else { 1.5 };
        a + f64::from(*n) * f64::from(*price) * k
    })
}

/// Dividend credited to a shareholder for one quarterly payment (finance.md 6) [S]:
/// `ftol(shares * d / 4)`, `d` the annual dividend actually paid.
pub fn holder_dividend(shares: i32, annual: f64) -> i32 {
    ftol(f64::from(shares) * annual / 4.0)
}

/// Why a bond could not be issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BondError {
    TooMany,
    RatingTooLow,
}

/// Outcome of the quarterly dividend (finance.md 6) [S].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DividendOutcome {
    /// Nothing to pay (`d <= 0` from the start).
    None,
    /// Suspended: cash would drop too far or the board cap is ~0 (LNG 2274).
    Suspended,
    /// Cut to the board's cap (LNG 2713).
    Reduced,
    /// Paid as set.
    Paid,
}

/// Result of [`Company::pay_quarterly_dividend`]: the annual per-share `d` that was paid (0 when
/// nothing was paid). Shareholders receive [`holder_dividend`]`(shares, annual)` each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DividendPayment {
    pub outcome: DividendOutcome,
    pub annual: f64,
}

/// Inputs of the monthly step that come from outside the company record (finance.md 5) [S].
#[derive(Clone, Copy, Debug)]
pub struct MonthlyInputs {
    /// There is a human chairman: investor mood is pinned to 0.
    pub human_chairman: bool,
    /// Shares held by human players (`H` in the mood formula).
    pub human_shares: i32,
    /// `effect_multiplier(0xc)`: track maintenance.
    pub track_effect: f64,
    /// `effect_multiplier(0xf)`: overhead.
    pub overhead_effect: f64,
    /// `effect_multiplier(0x10)`: manager salary.
    pub manager_effect: f64,
    /// Salary of the current manager (table at `0x54fdad`).
    pub manager_salary: i32,
}

/// Outcome of [`Company::monthly_step`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonthlyReport {
    /// Dividend paid this month (quarter months only).
    pub dividend: Option<DividendPayment>,
    /// Cash the chairman receives in his personal account: `ftol(+0x134 / 12)`.
    pub chairman_pay: i32,
}

impl Company {
    // ---- ledger (finance.md section 1) -------------------------------------------------------

    /// `company_ledger_credit` (`0x439970`) [V]: revenue/expense `x` in category `cat`; goes to
    /// the year accumulator, to cash and (negated) to equity; invalidates the price cache.
    pub fn credit(&mut self, cat: usize, x: f64) {
        self.price_stamp = 0;
        self.cats[cat] += x;
        self.year_cats[cat] += x;
        self.price_stamp = 0;
        self.acct[ACCT_CASH] += x;
        self.acct[ACCT_EQUITY] -= x;
    }

    /// Account movement (`0x4398d0`) [V]: account 0 gains `x` against equity; account `k != 0`
    /// gains `x` and cash loses `x`.
    pub fn move_account(&mut self, account: usize, x: f64) {
        self.price_stamp = 0;
        if account == 0 {
            self.acct[0] += x;
            self.acct[ACCT_EQUITY] -= x;
        } else {
            self.acct[account] += x;
            self.acct[0] -= x;
        }
    }

    // ---- derived values ----------------------------------------------------------------------

    /// Book value: `-acct[7]`.
    pub fn equity(&self) -> f64 {
        -self.acct[ACCT_EQUITY]
    }

    /// Sum of the asset accounts 0..5 (what `0x43b650` truncates with `_ftol`).
    pub fn total_assets(&self) -> f64 {
        self.acct.iter().take(6).fold(0.0, |a, b| a + b)
    }

    /// Weighted average bond rate `sum(principal*rate) / sum(principal)` (0 without bonds).
    pub fn average_bond_rate(&self) -> f64 {
        if self.bonds.is_empty() {
            return 0.0;
        }
        let mut sum = 0.0f64;
        let mut tot = 0i32;
        for b in &self.bonds {
            tot += b.principal;
            sum += f64::from(b.principal) * f64::from(b.rate);
        }
        sum / f64::from(tot)
    }

    // ---- annualization and metrics (finance.md 2) --------------------------------------------

    fn raw_total(c: &[f64; 13], i: usize) -> f64 {
        match i {
            0..=12 => c[i],
            13 => sum_cats(c, 13),
            14 => sum_cats(c, 5),
            _ => c.iter().skip(5).take(8).fold(0.0, |a, b| a + b),
        }
    }

    /// `0x43a440(i)` [V for 13 and 14, S for the rest]: the current-year category/total
    /// annualized by the fraction of the year elapsed, blended with last year's value.
    pub fn annualized(&self, ctx: &Ctx, i: usize) -> f64 {
        let raw = Self::raw_total(&self.cats, i);
        if ctx.month == 12 && ctx.day == 28 {
            return raw;
        }
        // 1/336, kept as the f32 the original stores; clamped to [0.05, 0.95].
        let mut f = (f64::from(ctx.day + 28 * ctx.month - 28) * 0.002_976_190_476_190_476_4) as f32;
        if f64::from(f) < 0.05 {
            f = 0.05f32;
        }
        if f64::from(f) > 0.95 {
            f = 0.95f32;
        }
        let f = f64::from(f);
        if ctx.year <= self.founded {
            return raw / f;
        }
        let last = &self.arch_cats[0];
        let mut raw = raw;
        if matches!(i, 3 | 13 | 14) {
            raw += f * last[3];
        }
        f * raw + (1.0 - f) * Self::raw_total(last, i)
    }

    /// Live metrics 1..=4 of `0x43ad60` [V]: book value, revenue and profit per share, dividend.
    /// Other metric numbers are not needed by the price and return 0 (finance.md 1, TODO: the
    /// archive-only metrics 5..13 are not modelled; see "Em aberto").
    pub fn live_metric(&self, ctx: &Ctx, m: usize) -> f64 {
        let s = f64::from(self.shares);
        match m {
            1 => -self.acct[ACCT_EQUITY] / s,
            2 => self.annualized(ctx, 14) / s,
            3 => self.annualized(ctx, 13) / s,
            4 => f64::from(self.dividend),
            _ => 0.0,
        }
    }

    fn metric(&self, ctx: &Ctx, k: usize, m: usize) -> f64 {
        if k == 0 {
            self.live_metric(ctx, m)
        } else {
            self.arch_metrics[k][m]
        }
    }

    // ---- share price (finance.md 3) ----------------------------------------------------------

    /// Fundamental share price (`0x43aea0`) [V], cached per game minute: if the stamp equals the
    /// current minute the cached value is returned untouched; any ledger entry clears the stamp.
    pub fn fundamental_price(&mut self, ctx: &Ctx) -> f64 {
        if self.price_stamp == ctx.minutes {
            return self.price_cache;
        }
        let (p, nodiv) = self.compute_fundamental(ctx);
        self.no_div_price = nodiv;
        self.price_cache = p;
        self.price_stamp = ctx.minutes;
        p
    }

    fn compute_fundamental(&self, ctx: &Ctx) -> (f64, f32) {
        const BASE_W: [f64; 4] = [40.0, 10.0, 20.0, 30.0];
        let (mut wsum, mut acc_e, mut acc_r) = (0i32, 0.0f64, 0.0f64);
        for i in 0..5usize {
            if ctx.year - i as i32 >= self.founded {
                let wi = if i == 0 {
                    ctx.month * 5 - 5
                } else {
                    (5 - i as i32) * 12
                };
                wsum += wi;
                acc_e += f64::from(wi) * self.metric(ctx, i, 3);
                acc_r += f64::from(wi) * self.metric(ctx, i, 2);
            }
        }
        let (mut vsum, mut cont, mut dsum) = (0i32, 0.0f64, 0.0f64);
        for i in 1..10usize {
            if ctx.year - i as i32 >= self.founded {
                let v = 10 - i as i32;
                vsum += v;
                dsum += f64::from(v) * self.arch_metrics[i][4];
                // The original compares the year with itself here, so this is always true
                // (finance.md 3.1, faithful bug); the cut-off year is therefore the same.
                if ctx.year - i as i32 > self.founded {
                    cont += f64::from(v);
                }
            }
        }
        let mut crit = [0.0f64; 4];
        if wsum != 0 {
            crit[0] = (acc_e / f64::from(wsum)) * 10.0;
            crit[1] = acc_r / f64::from(wsum);
        }
        if vsum != 0 {
            let mut d = (dsum / f64::from(vsum)) as f32;
            let prev = self.arch_metrics[1][4];
            let t = ((prev + prev) + f64::from(d)) * f64::from(0.333_333_34f32);
            if f64::from(d) >= t {
                d = t as f32;
            }
            crit[2] = (((cont / f64::from(vsum)) * 0.5) + 0.6) * f64::from(d) * 30.0;
        }
        crit[3] = self.live_metric(ctx, 1);
        let mut m = 1.0f64;
        for (k, v) in crit.iter().enumerate() {
            if k != 2 && *v >= m {
                m = *v;
            }
        }
        let m = m * 1.13;
        if crit[2] >= m {
            crit[2] = m;
        }
        for v in &mut crit {
            if *v < 0.0 {
                *v = 0.0;
            }
        }
        let mut wt = BASE_W;
        let age = ctx.year - self.founded;
        if age < 5 {
            for (j, w) in wt.iter_mut().enumerate() {
                *w *= f64::from(ctx.tables.age_weights[age.max(0) as usize][j]);
            }
        }
        let (mut imax, mut imin) = (0usize, 0usize);
        for k in 0..4 {
            if crit[k] > crit[imax] {
                imax = k;
            }
            if crit[k] < crit[imin] {
                imin = k;
            }
        }
        wt[imax] *= 1.25;
        wt[imin] *= 0.8;
        let (mut x, mut wx, mut y, mut wy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for k in 0..4 {
            let t = crit[k] * wt[k];
            x += t;
            wx += wt[k];
            if k != 2 {
                y += t;
                wy += wt[k];
            }
        }
        let e = ctx.econ();
        let mut nodiv = ((y / wy) * (e + 1.0) * 0.5) as f32;
        if f64::from(nodiv) < 0.5 {
            nodiv = 0.5;
        }
        let mut p = ctx.price_effect * ((x / wx) * e);
        let since = ctx.year - self.bankrupt_year;
        if since < 10 {
            p *= f64::from(since) * 0.025 + 0.75;
        }
        if p < 1.0 {
            p = 1.0;
        }
        (p, nodiv)
    }

    /// Trade price for a deal of `n` shares (`0x43b410`) [V]: `n > 0` buy, `n < 0` sell, 0 only
    /// quotes. With `commit` the trade pressure (`+0x34`) and the last price (`+0x427e`) are
    /// stored. Calls [`Self::fundamental_price`] (and therefore refreshes its cache).
    pub fn trade_price(&mut self, ctx: &Ctx, n: i32, commit: bool) -> i32 {
        let s = f64::from(self.shares);
        let mut pf = self.fundamental_price(ctx);
        if f64::from(self.init_price) < pf && ctx.year - self.founded <= 5 {
            let mut m = (ctx.minutes - self.created) / 40320;
            if m >= 50 {
                m = 50;
            }
            pf = (f64::from(50 - m) * f64::from(self.init_price) + f64::from(50 + m) * pf) * 0.01;
        }
        let mut x = f64::from(n) / s;
        if x > 0.2 {
            x = f64::from(0.2f32);
        }
        if x < -0.2 {
            x = f64::from(-0.2f32);
        }
        let press = if n != 0 {
            (1.0 - x) * f64::from(self.pressure) + f64::from(n)
        } else {
            f64::from(self.pressure)
        };
        let p = pow(s * 5e-5, 0.33);
        let mut r = ((p * (100.0 / f64::from(ctx.liquidity))) * press * 1.4 + s) / s;
        if r < 0.6 {
            r = (r - 0.6) * 0.5 + 0.6;
        }
        if r < 0.45 {
            r = (r - 0.45) * 0.5 + 0.45;
        }
        if r < 0.3 {
            r = 0.3;
        } else {
            if r > 1.7 {
                r = (r - 1.7) * 0.5 + 1.7;
            }
            if r > 2.5 {
                r = (r - 2.5) * 0.5 + 2.5;
            }
            if r > 4.0 {
                r = (r - 4.0) * 0.5 + 4.0;
            }
            if r > 6.0 {
                r = 6.0;
            }
        }
        let price = ftol(r * pf + 0.5);
        if commit {
            self.pressure = press as f32;
            self.last_price = price as f32;
        }
        price
    }

    /// Stock split rule at year end (`0x43d800` step 4, finance.md 8) [S]. `price` is the
    /// committed quote; above 100 the shares are multiplied by `n = ftol(last_price * 0.02)` and
    /// price-like fields divided. Returns the split factor (the caller multiplies every
    /// shareholder's position), or `None` when there is no split.
    pub fn split_if_needed(&mut self, price: i32) -> Option<i32> {
        if price <= 100 {
            return None;
        }
        let n = ftol(f64::from(self.last_price) * 0.02);
        if n <= 1 {
            return None;
        }
        let nf = f64::from(n);
        self.shares = self.shares.wrapping_mul(n);
        self.pressure = (f64::from(self.pressure) * nf) as f32;
        self.dividend = (f64::from(self.dividend) / nf) as f32;
        self.dividends_paid = (f64::from(self.dividends_paid) / nf) as f32;
        self.init_price = (f64::from(self.init_price) / nf) as f32;
        for row in self.arch_metrics.iter_mut().skip(1) {
            for m in [0usize, 1, 2, 3, 4, 7] {
                row[m] /= nf;
            }
        }
        Some(n)
    }

    /// Monthly decay and quote commit (`0x4399d0` step 7, finance.md 5) [S]: pressure x0.9, a
    /// committed quote, and the month-end price snapshot.
    pub fn decay_and_quote(&mut self, ctx: &Ctx) {
        self.pressure = (f64::from(self.pressure) * 0.9) as f32;
        self.trade_price(ctx, 0, true);
        self.month_end_price = self.last_price;
    }

    // ---- ratings, bonds, interest (finance.md 4) ---------------------------------------------

    /// Credit rating (`0x43b650`) [V]: integer 0..=10 (0 = F, 10 = AAA) as an `f64`. `x` is the
    /// hypothetical extra debt (0 for the current rating).
    pub fn credit_rating(&self, ctx: &Ctx, x: i32) -> f64 {
        // Weighted average of (profit - interest) over the last four years.
        let (mut acc, mut ebx) = (0.0f64, 0i32);
        for i in 1..=4i32 {
            ebx += 5 - i;
            if ctx.year - i >= self.founded {
                let cats = &self.arch_cats[(i - 1) as usize];
                let ni = cats.iter().fold(0.0, |a, b| a + b);
                acc += (ni - cats[CAT_INTEREST]) * f64::from(5 - i);
            }
        }
        let a = acc / f64::from(ebx);
        let avg_rate = self.average_bond_rate();
        // `acct[6]` is negative, so the extra debt X adds with the opposite sign: faithful bug.
        let mut est = (self.acct[ACCT_DEBT] + f64::from(x)) * avg_rate;
        let cash = self.acct[ACCT_CASH];
        if cash < 0.0 {
            est += (prime_rate_for(ctx.prime_rate, ctx.prime_effect_pct) + 0.03) * cash;
        }
        let cov = if est == 0.0 { 10.0 } else { a / (-est) };
        let mut s1 = if cov >= 5.0 {
            10.0
        } else if cov > 2.0 {
            10.0 - (5.0 - cov)
        } else if cov > 1.0 {
            10.0 - ((2.0 - cov) * 2.0 + 3.0)
        } else if cov > 0.0 {
            10.0 - ((1.0 - cov) * 4.0 + 5.0)
        } else {
            0.0
        };
        let age = ctx.year - self.founded;
        if age < 5 {
            s1 += f64::from(5 - age) * 0.1 * (10.0 - s1);
        }
        let total = ftol(self.total_assets());
        let ratio = if total > 0 {
            (-self.acct[ACCT_DEBT] + f64::from(x)) / f64::from(total)
        } else {
            1.0
        };
        let s2 = if ratio <= 0.1 {
            10.0
        } else if ratio < 0.25 {
            10.0 - ratio * 4.0
        } else if ratio < 0.5 {
            10.0 - ((ratio - 0.25) * 8.0 + 1.0)
        } else if ratio < 0.75 {
            10.0 - ((ratio - 0.5) * 12.0 + 3.0)
        } else if ratio < 1.0 {
            10.0 - ((ratio - 0.75) * 16.0 + 6.0)
        } else {
            0.0
        };
        let p = (s1 * s2 * 0.1) * 2.0;
        let q = p + (s2 + s1) * 0.5;
        let mut r = q * 0.333_333_333_333_333_3 * ctx.econ();
        let eq = -self.acct[ACCT_EQUITY];
        if eq < 1_000_000.0 {
            // 0x3727c5ac = 1e-5 as f32.
            r -= 10.0 - eq * f64::from(f32::from_bits(0x3727_c5ac));
        }
        if age < 5 {
            let a = age.max(1);
            r -= f64::from(5 - a) * 0.5;
            let cap = f64::from(a + 5);
            if r >= cap {
                r = cap;
            }
        }
        let ba = ctx.year - self.bankrupt_year;
        if ba < 15 {
            r = f64::from(ba) * r * 0.0666;
        }
        let v = f64::from(ftol(r + 0.5)) + f64::from(ctx.rating_effect);
        v.clamp(0.0, 10.0)
    }

    /// Rate of a new bond (`0x4392e0`) [V]: prime plus 1% per level below AAA, with `x` the new
    /// principal included in the rating.
    pub fn bond_rate(&self, ctx: &Ctx, x: i32) -> f64 {
        prime_rate_for(ctx.prime_rate, ctx.prime_effect_pct)
            + (10.0 - self.credit_rating(ctx, x)) * 0.01
    }

    /// Monthly cash interest for this company (finance.md 5 step 4) [S]. Negative = expense.
    pub fn monthly_cash_interest(&self, ctx: &Ctx) -> f64 {
        let prime = prime_rate_for(ctx.prime_rate, ctx.prime_effect_pct);
        let cash = self.acct[ACCT_CASH];
        let rating = if cash < 0.0 {
            self.credit_rating(ctx, 0)
        } else {
            0.0
        };
        cash_interest(cash, prime, rating)
    }

    /// Quarterly bond interest (finance.md 4.4) [S]: `avg_rate * debt * 0.25` (debt is
    /// negative, so this is an expense). Always at the weighted average rate.
    pub fn quarterly_bond_interest(&self) -> f64 {
        self.average_bond_rate() * self.acct[ACCT_DEBT] * 0.25
    }

    /// Issues a bond (`0x439320`) [S]. `underwriting_fee` is the flag that charges 2% in the fees
    /// category. The rating minimum is ignored when `waive_rating` (`0x5748fc != 0`).
    pub fn issue_bond(
        &mut self,
        ctx: &Ctx,
        principal: i32,
        term_years: i32,
        underwriting_fee: bool,
        waive_rating: bool,
    ) -> Result<(), BondError> {
        if self.bonds.len() >= MAX_BONDS {
            return Err(BondError::TooMany);
        }
        if !waive_rating && self.credit_rating(ctx, 0) < MIN_BOND_RATING {
            return Err(BondError::RatingTooLow);
        }
        let rate = self.bond_rate(ctx, principal) as f32;
        if underwriting_fee {
            // Sign assumed: expense categories are negative (finance.md section 1).
            self.credit(CAT_FEES, -(0.02 * f64::from(principal)));
        }
        self.move_account(ACCT_DEBT, -f64::from(principal));
        self.bonds.push(Bond {
            principal,
            maturity_year: ctx.year + term_years,
            rate,
        });
        Ok(())
    }

    /// Repays bond `i` (`0x4391b0`) [S], with the 2% prepayment fee when `penalty`.
    pub fn repay_bond(&mut self, i: usize, penalty: bool) {
        let principal = self.bonds[i].principal;
        self.move_account(ACCT_DEBT, f64::from(principal));
        if penalty {
            self.credit(CAT_FEES, -(0.02 * f64::from(principal)));
        }
        self.bonds.remove(i);
    }

    /// Year-end maturity (finance.md 4.4) [S]: every bond with `maturity == year + 1` is repaid
    /// without penalty.
    pub fn mature_bonds(&mut self, year: i32) {
        let mut i = 0;
        while i < self.bonds.len() {
            if self.bonds[i].maturity_year == year + 1 {
                self.repay_bond(i, false);
            } else {
                i += 1;
            }
        }
    }

    // ---- shares (finance.md 3.4) -------------------------------------------------------------

    /// Issues `n` new shares (`0x439060`, player path) [S]; returns the price per share. The
    /// price is the committed quote of `ftol(-0.7*n)` shares; 5% underwriting fee in the fees
    /// category. TODO: the date/time stamp `+0x160` and the UI limits (LNG 276..278) are not
    /// modelled (finance.md "Em aberto").
    pub fn issue_shares(&mut self, ctx: &Ctx, n: i32) -> i32 {
        let price = self.trade_price(ctx, ftol(-0.7 * f64::from(n)), true);
        self.credit(CAT_FEES, -(f64::from(n) * f64::from(price) * 0.05));
        self.shares += n;
        self.move_account(ACCT_CASH, f64::from(n) * f64::from(price));
        price
    }

    /// Buys back `n` of the company's own shares (`0x438f70`) [S]; returns the price.
    pub fn repurchase_shares(&mut self, ctx: &Ctx, n: i32) -> i32 {
        let price = self.trade_price(ctx, ftol(0.7 * f64::from(n)), true);
        let fee = f64::from(BROKERAGE) * f64::from(n) * f64::from(price);
        self.credit(CAT_FEES, -fee);
        self.shares -= n;
        self.move_account(ACCT_CASH, -(f64::from(n) * f64::from(price)));
        price
    }

    // ---- dividends (finance.md 6) ------------------------------------------------------------

    /// Maximum annual dividend per share the board supports (`0x43edf0`, 6.1) [V].
    pub fn max_dividend(&self, ctx: &Ctx) -> f64 {
        let s1 = f64::from(self.shares) + 1.0;
        let cashps = f32r(self.acct[ACCT_CASH] / s1);
        let n = (ctx.year - self.founded).min(3);
        let first = if ctx.month == 12 { 0 } else { 1 };
        let (mut ni, mut rv) = (0.0f64, 0.0f64);
        for i in first..=n {
            let (profit, revenue) = if i == 0 {
                (sum_cats(&self.cats, 13), sum_cats(&self.cats, 5))
            } else {
                let c = &self.arch_cats[(i - 1) as usize];
                (sum_cats(c, 13), sum_cats(c, 5))
            };
            if profit >= ni {
                ni = profit;
            }
            if revenue >= rv {
                rv = revenue;
            }
        }
        let x = if rv > ni { rv } else { ni };
        let half = x * 0.5;
        let mut y = if ni < half { ni } else { half };
        if n <= 1 {
            let floor = f64::from(self.shares) * f64::from(self.init_price) * 0.05;
            if y <= floor {
                y = floor;
            }
        }
        let mut yv = y / s1 * 1.25;
        if cashps <= yv {
            yv = cashps;
        }
        let y = f64::from(yv as f32);
        let reference = if n == 0 {
            f64::from(self.init_price)
        } else {
            self.arch_metrics[1][0]
        };
        let cap = reference * 0.35;
        let m = if cap < y { cap } else { y };
        if m > 0.0 { m } else { 0.0 }
    }

    /// Quarterly dividend (March, June, September, December; finance.md 6) [S]. Applies the
    /// suspension and board-cut rules, then moves the money (cash down, equity up, outside the
    /// income statement) and adds `d/4` to `dividends_paid`. Returns what was paid so the caller
    /// can credit shareholders with [`holder_dividend`].
    ///
    /// TODO(finance.md "Em aberto"): whether the suspension/cut is written back to `+0x30` is not
    /// known; this keeps `dividend` unchanged. The meaning of "patrimonio" in the `-0.2 *` test is
    /// assumed to be the book value ([`Self::equity`]).
    pub fn pay_quarterly_dividend(&mut self, ctx: &Ctx) -> DividendPayment {
        let mut d = f64::from(self.dividend);
        let mut outcome = if d > 0.0 {
            DividendOutcome::Paid
        } else {
            DividendOutcome::None
        };
        let paid = f64::from(self.shares) * d / 4.0;
        let cash_after = self.acct[ACCT_CASH] - paid;
        let max = self.max_dividend(ctx);
        if d > 0.0 && (cash_after < -0.2 * self.equity() || max <= 0.01) {
            d = 0.0;
            outcome = DividendOutcome::Suspended;
        } else if max < 0.75 * d {
            d = max;
            outcome = DividendOutcome::Reduced;
        }
        if d > 0.0 {
            let amount = f64::from(self.shares) * d / 4.0;
            self.acct[ACCT_CASH] -= amount;
            self.acct[ACCT_EQUITY] += amount;
            self.price_stamp = 0;
            self.dividends_paid += (d / 4.0) as f32;
        }
        DividendPayment { outcome, annual: d }
    }

    // ---- bankruptcy (finance.md 9) -----------------------------------------------------------

    /// Declares bankruptcy (`0x43bc30`) [V for the ledger and the shares, S for the rest]: every
    /// bond principal is halved (truncating), half the original debt becomes equity, positive
    /// cash is confiscated and negative cash halved, shares grow 50% rounded to thousands, and
    /// the bankruptcy year starts the price (10 years) and rating (15 years) penalties. Ends with
    /// the quote the original takes (`0x43b410(0, 0)`), before the year is stored.
    pub fn declare_bankruptcy(&mut self, ctx: &Ctx) {
        let mut total = 0i32;
        for b in &mut self.bonds {
            total += b.principal;
            b.principal /= 2;
        }
        let h = f64::from(total / 2);
        self.price_stamp = 0;
        self.acct[ACCT_DEBT] += h;
        let cash = self.acct[ACCT_CASH] - h;
        let cash = cash + h;
        self.acct[ACCT_CASH] = cash;
        self.acct[ACCT_EQUITY] -= h;
        let x = if cash > 0.0 { cash } else { cash * 0.5 };
        self.price_stamp = 0;
        self.acct[ACCT_CASH] = cash + (-x);
        self.acct[ACCT_EQUITY] -= -x;
        self.shares = ftol(f64::from(self.shares) * 1.5 * 0.001) * 1000;
        self.trade_price(ctx, 0, false);
        self.bankrupt_year = ctx.year;
    }

    // ---- monthly step (finance.md 5) ---------------------------------------------------------

    /// One monthly step (`0x4399d0`, day 28 at 23:59) [S]. The caller pays the returned
    /// dividend to the shareholders and `chairman_pay` to the chairman.
    ///
    /// TODO(finance.md "Em aberto"): whether `overhead/12` and `salary/12` are integer or float
    /// divisions is not established; float is used.
    pub fn monthly_step(&mut self, ctx: &Ctx, inp: &MonthlyInputs) -> MonthlyReport {
        // 1. Investor attitude.
        if inp.human_chairman {
            self.investor_mood = 0.0;
        } else {
            let h = f64::from(inp.human_shares);
            let mood = f64::from(self.investor_mood) * 0.92
                + ((1.0 - h / (f64::from(self.shares) + 1.0)) + 1.0) * 0.5 * 0.08;
            self.investor_mood = mood as f32;
        }
        // 2. Track maintenance (category 5).
        let track = self.acct[ACCT_TRACK];
        let upkeep = if track < 0.0 { 0.0 } else { track * -0.005 };
        self.credit(CAT_TRACK, upkeep * inp.track_effect);
        // 3. Overhead.
        self.credit(
            CAT_OVERHEAD,
            -(f64::from(self.overhead)) * MONTH * inp.overhead_effect,
        );
        // 4. Cash interest.
        let interest = self.monthly_cash_interest(ctx);
        self.credit(CAT_INTEREST, interest);
        // 5. Quarter: bond interest and dividends.
        let mut dividend = None;
        if ctx.month % 3 == 0 {
            let bi = self.quarterly_bond_interest();
            self.credit(CAT_INTEREST, bi);
            dividend = Some(self.pay_quarterly_dividend(ctx));
        }
        // 6. Salaries.
        let chairman = f64::from(self.chairman_salary);
        self.credit(CAT_SALARY, -chairman * MONTH);
        self.credit(
            CAT_SALARY,
            -f64::from(inp.manager_salary) * MONTH * inp.manager_effect,
        );
        // 7. Pressure decay and quote.
        self.decay_and_quote(ctx);
        MonthlyReport {
            dividend,
            chairman_pay: ftol(chairman * MONTH),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> FinanceTables {
        FinanceTables {
            econ_stock: [0.8, 0.9, 1.0, 1.1, 1.2],
            age_weights: [
                [0, 0, 0, 100],
                [25, 25, 35, 100],
                [50, 50, 65, 100],
                [75, 75, 85, 100],
                [100, 100, 100, 100],
            ],
        }
    }

    fn ctx(t: &FinanceTables) -> Ctx<'_> {
        Ctx {
            tables: t,
            year: 1900,
            month: 6,
            day: 10,
            minutes: 1_000_000,
            econ_state: 2,
            prime_rate: prime_rate_for_state(2),
            liquidity: 100.0,
            price_effect: 1.0,
            prime_effect_pct: 0,
            rating_effect: 0,
        }
    }

    #[test]
    fn prime_is_whole_percent() {
        assert_eq!(prime_rate_for(prime_rate_for_state(2), 0), 0.07);
        assert_eq!(prime_rate_for(prime_rate_for_state(0), 0), 0.09);
    }

    #[test]
    fn cash_interest_signs() {
        assert!(cash_interest(100_000.0, 0.07, 5.0) > 0.0);
        assert!(cash_interest(-100_000.0, 0.07, 5.0) < 0.0);
        // Overdraft costs more at a worse rating.
        assert!(cash_interest(-1e5, 0.07, 2.0) < cash_interest(-1e5, 0.07, 8.0));
    }

    #[test]
    fn ledger_keeps_double_entry() {
        let mut c = Company::default();
        c.credit(0, 1000.0);
        c.credit(5, -250.0);
        c.move_account(ACCT_DEBT, -500.0);
        let sum: f64 = c.acct.iter().sum();
        assert_eq!(sum, 0.0);
    }

    #[test]
    fn price_cache_is_per_minute() {
        let t = tables();
        let cx = ctx(&t);
        let mut c = Company {
            shares: 100_000,
            founded: 1890,
            acct: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -5e6],
            init_price: 10.0,
            ..Company::default()
        };
        let p = c.fundamental_price(&cx);
        c.acct[ACCT_EQUITY] = -9e6;
        assert_eq!(c.fundamental_price(&cx), p);
        c.credit(0, 1.0);
        assert_ne!(c.fundamental_price(&cx), p);
    }

    #[test]
    fn bankruptcy_halves_debt_and_adds_shares() {
        let t = tables();
        let cx = ctx(&t);
        let mut c = Company {
            shares: 100_000,
            founded: 1890,
            init_price: 10.0,
            bonds: vec![Bond {
                principal: 500_001,
                maturity_year: 1920,
                rate: 0.08,
            }],
            acct: [-1000.0, 0.0, 0.0, 0.0, 0.0, 0.0, -500_001.0, 501_001.0],
            ..Company::default()
        };
        c.declare_bankruptcy(&cx);
        assert_eq!(c.bonds[0].principal, 250_000);
        assert_eq!(c.shares, 150_000);
        assert_eq!(c.bankrupt_year, 1900);
        assert_eq!(c.acct[ACCT_CASH], -500.0);
    }

    #[test]
    fn split_scales_price_columns() {
        let mut c = Company {
            shares: 1000,
            last_price: 250.0,
            dividend: 5.0,
            ..Company::default()
        };
        c.arch_metrics[1][0] = 300.0;
        assert_eq!(c.split_if_needed(250), Some(5));
        assert_eq!(c.shares, 5000);
        assert_eq!(c.dividend, 1.0);
        assert_eq!(c.arch_metrics[1][0], 60.0);
        assert_eq!(c.split_if_needed(80), None);
    }
}
