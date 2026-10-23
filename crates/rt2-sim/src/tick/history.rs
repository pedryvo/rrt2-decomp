//! `history_snapshot` (`0x4e9770`, docs/spec/world-tick.md 3.4 item 7 and 10): the yearly
//! record of the player and of the rankings, rebuilt from scratch in the state block at
//! `0x570a82` (state offset `0x59a`, 0x3283 bytes) on 28 December, after the annual reviews and
//! before `company_year_end` and `person_year_step` shift the archives.

use super::company::{PE_CASH, PE_HOLDINGS, PE_HUMAN};
use super::*;
use crate::company_record::{CO_ARCH_METRICS, CO_CATS, CO_FOUNDED, CO_SHARES};
use crate::fp::ftol as ftol_;

/// State offset of the record (`0x570a82 - 0x5704e8`).
pub(super) const HIST_BASE: usize = 0x59A;
/// Size of the record: `0xca0` dwords + 3 bytes.
pub(super) const HIST_SIZE: usize = 0x3283;
const PE_ARCHIVE: usize = 0x133;
/// Sentinel stored for the years before a company existed (`0x40c81b8000000000`).
const NOT_FOUNDED: f64 = 12343.0;
const CO_BASE_DIV: usize = 0x474;
const ST_PLAYER_SET_COUNT: usize = 0x28D;
const ST_PLAYER_SET_IDS: usize = 0x291;

/// Company-side helpers on a raw record (`0x43a660`, `0x43a7d0`).
pub(crate) mod co {
    use super::*;

    /// `0x43a660(co, i)`: totals of the current year (0..12 the category, 13 the sum of 13, 14 the
    /// sum of the first 5).
    pub fn cat_live(r: &[u8], i: usize) -> f64 {
        sum_row(r, CO_CATS, i)
    }

    /// `0x43a7d0(co, year, i)` for a year that is not the current one (and not the 9001 marker):
    /// the archive row of `now - year - 1` years.
    pub fn cat_archive(r: &[u8], now: i32, year: i32, i: usize) -> f64 {
        let k = (now - year - 1) as usize;
        sum_row(r, 0x42ee + 0x68 * k, i)
    }

    fn sum_row(r: &[u8], base: usize, i: usize) -> f64 {
        if i < 13 {
            return f64_at(r, base + 8 * i);
        }
        let n = if i == 13 { 13 } else { 5 };
        (0..n).fold(0.0, |a, j| a + f64_at(r, base + 8 * j))
    }

    /// `0x43c730`: the rating step 0..=12 from `+0x46a`.
    pub fn rating(r: &[u8]) -> u32 {
        let v = i32_at(r, 0x46a).wrapping_add(0x3a).wrapping_div(9);
        v.clamp(0, 12) as u32
    }

    /// `0x43ad60(co, 9)` and `(co, 11)`.
    pub fn metric9(r: &[u8]) -> i32 {
        i32_at(r, 0x196)
    }
    pub fn metric11(r: &[u8]) -> i32 {
        i32_at(r, 0x19a).wrapping_div(i32_at(r, 0x196).max(1))
    }
}

impl SimState {
    /// The person the interface plays (`0x566ca4`) and its company (`0x574414`): the clone takes
    /// the first human person and its chairman company (as `is_ui_company`).
    pub(super) fn local_player(&self) -> (usize, usize) {
        for p in pool::list_order(&self.save.persons) {
            if let Some(r) = pool::record(&self.save.persons, p)
                && i32_at(&r.data, PE_HUMAN) != 0
            {
                return (p, i32_at(&r.data, 0x263).max(0) as usize);
            }
        }
        (0, 0)
    }

    /// `0x4a76b0(person, 1)`: value of the shares at the current price.
    fn person_stock_value(&mut self, w: &World, slot: usize) -> f64 {
        let Some(rec) = pool::record(&self.save.persons, slot) else {
            return 0.0;
        };
        let holdings: Vec<i32> = (0..36)
            .map(|i| i32_at(&rec.data, PE_HOLDINGS + 4 * i))
            .collect();
        let set = self.company_set();
        let mut v = 0.0f64;
        for (i, &c) in set.iter().enumerate().take(36) {
            if holdings[i] != 0 {
                let p = self.company_quote(w.minutes, c, false);
                v += f64::from(p) * f64::from(holdings[i]);
            }
        }
        v
    }

    /// `0x4a7760(person, year, 2)`: net worth (cash + shares) of `year`, live for the current one,
    /// from the archive otherwise.
    fn person_worth_of_year(&mut self, w: &World, slot: usize, year: i32, now: i32) -> f64 {
        if year == now {
            let stocks = self.person_stock_value(w, slot);
            let cash =
                pool::record(&self.save.persons, slot).map_or(0.0, |r| f64_at(&r.data, PE_CASH));
            return cash + stocks;
        }
        let k = (now - year) as usize;
        pool::record(&self.save.persons, slot).map_or(0.0, |r| {
            let o = PE_ARCHIVE - 0x10 + 0x10 * k;
            f64_at(&r.data, o) + f64_at(&r.data, o + 8)
        })
    }

    /// `history_snapshot(&0x570a82)` (`0x4e9770`).
    pub(super) fn history_snapshot(&mut self, w: &mut World) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "history_snapshot",
                "history_snapshot 0x4e9770 on a 0x233C file",
                "world-tick.md 3.4 item 7"
            );
            return;
        }
        let d = date_of(w.minutes);
        let now = i32::from(d.year);
        let start = i32::from(u16_at(&self.save.state, 0x20));
        let (local_person, local_co) = self.local_player();
        let mut rec = vec![0u8; HIST_SIZE];
        let put32 =
            |r: &mut Vec<u8>, o: usize, v: u32| r[o..o + 4].copy_from_slice(&v.to_le_bytes());
        let put64 =
            |r: &mut Vec<u8>, o: usize, v: f64| r[o..o + 8].copy_from_slice(&v.to_le_bytes());
        put32(&mut rec, 0, 1);
        put32(&mut rec, 4, now as u32);
        put32(&mut rec, 8, (now - start + 1).min(9) as u32);
        put32(&mut rec, 0xc, local_co as u32);
        rec[0x53] = 0xff;
        rec[0x54] = 0xff;
        rec[0x55] = 0;
        if local_co != 0
            && let Some(cr) = pool::record(&self.save.companies, local_co)
        {
            let c = cr.data.clone();
            let me = pool::record(&self.save.persons, local_person).map(|p| p.data.clone());
            // 0x4a80c0: the local person holds every share of the company.
            let idx = self
                .company_set()
                .iter()
                .position(|&x| x == local_co)
                .map_or(-1i32, |i| i as i32);
            if let Some(p) = me {
                let o = (PE_HOLDINGS as i32 + 4 * idx) as usize;
                rec[0x55] = u8::from(i32_at(&p, o) == i32_at(&c, CO_SHARES));
            }
            put64(&mut rec, 0x10, co::cat_live(&c, 14));
            put64(&mut rec, 0x18, co::cat_live(&c, 13));
            put32(&mut rec, 0x20, co::rating(&c));
            put64(&mut rec, 0x24, f64::from(i32_at(&c, 0x134)));
            let prev = if i32_at(&c, CO_FOUNDED) == now {
                f64::from(i32_at(&c, 0x134).wrapping_sub(10_000))
            } else {
                // 0x43a160(co, year - 1, 13): the metric 13 archived one year ago.
                f64::from(i32_at(&c, 0x134)) - f64_at(&c, CO_ARCH_METRICS + 0x70 + 8 * 13)
            };
            put64(&mut rec, 0x4b, prev);
            put64(&mut rec, 0x2c, f64::from(i32_at(&c, CO_BASE_DIV)));
            let m9 = f64::from(co::metric9(&c));
            put64(&mut rec, 0x39, m9);
            let sum = co::cat_live(&c, 1) + co::cat_live(&c, 0);
            let den = if m9 < 1.0 { 1.0 } else { m9 };
            put64(&mut rec, 0x41, f64::from(ftol_(sum)) / den);
            let m11 = co::metric11(&c) as u16;
            rec[0x49..0x4b].copy_from_slice(&m11.to_le_bytes());
            if now & 1 != 0 {
                for i in 0..2 {
                    rec[0x53 + i] = c[0x140 + 4 * i];
                }
            }
        }
        rec[0x38] = self.save.state[ST_ECON_STATE];
        put32(&mut rec, 0x34, 4);

        // Company rankings: by revenue (0xe) and by profit (0xd), best first.
        let set = self.company_set();
        let n = set.len();
        put32(&mut rec, 0x105b, n as u32);
        for k in 0..2usize {
            let metric = if k == 0 { 14 } else { 13 };
            let mut used = vec![false; n];
            for rank in 0..n {
                let (mut best_id, mut best_key, mut best_i) = (0usize, 0i32, 0usize);
                for (i, &id) in set.iter().enumerate() {
                    if used[i] {
                        continue;
                    }
                    let key = pool::record(&self.save.companies, id)
                        .map_or(0, |r| ftol_(co::cat_live(&r.data, metric)));
                    if best_id == 0 || best_key < key {
                        best_id = id;
                        best_key = key;
                        best_i = i;
                    }
                }
                put32(&mut rec, 0x105f + 4 * (rank + k * 0x24), best_id as u32);
                let cr = pool::record(&self.save.companies, best_id).map(|r| r.data.clone());
                for j in 0..10i32 {
                    let y = now - j;
                    let o = 0x117f + 8 * ((rank + k * 0x24) * 10 + j as usize);
                    let Some(c) = cr.as_ref() else { continue };
                    // Before the foundation the original stores the sentinel 0x40c81b8000000000.
                    let v = if y < i32_at(c, CO_FOUNDED) {
                        NOT_FOUNDED
                    } else if y == now {
                        co::cat_live(c, metric)
                    } else {
                        co::cat_archive(c, now, y, metric)
                    };
                    put64(&mut rec, o, v);
                }
                if best_i < n {
                    used[best_i] = true;
                }
            }
        }

        // Player rankings by net worth.
        let pset: Vec<usize> = {
            let cnt = i32_at(&self.save.state, ST_PLAYER_SET_COUNT).max(0) as usize;
            (0..cnt.min(36))
                .map(|i| usize::from(u16_at(&self.save.state, ST_PLAYER_SET_IDS + 2 * i)))
                .collect()
        };
        let pn = pset.len();
        put32(&mut rec, 0x27ff, pn as u32);
        let mut used = vec![false; pn];
        for rank in 0..pn {
            let (mut best_id, mut best_key, mut best_i) = (0usize, 0i32, 0usize);
            for (i, &id) in pset.iter().enumerate() {
                if used[i] {
                    continue;
                }
                let stocks = self.person_stock_value(w, id);
                let cash =
                    pool::record(&self.save.persons, id).map_or(0.0, |r| f64_at(&r.data, PE_CASH));
                let key = ftol_(stocks + cash);
                if best_id == 0 || best_key < key {
                    best_id = id;
                    best_key = key;
                    best_i = i;
                }
            }
            if 0x2803 + 4 * rank + 4 <= HIST_SIZE {
                put32(&mut rec, 0x2803 + 4 * rank, best_id as u32);
            }
            for j in 0..10i32 {
                let v = self.person_worth_of_year(w, best_id, now - j, now);
                let o = 0x2883 + 8 * (rank * 10 + j as usize);
                if o + 8 <= HIST_SIZE {
                    put64(&mut rec, o, v);
                }
            }
            if best_i < pn {
                used[best_i] = true;
            }
        }
        // `rule_collect_status(rec + 0x56)` (`0x458210`): the empty string `0x5667f0`, then the
        // trigger 7 rules evaluated in pool order (`0x455f30` with the local flag: CRT draws, no
        // effects) append their texts; the scan stops when the game-over state changes. Same
        // walk as `event_fire_trigger(7)`. Seen in Lenin (English).MP2 at the first year end.
        self.status_buf = Some(Vec::new());
        self.event_trigger(w, 7);
        let text = self.status_buf.take().unwrap_or_default();
        let room = 0x105b - 0x56 - 1;
        let n = text.len().min(room);
        rec[0x56..0x56 + n].copy_from_slice(&text[..n]);
        rec[0x56 + n] = 0;
        self.save.state[HIST_BASE..HIST_BASE + HIST_SIZE].copy_from_slice(&rec);
        self.log.ran("history_snapshot");
    }
}
