//! Differential tests for the train simulation (docs/spec/trains.md): grade of a track
//! segment, the speed curve of a locomotive, one acceleration step and the breakdown
//! probability. Each test builds the inputs in the emulated memory, runs the original
//! function and compares it with a Rust reference. Skipped when the executable is absent.

use rt2_oracle::Oracle;
use rt2_sim::data::GameData;
use rt2_sim::effects::global_modifier;
use rt2_sim::exe::Exe;
use rt2_sim::rng::GameRng;
use rt2_sim::track::tile_distance;
use rt2_sim::trains::{
    AccelInput, BreakdownInput, CheckInput, CheckOutcome, Engine, MonthlyInput, SpeedMods,
    TargetInput, TheftInput, VehicleGrade, accel_step, breakdown_chance, breakdown_check,
    engine_max_speed, monthly_costs, purchase_price, target_speed, town_excess,
    track_segment_grade, train_grade,
};
use std::cell::Cell;
use std::rc::Rc;
use unicorn_engine::RegisterX86;

fn oracle() -> Option<Oracle> {
    let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    });
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return None;
    }
    Some(Oracle::load_file(&path).expect("load exe"))
}

/// Fixed-seed xorshift32 used only to generate test inputs.
struct Xs(u32);
impl Xs {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
}

// ---- addresses ---------------------------------------------------------------------------------

const GRADE_FN: u32 = 0x0042_84e0; // fastcall(ECX = height ptr, EDX = dir in, [dir out]) -> ST0
const SPEED_FN: u32 = 0x004c_fea0; // fastcall(ECX = engine, EDX = tons, [grade f32, curve, age, company, player])
const ACCEL_FN: u32 = 0x004d_0400; // thiscall(ECX = train, [now])
const BREAKDOWN_P: u32 = 0x004c_f030; // thiscall(ECX = train, [tiles, check_age]) -> ST0

const ENGINES: u32 = 0x0053_cec0; // 66 records of 0x3a bytes
const ENGINE_COUNT: u32 = 66;
const VEHICLES: u32 = 0x0053_ddb8; // 175 records of 0x39 bytes
const VEHICLE_COUNT: u32 = 175;
const MODIFIERS: u32 = 0x0057_07c6; // i16 percent per modifier id
const GAME_DATE: u32 = 0x0057_04f8; // u16 year, u8 month, u8 day
const VEHICLE_LIST: u32 = 0x0057_4c70; // pointer to the vehicle collection
const TILES: u32 = 0x0057_4c24; // pointer to the tile array (0xf bytes per tile)
const TILES_W: u32 = 0x0055_e7a4;
const HEIGHTS: u32 = 0x0057_4c28; // pointer to the vertex height array (3 bytes per vertex)
const HEIGHTS_W: u32 = 0x0056_09ac;
const CORNERS: u32 = 0x0058_0fd8; // i32[4] vertex offsets of the tile corners
const RELIABILITY: u32 = 0x0055_c52c; // f32[11] breakdowns per tile by reliability class
const DOUBLE_SPEED: u32 = 0x0057_4c08;
const NOWRECK: u32 = 0x0057_4b74;

const MAP_W: i32 = 16;

// ---- x87 / CRT helpers -------------------------------------------------------------------------

/// Empties the x87 stack and sets 53-bit precision (MSVC CRT default), as in time.md.
fn fpu_reset(o: &mut Oracle) {
    let uc = o.uc();
    uc.reg_write(RegisterX86::FPCW, 0x27f).unwrap();
    uc.reg_write(RegisterX86::FPSW, 0).unwrap();
    uc.reg_write(RegisterX86::FPTAG, 0xffff).unwrap();
}

fn rd_i32(o: &Oracle, a: u32) -> i32 {
    o.read_u32(a).unwrap() as i32
}

fn rd_u8(o: &Oracle, a: u32) -> u8 {
    o.read_mem(a, 1).unwrap()[0]
}

fn rd_u16(o: &Oracle, a: u32) -> u16 {
    let b = o.read_mem(a, 2).unwrap();
    u16::from_le_bytes([b[0], b[1]])
}

fn rd_f32(o: &Oracle, a: u32) -> f32 {
    f32::from_bits(o.read_u32(a).unwrap())
}

fn sim_data() -> GameData {
    let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    });
    GameData::load(&Exe::from_bytes(std::fs::read(path).expect("read exe")).expect("parse exe"))
        .expect("load tables")
}

// ---- game data (read from the emulated .data, never typed in) ----------------------------------

/// rt2-sim's view of engine `e`: the record parsed by `Engine::from_raw` plus the `+0x2e` weight
/// that the oracle memory holds after `link_engines`.
fn engine(o: &Oracle, e: u32) -> Engine {
    let r = o.read_mem(ENGINES + e * 0x3a, ENGINE_RECORD).unwrap();
    let mut eng = Engine::from_raw(&r);
    eng.base_tons = rd_i32(o, ENGINES + e * 0x3a + 0x2e);
    eng
}

const ENGINE_RECORD: usize = 0x3a;

/// Vehicle type record: (kind 0 engine / 1 tender / 2 car, engine index, empty t, loaded t).
fn vehicle_type(o: &Oracle, t: u32) -> (u8, i8, i32, i32) {
    let r = VEHICLES + t * 0x39;
    (
        rd_u8(o, r + 0x2e),
        rd_u8(o, r + 0x2f) as i8,
        i32::from(rd_u16(o, r + 0x18)),
        i32::from(rd_u16(o, r + 0x1a)),
    )
}

/// What 0x471b10 does at map start: engine +0x2e = loaded tons of its engine vehicle plus
/// its tender. Returns, per engine, the engine vehicle type and the tender type.
fn link_engines(o: &mut Oracle) -> Vec<(u32, Option<u32>)> {
    let mut out = vec![(0u32, None); ENGINE_COUNT as usize];
    for t in 0..VEHICLE_COUNT {
        let (kind, e, _, _) = vehicle_type(o, t);
        if e < 0 {
            continue;
        }
        match kind {
            0 => out[e as usize].0 = t,
            1 => out[e as usize].1 = Some(t),
            _ => {}
        }
    }
    // rt2-sim's loader must derive the same weights from the executable file
    let sim = sim_data();
    for (e, &(ev, tv)) in out.iter().enumerate() {
        let mut w = vehicle_type(o, ev).3;
        if let Some(tv) = tv {
            w += vehicle_type(o, tv).3;
        }
        o.write_u32(ENGINES + e as u32 * 0x3a + 0x2e, w as u32)
            .unwrap();
        assert_eq!(sim.engines[e].base_tons, w, "engine {e} +0x2e");
    }
    out
}

fn car_types(o: &Oracle) -> Vec<u32> {
    (0..VEHICLE_COUNT)
        .filter(|&t| vehicle_type(o, t).0 == 2)
        .collect()
}

/// `0x46b480` with company = 0 and player = 0: only the global percent applies.
fn modifier(o: &Oracle, id: u32) -> f64 {
    global_modifier(rd_u16(o, MODIFIERS + 2 * id) as i16)
}

fn speed_mods(o: &Oracle, kind: u8) -> SpeedMods {
    SpeedMods {
        power_all: modifier(o, 0x2a),
        power_kind: modifier(o, u32::from(kind) + 0x2b),
        speed_all: modifier(o, 0x23),
        speed_kind: modifier(o, u32::from(kind) + 0x24),
    }
}

fn randomize_modifiers(o: &mut Oracle, xs: &mut Xs) {
    for id in 0x11..0x34u32 {
        let pct: i16 = match xs.below(3) {
            0 => 0,
            1 => xs.range(-60, 60) as i16,
            _ => xs.range(-95, 250) as i16,
        };
        o.write_mem(MODIFIERS + 2 * id, &pct.to_le_bytes()).unwrap();
    }
}

// ---- references --------------------------------------------------------------------------------

/// `0x4284e0` through rt2-sim (the implementation under test).
fn grade_ref(h: &dyn Fn(i32) -> i32, corners: [i32; 4], din: i32, dout: i32) -> f64 {
    track_segment_grade(h, corners, din, dout)
}

/// `0x4cfea0` through rt2-sim: maximum speed (mph, 16.16) of engine `e` hauling `tons` on `grade`.
fn speed_ref(o: &Oracle, e: u32, tons: i32, grade: f32, curve: bool, age: i32) -> i32 {
    let eng = engine(o, e);
    engine_max_speed(&eng, tons, grade, curve, age, &speed_mods(o, eng.kind))
}

#[derive(Clone)]
struct Veh {
    typ: u32,
    x: i32,
    y: i32,
    din: u8,
    dout: u8,
    on_track: bool,
    load: i32,
}

#[derive(Clone)]
struct Train {
    engine: u32,
    vehs: Vec<Veh>,
    speed: i32,
    last: i32,
    tons: i32,
    flags_bb: u8,
    bought: u16,
    on_track: bool,
    sand: f32,
    oil: f32,
    water: f32,
    throttle: f32,
}

struct World {
    heights: Vec<i16>,
    corners: [i32; 4],
    year: u16,
    double_speed: bool,
    nowreck: bool,
}

impl World {
    fn veh_tons(&self, o: &Oracle, v: &Veh) -> i32 {
        let (_, _, empty, loaded) = vehicle_type(o, v.typ);
        if v.load == 0 { empty } else { loaded }
    }
    fn veh_grade(&self, v: &Veh) -> f64 {
        let base = v.y * (MAP_W + 1) + v.x;
        let h = |off: i32| i32::from(self.heights[(base + off) as usize]);
        grade_ref(&h, self.corners, i32::from(v.din), i32::from(v.dout))
    }
    /// Weighted grade of the first two vehicles (0x4d00c0 / 0x4cf030 / 0x4cfd40), via rt2-sim.
    fn train_grade(&self, o: &Oracle, t: &Train) -> f64 {
        let vs: Vec<VehicleGrade> = t
            .vehs
            .iter()
            .map(|v| VehicleGrade {
                on_track: v.on_track,
                tons: self.veh_tons(o, v),
                grade: self.veh_grade(v),
            })
            .collect();
        train_grade(t.on_track, &vs)
    }
}

/// `0x4d00c0` through rt2-sim: target speed (16.16 mph) and the "restricted" flag (curve).
fn target_ref(o: &Oracle, w: &World, t: &Train) -> (i32, bool) {
    let curve = t
        .vehs
        .iter()
        .filter(|v| v.on_track)
        .any(|v| (i32::from(v.din) - i32::from(v.dout)) & 3 != 0);
    let eng = engine(o, t.engine);
    target_speed(
        &eng,
        &TargetInput {
            tons: t.tons,
            grade: w.train_grade(o, t),
            curve,
            age: i32::from(w.year) - i32::from(t.bought),
            sand: t.sand,
            water: t.water,
            throttle: t.throttle,
            stopped_by_state: false,
            double_speed: w.double_speed,
            mods: speed_mods(o, eng.kind),
        },
    )
}

/// `0x4d0400`, moving branch, through rt2-sim: new speed after advancing to `now`.
fn accel_ref(o: &Oracle, w: &World, t: &Train, now: i32) -> i32 {
    let eng = engine(o, t.engine);
    let (target, restricted) = target_ref(o, w, t);
    accel_step(
        &eng,
        &AccelInput {
            speed: t.speed,
            target,
            restricted,
            broken: false,
            last_update: t.last,
            now,
            accel_all: modifier(o, 0x2e),
            accel_kind: modifier(o, u32::from(eng.kind) + 0x2f),
        },
    )
}

/// `0x4cf030` through rt2-sim: breakdown probability for `tiles` tiles travelled.
fn breakdown_ref(o: &Oracle, w: &World, t: &Train, tiles: i32, check_age: bool) -> f64 {
    let eng = engine(o, t.engine);
    breakdown_chance(
        &eng,
        &BreakdownInput {
            tiles,
            check_age,
            year: w.year,
            bought: t.bought,
            reliability_rate: rd_f32(o, RELIABILITY + 4 * eng.reliability_class as u32),
            throttle: t.throttle,
            reliability_mod: modifier(o, 0x27),
            grade: w.train_grade(o, t),
            tons: t.tons,
            caboose: t.flags_bb & 2 != 0,
            oil: t.oil,
            nowreck: w.nowreck,
        },
    )
}

// ---- emulated world ----------------------------------------------------------------------------

const VEH_STRIDE: u32 = 0x60;
const MAX_VEHS: u32 = 8;

struct Mem {
    vehs: u32,
    heights: u32,
    train: u32,
}

fn setup_world(o: &mut Oracle) -> Mem {
    let coll = o.alloc(0x40).unwrap();
    let vehs = o.alloc(VEH_STRIDE * (MAX_VEHS + 1)).unwrap();
    o.write_u32(coll + 8, VEH_STRIDE).unwrap();
    o.write_u32(coll + 0x28, vehs).unwrap();
    o.write_u32(VEHICLE_LIST, coll).unwrap();
    let tiles = o.alloc((MAP_W * MAP_W * 0xf) as u32).unwrap();
    o.write_u32(TILES, tiles).unwrap();
    o.write_u32(TILES_W, MAP_W as u32).unwrap();
    let hw = MAP_W + 1;
    let heights = o.alloc((hw * hw * 3) as u32).unwrap();
    o.write_u32(HEIGHTS, heights).unwrap();
    o.write_u32(HEIGHTS_W, hw as u32).unwrap();
    let corners = [0, 1, hw + 1, hw];
    for (k, c) in corners.iter().enumerate() {
        o.write_u32(CORNERS + 4 * k as u32, *c as u32).unwrap();
    }
    let train = o.alloc(0x200).unwrap();
    Mem {
        vehs,
        heights,
        train,
    }
}

fn random_world(o: &mut Oracle, m: &Mem, xs: &mut Xs) -> World {
    let hw = MAP_W + 1;
    let spread = [2, 6, 14][xs.below(3) as usize];
    let heights: Vec<i16> = (0..hw * hw).map(|_| xs.range(0, spread) as i16).collect();
    for (i, h) in heights.iter().enumerate() {
        o.write_mem(m.heights + 3 * i as u32, &h.to_le_bytes())
            .unwrap();
    }
    let year = xs.range(1830, 2030) as u16;
    let date = u32::from(year) | (u32::from(xs.range(1, 12) as u8) << 16) | (1 << 24);
    o.write_u32(GAME_DATE, date).unwrap();
    let double_speed = xs.below(8) == 0;
    o.write_u32(DOUBLE_SPEED, u32::from(double_speed)).unwrap();
    let nowreck = xs.below(8) == 0;
    o.write_u32(NOWRECK, u32::from(nowreck)).unwrap();
    World {
        heights,
        corners: [0, 1, hw + 1, hw],
        year,
        double_speed,
        nowreck,
    }
}

fn random_dir_pair(xs: &mut Xs) -> (u8, u8) {
    let din = xs.below(8) as u8;
    let dout = match xs.below(4) {
        0 | 1 => (din + 4) & 7,                        // straight
        2 => (din + [3, 5][xs.below(2) as usize]) & 7, // gentle curve
        _ => xs.below(8) as u8,
    };
    (din, dout)
}

fn random_train(
    o: &Oracle,
    links: &[(u32, Option<u32>)],
    cars: &[u32],
    w: &World,
    xs: &mut Xs,
) -> Train {
    let e = xs.below(ENGINE_COUNT);
    let mut types = vec![links[e as usize].0];
    if let Some(t) = links[e as usize].1 {
        types.push(t);
    }
    for _ in 0..xs.below(MAX_VEHS + 1 - types.len() as u32) {
        types.push(cars[xs.below(cars.len() as u32) as usize]);
    }
    let vehs: Vec<Veh> = types
        .iter()
        .map(|&typ| {
            let (din, dout) = random_dir_pair(xs);
            Veh {
                typ,
                x: xs.range(0, MAP_W - 1),
                y: xs.range(0, MAP_W - 1),
                din,
                dout,
                on_track: xs.below(6) != 0,
                load: if xs.below(2) == 0 { 0 } else { xs.range(1, 40) },
            }
        })
        .collect();
    let mut t = Train {
        engine: e,
        vehs,
        speed: 0,
        last: 0,
        tons: 0,
        flags_bb: [0u8, 2][xs.below(2) as usize],
        bought: (i32::from(w.year) - xs.range(-1, 60)) as u16,
        on_track: xs.below(5) != 0,
        sand: [100.0, 50.0, 1.0, 0.0][xs.below(4) as usize],
        oil: [100.0, 63.5, 1.0, 0.5][xs.below(4) as usize],
        water: [100.0, 30.0, 1.0, 0.0][xs.below(4) as usize],
        throttle: [85.0, 100.0, 42.5, 60.0, 93.0][xs.below(5) as usize],
    };
    t.tons = if xs.below(4) == 0 {
        xs.range(0, 1500)
    } else {
        t.vehs.iter().map(|v| w.veh_tons(o, v)).sum()
    };
    t.speed = xs.range(0, 120 << 16);
    t.last = if xs.below(6) == 0 {
        0
    } else {
        xs.range(1, 1 << 24)
    };
    t
}

fn write_train(o: &mut Oracle, m: &Mem, t: &Train) {
    let n = t.vehs.len() as u32;
    for (i, v) in t.vehs.iter().enumerate() {
        let id = i as u32 + 1;
        let a = m.vehs + id * VEH_STRIDE;
        o.write_mem(a, &[0u8; VEH_STRIDE as usize]).unwrap();
        let next: i16 = if id < n { id as i16 + 1 } else { 0 };
        o.write_mem(a + 2, &next.to_le_bytes()).unwrap();
        o.write_mem(a + 8, &[v.typ as u8]).unwrap();
        o.write_mem(a + 9, &(v.x as i16).to_le_bytes()).unwrap();
        o.write_mem(a + 0xb, &(v.y as i16).to_le_bytes()).unwrap();
        o.write_mem(a + 0x23, &[v.din, v.dout]).unwrap();
        o.write_mem(a + 0x2a, &[u8::from(v.on_track)]).unwrap();
        o.write_u32(a + 0x2d, v.load as u32).unwrap();
    }
    let a = m.train;
    o.write_mem(a, &[0u8; 0x200]).unwrap();
    o.write_u32(a, 1).unwrap(); // head vehicle id
    o.write_u32(a + 0x69, t.speed as u32).unwrap();
    o.write_u32(a + 0x6d, t.last as u32).unwrap();
    o.write_mem(a + 0x9c, &[u8::from(t.on_track)]).unwrap();
    o.write_u32(a + 0xaf, t.tons as u32).unwrap();
    o.write_mem(a + 0xbb, &[t.flags_bb]).unwrap();
    o.write_mem(a + 0xbd, &t.bought.to_le_bytes()).unwrap();
    for (off, v) in [
        (0xcf, t.sand),
        (0xd3, t.oil),
        (0xd7, t.water),
        (0xdb, t.throttle),
    ] {
        o.write_u32(a + off, v.to_bits()).unwrap();
    }
}

// ---- tests -------------------------------------------------------------------------------------

#[test]
fn track_grade_matches_original() {
    let Some(mut o) = oracle() else { return };
    let m = setup_world(&mut o);
    let mut xs = Xs(0x6a09_e667);
    let mut w = random_world(&mut o, &m, &mut xs);
    for case in 0..20_000 {
        if case % 200 == 0 {
            w = random_world(&mut o, &m, &mut xs);
        }
        let v = Veh {
            typ: 0,
            x: xs.range(0, MAP_W - 1),
            y: xs.range(0, MAP_W - 1),
            din: xs.below(8) as u8,
            dout: xs.below(8) as u8,
            on_track: true,
            load: 0,
        };
        let ptr = m.heights + 3 * (v.y * (MAP_W + 1) + v.x) as u32;
        fpu_reset(&mut o);
        o.call_fastcall(GRADE_FN, ptr, u32::from(v.din), &[u32::from(v.dout)])
            .unwrap();
        let got = o.st0().unwrap();
        let want = w.veh_grade(&v);
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "case {case}: got {got} want {want}"
        );
    }
}

#[test]
fn engine_speed_curve_matches_original() {
    let Some(mut o) = oracle() else { return };
    link_engines(&mut o);
    let mut xs = Xs(0xbb67_ae85);
    let mut checked = 0;
    for case in 0..30_000 {
        if case % 500 == 0 {
            randomize_modifiers(&mut o, &mut xs);
        }
        let e = xs.below(ENGINE_COUNT);
        let tons = match xs.below(3) {
            0 => xs.range(0, 400),
            1 => xs.range(0, 1500),
            _ => xs.range(0, 6000),
        };
        let grade = match xs.below(4) {
            0 => 0.0f32,
            1 => (xs.range(-30, 30) as f32) * 0.005,
            2 => {
                (xs.range(-12, 12) as f64 * 0.005 * [1.0, 0.85, 0.7092][xs.below(3) as usize])
                    as f32
            }
            _ => (xs.range(-2000, 2000) as f32) * 1e-4,
        };
        let curve = xs.below(3) == 0;
        let age = xs.range(-3, 90);
        fpu_reset(&mut o);
        let got = o
            .call_fastcall(
                SPEED_FN,
                e,
                tons as u32,
                &[grade.to_bits(), u32::from(curve), age as u32, 0, 0],
            )
            .unwrap() as i32;
        let want = speed_ref(&o, e, tons, grade, curve, age);
        assert_eq!(
            got, want,
            "case {case}: engine {e} tons {tons} grade {grade} curve {curve} age {age}"
        );
        checked += 1;
    }
    assert_eq!(checked, 30_000);
}

/// The base-level speed table of the strategy guide appendix (A-1) is this function with
/// the cargo tons added on top of engine + tender: spot-check a few rows.
#[test]
fn engine_speed_reproduces_guide_table() {
    let Some(mut o) = oracle() else { return };
    link_engines(&mut o);
    // (engine index, [0@0%, 150@0%, 300@0%, 150@4%, 300@4%]) from the appendix table.
    let rows: [(u32, [i32; 5]); 4] = [
        (0, [15, 6, 3, 1, 1]),       // Stephenson Rocket
        (19, [55, 50, 45, 31, 19]),  // 2-8-2 Mikado
        (24, [68, 67, 62, 48, 35]),  // 4-8-8-4 Big Boy
        (53, [100, 87, 74, 43, 20]), // GG1
    ];
    for (e, want) in rows {
        let base = engine(&o, e).base_tons;
        for (k, (tons, grade)) in [
            (0, 0.0f32),
            (150, 0.0),
            (300, 0.0),
            (150, 0.04),
            (300, 0.04),
        ]
        .into_iter()
        .enumerate()
        {
            fpu_reset(&mut o);
            let s = o
                .call_fastcall(
                    SPEED_FN,
                    e,
                    (base + tons) as u32,
                    &[grade.to_bits(), 0, 0, 0, 0],
                )
                .unwrap() as i32;
            assert_eq!(s >> 16, want[k], "engine {e} column {k}");
        }
    }
}

#[test]
fn train_accel_step_matches_original() {
    let Some(mut o) = oracle() else { return };
    let links = link_engines(&mut o);
    let cars = car_types(&o);
    let m = setup_world(&mut o);
    let mut xs = Xs(0x3c6e_f372);
    let mut w = random_world(&mut o, &m, &mut xs);
    for case in 0..10_000 {
        if case % 250 == 0 {
            w = random_world(&mut o, &m, &mut xs);
            randomize_modifiers(&mut o, &mut xs);
        }
        let t = random_train(&o, &links, &cars, &w, &mut xs);
        let now = t.last.wrapping_add(match xs.below(3) {
            0 => xs.range(0, 60),
            1 => xs.range(0, 1439),
            _ => xs.range(-5, 200_000),
        });
        write_train(&mut o, &m, &t);
        fpu_reset(&mut o);
        o.call_thiscall(ACCEL_FN, m.train, &[now as u32]).unwrap();
        let got = rd_i32(&o, m.train + 0x69);
        let want = accel_ref(&o, &w, &t, now);
        assert_eq!(
            got,
            want,
            "case {case}: engine {} speed {:#x} last {} now {now} target {:?}",
            t.engine,
            t.speed,
            t.last,
            target_ref(&o, &w, &t)
        );
        assert_eq!(
            rd_i32(&o, m.train + 0x6d),
            now,
            "case {case}: last-update time"
        );
    }
}

#[test]
fn breakdown_chance_matches_original() {
    let Some(mut o) = oracle() else { return };
    let links = link_engines(&mut o);
    let cars = car_types(&o);
    let m = setup_world(&mut o);
    let mut xs = Xs(0xa54f_f53a);
    let mut w = random_world(&mut o, &m, &mut xs);
    for case in 0..10_000 {
        if case % 250 == 0 {
            w = random_world(&mut o, &m, &mut xs);
            randomize_modifiers(&mut o, &mut xs);
        }
        let t = random_train(&o, &links, &cars, &w, &mut xs);
        let tiles = xs.range(0, 40);
        let check_age = xs.below(4) != 0;
        write_train(&mut o, &m, &t);
        fpu_reset(&mut o);
        o.call_thiscall(BREAKDOWN_P, m.train, &[tiles as u32, u32::from(check_age)])
            .unwrap();
        let got = o.st0().unwrap();
        let want = breakdown_ref(&o, &w, &t, tiles, check_age);
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "case {case}: engine {} tiles {tiles} got {got:e} want {want:e}",
            t.engine
        );
    }
}

// ---- monthly costs (0x4d4390) ------------------------------------------------------------------

const MONTHLY_FN: u32 = 0x004d_4390; // thiscall(ECX = train)
const COMPANIES: u32 = 0x0057_4c6c;
const PERSONS: u32 = 0x0057_4c68;
const PRICE_LEVEL: u32 = 0x0056_7d6d; // u8[engine kind]: index into the fuel/maintenance levels
const FUEL_LEVELS: u32 = 0x0055_2ae8;
const MAINT_LEVELS: u32 = 0x0055_2af4;
const COMPANY_STRIDE: u32 = 0x5000;

/// A list object as seen by `0x4f18b0`: record `id` at `base + id * stride`.
fn make_list(o: &mut Oracle, global: u32, stride: u32, n: u32) -> u32 {
    let base = o.alloc(stride * (n + 1)).unwrap();
    let hdr = o.alloc(0x40).unwrap();
    o.write_u32(hdr + 8, stride).unwrap();
    o.write_u32(hdr + 0x28, base).unwrap();
    o.write_u32(global, hdr).unwrap();
    base
}

/// Cargo field of a vehicle type (`+0x30`); `0x7f` = carries nothing.
fn vehicle_cargo(o: &Oracle, t: u32) -> u8 {
    rd_u8(o, VEHICLES + t * 0x39 + 0x30)
}

#[test]
fn train_monthly_costs_match_original() {
    let Some(mut o) = oracle() else { return };
    let links = link_engines(&mut o);
    let cars = car_types(&o);
    let m = setup_world(&mut o);
    let companies = make_list(&mut o, COMPANIES, COMPANY_STRIDE, 2);
    let persons = make_list(&mut o, PERSONS, 0x200, 2);
    let mut xs = Xs(0x510e_527f);
    let mut w = random_world(&mut o, &m, &mut xs);
    let (mut humans, mut ai) = (0, 0);
    for case in 0..10_000 {
        if case % 250 == 0 {
            w = random_world(&mut o, &m, &mut xs);
            randomize_modifiers(&mut o, &mut xs);
            for k in 0..8u32 {
                o.write_mem(PRICE_LEVEL + k, &[xs.below(3) as u8]).unwrap();
            }
        }
        let t = random_train(&o, &links, &cars, &w, &mut xs);
        write_train(&mut o, &m, &t);
        let human = xs.below(2) == 0;
        // company 1, person 1 (human when its +0x122 is set)
        let crec = companies + COMPANY_STRIDE;
        o.write_mem(crec, &vec![0u8; COMPANY_STRIDE as usize])
            .unwrap();
        o.write_u32(crec + 0x23, 1).unwrap();
        o.write_u32(persons + 0x200 + 0x122, u32::from(human))
            .unwrap();
        o.write_u32(m.train + 0x41, 1).unwrap();
        let (ton_cells, cells) = (xs.range(0, 400_000), xs.range(0, 3_000));
        o.write_u32(m.train + 0xa2, ton_cells as u32).unwrap();
        o.write_u32(m.train + 0xe1, cells as u32).unwrap();
        fpu_reset(&mut o);
        o.call_thiscall(MONTHLY_FN, m.train, &[]).unwrap();
        let got = [1u32, 2, 3].map(|c| rd_i32(&o, m.train + 0xfd + 4 * c));
        let eng = engine(&o, t.engine);
        let level = u32::from(rd_u8(&o, PRICE_LEVEL + u32::from(eng.kind)));
        let want = monthly_costs(
            &eng,
            &MonthlyInput {
                age: i32::from(w.year) - i32::from(t.bought),
                ton_cells,
                cells,
                oil: if human { t.oil } else { 100.0 },
                cargo_vehicles: t
                    .vehs
                    .iter()
                    .filter(|v| vehicle_cargo(&o, v.typ) != 0x7f)
                    .count() as i32,
                fuel_level: rd_f32(&o, FUEL_LEVELS + 4 * level),
                maint_level: rd_f32(&o, MAINT_LEVELS + 4 * level),
                fuel_kind: modifier(&o, 0x12 + u32::from(eng.kind)),
                fuel_all: modifier(&o, 0x11),
                maint_kind: modifier(&o, 0x1b + u32::from(eng.kind)),
                maint_all: modifier(&o, 0x15),
                car_cost: modifier(&o, 0x19),
            },
        );
        assert_eq!(
            got,
            [want.maintenance, want.cars, want.fuel],
            "case {case}: engine {} human {human}",
            t.engine
        );
        assert_eq!(rd_i32(&o, m.train + 0xa2), 0);
        assert_eq!(rd_i32(&o, m.train + 0xe1), 0);
        if human {
            humans += 1;
        } else {
            ai += 1;
        }
    }
    eprintln!("monthly costs: {humans} human, {ai} AI cases identical");
}

// ---- purchase price (0x4cdf80, fragment 0x4ce30c..0x4ce359) ------------------------------------

/// `train_create` allocates vehicles and formats names, which the emulator cannot do; the price
/// is computed by one contiguous fragment, run here on its own with the registers it expects
/// (`ESI` = train, `EDI` = engine index * 0x3a, `EBX` = 0): `train.+8 = ftol(mod(0x1a) *
/// mod(0x1b + kind) * price)`.
#[test]
fn train_purchase_price_matches_original() {
    let Some(mut o) = oracle() else { return };
    link_engines(&mut o);
    let train = o.alloc(0x200).unwrap();
    let mut xs = Xs(0x9b05_688c);
    for case in 0..10_000 {
        if case % 100 == 0 {
            randomize_modifiers(&mut o, &mut xs);
        }
        let e = xs.below(ENGINE_COUNT);
        o.write_mem(train, &[0u8; 0x200]).unwrap();
        o.write_u32(train + 8, 0xdead_beef).unwrap();
        fpu_reset(&mut o);
        let uc = o.uc();
        for (r, v) in [
            (RegisterX86::ESI, u64::from(train)),
            (RegisterX86::EDI, u64::from(e * 0x3a)),
            (RegisterX86::EBX, 0),
            (RegisterX86::ESP, 0x700f_fc00),
        ] {
            uc.reg_write(r, v).unwrap();
        }
        uc.emu_start(0x004c_e30c, 0x004c_e359, 0, 100_000).unwrap();
        let got = rd_i32(&o, train + 8);
        let eng = engine(&o, e);
        let want = purchase_price(
            modifier(&o, 0x1a),
            modifier(&o, 0x1b + u32::from(eng.kind)),
            &eng,
        );
        assert_eq!(got, want, "case {case}: engine {e}");
    }
}

// ---- breakdown / crash / theft dice (0x4cf390) -------------------------------------------------

const CHECK_FN: u32 = 0x004c_f390; // thiscall(ECX = train)
const RNG_STATE: u32 = 0x0055_d2c0;
const TOWNS: u32 = 0x0057_4c5c;
const NO_CHECK: u32 = 0x0057_48fc;
const NO_ACCIDENTS: u32 = 0x0057_0229;
const THEFT_PCT: u32 = 0x0057_3e02;
const THIEVES: u32 = 0x0057_3db7; // 4 records of 0x18 bytes: flags, u16 year, ...
const STOPS: [(u32, u32); 4] = [
    (0x004c_e730, 1), // train_crash
    (0x004c_e4c0, 2), // train_breakdown
    (0x004c_eaa0, 3), // theft
    (0x0047_6910, 4), // string lookup of the arrest message
];

#[test]
fn breakdown_check_dice_match_original() {
    let Some(mut o) = oracle() else { return };
    let links = link_engines(&mut o);
    let cars = car_types(&o);
    let m = setup_world(&mut o);
    let companies = make_list(&mut o, COMPANIES, COMPANY_STRIDE, 2);
    let persons = make_list(&mut o, PERSONS, 0x200, 2);
    let towns = make_list(&mut o, TOWNS, 0x30, 4);
    let thit = Rc::new(Cell::new(0u32));
    for (addr, code) in STOPS {
        let hit = Rc::clone(&thit);
        o.uc()
            .add_code_hook(u64::from(addr), u64::from(addr), move |uc, _, _| {
                hit.set(code);
                let _ = uc.emu_stop();
            })
            .unwrap();
    }
    // town list header: capacity 4, bitmap with slot 0 taken
    let hdr = o.read_u32(TOWNS).unwrap();
    let bitmap = o.alloc(16).unwrap();
    o.write_mem(bitmap, &[1u8]).unwrap();
    o.write_u32(hdr + 0x10, 4).unwrap();
    o.write_u32(hdr + 0x34, bitmap).unwrap();
    o.write_u32(NO_CHECK, 0).unwrap();
    let mut xs = Xs(0x1f83_d9ab);
    let mut w = random_world(&mut o, &m, &mut xs);
    let mut counts = [0u32; 6];
    for case in 0..30_000 {
        if case % 250 == 0 {
            w = random_world(&mut o, &m, &mut xs);
            randomize_modifiers(&mut o, &mut xs);
        }
        let t = random_train(&o, &links, &cars, &w, &mut xs);
        write_train(&mut o, &m, &t);
        let tiles = xs.range(10, 80);
        o.write_mem(m.train + 0xa0, &(tiles as i16).to_le_bytes())
            .unwrap();
        let human = xs.below(2) == 0;
        let crec = companies + COMPANY_STRIDE;
        o.write_mem(crec, &vec![0u8; COMPANY_STRIDE as usize])
            .unwrap();
        o.write_u32(crec + 0x23, 1).unwrap();
        o.write_u32(persons + 0x200 + 0x122, u32::from(human))
            .unwrap();
        o.write_u32(m.train + 0x41, 1).unwrap();
        // heavy cargo for the passenger/mail test is random per vehicle already (v.load)
        let no_acc = xs.below(6) == 0;
        o.write_mem(NO_ACCIDENTS, &[u8::from(no_acc)]).unwrap();
        let pct = xs.range(0, 400) as i16;
        o.write_mem(THEFT_PCT, &pct.to_le_bytes()).unwrap();
        // a town for the proximity branch, or none
        let town = xs.below(3) != 0;
        let (tx, ty, level) = (xs.range(0, 60), xs.range(0, 60), xs.below(4) as u8);
        o.write_u32(hdr + 0x14, u32::from(town)).unwrap();
        let trec = towns + 0x30;
        o.write_mem(trec, &[0u8; 0x30]).unwrap();
        o.write_mem(trec, &1u16.to_le_bytes()).unwrap();
        o.write_mem(trec + 4, &(tx as i16).to_le_bytes()).unwrap();
        o.write_mem(trec + 6, &(ty as i16).to_le_bytes()).unwrap();
        o.write_mem(trec + 0x2c, &[level]).unwrap();
        // thief table: flags (bit 1 usable, bit 0 used) and the year from which it applies
        let mut eligible = 0;
        for k in 0..4u32 {
            let flags = xs.below(4) as u8;
            let from = xs.range(1800, 2020) as i16;
            o.write_mem(THIEVES + 0x18 * k, &[flags]).unwrap();
            o.write_mem(THIEVES + 0x18 * k + 1, &from.to_le_bytes())
                .unwrap();
            if flags & 2 != 0 && flags & 1 == 0 && i32::from(from) <= i32::from(w.year) {
                eligible += 1;
            }
        }
        let seed = xs.next();
        o.write_u32(RNG_STATE, seed).unwrap();
        thit.set(0);
        fpu_reset(&mut o);
        let r = o.call_thiscall(CHECK_FN, m.train, &[]);
        let outcome = match thit.get() {
            0 => {
                r.unwrap();
                0
            }
            c => c,
        };
        // implementation under test
        let (vx, vy) = (t.vehs[0].x, t.vehs[0].y);
        let carries_people = t
            .vehs
            .iter()
            .any(|v| v.load != 0 && vehicle_cargo(&o, v.typ) <= 1);
        let input = CheckInput {
            tiles,
            chance: breakdown_ref(&o, &w, &t, tiles, true) as f32,
            human,
            no_accidents: no_acc,
            year: w.year,
            theft: TheftInput {
                pct,
                nearby_excess: town.then(|| town_excess(tile_distance(vx, vy, tx, ty), level)),
                theft_mod: modifier(&o, 0x21) as f32,
                caboose: t.flags_bb & 2 != 0,
                carries_people,
                eligible_thieves: eligible,
            },
        };
        let mut rng = GameRng::new(seed);
        let want = breakdown_check(&mut rng, &input);
        let want_code = match want {
            CheckOutcome::Idle => 0,
            CheckOutcome::Crash => 1,
            CheckOutcome::Breakdown => 2,
            CheckOutcome::Theft => 3,
            CheckOutcome::Arrested { .. } => 4,
        };
        assert_eq!(
            outcome, want_code,
            "case {case}: engine {} tiles {tiles} human {human} want {want:?}",
            t.engine
        );
        assert_eq!(
            rd_i32(&o, RNG_STATE) as u32,
            rng.state(),
            "case {case}: RNG state after {want:?}"
        );
        counts[want_code as usize] += 1;
    }
    eprintln!("dice outcomes [idle, crash, breakdown, theft, arrested]: {counts:?}");
}
