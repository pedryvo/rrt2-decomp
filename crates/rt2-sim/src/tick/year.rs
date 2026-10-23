//! Year-end hooks on the raw records (world-tick.md 3.4): `company_year_end` (`0x43d800`,
//! finance.md 8 and 16), `person_year_step` (`0x4a7880`), the engine reference speed of
//! `sim_year_end` items 1-2 (`ai_pick_engine` `0x4d86f0`, ai.md 4), and the building steps
//! `industry_year_step` (`0x40b450`) and `industry_year_reset` (`0x40b7c0`, world-growth.md 5).

use super::company::{PE_CASH, PE_HOLDINGS};
use super::*;
use crate::company_record::{CO_ACTIVE, CO_CHAIRMAN};
use crate::finance::Company;
use crate::fp::ftol as ftol_;
use crate::growth::{RatingInput, computer_owner_rating, industry_rating};

const ST_ENGINE_ALLOWED: usize = 0x17F; // 0x570667 u8[66]
const ST_REF_SPEED: usize = 0x3920; // 0x573e08
const ST_THEFT_PCT: usize = 0x391A; // 0x573e02 i16
const ST_THEFT_BASE: usize = 0x38B6; // 0x573d9e i32
const ST_CARGO_ON: usize = 0xED; // 0x5705d5
const ST_GROWTH: usize = 0xE9; // 0x5705d1 f32
const ST_YEAR_START: usize = 0x20; // 0x570508
const HDR_LEVEL: usize = 0x86EB; // 0x570223 i32
const HDR_RATE: usize = 0x86F3; // 0x57022b f32
const ST_YEAR_COUNTERS: usize = 0x381D; // 0x573d05, 09, 0d

const ENGINE_TABLE: u32 = 0x0053_cec0;
const VEHICLE_TABLE: u32 = 0x0053_ddb8;
const N_ENGINES: usize = 66;
const PE_ARCHIVE: usize = 0x133;

impl SimState {
    fn eng_u8(&self, e: usize, o: u32) -> u8 {
        self.data.image.u8(ENGINE_TABLE + 0x3a * e as u32 + o)
    }
    fn eng_i32(&self, e: usize, o: u32) -> i32 {
        self.data.image.i32(ENGINE_TABLE + 0x3a * e as u32 + o)
    }
    fn eng_years(&self, e: usize) -> (i32, i32) {
        let v = u32::from(self.eng_u8(e, 0x2c));
        (
            i32::from(self.data.image.u16(VEHICLE_TABLE + 0x39 * v + 0x1c)),
            i32::from(self.data.image.u16(VEHICLE_TABLE + 0x39 * v + 0x1e)),
        )
    }

    /// `engine_available(e, 0)` (`0x4683a0`, trains.md 2.4) with only the global effects.
    pub(super) fn engine_available(&self, e: usize, year: i32, month: i32) -> bool {
        let id = if e < 0x3e { e + 0x7f } else { e + 0xf3 };
        let m = i16_at(&self.save.state, ST_GLOBAL_EFFECTS + 2 * id);
        if m < 0 {
            return false;
        }
        if m > 0 {
            return true;
        }
        if self.eng_u8(e, 0) == 0 || self.save.state[ST_ENGINE_ALLOWED + e] == 0 {
            return false;
        }
        let (first, last) = self.eng_years(e);
        (first < year || (year == first && month > 5)) && year <= last
    }

    /// `ai_pick_engine(mode)` (`0x4d86f0`, ai.md 4) [V]: one `rng(80, 120)` per candidate.
    pub(super) fn pick_engine(&self, w: &mut World, mode: u32) -> i32 {
        let d = date_of(w.minutes);
        let (year, month) = (i32::from(d.year), i32::from(d.month));
        let mut best = -1i32;
        let mut best_score = -1i32;
        for pass in 0..3 {
            if best != -1 {
                continue;
            }
            for e in 0..N_ENGINES {
                if self.eng_u8(e, 0x28) == 2 && pass < 1 {
                    continue;
                }
                let ok = self.engine_available(e, year, month) || {
                    let (first, last) = self.eng_years(e);
                    pass >= 2 && first < year && year <= last
                };
                if !ok {
                    continue;
                }
                let speed = self.eng_i32(e, 0x14);
                let attr = if mode != 0 {
                    speed
                } else {
                    speed.wrapping_mul(self.eng_i32(e, 0x18))
                };
                let r = w.rng.range(80, 120);
                let score = (f64::from(r) * 0.01 * f64::from(attr)) as i32;
                if score > best_score {
                    best_score = score;
                    best = e as i32;
                }
            }
        }
        if best < 1 { 0 } else { best }
    }

    /// `ai_monthly_turn` (`0x436d60`, world-tick.md 3.5, ai.md 1): the port in
    /// [`crate::ai::monthly_turn`]. The commands it queues (industry purchases, command 0x2d)
    /// are executed right away here; the original runs them at the command step of the next
    /// frame, after the rest of `sim_month_end` (ai.md 8).
    pub(super) fn ai_monthly_turn(&mut self, w: &mut World) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "ai_monthly_turn",
                "ai_monthly_turn 0x436d60 on a 0x233C file",
                "world-tick.md 3.5"
            );
            return;
        }
        let (traders, mut cmds) = self.with_trainstep(w, crate::ai::monthly_turn_persons);
        for p in traders {
            self.ai_trade_stocks(w, p as usize);
        }
        let triggers = self.with_trainstep(w, |ts| {
            cmds.extend(crate::ai::monthly_turn_companies(ts));
            let mut triggers = Vec::new();
            for cmd in cmds {
                if let crate::ai::AiCommand::FireTrigger(k) = cmd {
                    triggers.push(k);
                } else {
                    crate::ai::apply_command(ts, cmd);
                }
            }
            triggers
        });
        for k in triggers {
            self.event_trigger(w, k);
        }
    }

    /// `sim_year_end` items 1 and 2: the mean of metric 11 and the engine reference speed
    /// `0x573e08` (world-tick.md 3.4).
    pub(super) fn year_reference_speed(&mut self, w: &mut World) {
        if !self.v2348() {
            return;
        }
        let set = self.company_set();
        let mut total = 0i32;
        for &c in &set {
            if let Some(r) = pool::record(&self.save.companies, c) {
                // metric 11: the year's mean speed, `+0x19a / max(+0x196, 1)`.
                total += i32_at(&r.data, 0x19a).wrapping_div(i32_at(&r.data, 0x196).max(1));
            }
        }
        let m = if set.is_empty() {
            0.0
        } else {
            f64::from(total) / set.len() as f64
        };
        let e = self.pick_engine(w, 1) as usize;
        let vel = f64::from(self.eng_i32(e, 0x14));
        let v = ftol_((vel * 0.5 + m) * 0.5).clamp(4, 200);
        self.save.state[ST_REF_SPEED] = v as u8;
        self.log.ran("year_ai_pick_engine");
    }

    /// `0x5705d1 = growth_factor` (`sim_year_end` item 8, world-growth.md 3.2).
    pub(super) fn year_growth_factor(&mut self, w: &World) {
        if !self.v2348() {
            return;
        }
        let year = i32::from(date_of(w.minutes).year);
        let start = i32::from(u16_at(&self.save.state, ST_YEAR_START));
        let rate = f32::from_bits(u32_at(&self.save.header, HDR_RATE));
        let level = i32_at(&self.save.header, HDR_LEVEL);
        let g = crate::growth::growth_factor(rate, year - start, level);
        put_f32(&mut self.save.state, ST_GROWTH, g);
    }

    /// `company_year_end` (`0x43d800`, finance.md 8 and 16) for the company at `slot`.
    pub(super) fn company_year_end(&mut self, w: &mut World, slot: usize) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "company_year_end",
                "company_year_end 0x43d800 on a 0x233C file",
                "finance.md 8"
            );
            return;
        }
        let Some(rec) = pool::record(&self.save.companies, slot) else {
            return;
        };
        if i32_at(&rec.data, CO_ACTIVE) == 0 {
            return;
        }
        let d = rec.data.clone();
        let date = date_of(w.minutes);
        let year = i32::from(date.year);
        let mut co = Company::from_record(&d);
        let set = self.company_set();
        let idx = set.iter().position(|&c| c == slot);

        // 1. Archive the 14 metrics (16.2), the categories and the accounts; zero the year. The
        // original shifts the metric rows first and then stores the live metrics into row 1 one
        // by one, so the weighted return (metric 5) already sees the new metrics 0..4 in row 1
        // and last year's row in row 2 (observed: Example, company 4).
        let (mut m, live) = ([0.0f64; 14], {
            let ctx = self.fin_ctx(w.minutes, slot);
            let p = f64::from(co.trade_price(&ctx, 0, false));
            [
                p,
                co.live_metric(&ctx, 1),
                co.live_metric(&ctx, 2),
                co.live_metric(&ctx, 3),
                co.live_metric(&ctx, 4),
            ]
        });
        for k in (2..=10).rev() {
            co.arch_metrics[k] = co.arch_metrics[k - 1];
        }
        co.arch_metrics[1][..5].copy_from_slice(&live);
        m[..5].copy_from_slice(&live);
        {
            let ctx = self.fin_ctx(w.minutes, slot);
            m[5] = weighted_return(&co, &ctx, live[0]);
            m[6] = co.credit_rating(&ctx, 0);
        }
        m[7] = f64::from(co.dividends_paid);
        m[8] = f64::from(i32_at(&d, 0x192));
        m[9] = f64::from(i32_at(&d, 0x196));
        m[10] = f64::from(
            (i32_at(&d, 0x186).wrapping_add(2i32.wrapping_mul(i32_at(&d, 0x18a)))).wrapping_mul(10),
        );
        m[11] = f64::from(i32_at(&d, 0x19a).wrapping_div(i32_at(&d, 0x196).max(1)));
        m[12] = f64::from(i32_at(&d, 0x46a));
        m[13] = f64::from(i32_at(&d, 0x134));
        co.arch_metrics[1] = m;
        for k in (1..10).rev() {
            co.arch_cats[k] = co.arch_cats[k - 1];
        }
        co.arch_cats[0] = co.cats;
        co.cats = [0.0; 13];
        // Accounts and the zeroed counters live outside the finance struct.
        let mut raw = d.clone();
        co.write_record(&mut raw);
        {
            use crate::company_record::CO_ARCH_ACCT;
            // arch_acct[k] <- arch_acct[k-1] (k = 9..1), arch_acct[0] <- acct.
            for k in (1..10).rev() {
                let (a, b) = (CO_ARCH_ACCT + 0x40 * (k - 1), CO_ARCH_ACCT + 0x40 * k);
                let src: Vec<u8> = raw[a..a + 0x40].to_vec();
                raw[b..b + 0x40].copy_from_slice(&src);
            }
            for i in 0..8 {
                let v = co.acct[i];
                put_f64(&mut raw, CO_ARCH_ACCT + 8 * i, v);
            }
        }
        put_f32(&mut raw, 0x38, 0.0);
        co.dividends_paid = 0.0;
        for o in [0x192usize, 0x196, 0x19a] {
            put_u32(&mut raw, o, 0);
        }
        for t in 0..32usize {
            let b = 0x479 + t * 0x1ed;
            put_u16(&mut raw, b + 5, 0);
            // The u16 right after the first 44-entry array is cleared too (`0x43d800`:
            // `*(u16 *)(p + 0x58) = 0` with `p = rec + 0x51a + t * 0x1ed`; seen at the first
            // year end of the Example, territory 1 of companies 4 and 5).
            put_u16(&mut raw, b + 0xf9, 0);
            for i in 0..44 {
                put_u16(&mut raw, b + 0xa1 + 2 * i, 0);
                put_u16(&mut raw, b + 0x195 + 2 * i, 0);
            }
        }

        // 2. Overhead (16.1).
        let mut acc = 0i32;
        let mut n = 0i32;
        // Original bug: the zeroed new year counts and the third year is skipped; the fix
        // averages the archives k = 0..2 (the three completed years).
        let full_years = self.rules.has(crate::ruleset::Fix::OverheadAverage);
        for i in 0..3i32 {
            let y = year - i;
            if full_years {
                if y >= co.founded {
                    let sum = co.arch_cats[i as usize]
                        .iter()
                        .take(5)
                        .fold(0.0, |a, b| a + b);
                    acc = ftol_(f64::from(acc) + sum);
                    n += 1;
                }
                continue;
            }
            if y >= co.founded {
                let sum = if y == year {
                    0.0
                } else {
                    co.arch_cats[(i - 1) as usize]
                        .iter()
                        .take(5)
                        .fold(0.0, |a, b| a + b)
                };
                acc = ftol_(f64::from(acc) + sum);
                n += 1;
            }
        }
        let avg = acc.wrapping_div(n.max(1));
        let q = avg / 100_000;
        let mut g = f64::from(q).max(1.0).sqrt();
        if g > 10.0 {
            g = (g - 10.0) * 0.5 + 10.0;
        }
        if g > 15.0 {
            g = (g - 15.0) * 0.5 + 15.0;
        }
        if g > 20.0 {
            g = (g - 20.0) * 0.5 + 20.0;
        }
        co.overhead = ftol_(f64::from(avg) * g * 0.01).max(1000);

        // 3. Bonds that mature next year are repaid (4.4).
        co.mature_bonds(year);

        // 4. Stock split (8).
        let price = {
            let ctx = self.fin_ctx(w.minutes, slot);
            co.trade_price(&ctx, 0, true)
        };
        let split = co.split_if_needed(price);
        if split.is_some() {
            // `+0x4250 = 0` and a second committed quote at the new share count (0x43d800,
            // after the archive columns are divided): the cache, the "no dividend" price and
            // `+0x427e` are recomputed before the persons' net worth archive reads them.
            co.price_stamp = 0;
            let ctx = self.fin_ctx(w.minutes, slot);
            co.trade_price(&ctx, 0, true);
        }

        // Write back: the struct fields over the raw copy with the accounts archived.
        let mut out = raw;
        co.write_record(&mut out);
        if let Some(r) = pool::record_mut(&mut self.save.companies, slot) {
            r.data = out;
        }
        if let (Some(n), Some(i)) = (split, idx) {
            for p in pool::list_order(&self.save.persons) {
                if let Some(r) = pool::record_mut(&mut self.save.persons, p) {
                    let h = i32_at(&r.data, PE_HOLDINGS + 4 * i);
                    put_u32(&mut r.data, PE_HOLDINGS + 4 * i, h.wrapping_mul(n) as u32);
                }
            }
            self.log.ran("company_split_message");
            // The split headline (`0x43d800`): outside the editor, with the header byte
            // `0x570238` clear and the option `0x5706ab` (state `+0x1c3`) on, the company the
            // interface shows gets it as news (`news_post` `0x4a42f0`: CRT draw, news flag and
            // counter; any other company gets a message box, `0x46adb0`, nothing saved). Seen
            // in NewJapan.MP2 at the 1995 year end.
            if self.save.header.get(0x8700).copied().unwrap_or(0) == 0
                && self.save.state.get(0x1c3).copied().unwrap_or(0) != 0
                && self.local_player().1 == slot
            {
                self.fc_news_post(w);
            }
            self.ts_globals
                .news
                .push(crate::message::GameMessage::StockSplit {
                    company: slot as i32,
                    ratio: n,
                });
        }
        self.log.ran("company_year_end");
    }

    /// `person_year_step` (`0x4a7880`): shifts the net worth archive (9 entries of
    /// `{cash, value of the shares}`, 16 bytes each, from `+0x133`) and records this year.
    pub(super) fn person_year_step(&mut self, w: &World, slot: usize) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "person_year_step",
                "person_year_step 0x4a7880 on a 0x233C file",
                "world-tick.md 3.4 item 10"
            );
            return;
        }
        let Some(rec) = pool::record(&self.save.persons, slot) else {
            return;
        };
        let holdings: Vec<i32> = (0..36)
            .map(|i| i32_at(&rec.data, PE_HOLDINGS + 4 * i))
            .collect();
        let cash = f64_at(&rec.data, PE_CASH);
        let set = self.company_set();
        let mut value = 0.0f64;
        for (i, &c) in set.iter().enumerate() {
            let p = self.company_quote(w.minutes, c, false);
            value += f64::from(holdings[i]) * f64::from(p);
        }
        if let Some(r) = pool::record_mut(&mut self.save.persons, slot) {
            r.data
                .copy_within(PE_ARCHIVE..PE_ARCHIVE + 0x90, PE_ARCHIVE + 0x10);
            put_f64(&mut r.data, PE_ARCHIVE, cash);
            put_f64(&mut r.data, PE_ARCHIVE + 8, value);
        }
        self.log.ran("person_year_step");
    }

    /// Counters of the year that start at 1 January (`sim_year_end` item 17).
    pub(super) fn year_counters_reset(&mut self) {
        if !self.v2348() {
            return;
        }
        for o in 0..12 {
            self.save.state[ST_YEAR_COUNTERS + o] = 0;
        }
        // The theft percentage `0x573e02` (i16) creeps back to the scenario's `0x573d9e` (read as
        // an i32): `+= max((base - pct) * 0.05, 1.0)`, truncated (`0x4a50f2`, world-tick.md 3.4
        // item 17).
        let st = &mut self.save.state;
        let pct = i32::from(i16::from_le_bytes([st[ST_THEFT_PCT], st[ST_THEFT_PCT + 1]]));
        let base = i32_at(st, ST_THEFT_BASE);
        if pct < base {
            let d = (f64::from(base.wrapping_sub(pct)) * 0.05).max(1.0);
            let v = crate::fp::ftol(f64::from(pct) + d) as i16;
            st[ST_THEFT_PCT..ST_THEFT_PCT + 2].copy_from_slice(&v.to_le_bytes());
        }
        self.log.ran("year_theft_pct");
    }

    // ---- buildings ----------------------------------------------------------------------

    fn cargo_on(&self, c: u8) -> bool {
        self.save
            .state
            .get(ST_CARGO_ON + usize::from(c))
            .is_some_and(|&b| b != 0)
    }

    /// `industry_year_step` (`0x40b450`, world-growth.md 5.1) for the building at `slot`.
    pub(super) fn industry_year_step(&mut self, w: &World, slot: usize) {
        if !self.v2348() {
            return;
        }
        let Some(rec) = pool::record(&self.save.objects, slot) else {
            return;
        };
        let d = &rec.data;
        let ty = usize::from(d[industry::OFF_TYPE]);
        if ty == 0 || d[industry::OFF_INACTIVE] != 0 {
            return;
        }
        let owner = i32::from(u16_at(d, 0x97));
        let shape = usize::from(d[6]);
        let mut delivered = [0i8; 4];
        let mut produced = [0i8; 4];
        for k in 0..4 {
            delivered[k] = d[0x63 + k] as i8;
            produced[k] = d[0x67 + k] as i8;
        }
        let territory = i32::from(d[0x9d] as i8);
        let price = if owner != 0 {
            i32_at(d, 0x99)
        } else {
            let mult = self.effect_table().multiplier(0xd, territory);
            crate::growth::building_price(self.data.growth.shape_price[shape], f64::from(mult))
        };
        let year = i32::from(date_of(w.minutes).year);
        let cargo_on = |c: u8| self.cargo_on(c);
        let out = industry_rating(&RatingInput {
            ty: &self.types[ty],
            year,
            cargo_on: &cargo_on,
            sandbox: self.sandbox(),
            peer_version: 0x2334,
            received: u16_at(d, 0x34),
            delivered,
            produced,
            old: f32_at(d, industry::OFF_QUALITY),
            price,
        });
        let mut rating = out.rating;
        let mut profit = out.profit;
        let mut profit_k = out.profit_k;
        if owner != 0 && !self.company_is_human(owner as usize) {
            rating = computer_owner_rating(rating);
            profit = ftol_((f64::from(price) * (f64::from(i32::from(rating) - 100) * 0.01)) * 0.25);
            profit_k = (profit / 1000) as i16;
        }
        if let Some(r) = pool::record_mut(&mut self.save.objects, slot) {
            put_f32(&mut r.data, industry::OFF_QUALITY, out.smoothed);
            put_u16(&mut r.data, industry::OFF_RATING_PCT, rating);
            pool::put_i16(&mut r.data, 0x36, profit_k);
        }
        if owner != 0 {
            self.company_ledger_credit(owner as usize, 3, f64::from(profit));
        }
        self.log.ran("industry_year_step");
    }

    pub(super) fn company_is_human(&self, company: usize) -> bool {
        pool::record(&self.save.companies, company)
            .is_some_and(|c| self.person_is_human(i32_at(&c.data, CO_CHAIRMAN)))
    }

    /// `company_ledger_credit` (`0x439970`) on the raw record.
    pub(super) fn company_ledger_credit(&mut self, company: usize, cat: usize, x: f64) {
        if let Some(r) = pool::record_mut(&mut self.save.companies, company) {
            let mut co = Company::from_record(&r.data);
            co.credit(cat, x);
            co.write_record(&mut r.data);
        }
    }

    /// `industry_year_reset` (`0x40b7c0`, world-growth.md 5.1) for the building at `slot`.
    pub(super) fn industry_year_reset(&mut self, w: &World, slot: usize) {
        if !self.v2348() {
            return;
        }
        let Some(rec) = pool::record(&self.save.objects, slot) else {
            return;
        };
        if rec.data[industry::OFF_INACTIVE] != 0 {
            return;
        }
        let station = usize::from(u16_at(&rec.data, 8));
        let ty = rec.data[industry::OFF_TYPE];
        let mut data = rec.data.clone();
        put_u16(&mut data, 0x34, 0);
        data[0x63..0x67].fill(0);
        data[0x67..0x6b].fill(0);
        // `0x40b7c0` recounts every active building, type 0 included (its rating becomes
        // `ftol(+0x93 * 100 + 50)`: a lot built during the run gets 100 at its first year end;
        // seen in the Example at the 1877 year end, building 403).
        let _ = ty;
        {
            let types = std::mem::take(&mut self.types);
            let effects = self.effect_table();
            let env = Env {
                types: &types,
                now: w.minutes,
                year: i32::from(date_of(w.minutes).year),
                basic_model: self.save.state[ST_BASIC_MODEL] != 0,
                sandbox: self.sandbox(),
                overtime: false,
                econ_factor: 1.0,
                effects: &effects,
            };
            industry::recount_piles(&mut data, &self.save.pool_7c, &env);
            self.types = types;
        }
        if let Some(r) = pool::record_mut(&mut self.save.objects, slot) {
            r.data = data;
        }
        if station != 0
            && let Some(s) = pool::record_mut(&mut self.save.stations, station)
        {
            // `0x4beae0`: two dwords at `+0x59`/`+0x5d` (8 bytes) and the yearly counters of the
            // station (finance.md 13.4).
            s.data[0x59..0x61].fill(0);
            for o in [0x4b5usize, 0x4b7, 0x4b9] {
                put_u16(&mut s.data, o, 0);
            }
        }
        self.log.ran("industry_year_reset");
    }
}

/// `0x43abc0`: the weighted 5-year return (finance.md 16.2).
pub(super) fn weighted_return(co: &Company, ctx: &crate::finance::Ctx, price: f64) -> f64 {
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for i in 0..5i32 {
        if ctx.year - i < co.founded {
            continue;
        }
        // The metric rows are already shifted: row `k` holds the year `k - 1` years ago.
        let p0 = if ctx.year - i - 1 < co.founded {
            f64::from(co.init_price)
        } else {
            co.arch_metrics[(i + 1) as usize][0]
        };
        let pe = if i == 0 {
            price
        } else {
            co.arch_metrics[i as usize][0]
        };
        let dv = if i == 0 {
            f64::from(co.dividends_paid)
        } else {
            co.arch_metrics[i as usize][7]
        };
        let mut ret = if p0 > 0.0 { (dv + pe - p0) / p0 } else { 1.0 };
        let mut w = f64::from(5 - i);
        if i == 0 {
            let f = f64::from(ctx.month) / 12.0;
            ret /= f;
            w *= f;
        }
        num += f64::from(5 - i) * ret;
        den += w;
    }
    if den == 0.0 { 0.0 } else { num / den }
}
