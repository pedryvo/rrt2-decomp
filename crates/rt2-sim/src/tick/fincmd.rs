//! The financial commands of `cmd_execute` on the raw records (docs/spec/commands.md 2.1,
//! finance.md 3.4, 4.4, 6, 7, 14, 15; the types are in [`crate::fincmd`]).
//!
//! The arithmetic follows the original's code read in the disassembly (the x87 runs with a
//! 53-bit mantissa, so `f64` reproduces it); each function names the address it ports. What the
//! original does only for the local player's UI (sounds, windows, the `DAT_00574414` selection)
//! is left out. The scenario permission bytes and the finance model flags are read from the state
//! block (`VA - 0x5704e8`).
//!
//! Not modelled: the company list refresh of the UI; `net_min_peer_version` (a local game):
//! the voting functions require both companies to be active, as the oracle test does; the
//! "buy" of the editor mode (`0x5748f8` network lock) in `manager_pick`.

use super::company::{PE_ACTIVE, PE_CASH, PE_HOLDINGS, PE_HUMAN, ST_SET_COUNT, ST_SET_IDS};
use super::review::ST_MANAGER_USE;
use super::*;
use crate::calendar::Date;
use crate::company_record::{
    CO_ACCT, CO_ACTIVE, CO_BOND_COUNT, CO_BONDS, CO_CHAIRMAN, CO_MANAGER, CO_SHARES,
};
use crate::finance::{
    ACCT_CASH, ACCT_EQUITY, BROKERAGE, CAT_FEES, Company, MAX_BONDS, MIN_BOND_RATING,
};
use crate::fincmd::{ElectionRec, FinCmd, FinError, FinOutcome, MergerVoteRec, PendingVote};
use crate::message::{GameMessage, MarginTradeKind, VoteSide, VoteTally};

const PE_CHAIR_OF: usize = 0x263;
const PE_REP: usize = 0x52b;
const PE_ELECT_STAMP: usize = 0x527;
const PE_DEADLINE: usize = 0x544;
const PE_NAME: usize = 8;
const CO_NAME: usize = 4;
const CO_ISSUE_DATE: usize = 0x160;
const CO_ISSUE_TIME: usize = 0x164;
const CO_MERGER_FAIL: usize = 0x14c;
const CO_CHAIR_SINCE: usize = 0x470;
const CO_CHAIR_REP: usize = 0x46a;
const CO_MOOD: usize = 0x4276;
const CO_CHAIR_SALARY: usize = 0x134;

// Permission bytes and model flags (VA - 0x5704e8).
const PERM_SELL: u32 = 0x0057_06a9;
const PERM_BUY: u32 = 0x0057_06aa;
const PERM_BANKRUPT: u32 = 0x0057_06ac;
const PERM_ISSUE: u32 = 0x0057_06ad;
const PERM_BUYBACK: u32 = 0x0057_06ae;
const PERM_BOND: u32 = 0x0057_06af;
const PERM_REPAY: u32 = 0x0057_06b0;
const PERM_DIVIDEND: u32 = 0x0057_06b1;
const PERM_MERGER: u32 = 0x0057_06b4;
const PERM_TAKEOVER: u32 = 0x0057_06bb;
const PERM_RESIGN: u32 = 0x0057_06bc;
const PERM_SHORT: u32 = 0x0057_06bf;
const PERM_MARGIN: u32 = 0x0057_06c0;
const MODEL_MARGIN: u32 = 0x0057_3d72;
const MODEL_SHORT: u32 = 0x0057_3d73;
const MODEL_STOCK: u32 = 0x0057_3d74;
const MODEL_EDITOR: u32 = 0x0057_3d7a;
const ST_BASE: u32 = 0x0057_04e8;

/// Minimum price of a short sale (`0x535784`, f32 5.0).
const SHORT_MIN_PRICE: f64 = 5.0;
/// Minutes in a year of 12 x 28 days minus a day (`0x761ff`): the waiting time of the windows.
pub const YEAR_WAIT: i32 = 0x761ff;

fn cstr(b: &[u8]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).into_owned()
}

/// State of the margin routine across the nested calls of `0x46bb10` (`DAT_00573f5c` and
/// `DAT_00574814`).
#[derive(Default)]
pub(super) struct MarginGuard {
    running: bool,
    again: bool,
}

impl SimState {
    fn st8(&self, va: u32) -> bool {
        self.save.state[(va - ST_BASE) as usize] != 0
    }

    fn editor_mode(&self) -> bool {
        self.st8(MODEL_EDITOR)
    }

    pub(super) fn person_ids(&self) -> Vec<usize> {
        pool::list_order(&self.save.persons)
    }

    fn person_data(&self, person: usize) -> Option<&[u8]> {
        pool::record(&self.save.persons, person).map(|r| r.data.as_slice())
    }

    fn company_data(&self, slot: usize) -> Option<&[u8]> {
        pool::record(&self.save.companies, slot).map(|r| r.data.as_slice())
    }

    fn company_live(&self, slot: usize) -> bool {
        self.company_data(slot)
            .is_some_and(|d| i32_at(d, CO_ACTIVE) != 0)
    }

    /// `0x43a9e0`: the position of the company in the company set (the index of the holdings).
    pub(super) fn company_idx(&self, slot: usize) -> Option<usize> {
        self.company_set().iter().position(|&c| c == slot)
    }

    fn holdings(&self, person: usize, idx: usize) -> i32 {
        self.person_data(person)
            .map_or(0, |d| i32_at(d, PE_HOLDINGS + 4 * idx))
    }

    fn set_holdings(&mut self, person: usize, idx: usize, v: i32) {
        if let Some(r) = pool::record_mut(&mut self.save.persons, person) {
            put_u32(&mut r.data, PE_HOLDINGS + 4 * idx, v as u32);
        }
    }

    fn person_cash(&self, person: usize) -> f64 {
        self.person_data(person).map_or(0.0, |d| f64_at(d, PE_CASH))
    }

    fn person_name(&self, person: usize) -> String {
        self.person_data(person)
            .map_or(String::new(), |d| cstr(&d[PE_NAME..0x27]))
    }

    fn company_name(&self, slot: usize) -> String {
        self.company_data(slot)
            .map_or(String::new(), |d| cstr(&d[CO_NAME..CO_NAME + 0x1f]))
    }

    /// `trade_price(n, commit)` (`0x43b410`) on the record, storing the record back (the call
    /// refreshes the price cache).
    pub(super) fn fc_quote(&mut self, now: i32, slot: usize, n: i32, commit: bool) -> i32 {
        let Some(r) = pool::record(&self.save.companies, slot) else {
            return 0;
        };
        let mut co = Company::from_record(&r.data);
        let price = {
            let ctx = self.fin_ctx(now, slot);
            co.trade_price(&ctx, n, commit)
        };
        if let Some(r) = pool::record_mut(&mut self.save.companies, slot) {
            co.write_record(&mut r.data);
        }
        price
    }

    /// The quote of a deal of `n` shares without committing it (`0x43b410(n, 0)`), computed on a
    /// copy of the record: the price cache the original refreshes is not written (the executors
    /// call [`SimState::fc_quote`] where the original's calls leave it behind).
    pub(super) fn fc_price(&self, now: i32, slot: usize, n: i32) -> i32 {
        let Some(r) = pool::record(&self.save.companies, slot) else {
            return 0;
        };
        let mut co = Company::from_record(&r.data);
        let ctx = self.fin_ctx(now, slot);
        co.trade_price(&ctx, n, false)
    }

    /// `company_free_float` (`0x43e580`): shares not held long by active persons.
    pub(super) fn fc_free_float(&self, slot: usize) -> i32 {
        let shares = self.company_data(slot).map_or(0, |d| i32_at(d, CO_SHARES));
        let mut held = 0i32;
        if let Some(idx) = self.company_idx(slot) {
            for p in self.person_ids() {
                if let Some(d) = self.person_data(p)
                    && i32_at(d, PE_ACTIVE) != 0
                {
                    let h = i32_at(d, PE_HOLDINGS + 4 * idx);
                    if h > 0 {
                        held += h;
                    }
                }
            }
        }
        shares - held
    }

    /// `0x43e340(flag)`: how many shares may still be sold short:
    /// `((shares - long) >> (flag ? 2 : 1)) + short` (the shorts are negative).
    pub(super) fn fc_short_limit(&self, slot: usize, quarter: bool) -> i32 {
        let shares = self.company_data(slot).map_or(0, |d| i32_at(d, CO_SHARES));
        let (mut long, mut short) = (0i32, 0i32);
        if let Some(idx) = self.company_idx(slot) {
            for p in self.person_ids() {
                if let Some(d) = self.person_data(p)
                    && i32_at(d, PE_ACTIVE) != 0
                {
                    let h = i32_at(d, PE_HOLDINGS + 4 * idx);
                    if h > 0 {
                        long += h;
                    } else {
                        short += h;
                    }
                }
            }
        }
        ((shares - long) >> if quarter { 2 } else { 1 }) + short
    }

    /// The weighted position sum of `0x4a6f60`/`0x4a7a40`: each position times the last price,
    /// `0.5x` when long, `1.5x` when short (`x87` order: `f*0.5 + f + sum`).
    pub(super) fn fc_weighted(&self, person: &[u8]) -> f64 {
        let set = self.company_set();
        let mut s = 0.0f64;
        for i in 0..36 {
            let h = i32_at(person, PE_HOLDINGS + 4 * i);
            if h == 0 {
                continue;
            }
            let q = set
                .get(i)
                .and_then(|&c| self.company_data(c))
                .map_or(0.0, |d| f32_at(d, 0x427e));
            let f = f64::from(h) * f64::from(q);
            s = if f < 0.0 {
                f * 0.5 + f + s
            } else {
                f * 0.5 + s
            };
        }
        s
    }

    /// Buying power `0x4a7a40` uses: `weighted + cash`.
    pub(super) fn fc_power(&self, person: usize) -> f64 {
        self.person_data(person).map_or(0.0, |d| {
            self.fc_weighted(d)
                + f64::from_bits(u64::from_le_bytes(
                    d[PE_CASH..PE_CASH + 8].try_into().unwrap(),
                ))
        })
    }

    /// `person_can_buy` (`0x4a6f60(person, idx, margin, n)`).
    pub(super) fn fc_can_buy(
        &self,
        now: i32,
        person: usize,
        idx: usize,
        margin: bool,
        n: i32,
    ) -> bool {
        let margin = margin && self.st8(MODEL_MARGIN) && self.st8(PERM_MARGIN);
        let Some(&co) = self.company_set().get(idx) else {
            return false;
        };
        let chairman = self.company_data(co).map_or(0, |d| i32_at(d, CO_CHAIRMAN));
        if !(self.st8(MODEL_STOCK) || chairman == person as i32) {
            return false;
        }
        let n = if n == 0 {
            self.fc_lot(now, person, idx, true)
        } else {
            n
        };
        if self.holdings(person, idx) <= n.wrapping_neg() {
            return true;
        }
        if !self.st8(PERM_BUY) {
            return false;
        }
        if n > self.fc_free_float(co) {
            return false;
        }
        let cash = self.person_cash(person);
        let p = f64::from(self.fc_price(now, co, 0));
        if p * f64::from(n) <= cash {
            let sum = self
                .person_data(person)
                .map_or(0.0, |d| self.fc_weighted(d));
            let p2 = f64::from(self.fc_price(now, co, 0));
            if p2 * f64::from(n) <= sum + cash {
                return true;
            }
        }
        if margin {
            let sum = self
                .person_data(person)
                .map_or(0.0, |d| self.fc_weighted(d));
            let p3 = f64::from(self.fc_price(now, co, 0));
            if p3 * f64::from(n) <= sum + cash {
                return true;
            }
        }
        false
    }

    /// `person_can_sell` (`0x4a7190(person, idx, short, cover, n)`).
    pub(super) fn fc_can_sell(
        &self,
        now: i32,
        person: usize,
        idx: usize,
        short: bool,
        cover: bool,
        n: i32,
    ) -> bool {
        let short = short && self.st8(MODEL_SHORT) && self.st8(PERM_SHORT);
        if !self.st8(PERM_SELL) {
            return false;
        }
        let n = if n == 0 {
            self.fc_lot(now, person, idx, false)
        } else {
            n
        };
        if n <= self.holdings(person, idx) {
            return true;
        }
        if short && let Some(&co) = self.company_set().get(idx) {
            let chairman = self.company_data(co).map_or(0, |d| i32_at(d, CO_CHAIRMAN));
            if chairman != person as i32 && n < self.fc_short_limit(co, cover) {
                let p = f64::from(self.fc_price(now, co, 0));
                if SHORT_MIN_PRICE <= p {
                    let sum = self
                        .person_data(person)
                        .map_or(0.0, |d| self.fc_weighted(d));
                    let cash = self.person_cash(person);
                    if p * f64::from(n) <= sum + cash {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `person_trade_lot` (`0x4a6e00(person, idx, buy)`): the lot of a human's trade, the
    /// largest multiple of 1000 around `4% of the position + 1000` the checks allow; 1000 for a
    /// computer person.
    pub(super) fn fc_lot(&self, now: i32, person: usize, idx: usize, buy: bool) -> i32 {
        let human = self
            .person_data(person)
            .is_some_and(|d| i32_at(d, PE_HUMAN) != 0);
        if !human {
            return 1000;
        }
        let h = self.holdings(person, idx);
        let mut n;
        if !buy {
            n = ftol(f64::from(h.wrapping_abs()) * 0.04 + 1000.0) / 1000 * 1000;
            let short = h < 1000;
            let mut ok = false;
            while 1000 < n && !ok {
                n -= 1000;
                ok = self.fc_can_sell(now, person, idx, short, false, n);
            }
        } else {
            n = ftol(f64::from(h) * 0.04 + 1000.0) / 1000 * 1000;
            let first = self.fc_can_buy(now, person, idx, false, 1000);
            let mut ok = false;
            while 1000 < n && !ok {
                n -= 1000;
                ok = self.fc_can_buy(now, person, idx, !first, n);
            }
        }
        let r = (n + 500) / 1000 * 1000;
        if r > 999 { r } else { 1000 }
    }

    /// `person_buy_stock` (`0x4a7300(person, idx, n, margin, forced)`); returns whether it traded.
    #[allow(clippy::too_many_arguments)] // the original's `0x4a7300` takes these
    pub(super) fn fc_buy(
        &mut self,
        now: i32,
        w: &mut World,
        mg: &mut MarginGuard,
        person: usize,
        idx: usize,
        n: i32,
        margin: bool,
        forced: bool,
    ) -> Option<(i32, f64)> {
        if !forced && !self.fc_can_buy(now, person, idx, margin, 0) {
            return None;
        }
        let co = *self.company_set().get(idx)?;
        self.fc_quote(now, co, n, true);
        let after = self.fc_price(now, co, 0);
        let cost = ftol(f64::from(after) * f64::from(n));
        let h = self.holdings(person, idx);
        self.set_holdings(person, idx, h.wrapping_add(n));
        let debit = ftol((f64::from(BROKERAGE) + 1.0) * f64::from(cost));
        let cash = self.person_cash(person) - f64::from(debit);
        if let Some(r) = pool::record_mut(&mut self.save.persons, person) {
            put_f64(&mut r.data, PE_CASH, cash);
        }
        self.fc_margin_check(now, w, mg);
        Some((after, -f64::from(debit)))
    }

    /// `person_sell_stock` (`0x4a73d0(person, idx, n, forced)`).
    #[allow(clippy::too_many_arguments)] // the original's `0x4a73d0` takes these
    pub(super) fn fc_sell(
        &mut self,
        now: i32,
        w: &mut World,
        mg: &mut MarginGuard,
        person: usize,
        idx: usize,
        n: i32,
        forced: bool,
    ) -> Option<(i32, f64)> {
        let held = self.holdings(person, idx);
        if !(forced || (0 < n && n <= held)) {
            return None;
        }
        let co = *self.company_set().get(idx)?;
        let mut n = n;
        if held < n && !forced {
            n = held;
        }
        let price = self.fc_quote(now, co, n.wrapping_neg(), true);
        let gross = price.wrapping_mul(n);
        self.set_holdings(person, idx, held.wrapping_sub(n));
        let credit = ftol((1.0 - f64::from(BROKERAGE)) * f64::from(gross));
        let cash = self.person_cash(person) + f64::from(credit);
        if let Some(r) = pool::record_mut(&mut self.save.persons, person) {
            put_f64(&mut r.data, PE_CASH, cash);
        }
        self.fc_margin_check(now, w, mg);
        Some((price, f64::from(credit)))
    }

    /// `ai_person_trade_stocks` (`0x4a8af0`, ai.md 9.2) for the computer person `person`: a
    /// greedy loop that buys or sells one lot of 1000 shares per round while the best score
    /// is at least 1.15. Scores (x87 chains, the f32 locals rounded as in the original):
    /// buy `fair / price(+1000)`, sell `price(-1000) / fair` (`fair` = `+0x427a`, x1.1 for the
    /// person's own company), adjusted by the persona's investing (`0x551e97`), margin
    /// (`0x551e8f`) and short-selling (`0x551e93`) traits and the leverage `L` = holdings value
    /// / net worth (`0x4a6d00`, `0x4a7620`). No RNG; the interface news (LNG 0x911..0x914)
    /// only names the local company's trades.
    pub(super) fn ai_trade_stocks(&mut self, w: &mut World, person: usize) {
        let now = w.minutes;
        let mut mg = MarginGuard::default();
        let trait_of = |s: &Self, base: u32| -> f64 {
            let k = s
                .person_data(person)
                .map_or(0, |d| i32::from(d[0x52f] as i8));
            f64::from(s.data.image.i32(base.wrapping_add((k * 0x23) as u32)))
        };
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 100_000 {
                break;
            }
            let Some(pd) = self.person_data(person) else {
                return;
            };
            let cash = f64_at(pd, PE_CASH);
            let chair_of = i32_at(pd, PE_CHAIR_OF);
            let m = if cash <= 100_000.0 {
                1.0f32
            } else {
                (cash * 1e-5) as f32
            };
            let set = self.company_set();
            let mut value = 0.0f64;
            for (i, &c) in set.iter().enumerate().take(36) {
                let h = self.holdings(person, i);
                if h != 0 {
                    value += f64::from(self.fc_price(now, c, 0)) * f64::from(h);
                }
            }
            let worth = value + cash;
            let lev = (value / worth) as f32;
            let lf = f64::from(lev);
            let mut best = 0.0f32;
            let mut best_co = 0i32;
            let mut best_idx = 0usize;
            let mut buy = true;
            for (idx, &c) in set.iter().enumerate() {
                let can_buy = self.fc_can_buy(now, person, idx, true, 0);
                let can_sell = self.fc_can_sell(now, person, idx, true, true, 0);
                let Some(cd) = self.company_data(c) else {
                    continue;
                };
                let own = i32_at(cd, CO_CHAIRMAN) == person as i32;
                let other = !own && chair_of != 0;
                if !can_buy && !can_sell {
                    continue;
                }
                let mut fair = f32_at(cd, 0x427a);
                if own {
                    fair = (f64::from(fair) * 1.1) as f32;
                }
                if can_buy {
                    let price = self.fc_price(now, c, 1000) as f32;
                    let mut a = f64::from(fair) / f64::from(price);
                    if other {
                        a = (a - 1.0) * (trait_of(self, 0x0055_1e97) * 0.01) + 1.0;
                    }
                    if f64::from(price) * 1000.0 > cash {
                        a = (a - 1.0) * (trait_of(self, 0x0055_1e8f) * 0.005) + 1.0;
                        if lf > 1.25 {
                            a = (a - 1.0) / lf + 1.0;
                        }
                    }
                    if a > 0.95 {
                        a *= f64::from(m);
                    }
                    if a > f64::from(best) {
                        best = a as f32;
                        best_co = c as i32;
                        best_idx = idx;
                        buy = true;
                    }
                }
                if can_sell {
                    let price = self.fc_price(now, c, -1000);
                    let mut v = f64::from(price) / f64::from(fair);
                    if other {
                        v = (v - 1.0) * (trait_of(self, 0x0055_1e97) * 0.005) + 1.0;
                    }
                    let h = self.holdings(person, idx);
                    if h < 1 {
                        v = (v - 1.0) * (trait_of(self, 0x0055_1e93) * 0.005) + 1.0;
                    } else if v > 0.92 {
                        let mut l2 = if lf <= 1.1 {
                            lf
                        } else {
                            (lf - 1.1) * 3.0 + 1.1
                        };
                        if l2 < 0.5 {
                            l2 = 0.5;
                        }
                        v = l2 * (v - 0.9) + 0.9;
                    }
                    if !other {
                        v = if h <= 1000 {
                            0.0
                        } else {
                            (v - 1.0) * 0.6 + 1.0
                        };
                    }
                    if v > f64::from(best) {
                        best = v as f32;
                        best_co = c as i32;
                        best_idx = idx;
                        buy = false;
                    }
                }
            }
            if best_co == 0 || f64::from(best) < 1.15 {
                break;
            }
            self.log.ran("ai_person_trade");
            if buy {
                self.fc_buy(now, w, &mut mg, person, best_idx, 1000, true, false);
            } else {
                self.fc_sell(now, w, &mut mg, person, best_idx, 1000, true);
            }
        }
        self.log.ran("ai_person_trade_stocks");
    }

    /// `margin_check_all` (`0x46bb10`): runs the broker's routine for every person of the
    /// player set until a pass sells nothing. Nested calls (from the broker's own sales) only
    /// ask for another pass.
    pub(super) fn fc_margin_check(&mut self, now: i32, w: &mut World, mg: &mut MarginGuard) {
        mg.again = true;
        if mg.running || !self.st8(MODEL_MARGIN) {
            return;
        }
        mg.running = true;
        loop {
            mg.again = false;
            for p in self.player_set() {
                self.fc_margin_call(now, w, mg, p);
            }
            if !mg.again {
                break;
            }
        }
        mg.running = false;
    }

    /// The broker for one person (`0x4a7a40`, finance.md 7.3).
    fn fc_margin_call(&mut self, now: i32, w: &mut World, mg: &mut MarginGuard, person: usize) {
        let Some(data) = self.person_data(person).map(<[u8]>::to_vec) else {
            return;
        };
        let human = i32_at(&data, PE_HUMAN) != 0;
        let mut had_positions = false;
        let n_set = self.company_set().len();
        let power = |s: &SimState, p: usize| s.fc_power(p);
        if power(self, person) >= 0.0 {
            if let Some(r) = pool::record_mut(&mut self.save.persons, person) {
                put_u32(&mut r.data, PE_DEADLINE, 0);
            }
        } else {
            for i in 0..n_set {
                if self.holdings(person, i) != 0 {
                    had_positions = true;
                }
            }
            if !had_positions {
                return;
            }
            let deadline = i32_at(&data, PE_DEADLINE);
            if !human || (deadline != 0 && deadline <= now) {
                for pass in 0..2 {
                    for i in 0..n_set {
                        let chair_of_i = self
                            .company_set()
                            .get(i)
                            .and_then(|&c| self.company_data(c))
                            .map_or(0, |d| i32_at(d, CO_CHAIRMAN));
                        if pass == 0 && chair_of_i == person as i32 {
                            continue;
                        }
                        let (mut sold, mut covered) = (0i32, 0i32);
                        if self.holdings(person, i) > 0 {
                            loop {
                                if power(self, person) >= 0.0 {
                                    break;
                                }
                                sold += 1000;
                                self.fc_sell(now, w, mg, person, i, 1000, false);
                                if self.holdings(person, i) <= 0 {
                                    break;
                                }
                            }
                            if sold > 0 && human {
                                self.fc_margin_msg(person, i, sold, MarginTradeKind::Sold);
                            }
                        }
                        if self.holdings(person, i) < 0 {
                            loop {
                                if power(self, person) >= 0.0 {
                                    break;
                                }
                                covered += 1000;
                                self.fc_cover(now, w, mg, person, i);
                                if self.holdings(person, i) >= 0 {
                                    break;
                                }
                            }
                            if covered > 0 && human {
                                self.fc_margin_msg(person, i, covered, MarginTradeKind::Covered);
                            }
                        }
                    }
                }
                if let Some(r) = pool::record_mut(&mut self.save.persons, person) {
                    put_u32(&mut r.data, PE_DEADLINE, 0);
                }
            } else {
                if human && deadline == 0 {
                    let d = date_of(now);
                    self.ts_globals.news.push(GameMessage::MarginWarning {
                        person: person as i32,
                        month: d.month % 12,
                    });
                    if w.speed > 4 {
                        w.change_speed(4);
                    }
                }
                if deadline == 0 {
                    let d = date_of(now);
                    let t = Date::new(d.year, d.month, 28, 23, 59).to_minutes() + 0x9d7f;
                    if let Some(r) = pool::record_mut(&mut self.save.persons, person) {
                        put_u32(&mut r.data, PE_DEADLINE, t as u32);
                    }
                }
            }
        }
        // Short positions of companies without free float are covered by force.
        for i in 0..36 {
            if self.holdings(person, i) >= 0 {
                continue;
            }
            let Some(&co) = self.company_set().get(i) else {
                continue;
            };
            let mut forced = 0i32;
            loop {
                if self.fc_short_limit(co, false) >= 0 {
                    break;
                }
                forced += 1000;
                self.fc_cover(now, w, mg, person, i);
                if self.holdings(person, i) >= 0 {
                    break;
                }
            }
            if forced > 0 && human {
                self.fc_margin_msg(person, i, forced, MarginTradeKind::Squeeze);
            }
        }
    }

    /// The broker's cover of 1000 shares (`0x4a7a40`): the purchase of `person_buy_stock` for a
    /// lot of 1000 without the permission checks, then another margin pass.
    fn fc_cover(
        &mut self,
        now: i32,
        w: &mut World,
        mg: &mut MarginGuard,
        person: usize,
        idx: usize,
    ) {
        let Some(&co) = self.company_set().get(idx) else {
            return;
        };
        self.fc_quote(now, co, 1000, true);
        let after = self.fc_price(now, co, 0);
        let h = self.holdings(person, idx);
        self.set_holdings(person, idx, h + 1000);
        let cost = ftol(f64::from(after) * 1000.0);
        let debit = ftol((f64::from(BROKERAGE) + 1.0) * f64::from(cost));
        let cash = self.person_cash(person) - f64::from(debit);
        if let Some(r) = pool::record_mut(&mut self.save.persons, person) {
            put_f64(&mut r.data, PE_CASH, cash);
        }
        self.fc_margin_check(now, w, mg);
    }

    fn fc_margin_msg(&mut self, person: usize, idx: usize, shares: i32, kind: MarginTradeKind) {
        let company = self.company_set().get(idx).copied().unwrap_or(0);
        let name = self.company_name(company);
        self.ts_globals.news.push(GameMessage::MarginTrade {
            person: person as i32,
            company: company as i32,
            company_name: name,
            shares,
            kind,
        });
    }
}

// ---- the news list and the local player ----------------------------------------------------------

/// Header offset of the local company (`0x570574`, the saved copy of `DAT_00574414`).
const HDR_LOCAL_COMPANY: usize = 0x8a3c;
/// Header offset of `DAT_00570512`, cleared when the local company goes away.
const HDR_LOCAL_FLAG: usize = 0x89da;
/// State offsets of `DAT_005707bd` (news flag) and `DAT_005707be` (news counter).
const ST_NEWS_FLAG: usize = 0x2d5;
const ST_NEWS_COUNTER: usize = 0x2d6;

impl SimState {
    /// `news_post` (`0x4a42f0`): the status line of the original adds an item to the news list
    /// (not part of a save) with a random id from the CRT generator, sets the "news" flag and
    /// numbers the item. Only the counter, the flag and the CRT draw are state.
    pub(super) fn fc_news_post(&mut self, w: &mut World) {
        let _ = w.crt.range(0, 10000);
        self.save.state[ST_NEWS_FLAG] = 1;
        let n = u32_at(&self.save.state, ST_NEWS_COUNTER).wrapping_add(1);
        put_u32(&mut self.save.state, ST_NEWS_COUNTER, n);
    }

    /// The local player's person (`DAT_00566ca4`): the first active human of the person list.
    pub(super) fn fc_local_person(&self) -> i32 {
        self.person_ids()
            .into_iter()
            .find(|&p| {
                self.person_data(p)
                    .is_some_and(|d| i32_at(d, PE_ACTIVE) != 0 && i32_at(d, PE_HUMAN) != 0)
            })
            .map_or(0, |p| p as i32)
    }

    /// `set_local_company` (`0x467fd0`): the saved copy of the local company id.
    fn fc_set_local_company(&mut self, c: i32) {
        let h = &mut self.save.header;
        if i32_at(h, HDR_LOCAL_COMPANY) == c {
            return;
        }
        put_u32(h, HDR_LOCAL_COMPANY, c.max(0) as u32);
        if c < 1 {
            h[HDR_LOCAL_FLAG] = 0;
        }
    }
}

// ---- company primitives ----------------------------------------------------------------------

impl SimState {
    /// Runs `f` on the finance view of the record (`Company`) and stores it back.
    fn fc_with_company<R>(
        &mut self,
        now: i32,
        slot: usize,
        f: impl FnOnce(&mut Company, &crate::finance::Ctx) -> R,
    ) -> Option<R> {
        let r = pool::record(&self.save.companies, slot)?;
        let mut co = Company::from_record(&r.data);
        let out = {
            let ctx = self.fin_ctx(now, slot);
            f(&mut co, &ctx)
        };
        let r = pool::record_mut(&mut self.save.companies, slot)?;
        co.write_record(&mut r.data);
        Some(out)
    }

    /// `company_issue_stock(n, 0)` (`0x439060`): sells `n` new shares at the quote of
    /// `ftol(-0.7 n)` shares, 5% underwriting fee in the fees category, and stamps the date of
    /// the issue (`+0x160`, `+0x164`). Returns the price.
    fn fc_issue_stock(&mut self, now: i32, slot: usize, n: i32) -> Option<i32> {
        let price = self.fc_with_company(now, slot, |co, ctx| {
            let q = ftol(-f64::from(n) * 0.7);
            let price = co.trade_price(ctx, q, true);
            co.price_stamp = 0;
            let d = -f64::from(n.wrapping_mul(price)) * 0.05;
            co.cats[CAT_FEES] += d;
            co.year_cats[CAT_FEES] += d;
            co.price_stamp = 0;
            co.acct[ACCT_CASH] += d;
            co.acct[ACCT_EQUITY] -= d;
            co.price_stamp = 0;
            co.shares = co.shares.wrapping_add(n);
            let g = f64::from(n.wrapping_mul(price));
            co.acct[ACCT_CASH] += g;
            co.acct[ACCT_EQUITY] -= g;
            price
        })?;
        let d = date_of(now);
        let r = pool::record_mut(&mut self.save.companies, slot)?;
        put_u16(&mut r.data, CO_ISSUE_DATE, d.year);
        r.data[CO_ISSUE_DATE + 2] = d.month;
        r.data[CO_ISSUE_DATE + 3] = d.day;
        r.data[CO_ISSUE_TIME] = d.hour;
        r.data[CO_ISSUE_TIME + 1] = d.minute;
        Some(price)
    }

    /// `company_buy_back_stock(n)` (`0x438f70`): buys `n` shares back at the quote of
    /// `ftol(0.7 n)` shares with the brokerage in the fees category (all in `f64` of the x87:
    /// `0.03 * (n * price)`). Returns the price.
    fn fc_buy_back(&mut self, now: i32, slot: usize, n: i32) -> Option<i32> {
        self.fc_with_company(now, slot, |co, ctx| {
            let q = ftol(f64::from(n) * 0.7);
            let price = co.trade_price(ctx, q, true);
            let prod = f64::from(n) * f64::from(price);
            co.price_stamp = 0;
            let fee = -(f64::from(BROKERAGE) * prod);
            co.cats[CAT_FEES] += fee;
            co.year_cats[CAT_FEES] += fee;
            co.price_stamp = 0;
            let neg = -prod;
            co.shares = co.shares.wrapping_sub(n);
            co.acct[ACCT_CASH] = (fee + co.acct[ACCT_CASH]) + neg;
            co.acct[ACCT_EQUITY] = (co.acct[ACCT_EQUITY] - fee) - neg;
            price
        })
    }

    /// `company_set_manager(g, 1)` (`0x438780`): hires manager `g` (nothing if he works for a
    /// company) and charges the signing bonus; the candidates of every company that listed him are
    /// drawn again (shared with the scenario effects, `review.rs`).
    fn fc_set_manager(&mut self, w: &mut World, slot: usize, g: i32) -> Result<(), FinError> {
        if !(0..40).contains(&g) {
            return Err(FinError::NoSuchObject);
        }
        if self.save.state[ST_MANAGER_USE + g as usize] != 0 {
            return Err(FinError::ManagerTaken);
        }
        self.company_data(slot).ok_or(FinError::NoSuchObject)?;
        let minutes = w.minutes;
        self.company_set_manager(&mut w.rng, minutes, slot, g as usize, true);
        Ok(())
    }

    /// `company_set_chairman(new)` (`0x438900`): the old chairman loses reputation for a short
    /// term (`0x4a8100`, at least -99) and his post, the investors' mood moves, the salary is
    /// reset to 10 000 and the chair year starts.
    pub(super) fn fc_set_chairman(&mut self, now: i32, slot: usize, new: i32) {
        let Some(cd) = self.company_data(slot).map(<[u8]>::to_vec) else {
            return;
        };
        let old = i32_at(&cd, CO_CHAIRMAN);
        if old == new {
            return;
        }
        let year = i32::from(date_of(now).year);
        if old != 0 {
            let since = i32::from(pool::i16_at(&cd, CO_CHAIR_SINCE));
            let held = (year - since).clamp(0, 10);
            let penalty = (held - 10) * 7;
            if let Some(r) = pool::record_mut(&mut self.save.persons, old as usize) {
                let rep = (i32_at(&r.data, PE_REP) + penalty).max(-99);
                put_u32(&mut r.data, PE_REP, rep as u32);
                put_u32(&mut r.data, PE_CHAIR_OF, 0);
            }
            if old == self.fc_local_person() {
                self.fc_set_local_company(0);
            }
        }
        let human = new != 0 && self.person_is_human(new);
        let new_rep = if new != 0 {
            self.person_data(new as usize)
                .map_or(0, |d| i32_at(d, PE_REP))
        } else {
            0
        };
        if new != 0
            && let Some(r) = pool::record_mut(&mut self.save.persons, new as usize)
        {
            put_u32(&mut r.data, PE_CHAIR_OF, slot as u32);
        }
        if new != 0 && new == self.fc_local_person() {
            self.fc_set_local_company(slot as i32);
        }
        if let Some(r) = pool::record_mut(&mut self.save.companies, slot) {
            let d = &mut r.data;
            put_u32(d, CO_CHAIRMAN, new as u32);
            let mood = f32_at(d, CO_MOOD);
            put_f32(
                d,
                CO_MOOD,
                if human {
                    0.0
                } else {
                    (f64::from(mood) * 0.8 + 0.2) as f32
                },
            );
            put_u32(d, CO_CHAIR_REP, if new == 0 { 0 } else { new_rep as u32 });
            put_u32(d, CO_CHAIR_SALARY, 10_000);
            put_u16(d, CO_CHAIR_SINCE, year as u16);
        }
    }

    /// `company_chairman_takeover(person)` (`0x43c050`): the person leaves the other company he
    /// chaired and takes this one.
    fn fc_takeover(&mut self, w: &mut World, slot: usize, person: usize) {
        self.fc_news_post(w);
        let pname = self.person_name(person);
        let cname = self.company_name(slot);
        self.ts_globals.news.push(GameMessage::ChairmanChanged {
            company: slot as i32,
            person: person as i32,
            person_name: pname,
            company_name: cname,
        });
        let other = self
            .person_data(person)
            .map_or(0, |d| i32_at(d, PE_CHAIR_OF));
        if other != 0 {
            self.fc_set_chairman(w.minutes, other as usize, 0);
        }
        self.fc_set_chairman(w.minutes, slot, person as i32);
    }

    /// `company_liquidate(0)` (`0x438170`), after the merger moved everything away: frees the
    /// manager, removes the company column of every person's positions, removes the chairman's
    /// post, marks the company inactive and drops it from the company set.
    pub(super) fn fc_liquidate(&mut self, slot: usize) {
        // Track the company still owns goes ownerless (`0x437ab0(0, 0)`).
        for r in self.save.track_pieces.records.iter_mut() {
            if usize::from(r.data[2]) == slot {
                r.data[2] = 0;
            }
        }
        let idx = self.company_idx(slot);
        let n = self.company_set().len();
        let mgr = self
            .company_data(slot)
            .map_or(-1, |d| i32_at(d, CO_MANAGER));
        if mgr != -1 {
            if let Some(u) = self.save.state.get_mut(ST_MANAGER_USE + mgr as usize)
                && usize::from(*u) == slot
            {
                *u = 0;
            }
            if let Some(r) = pool::record_mut(&mut self.save.companies, slot) {
                put_u32(&mut r.data, CO_MANAGER, 0xffff_ffff);
            }
        }
        if let Some(i) = idx {
            for r in self.save.persons.records.iter_mut() {
                put_u32(&mut r.data, PE_HOLDINGS + 4 * i, 0);
                if i + 1 < n && i < n - 1 {
                    for j in i..n - 1 {
                        let v = i32_at(&r.data, PE_HOLDINGS + 4 * (j + 1));
                        put_u32(&mut r.data, PE_HOLDINGS + 4 * j, v as u32);
                    }
                }
                put_u32(&mut r.data, 0x1cf + 4 * n, 0);
            }
        }
        let chair = self
            .company_data(slot)
            .map_or(0, |d| i32_at(d, CO_CHAIRMAN));
        if chair != 0
            && let Some(r) = pool::record_mut(&mut self.save.persons, chair as usize)
        {
            put_u32(&mut r.data, PE_CHAIR_OF, 0);
        }
        if i32_at(&self.save.header, HDR_LOCAL_COMPANY) == slot as i32 {
            self.fc_set_local_company(0);
        }
        if let Some(r) = pool::record_mut(&mut self.save.companies, slot) {
            put_u32(&mut r.data, CO_ACTIVE, 0);
        }
        // `0x46a830`: out of the company set.
        let set = self.company_set();
        if let Some(i) = set.iter().position(|&c| c == slot) {
            let st = &mut self.save.state;
            for j in i..set.len() - 1 {
                let v = u16_at(st, ST_SET_IDS + 2 * (j + 1));
                put_u16(st, ST_SET_IDS + 2 * j, v);
            }
            put_u32(st, ST_SET_COUNT, set.len() as u32 - 1);
        }
    }

    /// `merger_execute(buyer, target, offer)` (`0x43c3c0`): the buyer takes the track, trains,
    /// stations, bonds, industries and accounts of the target, pays every holder `shares *
    /// offer` and the target is liquidated.
    fn fc_merge(&mut self, w: &mut World, buyer: usize, target: usize, offer: i32) {
        let now = w.minutes;
        self.fc_news_post(w);
        let bname = self.company_name(buyer);
        let tname = self.company_name(target);
        self.ts_globals.news.push(GameMessage::MergerDone {
            buyer: buyer as i32,
            buyer_name: bname,
            target_name: tname,
        });
        // Track pieces, trains (`+0x41`), stations (`+0x39`), industries (`+0x97`).
        for r in self.save.track_pieces.records.iter_mut() {
            if usize::from(r.data[2]) == target {
                r.data[2] = buyer as u8;
            }
        }
        let mut moved_trains = 0i32;
        for r in self.save.trains.records.iter_mut() {
            if u32_at(&r.data, 0x41) as usize == target {
                put_u32(&mut r.data, 0x41, buyer as u32);
                moved_trains += 1;
            }
        }
        let mut moved_stations = 0i32;
        for r in self.save.stations.records.iter_mut() {
            if u32_at(&r.data, 0x39) as usize == target {
                put_u32(&mut r.data, 0x39, buyer as u32);
                moved_stations += 1;
            }
        }
        // Every holder of the target is paid and loses the position.
        let tidx = self.company_idx(target);
        let shares = self
            .company_data(target)
            .map_or(0, |d| i32_at(d, CO_SHARES));
        if let Some(i) = tidx {
            for p in self.person_ids() {
                let h = self.holdings(p, i);
                if h != 0 {
                    let c = self.person_cash(p) + f64::from(h.wrapping_mul(offer));
                    if let Some(r) = pool::record_mut(&mut self.save.persons, p) {
                        put_f64(&mut r.data, PE_CASH, c);
                    }
                }
                self.set_holdings(p, i, 0);
            }
        }
        // The buyer pays; bonds (up to 20), industries and the eight accounts move over.
        let tdata = self
            .company_data(target)
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        let cost = f64::from(shares) * f64::from(offer);
        if let Some(r) = pool::record_mut(&mut self.save.companies, buyer) {
            let d = &mut r.data;
            put_u32(d, 0x4250, 0);
            let cash = f64_at(d, CO_ACCT) + -cost;
            put_f64(d, CO_ACCT, cash);
            let eq = f64_at(d, CO_ACCT + 8 * ACCT_EQUITY) - -cost;
            put_f64(d, CO_ACCT + 8 * ACCT_EQUITY, eq);
            let tn = i32_at(&tdata, CO_BOND_COUNT);
            for k in 0..tn.max(0) as usize {
                let n = i32_at(d, CO_BOND_COUNT);
                if n < 20 {
                    let src = CO_BONDS + 12 * k;
                    let dst = CO_BONDS + 12 * n as usize;
                    d[dst..dst + 12].copy_from_slice(&tdata[src..src + 12]);
                    put_u32(d, CO_BOND_COUNT, n as u32 + 1);
                }
            }
            for k in 0..8 {
                let v = f64_at(d, CO_ACCT + 8 * k) + f64_at(&tdata, CO_ACCT + 8 * k);
                put_f64(d, CO_ACCT + 8 * k, v);
            }
            for o in [0x182usize, 0x186, 0x18a, 0x18e] {
                let v = i32_at(d, o).wrapping_add(i32_at(&tdata, o));
                put_u32(d, o, v as u32);
            }
            let t = i32_at(d, 0x27).wrapping_add(moved_trains);
            put_u32(d, 0x27, t as u32);
            let s = i32_at(d, 0x172).wrapping_add(moved_stations);
            put_u32(d, 0x172, s as u32);
        }
        for r in self.save.objects.records.iter_mut() {
            if usize::from(pool::u16_at(&r.data, 0x97)) == target {
                put_u16(&mut r.data, 0x97, buyer as u16);
            }
        }
        self.fc_liquidate(target);
        self.fc_quote(now, buyer, 0, false);
    }
}

// ---- votes (finance.md 15) -------------------------------------------------------------------

impl SimState {
    /// The tally dialog of a vote (`0x4015a0`/`0x42ee00`): the persons that hold shares of
    /// `target_idx` and voted yes or no, the public's share of the free float and the totals.
    #[allow(clippy::too_many_arguments)]
    fn fc_tally(
        &self,
        persons: &[usize],
        vote: &[bool],
        idx: usize,
        shares: i32,
        float: i32,
        public_yes: i32,
        yes_total: i32,
    ) -> VoteTally {
        let (mut yes, mut no) = (VoteSide::default(), VoteSide::default());
        for (i, &p) in persons.iter().enumerate() {
            let h = self.holdings(p, idx);
            if h > 0 {
                if vote.get(i).copied().unwrap_or(false) {
                    yes.voters.push((p as i32, h));
                } else {
                    no.voters.push((p as i32, h));
                }
            }
        }
        yes.public = public_yes;
        yes.total = yes_total;
        no.public = float - public_yes;
        no.total = shares - yes_total;
        VoteTally { yes, no }
    }

    /// `merger_propose` (`0x48b4f0`) and the count (`0x4015a0`).
    fn fc_merger_propose(
        &mut self,
        w: &mut World,
        buyer: usize,
        target: usize,
        offer: i32,
    ) -> Result<FinOutcome, FinError> {
        if !self.company_live(buyer) || !self.company_live(target) {
            return Err(FinError::NoSuchObject);
        }
        let idx = self.company_idx(target).ok_or(FinError::NoSuchObject)?;
        let persons = self.person_ids();
        let n = persons.len();
        let mut rec = MergerVoteRec {
            id: w.votes.next_merger_id(),
            buyer: buyer as u16,
            target: target as u16,
            offer: offer as u16,
            answered: vec![true; n],
            vote: vec![false; n],
        };
        for (i, &p) in persons.iter().enumerate() {
            let Some(d) = self.person_data(p) else {
                continue;
            };
            if i32_at(d, PE_HOLDINGS + 4 * idx) > 0 {
                if i32_at(d, PE_CHAIR_OF) == buyer as i32 {
                    rec.vote[i] = true;
                } else if i32_at(d, PE_HUMAN) == 0 {
                    rec.vote[i] = false;
                } else {
                    rec.answered[i] = false;
                }
            }
        }
        let id = rec.id;
        w.votes.mergers.push(rec);
        self.fc_merger_count(w, id)
    }

    /// `0x4015a0`: counts the votes of the record `id` when everyone has answered.
    pub(super) fn fc_merger_count(
        &mut self,
        w: &mut World,
        id: u32,
    ) -> Result<FinOutcome, FinError> {
        let pos = w
            .votes
            .mergers
            .iter()
            .position(|m| m.id == id)
            .ok_or(FinError::NoSuchVote)?;
        if w.votes.mergers[pos].answered.iter().any(|a| !a) {
            return Ok(FinOutcome::VoteOpen { id });
        }
        let rec = w.votes.mergers[pos].clone();
        let (buyer, target) = (usize::from(rec.buyer), usize::from(rec.target));
        if !self.company_live(buyer) || !self.company_live(target) {
            return Ok(FinOutcome::Done);
        }
        let now = w.minutes;
        let idx = self.company_idx(target).ok_or(FinError::NoSuchObject)?;
        let shares = self
            .company_data(target)
            .map_or(0, |d| i32_at(d, CO_SHARES));
        let float = self.fc_free_float(target);
        let pq = self.fc_quote(now, target, 0, false);
        let pq = f64::from(pq as f32);
        let (pf, _) = {
            let r = pool::record(&self.save.companies, target).ok_or(FinError::NoSuchObject)?;
            let mut tco = Company::from_record(&r.data);
            let pf = {
                let ctx = self.fin_ctx(now, target);
                tco.fundamental_price(&ctx)
            };
            (pf, 0.0)
        };
        // `0x43ad60(buyer, 5)` quotes the buyer (refreshing its price cache) for the weighted return.
        let p5 = f64::from(self.fc_quote(now, buyer, 0, false));
        let r5 = {
            let b = pool::record(&self.save.companies, buyer).ok_or(FinError::NoSuchObject)?;
            let bco = Company::from_record(&b.data);
            let ctx = self.fin_ctx(now, buyer);
            super::year::weighted_return(&bco, &ctx, p5)
        };
        let eff = f64::from(self.effect_mult(0x22, buyer));
        let mut v = ((pf + pq) * f64::from(0.5f32)) as f32;
        if v < 1.0 {
            v = 1.0;
        }
        let a = (((f64::from(rec.offer) / eff) / f64::from(v) - 1.0) * 0.7 + 0.1) as f32;
        let b = ((r5 - 0.1) * 0.333_333_333_333_333_3 + f64::from(a)) as f32;
        let rnd = w.rng.range(-10, 10);
        let mut c = (f64::from(rnd) * 0.01 + f64::from(b)) as f32;
        let persons = self.person_ids();
        let mut votes = rec.vote.clone();
        let mut yes = 0i32;
        for (i, &p) in persons.iter().enumerate() {
            let (human, chair_of) = self.person_data(p).map_or((true, 0), |d| {
                (i32_at(d, PE_HUMAN) != 0, i32_at(d, PE_CHAIR_OF))
            });
            let h = self.holdings(p, idx);
            if !human && h > 0 {
                let mut q = (f64::from(c) - 0.1) as f32;
                if chair_of == target as i32 {
                    q = (f64::from(q) - 0.45) as f32;
                }
                let d = w.rng.range(-20, 20);
                q = (f64::from(d) * 0.01 + f64::from(q)) as f32;
                let roll = w.rng.range(0, 100);
                if let Some(vote) = votes.get_mut(i) {
                    *vote = f64::from(q) * 100.0 > f64::from(roll);
                }
            }
            if votes.get(i).copied().unwrap_or(false) && h > 0 {
                yes += h;
            }
        }
        if c < 0.0 {
            c = 0.0;
        }
        if f64::from(c) > 1.0 {
            c = 1.0;
        }
        let public = ftol(f64::from(float) * f64::from(c)) / 1000 * 1000;
        yes += public;
        let passed = yes > shares / 2;
        let tally = self.fc_tally(&persons, &votes, idx, shares, float, public, yes);
        let (bname, tname) = (self.company_name(buyer), self.company_name(target));
        self.ts_globals.news.push(GameMessage::MergerResult {
            buyer: buyer as i32,
            target: target as i32,
            buyer_name: bname,
            target_name: tname,
            offer: i32::from(rec.offer),
            passed,
            tally: tally.clone(),
        });
        if passed {
            self.fc_merge(w, buyer, target, i32::from(rec.offer));
        } else if let Some(r) = pool::record_mut(&mut self.save.companies, buyer) {
            put_u32(&mut r.data, CO_MERGER_FAIL, now as u32);
        }
        w.votes.mergers.retain(|m| m.id != id);
        Ok(FinOutcome::VoteResult { passed, tally })
    }

    /// `chairman_election_start` (`0x42ed10`) and the count (`0x42ee00`).
    fn fc_election_start(
        &mut self,
        w: &mut World,
        company: usize,
        challenger: usize,
    ) -> Result<FinOutcome, FinError> {
        if !self.company_live(company)
            || !self
                .person_data(challenger)
                .is_some_and(|d| i32_at(d, PE_ACTIVE) != 0)
        {
            return Err(FinError::NoSuchObject);
        }
        let idx = self.company_idx(company).ok_or(FinError::NoSuchObject)?;
        let persons = self.person_ids();
        let n = persons.len();
        let mut rec = ElectionRec {
            id: w.votes.next_election_id(),
            company: company as u16,
            challenger: challenger as u16,
            answered: vec![true; n],
            vote: vec![false; n],
        };
        for (i, &p) in persons.iter().enumerate() {
            let Some(d) = self.person_data(p) else {
                continue;
            };
            if i32_at(d, PE_HOLDINGS + 4 * idx) > 0 {
                if p == challenger {
                    rec.vote[i] = true;
                } else if i32_at(d, PE_HUMAN) == 0 {
                    rec.vote[i] = false;
                } else {
                    rec.answered[i] = false;
                }
            }
        }
        let id = rec.id;
        w.votes.elections.push(rec);
        self.fc_election_count(w, id)
    }

    /// `0x42ee00`: counts the votes of the contest `id` when everyone has answered.
    pub(super) fn fc_election_count(
        &mut self,
        w: &mut World,
        id: u32,
    ) -> Result<FinOutcome, FinError> {
        let pos = w
            .votes
            .elections
            .iter()
            .position(|m| m.id == id)
            .ok_or(FinError::NoSuchVote)?;
        if w.votes.elections[pos].answered.iter().any(|a| !a) {
            return Ok(FinOutcome::VoteOpen { id });
        }
        let rec = w.votes.elections[pos].clone();
        let (company, challenger) = (usize::from(rec.company), usize::from(rec.challenger));
        if !self.company_live(company) {
            return Ok(FinOutcome::Done);
        }
        let now = w.minutes;
        let idx = self.company_idx(company).ok_or(FinError::NoSuchObject)?;
        let shares = self
            .company_data(company)
            .map_or(0, |d| i32_at(d, CO_SHARES));
        let float = self.fc_free_float(company);
        let crep = self
            .company_data(company)
            .map_or(0, |d| i32_at(d, CO_CHAIR_REP));
        let prep = self
            .person_data(challenger)
            .map_or(0, |d| i32_at(d, PE_REP));
        let p0 =
            (0.2 - f64::from(crep) * 0.016_666_666_666_666_666 + f64::from(prep) * 0.0125) as f32;
        let rnd = w.rng.range(-10, 10);
        let ext = f64::from(rnd) * 0.01 + f64::from(p0);
        let p1 = ext as f32;
        let p = if ext > 1.0 {
            1.0
        } else if p1 < 0.0 {
            0.0
        } else {
            p1
        };
        let public = ftol(f64::from(float) * f64::from(p)) / 1000 * 1000;
        let mut yes = public;
        let persons = self.person_ids();
        let mut votes = rec.vote.clone();
        for (i, &pid) in persons.iter().enumerate() {
            let (human, chair_of) = self.person_data(pid).map_or((true, 0), |d| {
                (i32_at(d, PE_HUMAN) != 0, i32_at(d, PE_CHAIR_OF))
            });
            let h = self.holdings(pid, idx);
            if !human && h > 0 {
                let v = if chair_of != company as i32 {
                    let d = w.rng.range(-20, 20);
                    let q = (f64::from(d) * 0.01 + f64::from(p)) as f32;
                    let roll = w.rng.range(0, 100);
                    f64::from(q) * 100.0 > f64::from(roll)
                } else {
                    false
                };
                if let Some(x) = votes.get_mut(i) {
                    *x = v;
                }
            }
            if votes.get(i).copied().unwrap_or(false) && h > 0 {
                yes += h;
            }
        }
        let passed = yes > shares / 2;
        let tally = self.fc_tally(&persons, &votes, idx, shares, float, public, yes);
        let (cname, chname) = (self.company_name(company), self.person_name(challenger));
        self.ts_globals.news.push(GameMessage::ChairmanVote {
            company: company as i32,
            challenger: challenger as i32,
            company_name: cname,
            challenger_name: chname,
            passed,
            tally: tally.clone(),
        });
        if passed {
            self.fc_takeover(w, company, challenger);
        } else if let Some(r) = pool::record_mut(&mut self.save.persons, challenger) {
            put_u32(&mut r.data, PE_ELECT_STAMP, now as u32);
        }
        w.votes.elections.retain(|m| m.id != id);
        Ok(FinOutcome::VoteResult { passed, tally })
    }

    /// Records a vote (`0x48b4b0` / `0x42ecd0`) and counts.
    fn fc_vote(
        &mut self,
        w: &mut World,
        merger: bool,
        id: u32,
        person: usize,
        yes: bool,
    ) -> Result<FinOutcome, FinError> {
        // `0x4a75e0`: the position of the person in the player set.
        let pos = self
            .player_set()
            .iter()
            .position(|&p| p == person)
            .or_else(|| self.person_ids().iter().position(|&p| p == person))
            .ok_or(FinError::NoSuchObject)?;
        if merger {
            let m = w
                .votes
                .mergers
                .iter_mut()
                .find(|m| m.id == id)
                .ok_or(FinError::NoSuchVote)?;
            if pos < m.answered.len() {
                m.answered[pos] = true;
                m.vote[pos] = yes;
            }
            self.fc_merger_count(w, id)
        } else {
            let m = w
                .votes
                .elections
                .iter_mut()
                .find(|m| m.id == id)
                .ok_or(FinError::NoSuchVote)?;
            if pos < m.answered.len() {
                m.answered[pos] = true;
                m.vote[pos] = yes;
            }
            self.fc_election_count(w, id)
        }
    }

    /// The votes that wait for a human shareholder, in record order.
    pub(crate) fn fc_pending_votes(&self, w: &World) -> Vec<PendingVote> {
        let persons = self.person_ids();
        let mut out = Vec::new();
        for m in &w.votes.mergers {
            let Some(idx) = self.company_idx(usize::from(m.target)) else {
                continue;
            };
            for (i, &p) in persons.iter().enumerate() {
                if !m.answered.get(i).copied().unwrap_or(true) {
                    out.push(PendingVote {
                        merger: true,
                        id: m.id,
                        person: p as i32,
                        company: i32::from(m.buyer),
                        other: i32::from(m.target),
                        offer: i32::from(m.offer),
                        shares: self.holdings(p, idx),
                    });
                }
            }
        }
        for m in &w.votes.elections {
            let Some(idx) = self.company_idx(usize::from(m.company)) else {
                continue;
            };
            for (i, &p) in persons.iter().enumerate() {
                if !m.answered.get(i).copied().unwrap_or(true) {
                    out.push(PendingVote {
                        merger: false,
                        id: m.id,
                        person: p as i32,
                        company: i32::from(m.company),
                        other: i32::from(m.challenger),
                        offer: 0,
                        shares: self.holdings(p, idx),
                    });
                }
            }
        }
        out
    }
}

// ---- the executor and the checks of the windows ------------------------------------------------

impl SimState {
    /// `cmd_execute` for the financial ids: validates only that the ids are live (like the
    /// original) and runs the primitive. Margin calls, votes and news go to the output queues.
    pub(crate) fn fin_execute(
        &mut self,
        w: &mut World,
        cmd: FinCmd,
    ) -> Result<FinOutcome, FinError> {
        if !self.v2348() {
            return Err(FinError::NoSuchObject);
        }
        let now = w.minutes;
        let co = |c: i32| -> Result<usize, FinError> {
            if c < 0 {
                Err(FinError::NoSuchObject)
            } else {
                Ok(c as usize)
            }
        };
        match cmd {
            FinCmd::StockTrade {
                person,
                company,
                buy,
                flag,
            } => {
                let (p, c) = (co(person)?, co(company)?);
                if !self.company_live(c) || self.person_data(p).is_none() {
                    return Err(FinError::NoSuchObject);
                }
                let idx = self.company_idx(c).ok_or(FinError::NoSuchObject)?;
                // The original's `0x43b410` calls leave the price cache refreshed.
                self.fc_quote(now, c, 0, false);
                let mut mg = MarginGuard::default();
                let n = self.fc_lot(now, p, idx, buy);
                if buy {
                    let (price, cash) = self
                        .fc_buy(now, w, &mut mg, p, idx, n, flag, false)
                        .ok_or(FinError::CannotBuy)?;
                    Ok(FinOutcome::Trade {
                        shares: n,
                        price,
                        cash,
                    })
                } else {
                    let (price, cash) = self
                        .fc_sell(now, w, &mut mg, p, idx, n, flag)
                        .ok_or(FinError::CannotSell)?;
                    Ok(FinOutcome::Trade {
                        shares: -n,
                        price,
                        cash,
                    })
                }
            }
            FinCmd::ResignChairman { person, company } => {
                let c = co(company)?;
                let chair = self.company_data(c).ok_or(FinError::NoSuchObject)?;
                if i32_at(chair, CO_CHAIRMAN) == person {
                    self.fc_set_chairman(now, c, 0);
                }
                Ok(FinOutcome::Done)
            }
            FinCmd::SetManager { company, manager } => {
                self.fc_set_manager(w, co(company)?, manager)?;
                Ok(FinOutcome::Done)
            }
            FinCmd::StockIssue { company, n } => {
                let c = co(company)?;
                self.company_data(c).ok_or(FinError::NoSuchObject)?;
                if n > 0 {
                    let price = self
                        .fc_issue_stock(now, c, n)
                        .ok_or(FinError::NoSuchObject)?;
                    Ok(FinOutcome::Stock { shares: n, price })
                } else {
                    let price = self
                        .fc_buy_back(now, c, n.wrapping_neg())
                        .ok_or(FinError::NoSuchObject)?;
                    Ok(FinOutcome::Stock { shares: n, price })
                }
            }
            FinCmd::SetDividend {
                company,
                dividend_bits,
            } => {
                let r = pool::record_mut(&mut self.save.companies, co(company)?)
                    .ok_or(FinError::NoSuchObject)?;
                put_u32(&mut r.data, 0x30, dividend_bits);
                Ok(FinOutcome::Done)
            }
            FinCmd::Bankruptcy { company } => {
                let c = co(company)?;
                if !self.company_live(c) {
                    return Ok(FinOutcome::Done);
                }
                self.fc_with_company(now, c, |co, ctx| co.declare_bankruptcy(ctx))
                    .ok_or(FinError::NoSuchObject)?;
                Ok(FinOutcome::Done)
            }
            FinCmd::BondIssue {
                company,
                principal,
                term,
            } => {
                let c = co(company)?;
                let editor = self.editor_mode();
                let r = self
                    .fc_with_company(now, c, |co, ctx| {
                        co.issue_bond(ctx, principal, term, true, editor)
                            .map(|()| co.bonds.last().map_or(0.0, |b| b.rate))
                    })
                    .ok_or(FinError::NoSuchObject)?;
                match r {
                    Ok(rate) => Ok(FinOutcome::Bond { rate }),
                    Err(crate::finance::BondError::TooMany) => Err(FinError::TooManyBonds),
                    Err(crate::finance::BondError::RatingTooLow) => Err(FinError::RatingTooLow {
                        have: self.fc_rating(now, c, 0),
                        need: MIN_BOND_RATING as i32,
                    }),
                }
            }
            FinCmd::BondRepay { company, bond } => {
                let c = co(company)?;
                self.fc_with_company(now, c, |co, _| {
                    if (0..co.bonds.len() as i32).contains(&bond) {
                        co.repay_bond(bond as usize, true);
                    }
                })
                .ok_or(FinError::NoSuchObject)?;
                Ok(FinOutcome::Done)
            }
            FinCmd::MergerPropose {
                buyer,
                target,
                offer,
            } => {
                let (b, t) = (co(buyer)?, co(target)?);
                if !self.company_live(b) || !self.company_live(t) {
                    return Ok(FinOutcome::Done);
                }
                self.fc_merger_propose(w, b, t, offer)
            }
            FinCmd::MergerVote {
                proposal,
                person,
                yes,
            } => self.fc_vote(w, true, proposal, co(person)?, yes),
            FinCmd::ElectionStart {
                company,
                challenger,
            } => {
                let (c, p) = (co(company)?, co(challenger)?);
                self.fc_election_start(w, c, p)
            }
            FinCmd::ElectionVote {
                election,
                person,
                yes,
            } => self.fc_vote(w, false, election, co(person)?, yes),
            FinCmd::ChairmanTakeover { person, company } => {
                let (p, c) = (co(person)?, co(company)?);
                if self.person_data(p).is_none() || self.company_data(c).is_none() {
                    return Err(FinError::NoSuchObject);
                }
                self.fc_takeover(w, c, p);
                Ok(FinOutcome::Done)
            }
        }
    }

    /// The credit rating level 0..=10 of the company with `x` more debt (`0x43b650`).
    pub(crate) fn fc_rating(&self, now: i32, slot: usize, x: i32) -> i32 {
        let Some(d) = self.company_data(slot) else {
            return 0;
        };
        let co = Company::from_record(d);
        let ctx = self.fin_ctx(now, slot);
        ftol(co.credit_rating(&ctx, x))
    }

    /// Profit of year `y` (`0x43a7d0(y, 13)`) of the company.
    fn fc_profit(&self, slot: usize, now: i32, y: i32) -> f64 {
        let Some(d) = self.company_data(slot) else {
            return 0.0;
        };
        let co = Company::from_record(d);
        crate::ai::finance::profit_of_year(&co, i32::from(date_of(now).year), y)
    }

    /// The lot the issue window offers (`0x447730`): `max(1000, shares / 20000 * 1000)`.
    pub(crate) fn fc_issue_lot(&self, slot: usize) -> i32 {
        let shares = self.company_data(slot).map_or(0, |d| i32_at(d, CO_SHARES));
        (shares / 20000 * 1000).max(1000)
    }

    /// The lot the buy-back window offers (`0x447bb0`): 5% of the shares, limited by the cash
    /// (`cash / (price * 1.2)`) and by the free float, at least 1000; `0` when there is no float.
    pub(crate) fn fc_buyback_lot(&self, now: i32, slot: usize) -> i32 {
        let Some(d) = self.company_data(slot) else {
            return 0;
        };
        let mut lot = i32_at(d, CO_SHARES) / 20000 * 1000;
        let price = self.fc_price(now, slot, ftol(f64::from(lot) * 0.7));
        let cash = f64_at(d, CO_ACCT);
        let max = ftol(cash / (f64::from(price) * 1.2));
        if max < lot {
            lot = max / 1000 * 1000;
        }
        if lot < 1000 {
            lot = 1000;
        }
        lot.min(self.fc_free_float(slot))
    }

    /// The checks of the windows that build each packet (docs/spec/commands.md 2.1, 3.9), in
    /// the order of the original; `Ok` when the window would send the command.
    pub(crate) fn fin_check(&self, now: i32, cmd: &FinCmd) -> Result<(), FinError> {
        let live = |c: i32| -> Result<usize, FinError> {
            if c >= 0 && self.company_live(c as usize) {
                Ok(c as usize)
            } else {
                Err(FinError::NoSuchObject)
            }
        };
        let sandbox = self.editor_mode();
        let date = date_of(now);
        let year = i32::from(date.year);
        match *cmd {
            FinCmd::StockTrade {
                person,
                company,
                buy,
                flag,
            } => {
                let c = live(company)?;
                let p = person as usize;
                let idx = self.company_idx(c).ok_or(FinError::NoSuchObject)?;
                self.person_data(p).ok_or(FinError::NoSuchObject)?;
                if buy {
                    if !self.fc_can_buy(now, p, idx, flag, 0) {
                        return Err(FinError::CannotBuy);
                    }
                } else if !self.fc_can_sell(now, p, idx, flag, false, 0) {
                    return Err(FinError::CannotSell);
                }
                Ok(())
            }
            FinCmd::ResignChairman { person, company } => {
                let c = live(company)?;
                if !self.st8(PERM_RESIGN) {
                    return Err(FinError::NotAllowed);
                }
                if self.company_data(c).map_or(0, |d| i32_at(d, CO_CHAIRMAN)) != person {
                    return Err(FinError::NoSuchObject);
                }
                Ok(())
            }
            FinCmd::SetManager { company, manager } => {
                let c = live(company)?;
                if !(0..40).contains(&manager) {
                    return Err(FinError::NoSuchObject);
                }
                if self.save.state[ST_MANAGER_USE + manager as usize] != 0 {
                    return Err(FinError::ManagerTaken);
                }
                let eff = f64::from(self.effect_mult(0x10, c));
                let sal = self
                    .data
                    .image
                    .i32(super::company::MANAGER_TABLE + 0x11 * manager as u32);
                let fee = f64::from(ftol(eff * f64::from(sal)) / 2000 * 1000);
                let cash = self.company_data(c).map_or(0.0, |d| f64_at(d, CO_ACCT));
                if fee > cash {
                    return Err(FinError::NotEnoughCashManager);
                }
                Ok(())
            }
            FinCmd::StockIssue { company, n } => {
                let c = live(company)?;
                let d = self.company_data(c).ok_or(FinError::NoSuchObject)?;
                if n > 0 {
                    if !self.st8(PERM_ISSUE) {
                        return Err(FinError::NotAllowed);
                    }
                    let lot = self.fc_issue_lot(c);
                    let price = self.fc_price(now, c, ftol(-f64::from(lot) * 0.7));
                    if price < 5 && !sandbox {
                        return Err(FinError::UnderwritersRefuse { price });
                    }
                    let last = Date::new(
                        pool::u16_at(d, CO_ISSUE_DATE),
                        d[CO_ISSUE_DATE + 2],
                        d[CO_ISSUE_DATE + 3],
                        d[CO_ISSUE_TIME],
                        d[CO_ISSUE_TIME + 1],
                    )
                    .to_minutes();
                    if now - last <= YEAR_WAIT && !sandbox {
                        return Err(FinError::IssuedRecently);
                    }
                    Ok(())
                } else {
                    if !self.st8(PERM_BUYBACK) {
                        return Err(FinError::NotAllowed);
                    }
                    let lot = self.fc_buyback_lot(now, c);
                    if lot == 0 {
                        return Err(FinError::NoFloat);
                    }
                    let shares = i32_at(d, CO_SHARES);
                    if lot >= shares {
                        return Err(FinError::LastShares { n: shares });
                    }
                    let price = self.fc_price(now, c, ftol(f64::from(lot) * 0.7));
                    let need = (f64::from(BROKERAGE) + 1.0) * f64::from(price) * f64::from(lot);
                    if need > f64_at(d, CO_ACCT) && !sandbox {
                        return Err(FinError::NotEnoughCashBuyback);
                    }
                    Ok(())
                }
            }
            FinCmd::SetDividend {
                company,
                dividend_bits,
            } => {
                let c = live(company)?;
                if !self.st8(PERM_DIVIDEND) {
                    return Err(FinError::NotAllowed);
                }
                let d = self.company_data(c).ok_or(FinError::NoSuchObject)?;
                let cur = f32_at(d, 0x30);
                let cap = self.fc_dividend_cap(now, c) as f32;
                let cap = if cap <= cur { cur } else { cap };
                if f32::from_bits(dividend_bits) > cap {
                    return Err(FinError::DividendRefused);
                }
                Ok(())
            }
            FinCmd::Bankruptcy { company } => {
                let c = live(company)?;
                if !self.st8(PERM_BANKRUPT) {
                    return Err(FinError::NotAllowed);
                }
                let d = self.company_data(c).ok_or(FinError::NoSuchObject)?;
                let last = i32_at(d, 0x158);
                if year - last < 5 {
                    return Err(FinError::BankruptRecently {
                        allowed_from: last + 5,
                    });
                }
                if i32_at(d, 0x148) <= year - 2
                    && self.fc_profit(c, now, year - 2) <= 0.0
                    && self.fc_profit(c, now, year - 1) <= 0.0
                {
                    Ok(())
                } else {
                    Err(FinError::BankruptJudge)
                }
            }
            FinCmd::BondIssue { company, .. } => {
                let c = live(company)?;
                if !self.st8(PERM_BOND) {
                    return Err(FinError::NotAllowed);
                }
                let d = self.company_data(c).ok_or(FinError::NoSuchObject)?;
                if i32_at(d, CO_BOND_COUNT) >= MAX_BONDS as i32 {
                    return Err(FinError::TooManyBonds);
                }
                let have = self.fc_rating(now, c, 0);
                if (have as f64) < MIN_BOND_RATING && !sandbox {
                    return Err(FinError::RatingTooLow {
                        have,
                        need: MIN_BOND_RATING as i32,
                    });
                }
                Ok(())
            }
            FinCmd::BondRepay { company, bond } => {
                let c = live(company)?;
                if !self.st8(PERM_REPAY) {
                    return Err(FinError::NotAllowed);
                }
                let d = self.company_data(c).ok_or(FinError::NoSuchObject)?;
                let n = i32_at(d, CO_BOND_COUNT);
                if !(0..n).contains(&bond) {
                    return Err(FinError::NoSuchObject);
                }
                let principal = i32_at(d, CO_BONDS + 12 * bond as usize);
                if f64::from(principal) * f64::from_bits(0x3ff0_51eb_851e_b852) > f64_at(d, CO_ACCT)
                {
                    return Err(FinError::NotEnoughCashRepay);
                }
                Ok(())
            }
            FinCmd::MergerPropose {
                buyer,
                target,
                offer,
            } => {
                let b = live(buyer)?;
                let t = live(target)?;
                if !self.st8(PERM_MERGER) {
                    return Err(FinError::NotAllowed);
                }
                if self.company_set().len() < 2 {
                    return Err(FinError::NoOtherCompany);
                }
                let d = self.company_data(b).ok_or(FinError::NoSuchObject)?;
                let fail = i32_at(d, CO_MERGER_FAIL);
                if fail != 0 && (now.wrapping_sub(fail) as u32) <= YEAR_WAIT as u32 {
                    return Err(FinError::MergerCooldown);
                }
                let shares = self.company_data(t).map_or(0, |d| i32_at(d, CO_SHARES));
                if f64::from(shares) * f64::from(offer) > f64_at(d, CO_ACCT) && !sandbox {
                    return Err(FinError::NotEnoughCashMerger);
                }
                Ok(())
            }
            FinCmd::MergerVote { proposal, .. } => {
                // The window of the vote is only shown for a record that exists.
                let _ = proposal;
                Ok(())
            }
            FinCmd::ElectionStart {
                company,
                challenger,
            } => {
                let c = live(company)?;
                if !self.st8(PERM_TAKEOVER) {
                    return Err(FinError::NotAllowed);
                }
                let stamp = self
                    .person_data(challenger as usize)
                    .map_or(0, |d| i32_at(d, PE_ELECT_STAMP));
                if stamp != 0 && (now.wrapping_sub(stamp) as u32) < 0x76200 {
                    return Err(FinError::ElectionCooldown);
                }
                let _ = c;
                Ok(())
            }
            FinCmd::ElectionVote { .. } => Ok(()),
            FinCmd::ChairmanTakeover { person, company } => {
                let c = live(company)?;
                if !self.st8(PERM_TAKEOVER) {
                    return Err(FinError::NotAllowed);
                }
                let _ = (c, person);
                Ok(())
            }
        }
    }

    /// The board's dividend cap (`0x43edf0`).
    pub(crate) fn fc_dividend_cap(&self, now: i32, slot: usize) -> f64 {
        let Some(d) = self.company_data(slot) else {
            return 0.0;
        };
        let co = Company::from_record(d);
        let ctx = self.fin_ctx(now, slot);
        co.max_dividend(&ctx)
    }
}

// ---- the numbers the windows show --------------------------------------------------------------

impl SimState {
    pub(crate) fn fc_issue_quote(
        &self,
        now: i32,
        company: i32,
        w: &World,
    ) -> Option<crate::fincmd::IssueQuote> {
        let c = usize::try_from(company).ok()?;
        self.company_data(c)?;
        let lot = self.fc_issue_lot(c);
        let price = self.fc_price(now, c, ftol(-f64::from(lot) * 0.7));
        let gross = lot.wrapping_mul(price);
        let fee = ftol(f64::from(gross) * 0.05);
        let _ = w;
        let refused = self
            .fin_check(now, &FinCmd::StockIssue { company, n: lot })
            .err();
        Some(crate::fincmd::IssueQuote {
            lot,
            price,
            fee_rate: 0.05,
            gross,
            fee,
            refused,
        })
    }

    pub(crate) fn fc_buyback_quote(
        &self,
        now: i32,
        company: i32,
    ) -> Option<crate::fincmd::BuybackQuote> {
        let c = usize::try_from(company).ok()?;
        self.company_data(c)?;
        let lot = self.fc_buyback_lot(now, c);
        let price = self.fc_price(now, c, ftol(f64::from(lot) * 0.7));
        let gross = lot.wrapping_mul(price);
        let fee = ftol(f64::from(BROKERAGE) * f64::from(gross));
        let refused = self
            .fin_check(now, &FinCmd::StockIssue { company, n: -lot })
            .err();
        Some(crate::fincmd::BuybackQuote {
            lot,
            price,
            fee_rate: BROKERAGE,
            gross,
            fee,
            refused,
        })
    }

    pub(crate) fn fc_bond_quote(&self, now: i32, company: i32) -> Option<crate::fincmd::BondQuote> {
        let c = usize::try_from(company).ok()?;
        let d = self.company_data(c)?;
        let co = Company::from_record(d);
        let ctx = self.fin_ctx(now, c);
        let rate = co.bond_rate(&ctx, 500_000) as f32;
        let cmd = FinCmd::BondIssue {
            company,
            principal: 500_000,
            term: 30,
        };
        Some(crate::fincmd::BondQuote {
            principal: 500_000,
            term: 30,
            rate,
            rating: ftol(co.credit_rating(&ctx, 0)),
            min_rating: MIN_BOND_RATING as i32,
            bonds: co.bonds.len(),
            refused: self.fin_check(now, &cmd).err(),
        })
    }

    pub(crate) fn fc_bond_lines(&self, now: i32, company: i32) -> Vec<crate::fincmd::BondLine> {
        let Some(c) = usize::try_from(company).ok() else {
            return Vec::new();
        };
        let Some(d) = self.company_data(c) else {
            return Vec::new();
        };
        let co = Company::from_record(d);
        co.bonds
            .iter()
            .enumerate()
            .map(|(i, b)| crate::fincmd::BondLine {
                principal: b.principal,
                maturity_year: b.maturity_year,
                rate: b.rate,
                fee: ftol(f64::from(b.principal) * 0.02),
                refused: self
                    .fin_check(
                        now,
                        &FinCmd::BondRepay {
                            company,
                            bond: i as i32,
                        },
                    )
                    .err(),
            })
            .collect()
    }

    pub(crate) fn fc_dividend_info(
        &self,
        now: i32,
        company: i32,
    ) -> Option<crate::fincmd::DividendInfo> {
        let c = usize::try_from(company).ok()?;
        let d = self.company_data(c)?;
        let current = f32_at(d, 0x30);
        let cap = self.fc_dividend_cap(now, c) as f32;
        Some(crate::fincmd::DividendInfo {
            current,
            cap: if cap <= current { current } else { cap },
            shares: i32_at(d, CO_SHARES),
            permitted: self.st8(PERM_DIVIDEND),
        })
    }

    pub(crate) fn fc_trade_quote(
        &self,
        now: i32,
        person: i32,
        company: i32,
    ) -> Option<crate::fincmd::TradeQuote> {
        let (p, c) = (
            usize::try_from(person).ok()?,
            usize::try_from(company).ok()?,
        );
        let pd = self.person_data(p)?;
        let cd = self.company_data(c)?;
        let idx = self.company_idx(c)?;
        let margin_ok = self.fc_can_buy(now, p, idx, true, 0);
        let can_buy = self.fc_can_buy(now, p, idx, false, 0);
        Some(crate::fincmd::TradeQuote {
            last_price: f32_at(cd, 0x427e),
            price: self.fc_price(now, c, 0),
            held: i32_at(pd, PE_HOLDINGS + 4 * idx),
            shares: i32_at(cd, CO_SHARES),
            free_float: self.fc_free_float(c),
            cash: f64_at(pd, PE_CASH),
            power: self.fc_weighted(pd) + f64_at(pd, PE_CASH),
            buy_lot: self.fc_lot(now, p, idx, true),
            sell_lot: self.fc_lot(now, p, idx, false),
            can_buy,
            can_buy_margin: margin_ok,
            can_sell: self.fc_can_sell(now, p, idx, false, false, 0),
            can_short: self.fc_can_sell(now, p, idx, true, false, 0),
        })
    }

    pub(crate) fn fc_manager_info(
        &self,
        company: i32,
        index: i32,
    ) -> Option<crate::fincmd::ManagerInfo> {
        if !(0..40).contains(&index) {
            return None;
        }
        let c = usize::try_from(company).ok()?;
        self.company_data(c)?;
        let salary = self
            .data
            .image
            .i32(super::company::MANAGER_TABLE + 0x11 * index as u32);
        let eff = f64::from(self.effect_mult(0x10, c));
        let base = 0x0054_fdb1 + 0x11 * index as u32;
        let effects = (0..4u32)
            .filter_map(|k| {
                let e = self.data.image.u8(base + 2 * k);
                (e != 0).then(|| (e, self.data.image.i8(base + 2 * k + 1)))
            })
            .collect();
        Some(crate::fincmd::ManagerInfo {
            index,
            salary,
            fee: ftol(eff * f64::from(salary)) / 2000 * 1000,
            employed: self.save.state[ST_MANAGER_USE + index as usize] != 0,
            effects,
        })
    }

    pub(crate) fn fc_chairman_buttons(&self, person: i32, company: i32) -> (bool, bool) {
        let (Some(p), Some(c)) = (usize::try_from(person).ok(), usize::try_from(company).ok())
        else {
            return (false, false);
        };
        let Some(idx) = self.company_idx(c) else {
            return (false, false);
        };
        let (Some(pd), Some(cd)) = (self.person_data(p), self.company_data(c)) else {
            return (false, false);
        };
        if i32_at(cd, CO_CHAIRMAN) == person || !self.st8(0x0057_3d75) || !self.st8(PERM_TAKEOVER) {
            return (false, false);
        }
        let ratio = f64::from(i32_at(pd, PE_HOLDINGS + 4 * idx)) / f64::from(i32_at(cd, CO_SHARES));
        let sandbox = self.editor_mode();
        let assume = sandbox || ratio > 0.5;
        let contest = ratio >= 0.1;
        (assume, contest)
    }
}
