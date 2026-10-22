//! The world loop: `sim_advance` and the day/week/month/year hooks of docs/spec/world-tick.md.
//!
//! [`World::from_save`] builds the state from a parsed save, [`World::advance`] runs the loop and
//! [`World::to_save`] writes the state back. The save is kept whole (`Save` holds every byte of
//! the file, the pools with their `next`/`prev` arrays included), and each implemented step edits
//! the records it owns in place, so everything not simulated yet is written back unchanged. A
//! step that is not implemented is a [`todo_hook!`](crate::todo_hook): it is counted and cites
//! the spec item, see [`HookLog`].
//!
//! Fidelity assumptions that are not in the save (runtime globals of the original, all zero or
//! default in a freshly loaded game): no "Overtime" cheat, no multiplayer, `0x5748f8 == 0`,
//! `0x5794f0 == 0` (no station network rebuild pending), industry set 0 (`0x5704ac`), no queued
//! player commands. The state offsets below equal the offset from `game_running` `0x5704e8`
//! and hold for version 0x2348 files; a 0x233C file shifts them (map-gam.md 5.1), so the steps
//! that read them are skipped (and counted) on those.

use rt2_formats::map::{GameDate, MAGIC_2348, Save};

use crate::calendar::{Date, MINUTES_PER_DAY, minutes_to_date};
use crate::cargo::IndustryType;
use crate::data::GameData;
use crate::finance::{prime_rate_for, prime_rate_for_state};
use crate::fp::ftol;
use crate::hooks::HookLog;
use crate::industry::{self, EffectTable, Env};
use crate::pool::{
    self, f32_at, f64_at, i16_at, i32_at, put_f32, put_f64, put_u16, put_u32, u16_at,
};
use crate::todo_hook;
use crate::world::World;

// State block offsets (VA - 0x5704e8), version 0x2348.
const ST_PRIME_RATE: usize = 0x1E9; // 0x5706d1 f32
const ST_ECON_STATE: usize = 0x22F; // 0x570717 i32
const ST_ECON_MOM: usize = 0x233; // 0x57071b i32
const ST_ECON_DATE: usize = 0x237; // 0x57071f: year u16, month, day
const ST_ECON_TIME: usize = 0x23B; // 0x570723: hour, minute
const ST_ECON_DRIFT: usize = 0x23D; // 0x570725 i32
const ST_GLOBAL_EFFECTS: usize = 0x2DE; // 0x5707c6 i16[]
const ST_BASIC_MODEL: usize = 0x388F; // 0x573d77
// Header offsets (VA - 0x567b38).
const HDR_SANDBOX: usize = 0x86EF; // 0x570227
const HDR_NO_ECONOMY: usize = 0x86FF; // 0x570237

/// Effect id of the prime rate adjustment (`0x46bac0` asks `0x46b5d0` for `0x20`).
const EFFECT_PRIME: usize = 0x20;

/// Simulation state attached to a [`World`] built from a save.
#[derive(Clone, Debug)]
pub struct SimState {
    pub(crate) save: Save,
    data: Box<GameData>,
    /// Industry types of the active set, indices rebuilt for the current year (`0x471880`).
    types: Vec<IndustryType>,
    pub(crate) log: HookLog,
}

impl PartialEq for SimState {
    fn eq(&self, other: &Self) -> bool {
        self.save == other.save && self.log == other.log
    }
}
impl Eq for SimState {}

fn date_of(minutes: i32) -> Date {
    minutes_to_date(minutes)
}

impl World {
    /// Builds the simulation state from a parsed save: clock, speed, `game_seed`, and every pool
    /// kept raw. The game RNG is not in the file (map-gam.md 5.2): seed it with
    /// [`Command::SeedRng`](crate::Command::SeedRng) before advancing.
    pub fn from_save(save: &Save, data: &GameData) -> World {
        let mut w = World::new(
            Date::from_minutes(save.game_time_minutes),
            save.game_seed,
            0,
        );
        w.minutes = save.game_time_minutes;
        w.speed = save.game_speed;
        if save.game_speed != 0 {
            w.speed_resume = save.game_speed;
        }
        let mut sim = SimState {
            save: save.clone(),
            data: Box::new(data.clone()),
            types: Vec::new(),
            log: HookLog::default(),
        };
        sim.rebuild_types(i32::from(save.date.year));
        w.sim = Some(Box::new(sim));
        w
    }

    /// The state as a save of the version it was loaded from. Panics if the world was not built
    /// with [`World::from_save`].
    pub fn to_save(&self) -> Save {
        let sim = self.sim.as_ref().expect("World::to_save needs from_save");
        let mut s = sim.save.clone();
        let d = date_of(self.minutes);
        s.game_time_minutes = self.minutes;
        s.date = GameDate {
            year: d.year,
            month: d.month,
            day: d.day,
        };
        s.hour = d.hour;
        s.minute = d.minute;
        s.game_speed = self.speed;
        s.game_seed = self.game_seed;
        s.sync_scalars();
        s
    }

    /// Step counters of the world loop (implemented and missing steps); `None` when the world was
    /// not built from a save.
    pub fn hook_log(&self) -> Option<&HookLog> {
        self.sim.as_ref().map(|s| &s.log)
    }

    /// `sim_advance(dt)` (`0x4a5cb0`, world-tick.md section 2): advances `dt` game minutes,
    /// running the end-of-day/week/month/year hooks at every 23:59 crossed. Without a save only
    /// the clock moves.
    pub fn advance(&mut self, dt: i32) {
        match self.sim.take() {
            Some(mut sim) => {
                sim.sim_advance(self, dt);
                self.sim = Some(sim);
            }
            None => self.advance_minutes(dt),
        }
    }
}

impl SimState {
    fn v2348(&self) -> bool {
        self.save.layout.version == MAGIC_2348
    }

    fn hdr(&self, off: usize) -> u8 {
        self.save.header.get(off).copied().unwrap_or(0)
    }

    fn sandbox(&self) -> bool {
        self.hdr(HDR_SANDBOX) != 0
    }

    fn rebuild_types(&mut self, year: i32) {
        // Industry set 0 (`0x5704ac`, see the module notes).
        let mut types = self.data.industries[0].clone();
        for t in &mut types {
            t.rebuild_inputs(year);
        }
        self.types = types;
        self.log.ran("industry_rebuild_inputs");
    }

    /// Writes the clock into the save blocks that mirror it.
    fn set_clock(&mut self, w: &mut World, minutes: i32) {
        w.minutes = minutes;
        let d = date_of(minutes);
        self.save.game_time_minutes = minutes;
        self.save.date = GameDate {
            year: d.year,
            month: d.month,
            day: d.day,
        };
        self.save.hour = d.hour;
        self.save.minute = d.minute;
    }

    // ---- sim_advance (world-tick.md 2) ---------------------------------------------------

    fn sim_advance(&mut self, w: &mut World, dt: i32) {
        self.log.ran("sim_advance");
        // 1. Scheduled commands: none in a local game without a player.
        let mut dt = dt;
        if dt == 0 {
            return;
        }
        let cur = date_of(w.minutes);
        let new = date_of(w.minutes.wrapping_add(dt));
        // The original tests each field with `<`, not the date as a whole.
        if cur.year < new.year || cur.month < new.month || cur.day < new.day {
            let to_midnight = 1439 - (i32::from(cur.hour) * 60 + i32::from(cur.minute));
            self.sim_advance(w, to_midnight);
            self.day_end(w);
            dt -= to_midnight + 1;
            loop {
                let d = date_of(w.minutes);
                if (d.year, d.month, d.day) == (new.year, new.month, new.day) {
                    break;
                }
                self.sim_advance(w, 1439);
                self.day_end(w);
                dt -= MINUTES_PER_DAY;
            }
        }
        self.linear_pass(w, dt);
    }

    /// The linear path of `sim_advance` (world-tick.md 2.1), also run with `dt == 0`.
    fn linear_pass(&mut self, w: &mut World, dt: i32) {
        self.log.ran("linear_pass");
        let t = w.minutes.wrapping_add(dt);
        self.set_clock(w, t);
        // Map animations, pool_78 in list order (pattern A).
        let mut id = pool::first(&self.save.pool_78);
        while id != 0 {
            let next = pool::next(&self.save.pool_78, id);
            todo_hook!(
                self.log,
                "anim_object_step",
                "anim_object_step 0x4283e0 (map animation objects)",
                "world-tick.md 2.1 item 1"
            );
            id = next;
        }
        // Trains of the single local player, in list order (pattern A).
        let mut id = pool::first(&self.save.trains);
        while id != 0 {
            let next = pool::next(&self.save.trains, id);
            todo_hook!(
                self.log,
                "train_step",
                "train_step 0x4d0ce0 (movement, loading, breakdowns)",
                "world-tick.md 2.1 item 2; trains.md 4.5"
            );
            id = next;
        }
    }

    /// The 23:59 step (`fim_de_dia`, world-tick.md 2 and 3.1) of a local game.
    fn day_end(&mut self, w: &mut World) {
        self.log.ran("day_end");
        let day = date_of(w.minutes).day;
        if day.is_multiple_of(7) {
            self.week_end(w);
            if day == 28 {
                self.month_end(w);
                return;
            }
        }
        self.set_clock(w, w.minutes + 1);
    }

    // ---- week (world-tick.md 3.2) --------------------------------------------------------

    fn effect_table(&self) -> EffectTable {
        let mut t = EffectTable::default();
        if self.v2348() {
            let n = 0x140;
            t.global = (0..n)
                .map(|i| {
                    let o = ST_GLOBAL_EFFECTS + 2 * i;
                    self.save
                        .state
                        .get(o..o + 2)
                        .map_or(0, |_| i16_at(&self.save.state, o))
                })
                .collect();
        }
        for r in &self.save.territories.records {
            if r.data.len() >= 0x58 {
                let v = (0..(r.data.len() - 0x58) / 2)
                    .map(|i| i16_at(&r.data, 0x58 + 2 * i))
                    .collect();
                t.territories.insert(r.slot, v);
            }
        }
        t
    }

    fn week_end(&mut self, w: &mut World) {
        self.log.ran("sim_week_end");
        if !self.v2348() {
            todo_hook!(
                self.log,
                "state_layout_233c",
                "0x233C files shift the global state block; economy and effect fields not read",
                "map-gam.md 5.1"
            );
        }
        let state = if self.v2348() {
            i32_at(&self.save.state, ST_ECON_STATE).clamp(0, 4) as usize
        } else {
            2
        };
        let effects = self.effect_table();
        let types = std::mem::take(&mut self.types);
        let env = Env {
            types: &types,
            now: w.minutes,
            year: i32::from(date_of(w.minutes).year),
            basic_model: self.v2348() && self.save.state[ST_BASIC_MODEL] != 0,
            sandbox: self.sandbox(),
            overtime: false,
            econ_factor: self.data.production_econ[state],
            effects: &effects,
        };
        // Industries, pattern B: the next record is read after the call, the count is re-read.
        let mut i = 0usize;
        let mut cur = 0usize;
        while i < self.save.objects.count {
            cur = if i == 0 {
                pool::first(&self.save.objects)
            } else {
                pool::next(&self.save.objects, cur)
            };
            if cur == 0 {
                break;
            }
            let Some(rec) = pool::record(&self.save.objects, cur) else {
                break;
            };
            if rec.data[industry::OFF_TYPE] != 0 {
                let mut data = rec.data.clone();
                industry::recount_piles(&mut data, &self.save.pool_7c, &env);
                self.log.ran("industry_recount_piles");
                industry::production_tick(&mut data, &mut self.save.pool_7c, &env, &mut self.log);
                self.log.ran("industry_production_tick");
                if let Some(r) = pool::record_mut(&mut self.save.objects, cur) {
                    r.data = data;
                }
            }
            i += 1;
        }
        self.types = types;
        // Stations.
        let mut id = pool::first(&self.save.stations);
        while id != 0 {
            let next = pool::next(&self.save.stations, id);
            todo_hook!(
                self.log,
                "station_recompute_supply_demand",
                "station_recompute_supply_demand 0x4bebf0",
                "cargo.md 8; world-tick.md 3.2 item 2"
            );
            id = next;
        }
    }

    // ---- month (world-tick.md 3.3) -------------------------------------------------------

    fn event_trigger(&mut self, _code: u8) {
        if !self.save.rules.is_empty() {
            todo_hook!(
                self.log,
                "event_fire_trigger",
                "event_fire_trigger 0x457fe0: scenario rules (may draw RNG for random conditions)",
                "world-tick.md 3.7"
            );
        }
    }

    fn month_end(&mut self, w: &mut World) {
        self.log.ran("sim_month_end");
        let date = date_of(w.minutes);
        // 1. station_networks_rebuild (flag 0x5794f0 never set in a loaded game).
        self.event_trigger(4);
        self.event_trigger(3);
        if date.month == 2 || date.month == 8 {
            self.economy_state_step(w, date);
        }
        if date.month < 11 {
            self.town_bonus_roll(w, date);
        }
        if !self.sandbox() {
            self.expire_lots(w);
        }
        todo_hook!(
            self.log,
            "ai_monthly_turn",
            "ai_monthly_turn 0x436d60: AI persons and companies (RNG)",
            "world-tick.md 3.5"
        );
        // 8. Companies (pattern C).
        for _ in 0..self.save.companies.count {
            todo_hook!(
                self.log,
                "company_month_step",
                "company_month_step 0x4399d0 on the raw company record",
                "finance.md 5; world-tick.md 3.3 item 8"
            );
        }
        // 9. Persons (pattern C).
        for slot in pool::list_order(&self.save.persons) {
            self.person_month_step(slot);
        }
        for _ in 0..self.save.trains.count {
            todo_hook!(
                self.log,
                "train_monthly_costs",
                "train_monthly_costs 0x4d4390 on the raw train record",
                "trains.md 6.2; world-tick.md 3.3 item 10"
            );
        }
        // 11. Stations: delivery counters decay, then supply/demand.
        for slot in pool::list_order(&self.save.stations) {
            if let Some(r) = pool::record_mut(&mut self.save.stations, slot) {
                for k in 0..44 {
                    let o = 0x271 + 4 * k;
                    if o + 4 <= r.data.len() {
                        let v = f64::from(f32_at(&r.data, o)) * 0.94;
                        put_f32(&mut r.data, o, v as f32);
                    }
                }
            }
            todo_hook!(
                self.log,
                "station_recompute_supply_demand",
                "station_recompute_supply_demand 0x4bebf0",
                "cargo.md 8; world-tick.md 3.3 item 11"
            );
        }
        todo_hook!(
            self.log,
            "margin_check_all",
            "margin_check_all 0x46bb10: margin calls",
            "finance.md 7.3; world-tick.md 3.3 item 12"
        );
        // 13. `report_due_flag` is a UI flag.
        if date.month == 12 {
            self.year_end(w);
        } else {
            self.set_clock(w, w.minutes + 1);
        }
        self.event_trigger(2);
    }

    /// `economy_state_step` (`0x46a010`, cargo.md 9) [S]; the RNG draws are [D] (world-tick.md 4).
    fn economy_state_step(&mut self, w: &mut World, date: Date) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "economy_state_step",
                "economy_state_step 0x46a010 on a 0x233C state block",
                "cargo.md 9"
            );
            return;
        }
        if self.hdr(HDR_NO_ECONOMY) != 0 {
            return;
        }
        self.log.ran("economy_state_step");
        let st = &mut self.save.state;
        let now_idx = i32::from(date.year) * 12 + i32::from(date.month);
        let last_idx = i32::from(u16_at(st, ST_ECON_DATE)) * 12 + i32::from(st[ST_ECON_DATE + 2]);
        if now_idx == last_idx {
            return;
        }
        let state = i32_at(st, ST_ECON_STATE);
        put_u16(st, ST_ECON_DATE, date.year);
        st[ST_ECON_DATE + 2] = date.month;
        st[ST_ECON_DATE + 3] = date.day;
        st[ST_ECON_TIME] = date.hour;
        st[ST_ECON_TIME + 1] = date.minute;
        let drift = ftol(f64::from(i32_at(st, ST_ECON_DRIFT)) * 0.8)
            .wrapping_add((now_idx - last_idx).wrapping_mul(state - 2));
        put_u32(st, ST_ECON_DRIFT, drift as u32);
        let r = w.rng.range(-5, 5) + 2;
        let mom_old = i32_at(st, ST_ECON_MOM);
        let mom = ftol(f64::from(r) - f64::from(drift) * 0.1 + f64::from(mom_old)).clamp(-5, 5);
        put_u32(st, ST_ECON_MOM, mom as u32);
        let roll = w.rng.range(0, 10);
        let mut new_state = state;
        if mom.abs() > roll {
            new_state += if mom < 1 { -1 } else { 1 };
        }
        let new_state = new_state.clamp(0, 4);
        if new_state != state {
            todo_hook!(
                self.log,
                "economy_headline",
                "news headline of the new economic state (0x4a42f0 / 0x46aa90)",
                "cargo.md 9"
            );
            put_f32(st, ST_PRIME_RATE, prime_rate_for_state(new_state as usize));
            put_u32(st, ST_ECON_STATE, new_state as u32);
            for _ in 0..self.save.companies.count {
                todo_hook!(
                    self.log,
                    "company_economy_change",
                    "0x43b410 per company after an economic state change",
                    "cargo.md 9; finance.md"
                );
            }
        }
    }

    /// The monthly `rng_range(0, 10000)` draw and, on success, `town_bonus_offer` (`0x431660`,
    /// world-tick.md 3.3 item 5 and 3.6).
    fn town_bonus_roll(&mut self, w: &mut World, date: Date) {
        let width = self.save.map.width as i32;
        let years = (i32::from(date.year) - 1860).abs();
        let roll = w.rng.range(0, 10000);
        let threshold = (50 - years) * (54300 / width.max(1)) / 70;
        self.log.ran("town_bonus_roll");
        if roll < threshold {
            self.town_bonus_offer(w);
        }
    }

    /// `town_bonus_offer` (`0x431660`, world-tick.md 3.6) [S]: up to 500 tries of one RNG draw.
    fn town_bonus_offer(&mut self, w: &mut World) {
        self.log.ran("town_bonus_offer");
        let places = pool::list_order(&self.save.places);
        if places.is_empty() {
            return;
        }
        let stations: Vec<(i32, i32)> = pool::list_order(&self.save.stations)
            .into_iter()
            .filter_map(|s| pool::record(&self.save.stations, s))
            .map(|r| (i32_at(&r.data, 0xa), i32_at(&r.data, 0xe)))
            .collect();
        for _ in 0..500 {
            let i = w.rng.range(0, places.len() as i32 - 1) as usize;
            let slot = places[i];
            let Some(rec) = pool::record(&self.save.places, slot) else {
                continue;
            };
            let p = &rec.data;
            if p[0x10] != 0 || i32_at(p, 0x36) >= 1 {
                continue;
            }
            let (px, py) = (f64::from(i16_at(p, 4)), f64::from(i16_at(p, 6)));
            let near = stations.iter().any(|&(x, y)| {
                x >= i32::from(i16_at(p, 8))
                    && x <= i32::from(i16_at(p, 12))
                    && y >= i32::from(i16_at(p, 10))
                    && y <= i32::from(i16_at(p, 14))
                    && {
                        let (dx, dy) = (f64::from(x as f32) - px, f64::from(y as f32) - py);
                        (dx * dx + dy * dy).sqrt() < 9.0
                    }
            });
            if near {
                continue;
            }
            let class = usize::from(p[0x2c]);
            let bonus = self.data.town_bonus.get(class).copied().unwrap_or(0);
            if let Some(r) = pool::record_mut(&mut self.save.places, slot) {
                put_u32(&mut r.data, 0x36, bonus as u32);
            }
            todo_hook!(
                self.log,
                "town_bonus_message",
                "bonus offer news message (LNG 226) and map ping (0x475da0, 0x469320)",
                "world-tick.md 3.6"
            );
            return;
        }
    }

    /// Lot expiry (`sim_month_end` item 6, cargo.md 7.6): the due lots are detected, the removal
    /// (`0x466a60`: unlink from the pile, free the slot) is not implemented.
    fn expire_lots(&mut self, w: &mut World) {
        self.log.ran("lot_expiry_scan");
        for r in self.save.pool_7c.records.iter().rev() {
            let lot = &r.data;
            if u16_at(lot, industry::LOT_STATION) != 0 {
                continue;
            }
            let age = w.minutes.wrapping_sub(i32_at(lot, industry::LOT_CREATED));
            if age < 216_000 {
                continue;
            }
            let ind = usize::from(u16_at(lot, industry::LOT_INDUSTRY));
            if let Some(ir) = pool::record(&self.save.objects, ind) {
                let wait = self
                    .data
                    .cargo
                    .get(usize::from(lot[industry::LOT_CARGO]))
                    .map_or(0, |c| c.wait_scale);
                let shift = if ir.data[industry::OFF_STATIONS] == 0 {
                    2
                } else {
                    1
                };
                if age < (wait >> shift) {
                    continue;
                }
            }
            todo_hook!(
                self.log,
                "lot_expiry",
                "free an expired lot (0x466a60) and decrement the pile count",
                "cargo.md 7.6; world-tick.md 3.3 item 6"
            );
        }
    }

    /// `person_month_step` (`0x4a7590`, world-tick.md 3.3 item 9): the personal cash earns
    /// `(prime(0) - 0.04) * cash * (1/12)`, truncated to whole dollars.
    fn person_month_step(&mut self, slot: usize) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "person_month_step",
                "person_month_step 0x4a7590 on a 0x233C state block",
                "world-tick.md 3.3 item 9"
            );
            return;
        }
        let prime_rate = f32_at(&self.save.state, ST_PRIME_RATE);
        let effect = i16_at(&self.save.state, ST_GLOBAL_EFFECTS + 2 * EFFECT_PRIME);
        let prime = prime_rate_for(prime_rate, effect);
        if let Some(r) = pool::record_mut(&mut self.save.persons, slot) {
            let cash = f64_at(&r.data, 0x12a);
            let add = ftol((prime - 0.04) * cash * 0.08333333333333333);
            put_f64(&mut r.data, 0x12a, f64::from(add) + cash);
        }
        self.log.ran("person_month_step");
    }

    // ---- year (world-tick.md 3.4) --------------------------------------------------------

    fn year_end(&mut self, w: &mut World) {
        self.log.ran("sim_year_end");
        // Items 1-2: company metric average and ai_pick_engine (RNG [80,120] per candidate).
        todo_hook!(
            self.log,
            "year_ai_pick_engine",
            "company_metric(11) average and ai_pick_engine(1) 0x4d86f0 (RNG), 0x573e08",
            "world-tick.md 3.4 items 1-2"
        );
        for _ in 0..self.save.objects.count {
            todo_hook!(
                self.log,
                "industry_year_step",
                "industry_year_step 0x40b450",
                "world-tick.md 3.4 item 3 (not specified)"
            );
        }
        for _ in 0..self.save.companies.count {
            todo_hook!(
                self.log,
                "company_annual_review",
                "company_annual_review 0x43c760 (RNG)",
                "world-tick.md 3.4 item 4; finance.md 8.1"
            );
        }
        self.event_trigger(1);
        todo_hook!(
            self.log,
            "history_snapshot",
            "history_snapshot 0x4e9770 and the compound target 0x5705d1",
            "world-tick.md 3.4 items 7-8"
        );
        for _ in 0..self.save.companies.count {
            todo_hook!(
                self.log,
                "company_year_end",
                "company_year_end 0x43d800",
                "finance.md 8; world-tick.md 3.4 item 9"
            );
        }
        for _ in 0..self.save.persons.count {
            todo_hook!(
                self.log,
                "person_year_step",
                "person_year_step 0x4a7880 (net worth archive)",
                "world-tick.md 3.4 item 10"
            );
        }
        for _ in 0..self.save.trains.count {
            todo_hook!(
                self.log,
                "train_year_step",
                "train_year_step 0x4d4620 (yearly statistics shift)",
                "world-tick.md 3.4 item 11"
            );
        }
        // 12. Territories: `+0x52 (u16) = 0`.
        for slot in pool::list_order(&self.save.territories) {
            if let Some(r) = pool::record_mut(&mut self.save.territories, slot)
                && r.data.len() >= 0x54
            {
                put_u16(&mut r.data, 0x52, 0);
            }
        }
        self.log.ran("territory_year_step");
        if self.hdr(HDR_NO_ECONOMY) == 0 {
            todo_hook!(
                self.log,
                "town_year_step",
                "town_year_step 0x430a40 (growth.rs has the rules; the GrowthWorld/PickWorld traits are not yet implemented over the save pools)",
                "world-tick.md 3.4 item 13 (not specified)"
            );
        }
        // 14. Second weekly pass in the same minute.
        self.week_end(w);
        for _ in 0..self.save.objects.count {
            todo_hook!(
                self.log,
                "industry_year_reset",
                "industry_year_reset 0x40b7c0",
                "world-tick.md 3.4 item 15 (not specified)"
            );
        }
        todo_hook!(
            self.log,
            "track_piece_wear_rotation",
            "track piece wear: +0x1f = +0x1b; +0x1b = 0 (0x574c84, slots descending)",
            "world-tick.md 3.4 item 16"
        );
        // 17. 1 Jan 00:00.
        self.set_clock(w, w.minutes + 1);
        todo_hook!(
            self.log,
            "year_counters_reset",
            "zero 0x573d05/09/0d and update 0x573e02",
            "world-tick.md 3.4 item 17; time.md 7.5"
        );
        // 18. Industry type inputs for the new year.
        let year = i32::from(date_of(w.minutes).year);
        self.rebuild_types(year);
        todo_hook!(
            self.log,
            "year_start_messages",
            "historical messages (LNG 2630, 2851, 419, 417, 420)",
            "world-tick.md 3.4 item 19"
        );
        self.event_trigger(0);
    }
}
