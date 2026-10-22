//! Train rules (docs/spec/trains.md): segment grade, speed curve, target speed, acceleration
//! step, breakdown chance and monthly costs.
//!
//! Speeds are mph in 16.16 fixed point (`0x10000` = 1 mph). Functions take plain data; the
//! engine/vehicle tables come from the executable at run time ([`crate::data`]). The effect
//! modifiers are `effect_multiplier` results (see [`crate::effects`]) resolved by the caller.

use crate::fp::{f32r, ftol, pow};
use crate::rng::GameRng;

// ---- data tables ------------------------------------------------------------------------------

pub const ENGINE_RECORD_SIZE: usize = 0x3a;
pub const ENGINE_COUNT: usize = 66;
pub const VEHICLE_RECORD_SIZE: usize = 0x39;
pub const VEHICLE_COUNT: usize = 175;

/// Engine kind (`+0x28`).
pub const KIND_STEAM: u8 = 0;
pub const KIND_DIESEL: u8 = 1;
pub const KIND_ELECTRIC: u8 = 2;

/// Fields of an `engine_table` record (`0x53cec0`, trains.md 2.2) used by the simulation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Engine {
    pub enabled: bool,
    pub first_year: u16,
    pub last_year: u16,
    /// `+0x08`: reliability class 0..=10.
    pub reliability_class: i32,
    /// `+0x0c`
    pub price: i32,
    /// `+0x10`: base annual maintenance.
    pub maintenance: i32,
    /// `+0x14`
    pub top_mph: i32,
    /// `+0x18`
    pub power: i32,
    /// `+0x1c`: acceleration constant (lower = faster).
    pub accel: i32,
    /// `+0x20`
    pub free_tons: i32,
    /// `+0x24`: base fuel consumption.
    pub fuel: i32,
    /// `+0x28`: [`KIND_STEAM`], [`KIND_DIESEL`], [`KIND_ELECTRIC`].
    pub kind: u8,
    /// `+0x2e` (filled at map start by `0x471b10`): loaded tons of engine plus tender.
    pub base_tons: i32,
}

impl Engine {
    /// Parses a raw record; `base_tons` is left 0 (see [`crate::data::GameData`]).
    pub fn from_raw(r: &[u8]) -> Engine {
        let i32at = |k: usize| i32::from_le_bytes([r[k], r[k + 1], r[k + 2], r[k + 3]]);
        let u16at = |k: usize| u16::from_le_bytes([r[k], r[k + 1]]);
        Engine {
            enabled: r[0] != 0,
            first_year: u16at(1),
            last_year: u16at(3),
            reliability_class: i32at(0x08),
            price: i32at(0x0c),
            maintenance: i32at(0x10),
            top_mph: i32at(0x14),
            power: i32at(0x18),
            accel: i32at(0x1c),
            free_tons: i32at(0x20),
            fuel: i32at(0x24),
            kind: r[0x28],
            base_tons: 0,
        }
    }
}

/// Fields of a `vehicle_type_table` record (`0x53ddb8`, trains.md 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VehicleType {
    /// `+0x18`
    pub empty_tons: u16,
    /// `+0x1a`
    pub loaded_tons: u16,
    /// `+0x2e`: 0 engine, 1 tender, 2 car.
    pub species: u8,
    /// `+0x2f`: engine index (negative = none).
    pub engine: i8,
    /// `+0x30`: cargo (`0x7f` none).
    pub cargo: u8,
}

impl VehicleType {
    pub fn from_raw(r: &[u8]) -> VehicleType {
        VehicleType {
            empty_tons: u16::from_le_bytes([r[0x18], r[0x19]]),
            loaded_tons: u16::from_le_bytes([r[0x1a], r[0x1b]]),
            species: r[0x2e],
            engine: r[0x2f] as i8,
            cargo: r[0x30],
        }
    }
}

// ---- modifiers --------------------------------------------------------------------------------

/// Modifiers of the speed curve (trains.md 4.2): ids `0x2a`, `0x2b+t`, `0x23`, `0x24+t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedMods {
    /// `0x2a`: power, all engines.
    pub power_all: f64,
    /// `0x2b + kind`: power by engine kind.
    pub power_kind: f64,
    /// `0x23`: speed, all.
    pub speed_all: f64,
    /// `0x24 + kind`: speed by kind.
    pub speed_kind: f64,
}

impl SpeedMods {
    pub const NEUTRAL: SpeedMods = SpeedMods {
        power_all: 1.0,
        power_kind: 1.0,
        speed_all: 1.0,
        speed_kind: 1.0,
    };
}

// ---- grade ------------------------------------------------------------------------------------

/// `track_segment_grade` (`0x4284e0`, trains.md 4.1). `height(offset)` gives the vertex height
/// at a vertex offset relative to the tile origin, `corners` are `tile_corner_offsets`, and
/// `din`/`dout` the entry/exit directions 0..=7.
pub fn track_segment_grade(
    height: &dyn Fn(i32) -> i32,
    corners: [i32; 4],
    din: i32,
    dout: i32,
) -> f64 {
    let end = |d: i32| -> i32 {
        let k = d >> 1;
        if d & 1 != 0 {
            height(corners[((k + 1) & 3) as usize])
        } else {
            (height(corners[((k + 1) & 3) as usize]) + height(corners[k as usize])) >> 1
        }
    };
    let g = f64::from(end(dout) - end(din)) * 0.005;
    match (din & 1 != 0, dout & 1 != 0) {
        (false, false) => g,
        (true, true) => g * 0.7092,
        _ => g * 0.85,
    }
}

/// One of the first vehicles of a train, for [`train_grade`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VehicleGrade {
    pub on_track: bool,
    /// Vehicle weight in tons (`vehicle_weight`, `0x414460`).
    pub tons: i32,
    /// [`track_segment_grade`] of the vehicle's segment.
    pub grade: f64,
}

/// `train_grade` (`0x4cfd40`): weight-weighted grade of the first two vehicles on the track
/// (the slice may hold the whole train; only the first two are considered). 0 if the train is
/// not on the track.
pub fn train_grade(train_on_track: bool, vehicles: &[VehicleGrade]) -> f64 {
    if !train_on_track {
        return 0.0;
    }
    let (mut sum, mut acc) = (0i32, 0f64);
    for v in vehicles.iter().take(2) {
        if v.on_track {
            sum += v.tons;
            acc = f32r(v.grade * f64::from(v.tons) + acc);
        }
    }
    if sum == 0 {
        0.0
    } else {
        f32r(acc / f64::from(sum))
    }
}

/// `(20g + 1)^2.3` for uphill, `max(g, -0.03) * 10 + 1` otherwise (shared by 4.2 and 5.2).
fn grade_weight_multiplier(g: f64) -> f64 {
    if g > 0.0 {
        pow(g * 20.0 + 1.0, 2.3)
    } else {
        let g = if g < -0.03 { f64::from(-0.03f32) } else { g };
        g * 10.0 + 1.0
    }
}

// ---- speed curve ------------------------------------------------------------------------------

/// `engine_max_speed` (`0x4cfea0`, trains.md 4.2): max speed (16.16 mph) of `eng` hauling `tons`
/// on `grade`, on a `curve`, with the given train `age` in years.
pub fn engine_max_speed(
    eng: &Engine,
    tons: i32,
    grade: f32,
    curve: bool,
    age: i32,
    m: &SpeedMods,
) -> i32 {
    let age = age.min(50);
    let af = f32r(f64::from(age) * 0.005);
    let mut g = f64::from(grade);
    if g > 0.0 {
        g = f32r((g * 4.0 + 1.0) * g);
    }
    let w = if curve {
        ftol(f64::from(tons) * 1.4)
    } else {
        tons
    };
    let mult = grade_weight_multiplier(g);
    let mut eff = ftol(mult * f64::from(w)).wrapping_sub(eng.base_tons.wrapping_add(eng.free_tons));
    if eff < 0 {
        eff = 0;
    }
    let eff = ftol((af + 1.0) * f64::from(eff));
    let mut p = f32r(m.power_all * m.power_kind * f64::from(eng.power as f32) * 0.92);
    if p < 1.0 {
        p = 1.0;
    }
    let mut s = ftol(pow(0.98, f64::from(eff) / p) * f64::from(eng.top_mph << 16));
    s = ftol(f64::from(s) * (1.0 - af));
    if curve {
        s = ftol(f64::from(s) * 0.6);
    }
    s = ftol(m.speed_all * m.speed_kind * f64::from(s));
    s.max(0x10000)
}

/// Inputs of [`target_speed`] (trains.md 4.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetInput {
    /// Train total weight (`+0xaf`).
    pub tons: i32,
    /// [`train_grade`] of the train.
    pub grade: f64,
    /// Some vehicle on the track has `(entry - exit) & 3 != 0`.
    pub curve: bool,
    /// Train age in years (`year - +0xbd`).
    pub age: i32,
    /// `+0xcf`
    pub sand: f32,
    /// `+0xd7`
    pub water: f32,
    /// `+0xdb` throttle (85 = nominal).
    pub throttle: f32,
    /// Crashed, broken down, or the `+0xbc` stop state: the target is 0.
    pub stopped_by_state: bool,
    /// Global flag `0x574c08`: doubles the target.
    pub double_speed: bool,
    pub mods: SpeedMods,
}

/// `train_target_speed` (`0x4d00c0`, trains.md 4.3): `(target 16.16 mph, restricted)`.
///
/// TODO(trains.md "Em aberto", third item): structure classes 1 and 3 under the vehicles
/// (x0.45 / x0.7, which also set "restricted") are not modelled; the original `restricted`
/// output here is only the curve flag.
pub fn target_speed(eng: &Engine, t: &TargetInput) -> (i32, bool) {
    if t.stopped_by_state {
        return (0, false);
    }
    let mut g = t.grade;
    if f64::from(t.sand) <= 1.0 {
        g = f32r(g + g);
    }
    let mut s = engine_max_speed(eng, t.tons, g as f32, t.curve, t.age, &t.mods);
    s = ftol(f64::from(t.throttle) * f64::from(1.0f32 / 85.0) * f64::from(s));
    if eng.kind == KIND_STEAM && f64::from(t.water) <= 1.0 {
        s = ftol(f64::from(s) * 0.5);
    }
    if t.double_speed {
        s = s.wrapping_mul(2);
    }
    (s, t.curve)
}

/// Inputs of [`accel_step`] (trains.md 4.4, moving branch).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccelInput {
    /// Current speed (`+0x69`).
    pub speed: i32,
    pub target: i32,
    /// "Restricted" output of [`target_speed`].
    pub restricted: bool,
    /// Broken down or crashed (halves the time constant).
    pub broken: bool,
    /// `+0x6d`: minute of the last speed update (0 = none).
    pub last_update: i32,
    /// `game_time_minutes`.
    pub now: i32,
    /// Acceleration modifiers `0x2e` and `0x2f + kind`.
    pub accel_all: f64,
    pub accel_kind: f64,
}

/// `train_update_speed` (`0x4d0400`, trains.md 4.4, moving branch): the new speed. The caller
/// stores `now` as the new `+0x6d`. The result depends on the step size (time.md section 4).
pub fn accel_step(eng: &Engine, a: &AccelInput) -> i32 {
    let mut tau = eng.accel.wrapping_mul(1440);
    if a.speed > a.target {
        tau >>= if a.restricted { 4 } else { 1 };
    }
    if a.broken {
        tau >>= 1;
    }
    tau -= tau >> 2;
    let mut m = a.accel_all * a.accel_kind;
    if m < 0.1 {
        m = f64::from(0.1f32);
    }
    let tau2 = ftol(f64::from(tau) / m);
    let mut elapsed = if a.last_update != 0 {
        a.now.wrapping_sub(a.last_update)
    } else {
        0
    };
    elapsed = elapsed.min(tau2).max(0);
    let div = if a.target < 0 { tau2 / 2 } else { tau2 };
    let delta =
        ftol(f64::from(elapsed) / f64::from(div) * f64::from(a.target.wrapping_sub(a.speed)));
    a.speed.wrapping_add(delta)
}

// ---- breakdowns -------------------------------------------------------------------------------

/// Inputs of [`breakdown_chance`] (trains.md 5.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakdownInput {
    /// Cells travelled since the last check (`+0xa0`).
    pub tiles: i32,
    /// `check_age` argument: a new locomotive (age < 1) cannot break down.
    pub check_age: bool,
    /// Game year and the train's purchase year (`+0xbd`).
    pub year: u16,
    pub bought: u16,
    /// `reliability_rate[engine.+0x08]` (`0x55c52c`).
    pub reliability_rate: f32,
    /// `+0xdb`
    pub throttle: f32,
    /// `effect_multiplier(0x27)` (floor 0.01 applied here too).
    pub reliability_mod: f64,
    /// [`train_grade`] of the train.
    pub grade: f64,
    /// `+0xaf`
    pub tons: i32,
    /// Has a caboose (`+0xbb & 2`).
    pub caboose: bool,
    /// `+0xd3`
    pub oil: f32,
    /// `/nowreck` option or cheat with a single player: multiplies by 1e-4.
    pub nowreck: bool,
}

/// `train_breakdown_chance` (`0x4cf030`, trains.md 5.2), version >= 0x2334 rules (always true
/// in a local game).
pub fn breakdown_chance(eng: &Engine, b: &BreakdownInput) -> f64 {
    let age = i32::from(b.year) - i32::from(b.bought);
    if b.check_age && age < 1 {
        return 0.0;
    }
    let base = f32r(f64::from(b.tiles) * f64::from(b.reliability_rate));
    let thr = f64::from(b.throttle);
    let x = if thr > 85.0 {
        ((thr - 85.0) * 3.0 * (1.0 / 15.0) + 1.0) * base
    } else {
        base * thr * f64::from(1.0f32 / 85.0)
    };
    let x = f32r(x);
    let mut m = b.reliability_mod;
    if m < 0.01 {
        m = f64::from(0.01f32);
    }
    let x = f32r(x / m);
    let mult = grade_weight_multiplier(b.grade);
    let mut e =
        ftol(mult * f64::from(b.tons)).wrapping_sub(eng.base_tons.wrapping_add(eng.free_tons));
    if e < 0 {
        e = 0;
    }
    let mut y = (f64::from(e) + 150.0) * x * 0.005;
    if b.caboose {
        y *= 0.75;
    }
    y = y * f64::from(age + 30) * (1.0 / 30.0);
    if age <= 0 {
        y *= 0.3;
    } else if age <= 1 {
        y *= 0.6;
    }
    let oil = f64::from(b.oil);
    if oil < 1.0 {
        y *= 3.0;
    } else {
        y *= (100.0 - oil + 100.0) * 0.01;
    }
    let mut y = f32r(y * 0.7);
    if b.year >= 1970 {
        y = f32r(y * 0.7);
    } else if b.year >= 1920 {
        y = f32r(y * 0.85);
    }
    if b.nowreck {
        return y * 0.0001;
    }
    y
}

/// Result of a breakdown check roll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakdownOutcome {
    /// Nothing happened (the original then rolls for theft, see TODO on [`breakdown_roll`]).
    None,
    /// `train_breakdown` (`0x4ce4c0`): stopped for `rng_range(60, 120)` days.
    Breakdown,
    /// `train_crash` (`0x4ce730`): the train is destroyed.
    Crash,
}

/// The dice of `train_breakdown_check` (`0x4cf390`, trains.md 5.1) [S, no differential test].
///
/// `p` is `f32(breakdown_chance(...))`, already halved for a non-human company by the caller
/// (`p *= 0.5`, and 0 under flag `0x570229`). Draws from the game RNG in the original order:
/// `rng_range(0, 1000000)`, then `rng_range(0, 100)` only on a hit.
///
/// TODO(trains.md 6.4, "Em aberto"): the theft roll of the `None` branch needs town/station
/// proximity data and is not implemented, so the RNG sequence differs from the original there.
pub fn breakdown_roll(
    rng: &mut GameRng,
    p: f32,
    year: u16,
    crashes_enabled: bool,
) -> BreakdownOutcome {
    let roll = f64::from(rng.range(0, 1_000_000)) * 1e-6;
    if f64::from(p) > roll {
        let limit = if year >= 1970 {
            14
        } else if year >= 1920 {
            17
        } else {
            20
        };
        if rng.range(0, 100) < limit && crashes_enabled {
            BreakdownOutcome::Crash
        } else {
            BreakdownOutcome::Breakdown
        }
    } else {
        BreakdownOutcome::None
    }
}

// ---- costs ------------------------------------------------------------------------------------

/// `train_create` purchase price (`0x4cdf80`, trains.md 6.1) [S]: `ftol(mod(0x1a) * mod(0x1b+t) * price)`.
pub fn purchase_price(price_all: f64, price_kind: f64, eng: &Engine) -> i32 {
    ftol(price_all * price_kind * f64::from(eng.price))
}

/// Inputs of [`monthly_costs`] (trains.md 6.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonthlyInput {
    /// Train age in years.
    pub age: i32,
    /// `+0xa2`: ton x cells this month.
    pub ton_cells: i32,
    /// `+0xe1`: cells this month.
    pub cells: i32,
    /// `+0xd3` (100 for non-human companies).
    pub oil: f32,
    /// Number of vehicles whose cargo field is not `0x7f`.
    pub cargo_vehicles: i32,
    /// Fuel price level `0x552ae8[0x567d6d[t]]` and maintenance level `0x552af4[...]`.
    pub fuel_level: f32,
    pub maint_level: f32,
    /// Modifiers `0x12+t`, `0x11`, `0x1b+t`, `0x15`, `0x19`.
    pub fuel_kind: f64,
    pub fuel_all: f64,
    pub maint_kind: f64,
    pub maint_all: f64,
    pub car_cost: f64,
}

/// Monthly ledger entries of a train (negative numbers are expenses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonthlyCosts {
    pub fuel: i32,
    pub maintenance: i32,
    pub cars: i32,
}

/// `train_monthly_costs` (`0x4d4390`, trains.md 6.2) [S, no differential test]. The caller
/// resets `+0xa2` and `+0xe1` afterwards.
pub fn monthly_costs(eng: &Engine, m: &MonthlyInput) -> MonthlyCosts {
    let unit = m.ton_cells.wrapping_add(m.cells.wrapping_mul(200));
    let fa = f32r(f64::from(m.age + 100) * 0.01);
    let fuel = ftol(
        m.fuel_kind
            * m.fuel_all
            * fa
            * fa
            * f64::from(-unit)
            * f64::from(m.fuel_level)
            * f64::from(eng.fuel)
            * 0.005
            * 0.35,
    );
    let age_f = f32r((f64::from(m.age * 4 + 100) * 0.01) * (f64::from(m.age * 4 + 100) * 0.01));
    let oil_mult = if f64::from(m.oil) <= 1.0 { 3.0 } else { 1.0 };
    let maintenance = ftol(
        m.maint_kind
            * m.maint_all
            * f64::from(m.maint_level)
            * f64::from(eng.maintenance)
            * oil_mult
            * age_f
            * (-1.0 / 12.0),
    );
    let cars = ftol(m.car_cost * f64::from(-200 * m.cargo_vehicles));
    MonthlyCosts {
        fuel,
        maintenance,
        cars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic engine (not game data): 100 mph, power 1000, 10 t base.
    fn engine() -> Engine {
        Engine {
            enabled: true,
            first_year: 1800,
            last_year: 9999,
            reliability_class: 5,
            price: 100_000,
            maintenance: 12_000,
            top_mph: 60,
            power: 1000,
            accel: 30,
            free_tons: 0,
            fuel: 100,
            kind: KIND_DIESEL,
            base_tons: 100,
        }
    }

    #[test]
    fn flat_unloaded_train_reaches_top_speed() {
        // eff = 0 -> 0.98^0 * (60 << 16); no age
        let s = engine_max_speed(&engine(), 100, 0.0, false, 0, &SpeedMods::NEUTRAL);
        assert_eq!(s, 60 << 16);
        // age costs 0.5% per year (to 25%)
        let s10 = engine_max_speed(&engine(), 100, 0.0, false, 10, &SpeedMods::NEUTRAL);
        assert_eq!(s10, ftol(f64::from(60 << 16) * (1.0 - f32r(10.0 * 0.005))));
        // curve costs 40%
        let sc = engine_max_speed(&engine(), 72, 0.0, true, 0, &SpeedMods::NEUTRAL);
        assert_eq!(sc, ftol(f64::from(60 << 16) * 0.6));
        // never below 1 mph
        let slow = engine_max_speed(&engine(), 1_000_000, 0.1, true, 50, &SpeedMods::NEUTRAL);
        assert_eq!(slow, 0x10000);
    }

    #[test]
    fn heavier_and_uphill_is_slower() {
        let e = engine();
        let a = engine_max_speed(&e, 400, 0.0, false, 0, &SpeedMods::NEUTRAL);
        let b = engine_max_speed(&e, 800, 0.0, false, 0, &SpeedMods::NEUTRAL);
        let c = engine_max_speed(&e, 400, 0.04, false, 0, &SpeedMods::NEUTRAL);
        assert!(a > b && a > c);
    }

    #[test]
    fn grade_of_straight_segment() {
        // heights 0 at corner offsets 0,1 and 2 at 2,3: entering at direction 0 (edge between
        // corners 0 and 1) and leaving at 4 (edge between corners 2 and 3)
        let h = |off: i32| if off >= 2 { 2 } else { 0 };
        let g = track_segment_grade(&h, [0, 1, 2, 3], 0, 4);
        assert_eq!(g, 2.0 * 0.005);
        // one odd direction: x0.85; both odd: x0.7092
        let d = track_segment_grade(&h, [0, 1, 2, 3], 1, 4);
        assert_eq!(d, f64::from(2) * 0.005 * 0.85);
    }

    #[test]
    fn train_grade_weights_first_two_vehicles() {
        let v = |tons, grade| VehicleGrade {
            on_track: true,
            tons,
            grade,
        };
        assert_eq!(train_grade(false, &[v(10, 0.1)]), 0.0);
        let g = train_grade(true, &[v(100, 0.02), v(100, 0.04), v(100, 0.9)]);
        assert!((g - 0.03).abs() < 1e-6);
    }

    #[test]
    fn target_and_accel_follow_the_spec() {
        let e = engine();
        let t = TargetInput {
            tons: 100,
            grade: 0.0,
            curve: false,
            age: 0,
            sand: 100.0,
            water: 100.0,
            throttle: 85.0,
            stopped_by_state: false,
            double_speed: false,
            mods: SpeedMods::NEUTRAL,
        };
        let (target, restricted) = target_speed(&e, &t);
        assert_eq!(
            target,
            ftol(f64::from(85.0f32) * f64::from(1.0f32 / 85.0) * f64::from(60 << 16))
        );
        assert!(!restricted);
        assert_eq!(
            target_speed(
                &e,
                &TargetInput {
                    stopped_by_state: true,
                    ..t
                }
            ),
            (0, false)
        );
        // accelerating from standstill for a full time constant closes the whole gap
        let tau = e.accel * 1440 - ((e.accel * 1440) >> 2);
        let new = accel_step(
            &e,
            &AccelInput {
                speed: 0,
                target,
                restricted,
                broken: false,
                last_update: 1000,
                now: 1000 + tau,
                accel_all: 1.0,
                accel_kind: 1.0,
            },
        );
        assert_eq!(new, target);
        // braking is twice as fast: tau halves
        let braking = accel_step(
            &e,
            &AccelInput {
                speed: target,
                target: 0,
                restricted: false,
                broken: false,
                last_update: 1000,
                now: 1000 + tau,
                accel_all: 1.0,
                accel_kind: 1.0,
            },
        );
        assert_eq!(braking, 0);
    }

    #[test]
    fn rocket_time_constant_from_spec() {
        // trains.md 4.4: tau = accel * 1440 * 0.75; Rocket (accel 50) -> 54000 minutes
        let mut e = engine();
        e.accel = 50;
        let mut tau = e.accel * 1440;
        tau -= tau >> 2;
        assert_eq!(tau, 54_000);
    }

    fn breakdown_input() -> BreakdownInput {
        BreakdownInput {
            tiles: 10,
            check_age: true,
            year: 1900,
            bought: 1890,
            reliability_rate: 1.0 / 1400.0,
            throttle: 85.0,
            reliability_mod: 1.0,
            grade: 0.0,
            tons: 100,
            caboose: false,
            oil: 100.0,
            nowreck: false,
        }
    }

    #[test]
    fn breakdown_chance_rules() {
        let e = engine();
        let b = breakdown_input();
        let p = breakdown_chance(&e, &b);
        assert!(p > 0.0 && p < 1.0);
        // a new locomotive does not break down in its first year
        assert_eq!(
            breakdown_chance(&e, &BreakdownInput { bought: 1900, ..b }),
            0.0
        );
        // caboose -25% (before the final f32 roundings)
        let pc = breakdown_chance(&e, &BreakdownInput { caboose: true, ..b });
        assert!((pc / p - 0.75).abs() < 1e-5);
        // /nowreck divides by 10 000
        let pn = breakdown_chance(&e, &BreakdownInput { nowreck: true, ..b });
        assert!((pn / p - 1e-4).abs() < 1e-9);
        // empty oil triples
        let po = breakdown_chance(&e, &BreakdownInput { oil: 0.5, ..b });
        assert!((po / p - 3.0).abs() < 1e-4);
        // max throttle quadruples the base
        let pt = breakdown_chance(
            &e,
            &BreakdownInput {
                throttle: 100.0,
                ..b
            },
        );
        assert!((pt / p - 4.0 * 85.0 / 85.0).abs() < 1e-3);
    }

    #[test]
    fn breakdown_roll_is_deterministic_and_ordered() {
        let mut a = GameRng::new(99);
        let mut b = GameRng::new(99);
        let out = breakdown_roll(&mut a, 1.0, 1900, true);
        // p = 1 always hits: draws range(0,1e6) then range(0,100)
        let _ = b.range(0, 1_000_000);
        let c = b.range(0, 100);
        assert_eq!(a.state(), b.state());
        assert_eq!(out == BreakdownOutcome::Crash, c < 20);
        // p = 0 never hits and draws exactly one value
        let mut z = GameRng::new(99);
        assert_eq!(
            breakdown_roll(&mut z, 0.0, 1900, true),
            BreakdownOutcome::None
        );
        let mut one = GameRng::new(99);
        one.range(0, 1_000_000);
        assert_eq!(z.state(), one.state());
    }

    #[test]
    fn monthly_costs_are_expenses() {
        let e = engine();
        let c = monthly_costs(
            &e,
            &MonthlyInput {
                age: 10,
                ton_cells: 5000,
                cells: 100,
                oil: 100.0,
                cargo_vehicles: 5,
                fuel_level: 1.0,
                maint_level: 1.0,
                fuel_kind: 1.0,
                fuel_all: 1.0,
                maint_kind: 1.0,
                maint_all: 1.0,
                car_cost: 1.0,
            },
        );
        assert!(c.fuel < 0 && c.maintenance < 0);
        assert_eq!(c.cars, -1000);
        // maintenance grows with (1 + 0.04 age)^2 and triples without oil (trains.md 6.2)
        let base = MonthlyInput {
            age: 0,
            ton_cells: 0,
            cells: 0,
            oil: 100.0,
            cargo_vehicles: 0,
            fuel_level: 1.0,
            maint_level: 1.0,
            fuel_kind: 1.0,
            fuel_all: 1.0,
            maint_kind: 1.0,
            maint_all: 1.0,
            car_cost: 1.0,
        };
        let m0 = monthly_costs(&e, &base).maintenance;
        assert_eq!(m0, -1000); // 12000 / 12
        let dry = monthly_costs(&e, &MonthlyInput { oil: 0.0, ..base }).maintenance;
        assert_eq!(dry, -3000);
        assert_eq!(purchase_price(1.0, 1.0, &e), 100_000);
    }
}
