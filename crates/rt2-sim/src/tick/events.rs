//! Scenario triggers (`event_fire_trigger` `0x457fe0`, docs/spec/scenarios.md and world-tick.md
//! 3.7): the world side of [`crate::scenario`] over the raw pools of the save.
//!
//! The rule pool lives in the save ([`rt2_formats::map::Save::rules`], kept raw); the decoded
//! [`Scenario`] is built on the first trigger and the `fired` flags are written back to the raw
//! records after each call. Functions and effects whose data is not modelled return the
//! documented default (0) and are counted in the [`HookLog`](crate::hooks::HookLog).

use super::company::{PE_ACTIVE, PE_CASH, PE_EFFECTS, PE_HOLDINGS};
use super::*;
use crate::company_record::{CO_ACCT, CO_ACTIVE, CO_CATS, CO_CHAIRMAN, CO_EFFECTS, CO_YEAR_CATS};
use crate::finance::Company;
use crate::fp::ftol as ftol_;
use crate::rng::{CrtRng, GameRng};
use crate::scenario::{
    Confiscate, Message, Scenario, ScenarioEnv, ScenarioWorld, Scope, fire_trigger,
};

const ST_YEAR_START: usize = 0x20; // 0x570508
const ST_GAME_DATE: usize = 0x10; // 0x5704f8
const ST_LOADS_YEAR: usize = 0x381D; // 0x573d05
const ST_LOADS_LIFE: usize = 0x3829; // 0x573d11
const ST_TRACK_CELLS: usize = 0x3835; // 0x573d1d (+4 single, +8 double, +0xc electric)
const ST_GAME_VARS: usize = 0x38A6; // 0x573d8e
const ST_MULTIPLAYER: usize = 0x3A34; // 0x573f1c
const HDR_CAMPAIGN: usize = 0x86FC; // 0x570234

const CO_VARS: usize = 0x4219;
const CO_TRACK_LEFT: usize = 0x4260;
const CO_TERR_BLOCK: usize = 0x479;
const CO_TERR_STRIDE: usize = 0x1ed;
const PE_VARS: usize = 0x534;
const TERR_VARS: usize = 0x2b;
const TERR_FLAGS: usize = 0x44;
const TERR_EFFECTS: usize = 0x58;

impl SimState {
    /// `event_fire_trigger(code)` (`0x457fe0`).
    pub(super) fn event_trigger(&mut self, w: &mut World, code: u8) {
        if self.save.rules.is_empty() {
            return;
        }
        if !self.v2348() {
            todo_hook!(
                self.log,
                "event_fire_trigger",
                "event_fire_trigger 0x457fe0 on a 0x233C file",
                "world-tick.md 3.7"
            );
            return;
        }
        let mut sc = self.scenario.take().unwrap_or_else(|| {
            Scenario::new(
                self.save
                    .rules
                    .iter()
                    .map(crate::scenario::Rule::from_file)
                    .collect(),
                self.data.scenario_effects.clone(),
            )
        });
        // Pool state of the raw records (the engine reads the decoded copy).
        for (r, raw) in sc.rules.iter_mut().zip(&self.save.rules) {
            r.fired = raw.fixed.get(0x4c).copied().unwrap_or(0) != 0;
        }
        let minutes = w.minutes;
        let mut world = ScWorld {
            s: self,
            minutes,
            crt: &mut w.crt,
            choices: &mut w.choices,
            sc: Some(sc),
        };
        fire_trigger(code, &mut world, &mut w.rng);
        let sc = world.sc.take();
        if let Some(sc) = sc {
            for (r, raw) in sc.rules.iter().zip(self.save.rules.iter_mut()) {
                if raw.fixed.len() > 0x50 {
                    raw.fixed[0x4c] = u8::from(r.fired);
                }
            }
            // Undo rules of temporary effects (scenarios.md 4.4) join the saved pool.
            let n = self.save.rules.len();
            for r in sc.rules.iter().skip(n) {
                self.save.rules.push(r.to_file());
                self.log.ran("scenario_undo_rule");
            }
            self.scenario = Some(sc);
        }
        self.log.ran("event_fire_trigger");
    }

    /// `company_quote(commit)` = `0x43b410(0, commit)` on the raw record: returns the price.
    pub(super) fn company_quote(&mut self, minutes: i32, slot: usize, commit: bool) -> i32 {
        let Some(rec) = pool::record(&self.save.companies, slot) else {
            return 0;
        };
        // `0x43b410` returns 0.0 (`0x53500c`) for a company that does not exist (`+0x16e == 0`)
        // without touching the cache or `+0x427e` (the inactive company of #03 at the start).
        if i32_at(&rec.data, CO_ACTIVE) == 0 {
            return 0;
        }
        let mut co = Company::from_record(&rec.data);
        let price = {
            let ctx = self.fin_ctx(minutes, slot);
            co.trade_price(&ctx, 0, commit)
        };
        if let Some(r) = pool::record_mut(&mut self.save.companies, slot) {
            co.write_record(&mut r.data);
        }
        price
    }

    /// `0x469fb0(n)`: sets the economic state and prices every company again.
    pub(super) fn set_economy_state_to(&mut self, minutes: i32, n: i32) {
        let old = i32_at(&self.save.state, ST_ECON_STATE);
        if n == old {
            return;
        }
        put_f32(
            &mut self.save.state,
            ST_PRIME_RATE,
            prime_rate_for_state(n.clamp(0, 4) as usize),
        );
        put_u32(&mut self.save.state, ST_ECON_STATE, n as u32);
        for slot in self.company_set() {
            self.company_quote(minutes, slot, true);
        }
    }
}

struct ScWorld<'a> {
    s: &'a mut SimState,
    minutes: i32,
    crt: &'a mut CrtRng,
    choices: &'a mut crate::choice::Choices,
    sc: Option<Scenario>,
}

impl ScWorld<'_> {
    fn co(&self, company: i32) -> Option<&[u8]> {
        if company <= 0 {
            return None;
        }
        pool::record(&self.s.save.companies, company as usize).map(|r| r.data.as_slice())
    }
    fn pe(&self, person: i32) -> Option<&[u8]> {
        if person <= 0 {
            return None;
        }
        pool::record(&self.s.save.persons, person as usize).map(|r| r.data.as_slice())
    }
    fn te(&self, territory: i32) -> Option<&[u8]> {
        if territory <= 0 {
            return None;
        }
        pool::record(&self.s.save.territories, territory as usize).map(|r| r.data.as_slice())
    }
    fn st_i32(&self, o: usize) -> i32 {
        i32_at(&self.s.save.state, o)
    }
    fn skip(&mut self, name: &'static str, what: &'static str) {
        self.s.log.skip(name, what, "scenarios.md 3.5");
    }
    fn sum_cats(c: &[u8], base: usize, n: usize) -> f64 {
        (0..n).fold(0.0, |a, i| a + f64_at(c, base + 8 * i))
    }

    /// Territory block byte/word helpers of a company record.
    fn block_off(territory: i32) -> usize {
        CO_TERR_BLOCK + territory.max(0) as usize * CO_TERR_STRIDE
    }

    fn net_worth(&mut self, person: i32) -> f64 {
        let cash = self.pe(person).map_or(0.0, |p| f64_at(p, PE_CASH));
        let set = self.s.company_set();
        let mut total = cash;
        for (i, &c) in set.iter().enumerate() {
            let held = self
                .pe(person)
                .map_or(0, |p| i32_at(p, PE_HOLDINGS + 4 * i));
            let price = self.s.company_quote(self.minutes, c, false);
            total += f64::from(held) * f64::from(price);
        }
        total
    }

    /// `company_goodwill(territory)` (`0x43eca0`, scenarios.md 3.5): over the blocks of the
    /// territories `1..=count`, `v = loads_life/N + (track >> 4) + loads_year` (`i16` at block
    /// `+7`, `+3`, `+5`; `N = max(1, ftol((founded - year) * 1.5))`, so 1 for a company already
    /// founded); the standing `+0x4229` becomes `ftol((Σ standing*v / Σ v + standing) * 0.5)`
    /// when `Σ v > 0` (positive `v` only); with a territory (ignored when there is a single
    /// one) it is averaged with that territory's standing; times `effect(0x28)`, in `0..=10000`.
    fn goodwill(&self, company: i32, territory: i32) -> i32 {
        let Some(c) = self.co(company) else {
            return 0;
        };
        let n_terr = self.s.save.territories.count;
        let territory = if n_terr == 1 { 0 } else { territory };
        let mut g = i32_at(c, 0x4229);
        let year = i32::from(date_of(self.minutes).year);
        let n = ftol_(f64::from(i32_at(c, 0x148).wrapping_sub(year)) * 1.5).max(1);
        let (mut sum_v, mut sum_sv) = (0.0f64, 0.0f64);
        for t in 1..=n_terr {
            let b = Self::block_off(t as i32);
            if b + 9 > c.len() {
                break;
            }
            let v = i32::from(i16_at(c, b + 7)) / n
                + (i32::from(i16_at(c, b + 3)) >> 4)
                + i32::from(i16_at(c, b + 5));
            if v > 0 {
                sum_sv += f64::from(i32::from(i16_at(c, b + 1)).wrapping_mul(v));
                sum_v += f64::from(v);
            }
        }
        if sum_v > 0.0 {
            g = ftol_((sum_sv / sum_v + f64::from(g)) * 0.5);
        }
        if territory != 0 {
            g = (i32::from(i16_at(c, Self::block_off(territory) + 1)) + g) / 2;
        }
        let m = f64::from(self.s.effect_mult(0x28, company as usize));
        ftol_(m * f64::from(g.max(0))).clamp(0, 10_000)
    }

    /// `0x46b9a0`: some station network (`+0x57 < 0x100`) of `company` (0 = any owner) has a
    /// station in territory `terr` and another that satisfies `other`.
    fn connected(&self, company: i32, terr: i32, other: impl Fn(&[u8]) -> bool) -> bool {
        use std::collections::BTreeMap;
        let mut nets: BTreeMap<u8, (bool, bool)> = BTreeMap::new();
        for slot in pool::list_order(&self.s.save.stations) {
            let Some(r) = pool::record(&self.s.save.stations, slot) else {
                continue;
            };
            let d = &r.data;
            if company != 0 && i32_at(d, 0x39) != company {
                continue;
            }
            let net = d[0x57];
            if i32_at(d, 0x4ad) == terr {
                nets.entry(net).or_default().0 = true;
            }
            if other(d) {
                nets.entry(net).or_default().1 = true;
            }
        }
        nets.values().any(|&(a, b)| a && b)
    }
}

impl ScenarioEnv for ScWorld<'_> {
    fn value(&mut self, id: usize, param: i32, company: i32, person: i32, territory: i32) -> i32 {
        let year_now = i32::from(u16_at(&self.s.save.state, ST_GAME_DATE));
        let month_now = i32::from(self.s.save.state[ST_GAME_DATE + 2]);
        let day_now = i32::from(self.s.save.state[ST_GAME_DATE + 3]);
        let start_year = i32::from(u16_at(&self.s.save.state, ST_YEAR_START));
        match id {
            1 => 1,
            3 => self.s.player_set().len() as i32,
            4 => self.s.company_set().len() as i32,
            5 => i32::from(param > 0 && self.co(param).is_some_and(|c| i32_at(c, CO_ACTIVE) != 0)),
            6 => i32::from(param > 0 && self.pe(param).is_some_and(|p| i32_at(p, PE_ACTIVE) != 0)),
            7 => {
                let mut v = year_now - start_year;
                if month_now > 11 && day_now > 27 {
                    v += 1;
                }
                v
            }
            8 => start_year,
            9 => year_now,
            10 => month_now,
            11 => month_now + year_now * 100,
            12 => self.st_i32(ST_ECON_STATE),
            13 => self.st_i32(ST_LOADS_YEAR),
            14 => self.st_i32(ST_LOADS_LIFE),
            15..=18 => self.st_i32(ST_TRACK_CELLS + 4 * (id - 15)),
            19..=22 => self.st_i32(ST_GAME_VARS + 4 * (id - 19)),
            23 => self.co(company).map_or(0, |c| ftol_(f64_at(c, CO_ACCT))),
            24 => self
                .co(company)
                .map_or(0, |c| ftol_(f64_at(c, CO_ACCT + 48))),
            25 => self.co(company).map_or(0, |c| {
                ftol_((0..6).fold(0.0, |a, i| a + f64_at(c, CO_ACCT + 8 * i)))
            }),
            26 => self
                .co(company)
                .map_or(0, |c| ftol_(-f64_at(c, CO_ACCT + 56))),
            27 => self
                .co(company)
                .map_or(0, |c| ftol_(Self::sum_cats(c, CO_CATS, 5))),
            28 => self
                .co(company)
                .map_or(0, |c| ftol_(Self::sum_cats(c, CO_YEAR_CATS, 5))),
            29 => self
                .co(company)
                .map_or(0, |c| ftol_(Self::sum_cats(c, CO_CATS, 13))),
            30 => self
                .co(company)
                .map_or(0, |c| ftol_(Self::sum_cats(c, CO_YEAR_CATS, 13))),
            31 => self.co(company).map_or(0, |c| i32_at(c, 0x192)),
            32 => self.co(company).map_or(0, |c| i32_at(c, 0x19e)),
            33..=36 => self
                .co(company)
                .map_or(0, |c| i32_at(c, 0x182 + 4 * (id - 33))),
            // `CompanyGoodwill` = `0x43eca0(company, 0) / 100`; `CompanyTerritoryGoodwill`
            // passes the territory (needs both).
            37 if company != 0 => self.goodwill(company, 0) / 100,
            45 if company != 0 && territory != 0 => self.goodwill(company, territory) / 100,
            37 | 45 => 0,
            38 => company,
            39..=42 => self
                .co(company)
                .map_or(0, |c| i32_at(c, CO_VARS + 4 * (id - 39))),
            43 | 44 => {
                if company == 0 || territory == 0 {
                    return 0;
                }
                if id == 43 {
                    if self.te(param).is_none() {
                        return 0;
                    }
                    i32::from(self.connected(company, territory, |d| i32_at(d, 0x4ad) == param))
                } else {
                    if pool::record(&self.s.save.places, param.max(0) as usize).is_none() {
                        return 0;
                    }
                    i32::from(
                        self.connected(company, territory, |d| i32::from(u16_at(d, 0x53)) == param),
                    )
                }
            }
            46..=50 => {
                if company == 0 || territory == 0 {
                    return 0;
                }
                let o = Self::block_off(territory);
                let b = self.co(company).map_or(0, |c| c[o]);
                match id {
                    46 => i32::from(b & 1 != 0),
                    47 => i32::from(b & 2 != 0),
                    48 => i32::from(b & 8 != 0),
                    49 => i32::from(b & 4 != 0),
                    _ => i32::from(b != 0),
                }
            }
            51..=53 => {
                if company == 0 || territory == 0 {
                    return 0;
                }
                let o = Self::block_off(territory) + 3 + 2 * (id - 51);
                self.co(company).map_or(0, |c| i32::from(i16_at(c, o)))
            }
            54 => self.pe(person).map_or(0, |p| ftol_(f64_at(p, PE_CASH))),
            55 => {
                self.skip(
                    "scenario_fn_stock_value",
                    "PlayerStockValue 0x4a6d00 (not specified)",
                );
                0
            }
            56 => {
                if person == 0 {
                    return 0;
                }
                ftol_(self.net_worth(person))
            }
            57 => self
                .pe(person)
                .map_or(0, |p| i32::from(i32_at(p, 0x263) != 0)),
            58 => self.pe(person).map_or(0, |p| i32_at(p, 0x263)),
            59 => company,
            60..=63 => self
                .pe(person)
                .map_or(0, |p| i32_at(p, PE_VARS + 4 * (id - 60))),
            64 => self.te(territory).map_or(0, |t| i32::from(u16_at(t, 0x50))),
            65 => self.te(territory).map_or(0, |t| i32::from(u16_at(t, 0x52))),
            66 => self.te(territory).map_or(0, |t| i32_at(t, 0x54)),
            67 | 68 => {
                if territory == 0 {
                    return 0;
                }
                if id == 67 {
                    if self.te(param).is_none() {
                        return 0;
                    }
                    i32::from(self.connected(0, territory, |d| i32_at(d, 0x4ad) == param))
                } else {
                    if pool::record(&self.s.save.places, param.max(0) as usize).is_none() {
                        return 0;
                    }
                    i32::from(self.connected(0, territory, |d| i32::from(u16_at(d, 0x53)) == param))
                }
            }
            69 => territory,
            70..=73 => self
                .te(territory)
                .map_or(0, |t| i32_at(t, TERR_VARS + 4 * (id - 70))),
            91 | 92 | 105 => {
                if company == 0 || territory == 0 || !(0..32).contains(&param) {
                    return 0;
                }
                let m = |a: i32, b: i32| {
                    self.co(company).map_or(0, |c| {
                        i32::from(i16_at(
                            c,
                            0x482 + a as usize * CO_TERR_STRIDE + 2 * b as usize,
                        ))
                    })
                };
                match id {
                    91 => m(territory, param),
                    92 => m(territory, param) + m(param, territory),
                    _ => m(param, territory),
                }
            }
            // `[0x566d2a] + 1` / `[0x566d00] + 1` (campaign block, header + 0x898E) in a
            // campaign game (`0x570234`).
            93 | 94 => {
                if self.s.hdr(HDR_CAMPAIGN) == 0 {
                    0
                } else {
                    let b = self.s.save.layout.header_len + if id == 93 { 0x2a } else { 0 };
                    i32_at(&self.s.save.header, b) + 1
                }
            }
            95 => {
                if param > 0
                    && let Some(r) = pool::record(&self.s.save.stations, param as usize)
                {
                    i32::from(r.data[0x4bb])
                } else {
                    0
                }
            }
            96 => pool::list_order(&self.s.save.stations)
                .into_iter()
                .filter_map(|s| pool::record(&self.s.save.stations, s))
                .filter(|r| i32_at(&r.data, 0x45) == 0 && r.data[0x4bb] != 0)
                .count() as i32,
            97 | 101 => self.co(company).map_or(0, |c| {
                if i32_at(c, 0x148) < year_now {
                    let a = Company::from_record(c);
                    ftol_(a.arch_cats[0][3])
                } else {
                    0
                }
            }),
            98 => self
                .co(company)
                .map_or(0, |c| ftol_(f64_at(c, CO_ACCT + 40))),
            99 => i32::from(self.st_i32(ST_MULTIPLAYER) != 0),
            100 => i32::from(self.st_i32(ST_MULTIPLAYER) == 0),
            102 => self
                .co(company)
                .map_or(0, |c| ftol_(f64_at(c, CO_YEAR_CATS + 24))),
            103 | 104 => 0,
            106..=109 => {
                if company == 0 || territory == 0 || !(0..44).contains(&param) {
                    return 0;
                }
                let base = [0x51a, 0x4c2, 0x60e, 0x5b6][id - 106] - 0x479;
                let o = Self::block_off(territory) + base + 2 * param as usize;
                self.co(company).map_or(0, |c| i32::from(i16_at(c, o)))
            }
            110 | 111 => {
                if company == 0 || !(0..44).contains(&param) {
                    return 0;
                }
                let base = if id == 110 { 0x51a } else { 0x4c2 };
                self.co(company).map_or(0, |c| {
                    (0..32)
                        .map(|t| {
                            i32::from(i16_at(c, base + t * CO_TERR_STRIDE + 2 * param as usize))
                        })
                        .sum()
                })
            }
            112 => self.co(company).map_or(0, |c| {
                i32_at(c, 0x19a).wrapping_div(i32_at(c, 0x196).max(1))
            }),
            113 => self.co(company).map_or(0, |c| {
                i32_at(c, 0x1a6).wrapping_div(i32_at(c, 0x1a2).max(1))
            }),
            114 => pool::list_order(&self.s.save.track_pieces)
                .into_iter()
                .filter_map(|s| pool::record(&self.s.save.track_pieces, s))
                .filter(|r| r.data.get(0x23).is_some_and(|b| b & 0x80 != 0))
                .count() as i32,
            // `TerritoryCompletelyFlooded` (`0x454ae0` case 0x73): 1 unless some tile of the
            // territory (`+9`) has a terrain class (`0x581090[tile+0 >> 6]`) other than water.
            115 => {
                if territory == 0 {
                    return 0;
                }
                let t = rt2_formats::terrain::tables();
                i32::from(self.s.save.map.tiles.iter().all(|tl| {
                    let id = (u16::from(tl.var) | (u16::from(tl.ground) << 8)) >> 6;
                    i32::from(tl.region) != territory || t.class_of_id(id) == 0x12
                }))
            }
            116 => self.co(company).map_or(0, |c| i32_at(c, CO_TRACK_LEFT)),
            117 => {
                if company == 0 || !(0..32).contains(&param) {
                    return 0;
                }
                let o = Self::block_off(param);
                self.co(company).map_or(0, |c| i32::from(c[o] != 0))
            }
            _ => 0,
        }
    }

    fn crt_range(&mut self, lo: i32, hi: i32) -> i32 {
        self.crt.range(lo, hi)
    }
    fn companies(&self) -> Vec<i32> {
        pool::list_order(&self.s.save.companies)
            .into_iter()
            .map(|s| s as i32)
            .collect()
    }
    fn persons(&self) -> Vec<i32> {
        pool::list_order(&self.s.save.persons)
            .into_iter()
            .map(|s| s as i32)
            .collect()
    }
    fn territories(&self) -> Vec<i32> {
        pool::list_order(&self.s.save.territories)
            .into_iter()
            .map(|s| s as i32)
            .collect()
    }
}

impl ScenarioWorld for ScWorld<'_> {
    fn take_scenario(&mut self) -> Scenario {
        self.sc.take().unwrap_or_default()
    }
    fn put_scenario(&mut self, scenario: Scenario) {
        self.sc = Some(scenario);
    }
    fn date(&self) -> (i32, i32) {
        let d = date_of(self.minutes);
        (i32::from(d.year), i32::from(d.month))
    }
    fn economy_state(&self) -> i32 {
        self.st_i32(ST_ECON_STATE)
    }
    fn set_economy_state(&mut self, state: i32) {
        let m = self.minutes;
        self.s.set_economy_state_to(m, state);
    }
    fn indexed_companies(&self) -> Vec<i32> {
        self.s.company_set().into_iter().map(|c| c as i32).collect()
    }
    fn indexed_persons(&self) -> Vec<i32> {
        self.s.player_set().into_iter().map(|c| c as i32).collect()
    }
    fn company_person(&self, company: i32) -> i32 {
        self.co(company).map_or(0, |c| i32_at(c, CO_CHAIRMAN))
    }
    fn is_human_company(&self, company: i32) -> bool {
        self.s.person_is_human(self.company_person(company))
    }
    fn is_human_person(&self, person: i32) -> bool {
        self.s.person_is_human(person)
    }
    // `[0x574414]` / `[0x566ca4]`: the company and person the interface plays; the clone takes
    // the first human person and its chairman company (`SimState::local_player`).
    fn local_company(&self) -> i32 {
        self.s.local_player().1 as i32
    }
    fn local_person(&self) -> i32 {
        self.s.local_player().0 as i32
    }
    fn is_campaign(&self) -> bool {
        self.s.hdr(HDR_CAMPAIGN) != 0
    }
    fn is_multiplayer(&self) -> bool {
        self.st_i32(ST_MULTIPLAYER) != 0
    }
    fn person_name(&self, _person: i32) -> String {
        String::new()
    }
    fn company_name(&self, _company: i32) -> String {
        String::new()
    }
    fn territory_name(&self, _territory: i32) -> String {
        String::new()
    }
    fn company_loads_lifetime(&self, company: i32) -> i32 {
        self.co(company).map_or(0, |c| i32_at(c, 0x19e))
    }
    fn stations_visited(&self) -> i32 {
        0
    }
    fn status_buffer_active(&self) -> bool {
        self.s.status_buf.is_some()
    }
    fn show_message(&mut self, message: &Message) -> usize {
        // `rule_collect_status` (`0x458210`): with the buffer `[0x5665e0]` set, the text goes
        // to it (`0x455f30`): the separator LNG 2 (`"\n\n"` in every LNG) when the buffer
        // already holds more than 2 characters, then the substituted text (Latin-1).
        if let Some(buf) = self.s.status_buf.as_mut() {
            if buf.len() > 2 {
                buf.extend_from_slice(b"\n\n");
            }
            buf.extend(message.text.chars().map(|c| c as u32 as u8));
            return 0;
        }
        // A choice takes the player's button in interactive mode (`crate::choice`), button 0
        // otherwise; the other kinds only go to the output queue (message.rs). A newspaper
        // (`+0x60 == 1`) goes through `news_post` (`0x4a42f0`): one CRT draw, the news flag
        // `0x5707bd` and the counter `0x5707be` are the only saved state it touches.
        if message.kind == crate::scenario::MessageKind::Newspaper {
            let _ = self.crt.range(0, 10000);
            self.s.save.state[0x2d5] = 1;
            let n = u32_at(&self.s.save.state, 0x2d6).wrapping_add(1);
            put_u32(&mut self.s.save.state, 0x2d6, n);
        }
        self.s
            .ts_globals
            .news
            .push(crate::message::GameMessage::Scenario(message.clone()));
        self.s.log.ran("scenario_message");
        if message.kind == crate::scenario::MessageKind::Choice {
            self.choices.answer(message)
        } else {
            0
        }
    }
    fn credit_company(&mut self, company: i32, amount: i32) {
        let slot = company as usize;
        let Some(rec) = pool::record(&self.s.save.companies, slot) else {
            return;
        };
        let mut co = Company::from_record(&rec.data);
        co.credit(4, f64::from(amount));
        if let Some(r) = pool::record_mut(&mut self.s.save.companies, slot) {
            co.write_record(&mut r.data);
        }
    }
    fn credit_person(&mut self, person: i32, amount: i32) {
        self.s.person_add_cash(person as usize, f64::from(amount));
    }
    fn set_territory_rights(
        &mut self,
        territory: i32,
        company: i32,
        mask: u8,
        allow: bool,
        rng: &mut GameRng,
    ) {
        // `territory_set_rights` (`0x4c8540`, `build::rights`): the sub-territories, the
        // territory's own flags without a company, each company's block and `0x43df20`
        // (`+0x422d` / `+0x422e`: the right in every territory).
        let minutes = self.minutes;
        let crt = &mut *self.crt;
        self.s.with_trainstep_parts(minutes, rng, crt, |ts| {
            crate::ai::Ai::new(ts).territory_set_rights(territory, mask, allow, company, false);
        });
    }
    fn confiscate(
        &mut self,
        company: i32,
        territory: i32,
        what: Confiscate,
        destroy: bool,
        rng: &mut GameRng,
    ) {
        // `0x437d40` / `0x437db0` / `0x437f50` / `0x4380a0` (`build::confiscate`).
        let minutes = self.minutes;
        let crt = &mut *self.crt;
        self.s.with_trainstep_parts(minutes, rng, crt, |ts| {
            crate::ai::Ai::new(ts).territory_confiscate(company, territory, what, destroy);
        });
    }
    fn deactivate_company(&mut self, company: i32) {
        // Effect 189: `0x438170(company, 1)`.
        if pool::record(&self.s.save.companies, company as usize).is_none() {
            return;
        }
        let minutes = self.minutes;
        let crt = &mut *self.crt;
        // Nothing in it draws from the game RNG (train_delete, owners, holdings).
        let mut rng = GameRng::new(0);
        self.s
            .company_liquidate(minutes, &mut rng, crt, company as usize, true);
    }
    fn eliminate_player(&mut self, person: i32) {
        let minutes = self.minutes;
        self.s.player_remove(minutes, person as usize);
    }
    fn human_takes_player(&mut self, _person: i32) {
        self.skip("scenario_human_takes_player", "0x467bd0");
    }
    fn set_persona(&mut self, _person: i32, _index: i32) {
        self.skip("scenario_set_persona", "persona table copy (LNG texts)");
    }
    fn flood_territory(&mut self, _territory: i32, _amount: i32) {
        self.skip("scenario_flood", "0x4c8a30");
    }
    fn retire_train(&mut self, id: i32, rng: &mut GameRng) {
        // `0x4cdd50` only when the train is in the pool (`0x574c64`).
        let minutes = self.minutes;
        let crt = &mut *self.crt;
        self.s.with_trainstep_parts(minutes, rng, crt, |ts| {
            if ts.live(crate::trainstep::P::Train, id) {
                crate::ai::Ai::new(ts).train_delete(id);
            }
        });
    }
    fn set_territory_buyable(&mut self, territory: i32, which: &[usize], on: bool) {
        if let Some(r) = pool::record_mut(&mut self.s.save.territories, territory as usize) {
            for &i in which {
                r.data[TERR_FLAGS + i] = u8::from(on);
            }
        }
    }
    fn set_variable(&mut self, scope: Scope, index: usize, value: i32, add: bool) {
        let apply = |data: &mut [u8], o: usize| {
            let v = if add {
                i32_at(data, o).wrapping_add(value)
            } else {
                value
            };
            put_u32(data, o, v as u32);
        };
        match scope {
            Scope::Game => apply(&mut self.s.save.state, ST_GAME_VARS + 4 * index),
            Scope::Company(c) => {
                if let Some(r) = pool::record_mut(&mut self.s.save.companies, c as usize) {
                    apply(&mut r.data, CO_VARS + 4 * index);
                }
            }
            Scope::Person(p) => {
                if let Some(r) = pool::record_mut(&mut self.s.save.persons, p as usize) {
                    apply(&mut r.data, PE_VARS + 4 * index);
                }
            }
            Scope::Territory(t) => {
                if let Some(r) = pool::record_mut(&mut self.s.save.territories, t as usize) {
                    apply(&mut r.data, TERR_VARS + 4 * index);
                }
            }
        }
    }
    fn change_track_allowed(&mut self, company: i32, value: i32, add: bool) {
        if let Some(r) = pool::record_mut(&mut self.s.save.companies, company as usize) {
            let v = if add {
                i32_at(&r.data, CO_TRACK_LEFT).wrapping_add(value)
            } else {
                value
            };
            put_u32(&mut r.data, CO_TRACK_LEFT, v as u32);
        }
    }
    fn add_level(&mut self, scope: Scope, effect: usize, delta: i32) {
        let add = |data: &mut [u8], o: usize| {
            let v = i16_at(data, o).wrapping_add(delta as i16);
            pool::put_i16(data, o, v);
        };
        match scope {
            Scope::Game => add(&mut self.s.save.state, ST_GLOBAL_EFFECTS + 2 * effect),
            Scope::Company(c) => {
                if let Some(r) = pool::record_mut(&mut self.s.save.companies, c as usize) {
                    add(&mut r.data, CO_EFFECTS + 2 * effect);
                }
            }
            Scope::Person(p) => {
                if let Some(r) = pool::record_mut(&mut self.s.save.persons, p as usize) {
                    add(&mut r.data, PE_EFFECTS + 2 * effect);
                }
            }
            Scope::Territory(t) => {
                if let Some(r) = pool::record_mut(&mut self.s.save.territories, t as usize) {
                    add(&mut r.data, TERR_EFFECTS + 2 * effect);
                }
            }
        }
    }
    fn engines_changed(&mut self) {
        self.skip(
            "scenario_engines_changed",
            "0x574784: engine availability changed (consumer not traced)",
        );
    }
    fn manager_effect(&mut self, company: i32, manager: usize, rng: &mut GameRng) {
        if company > 0 {
            let minutes = self.minutes;
            self.s
                .company_set_manager(rng, minutes, company as usize, manager, false);
        }
    }
}
