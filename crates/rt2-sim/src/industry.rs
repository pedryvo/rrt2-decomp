//! Industries and houses on the save's industry pool (`list_industries` `0x574c74`, record
//! 0x9F bytes): the weekly pile recount and production of docs/spec/world-tick.md 3.2 and
//! docs/spec/cargo.md 7.2-7.3.
//!
//! The records stay raw bytes (docs/spec/cargo.md section 2 lists the fields used here); the
//! arithmetic comes from [`crate::cargo`]. Lot creation (`industry_spawn_lot` `0x40a770`) and the
//! pile recount (`0x40a610`) follow the exported code of the original: the spec describes them
//! in prose (cargo.md 7.3) and only the field offsets below are added.

use std::collections::BTreeMap;

use rt2_formats::map::Pool;

use crate::cargo::{IndustryType, NO_CARGO, production_multiplier, production_period};
use crate::effects::{EffectLevels, effect_multiplier};
use crate::fp::{f32r, ftol};
use crate::hooks::HookLog;
use crate::pool::{
    alloc, f32_at, i16_at, put_f32, put_i16, put_u16, put_u32, record, u16_at, u32_at,
};
use crate::todo_hook;

/// Offsets inside an industry record (cargo.md section 2, plus `0x40a610`/`0x40a770`).
pub const OFF_ID: usize = 0x00;
pub const OFF_TYPE: usize = 0x07;
pub const OFF_STATIONS: usize = 0x33;
pub const OFF_RATING: usize = 0x47;
pub const OFF_RATING_PCT: usize = 0x4b;
pub const OFF_PILE: usize = 0x4d;
pub const OFF_INACTIVE: usize = 0x4f;
pub const OFF_CLOCKS: usize = 0x53;
pub const OFF_PRODUCED: usize = 0x67;
pub const OFF_STOCK: usize = 0x6b;
pub const OFF_PILE_COUNT: usize = 0x8b;
pub const OFF_QUALITY: usize = 0x93;
pub const OFF_TERRITORY: usize = 0x9d;

/// Lot fields (cargo.md section 2).
pub const LOT_CARGO: usize = 0x00;
pub const LOT_INDUSTRY: usize = 0x01;
pub const LOT_STATION: usize = 0x03;
pub const LOT_CREATED: usize = 0x07;
pub const LOT_NEXT: usize = 0x0f;

/// Production stops below this rate (`0x5350b8`, f64 0.01).
const MIN_RATE: f64 = 0.01;
/// Lots waiting per output slot above which no more are made (`0x40a770`).
const MAX_WAITING: i16 = 0x32;

/// Event/manager effect values (`effect_multiplier` `0x46b480`, cargo.md 5) the production code
/// reads: the global array and the per-territory arrays.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectTable {
    /// `0x5707c6 + 2*e`.
    pub global: Vec<i16>,
    /// Territory slot -> `+0x58 + 2*e`.
    pub territories: BTreeMap<usize, Vec<i16>>,
}

impl EffectTable {
    /// `effect_multiplier(e, 0, 0, territory)`; territory 0 (or unknown) does not take part.
    pub fn multiplier(&self, effect: usize, territory: i32) -> f32 {
        let at = |v: &[i16], i: usize| v.get(i).copied().unwrap_or(0);
        effect_multiplier(EffectLevels {
            global: at(&self.global, effect),
            company: None,
            person: None,
            territory: usize::try_from(territory)
                .ok()
                .and_then(|t| self.territories.get(&t))
                .map(|v| at(v, effect)),
        })
    }
}

/// Everything the weekly industry step reads besides the records.
pub struct Env<'a> {
    /// Industry type table of the active set (indices rebuilt for the year).
    pub types: &'a [IndustryType],
    /// `game_time_minutes`.
    pub now: i32,
    pub year: i32,
    /// `0x573d77`: basic industry model.
    pub basic_model: bool,
    /// `0x570227`.
    pub sandbox: bool,
    /// "Overtime" cheat (`0x574c10`).
    pub overtime: bool,
    /// `production_econ[economy state]` (`0x550494`).
    pub econ_factor: f32,
    pub effects: &'a EffectTable,
}

fn slot_of_cargo(ty: &IndustryType, cargo: u8) -> Option<usize> {
    ty.slots.iter().position(|s| s.output == cargo)
}

fn stock(rec: &[u8]) -> [f32; 8] {
    let mut s = [0f32; 8];
    for (i, v) in s.iter_mut().enumerate() {
        *v = f32_at(rec, OFF_STOCK + 4 * i);
    }
    s
}

/// `industry_recount_piles(e, 0)` (`0x40a610`): recounts the lots of the pile that have no
/// station, per output slot, and resets the rating (`+0x47 = 1.0`, `+0x4b = ftol(q*100 + 50)`).
/// Stations are not recomputed (`param_1 == 0`).
pub fn recount_piles(rec: &mut [u8], lots: &Pool, env: &Env) {
    if rec[OFF_INACTIVE] != 0 {
        return;
    }
    let ty = &env.types[usize::from(rec[OFF_TYPE])];
    rec[OFF_PILE_COUNT..OFF_PILE_COUNT + 8].fill(0);
    let mut cur = usize::from(u16_at(rec, OFF_PILE));
    let mut guard = 0usize;
    while cur != 0 && guard <= lots.records.len() {
        guard += 1;
        let Some(lot) = record(lots, cur) else { break };
        if u16_at(&lot.data, LOT_STATION) == 0
            && let Some(i) = slot_of_cargo(ty, lot.data[LOT_CARGO])
        {
            let o = OFF_PILE_COUNT + 2 * i;
            let n = i16_at(rec, o).wrapping_add(1);
            put_i16(rec, o, n);
        }
        cur = usize::from(u16_at(&lot.data, LOT_NEXT));
    }
    put_f32(rec, OFF_RATING, 1.0);
    let pct = ftol(f64::from(f32_at(rec, OFF_QUALITY)) * 100.0 + 50.0);
    put_i16(rec, OFF_RATING_PCT, pct as i16);
}

/// `industry_spawn_lot(e, cargo)` (`0x40a770`): pushes a new lot on the head of the pile unless
/// the output slot already has 50 waiting. Linked stations (`+0x33 > 0`) would be told
/// (`0x468af0`, `0x4bebf0`): not implemented, counted.
fn spawn_lot(rec: &mut [u8], lots: &mut Pool, cargo: u8, env: &Env, log: &mut HookLog) {
    if rec[OFF_INACTIVE] != 0 {
        return;
    }
    let ty = &env.types[usize::from(rec[OFF_TYPE])];
    let Some(i) = slot_of_cargo(ty, cargo) else {
        return;
    };
    let waiting = i16_at(rec, OFF_PILE_COUNT + 2 * i);
    if waiting >= MAX_WAITING && !env.sandbox {
        return;
    }
    rec[OFF_PRODUCED + i] = rec[OFF_PRODUCED + i].wrapping_add(1);
    put_i16(rec, OFF_PILE_COUNT + 2 * i, waiting.wrapping_add(1));
    let mut lot = vec![0u8; lots.record_size];
    lot[LOT_CARGO] = cargo;
    lot[LOT_INDUSTRY..LOT_INDUSTRY + 2].copy_from_slice(&rec[OFF_ID..OFF_ID + 2]);
    put_u32(&mut lot, LOT_CREATED, env.now as u32);
    let head = u16_at(rec, OFF_PILE);
    put_u16(&mut lot, LOT_NEXT, head);
    match alloc(lots, lot) {
        Ok(slot) => put_u16(rec, OFF_PILE, slot as u16),
        Err(_) => todo_hook!(
            log,
            "pool_grow",
            "pool_alloc grows a full pool (0x4f14c0)",
            "world-tick.md 1; map-gam.md 5.3"
        ),
    }
    if rec[OFF_STATIONS] as i8 > 0 {
        todo_hook!(
            log,
            "industry_spawn_lot_stations",
            "lot count and supply/demand of the linked stations (0x468af0, 0x4bebf0)",
            "cargo.md 7.3, 8"
        );
    }
}

/// `industry_production_tick(e, 0)` (`0x40aa20`, cargo.md 7.3): for each output slot, creates
/// every lot that is due (one per loop turn, the slot clock advancing by one period).
pub fn production_tick(rec: &mut [u8], lots: &mut Pool, env: &Env, log: &mut HookLog) {
    if rec[OFF_INACTIVE] != 0 {
        return;
    }
    if env.sandbox {
        todo_hook!(
            log,
            "production_tick_sandbox",
            "sandbox branch of industry_production_tick (0x40aa20, flags at +0x9e)",
            "cargo.md 7.3 (sandbox)"
        );
        return;
    }
    let tid = rec[OFF_TYPE];
    let ty = &env.types[usize::from(tid)];
    let territory = i32::from(rec[OFF_TERRITORY] as i8);
    for k in 0..4 {
        loop {
            let st = stock(rec);
            let rate = ty.production_rate(k, tid, env.year, env.basic_model, env.sandbox, &st);
            if rate <= MIN_RATE {
                break;
            }
            let slot = &ty.slots[k];
            // Order of the original: effect of the output cargo first, then "all cargos".
            let e_out = env
                .effects
                .multiplier(0x5f + usize::from(slot.output), territory);
            let e_all = env.effects.multiplier(0x5e, territory);
            let m = production_multiplier(e_all, e_out, env.econ_factor);
            let period = production_period(rate, m, env.year, env.overtime) as u32;
            let co = OFF_CLOCKS + 4 * k;
            let mut clock = u32_at(rec, co);
            if (env.now as u32) < clock {
                clock = env.now as u32;
                put_u32(rec, co, clock);
            }
            if period == 0 {
                // The original divides by zero here.
                break;
            }
            let due = (env.now as u32).wrapping_sub(clock) / period;
            if due == 0 {
                break;
            }
            put_u32(rec, co, clock.wrapping_add(period));
            // Bonus inputs are consumed: `stock -= threshold` when the threshold is covered.
            for inp in slot.inputs {
                if inp.cargo != NO_CARGO {
                    let o = OFF_STOCK + 4 * usize::from(inp.index);
                    if o + 4 <= OFF_STOCK + 32 && inp.threshold <= f32_at(rec, o) {
                        put_f32(
                            rec,
                            o,
                            f32r(f64::from(f32_at(rec, o)) - f64::from(inp.threshold)) as f32,
                        );
                    }
                }
            }
            spawn_lot(rec, lots, slot.output, env, log);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{Slot, SlotInput};
    use rt2_formats::map::PoolRecord;

    const NONE: SlotInput = SlotInput {
        cargo: NO_CARGO,
        need: 0,
        threshold: 0.0,
        index: 0,
        weight: 0.0,
    };

    /// One primary slot of `rate` lots per year producing cargo 5.
    fn mine(rate: f32) -> IndustryType {
        let slot = |out: u8, rate: f32| Slot {
            year_from: if out == NO_CARGO { 0 } else { 1800 },
            year_to: if out == NO_CARGO { 0 } else { 9999 },
            mode: 1,
            output: out,
            rate,
            inputs: [NONE, NONE],
        };
        IndustryType {
            slots: [
                slot(5, rate),
                slot(NO_CARGO, 0.0),
                slot(NO_CARGO, 0.0),
                slot(NO_CARGO, 0.0),
            ],
            inputs: [NO_CARGO; 8],
            weights: [0; 8],
        }
    }

    fn lots() -> Pool {
        Pool {
            slots: 40,
            record_size: 0x15,
            linked: false,
            field_00: 0,
            field_04: 0,
            field_0c: 0,
            count: 0,
            pool_type: 11,
            head: 0,
            tail: 0,
            records: Vec::<PoolRecord>::new(),
            order: Vec::new(),
            bitmap: {
                let mut b = vec![0xFFu8; 6];
                b[0] &= !1;
                b
            },
            next: Vec::new(),
            prev: Vec::new(),
        }
    }

    fn record(id: u16, clock: u32) -> Vec<u8> {
        let mut r = vec![0u8; 0x9f];
        put_u16(&mut r, OFF_ID, id);
        r[OFF_TYPE] = 0; // type 0 of the test table
        put_u32(&mut r, OFF_CLOCKS, clock);
        r
    }

    fn env<'a>(types: &'a [IndustryType], effects: &'a EffectTable, now: i32) -> Env<'a> {
        Env {
            types,
            now,
            year: 1870,
            basic_model: false,
            sandbox: false,
            overtime: false,
            econ_factor: 1.0,
            effects,
        }
    }

    #[test]
    fn one_lot_is_made_per_elapsed_period_and_the_clock_advances() {
        // 2 lots per year: a period of 483840 / 2 = 241920 minutes.
        let types = [mine(2.0)];
        let fx = EffectTable::default();
        let mut log = HookLog::default();
        let mut pool = lots();
        let mut rec = record(7, 1_000_000);
        // 2.5 periods elapsed: the loop makes one lot per turn, so two lots, 0.5 period left.
        let now = 1_000_000 + 241_920 * 5 / 2;
        production_tick(&mut rec, &mut pool, &env(&types, &fx, now), &mut log);
        assert_eq!(pool.count, 2);
        assert_eq!(u32_at(&rec, OFF_CLOCKS), 1_000_000 + 2 * 241_920);
        assert_eq!(i16_at(&rec, OFF_PILE_COUNT), 2);
        assert_eq!(rec[OFF_PRODUCED], 2);
        // The pile is a stack: the head is the newest lot and points at the older one.
        let head = usize::from(u16_at(&rec, OFF_PILE));
        let top = crate::pool::record(&pool, head).unwrap();
        assert_eq!(top.data[LOT_CARGO], 5);
        assert_eq!(u16_at(&top.data, LOT_INDUSTRY), 7);
        assert_eq!(u32_at(&top.data, LOT_CREATED), now as u32);
        assert_eq!(u16_at(&top.data, LOT_NEXT), 1);
    }

    #[test]
    fn a_clock_in_the_future_is_pulled_back_and_nothing_is_made() {
        let types = [mine(2.0)];
        let fx = EffectTable::default();
        let mut log = HookLog::default();
        let mut pool = lots();
        let mut rec = record(7, 2_000_000);
        production_tick(&mut rec, &mut pool, &env(&types, &fx, 1_500_000), &mut log);
        assert_eq!(pool.count, 0);
        assert_eq!(u32_at(&rec, OFF_CLOCKS), 1_500_000);
    }

    #[test]
    fn the_pile_is_capped_at_fifty_lots_per_slot() {
        let types = [mine(2.0)];
        let fx = EffectTable::default();
        let mut log = HookLog::default();
        let mut pool = lots();
        let mut rec = record(7, 0);
        put_i16(&mut rec, OFF_PILE_COUNT, 50);
        production_tick(&mut rec, &mut pool, &env(&types, &fx, 241_920), &mut log);
        assert_eq!(pool.count, 0);
    }

    #[test]
    fn recount_counts_only_lots_without_a_station() {
        let types = [mine(2.0)];
        let fx = EffectTable::default();
        let mut log = HookLog::default();
        let mut pool = lots();
        let mut rec = record(7, 0);
        production_tick(
            &mut rec,
            &mut pool,
            &env(&types, &fx, 3 * 241_920),
            &mut log,
        );
        assert_eq!(pool.count, 3);
        // One lot was loaded by a station: it no longer counts.
        let first_lot = pool.records[0].slot;
        put_u16(&mut pool.records[0].data, LOT_STATION, 4);
        put_i16(&mut rec, OFF_PILE_COUNT, 99);
        recount_piles(&mut rec, &pool, &env(&types, &fx, 0));
        assert_eq!(
            i16_at(&rec, OFF_PILE_COUNT),
            2,
            "lot {first_lot} has a station"
        );
        assert_eq!(f32_at(&rec, OFF_RATING), 1.0);
    }
}
