//! Differential tests for the train simulation (docs/spec/trains.md): grade of a track
//! segment, the speed curve of a locomotive, one acceleration step and the breakdown
//! probability. Each test builds the inputs in the emulated memory, runs the original
//! function and compares it with a Rust reference. Skipped when the executable is absent.

use rt2_oracle::Oracle;
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

/// CRT `_ftol`: truncation toward zero, low 32 bits of the 64-bit result.
fn ftol(x: f64) -> i32 {
    if x.is_nan() {
        0
    } else {
        (x.trunc() as i64) as i32
    }
}

fn f32r(x: f64) -> f64 {
    x as f32 as f64
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

// ---- game data (read from the emulated .data, never typed in) ----------------------------------

#[derive(Clone, Copy)]
struct Engine {
    reliability: i32,
    top_mph: i32,
    power: i32,
    accel: i32,
    free_tons: i32,
    kind: u8,
    base_tons: i32, // +0x2e, filled at map start by 0x471b10
}

fn engine(o: &Oracle, e: u32) -> Engine {
    let r = ENGINES + e * 0x3a;
    Engine {
        reliability: rd_i32(o, r + 0x08),
        top_mph: rd_i32(o, r + 0x14),
        power: rd_i32(o, r + 0x18),
        accel: rd_i32(o, r + 0x1c),
        free_tons: rd_i32(o, r + 0x20),
        kind: rd_u8(o, r + 0x28),
        base_tons: rd_i32(o, r + 0x2e),
    }
}

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
    for (e, &(ev, tv)) in out.iter().enumerate() {
        let mut w = vehicle_type(o, ev).3;
        if let Some(tv) = tv {
            w += vehicle_type(o, tv).3;
        }
        o.write_u32(ENGINES + e as u32 * 0x3a + 0x2e, w as u32)
            .unwrap();
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
    let pct = (rd_u16(o, MODIFIERS + 2 * id) as i16).max(-99);
    let m = f32r(f64::from(pct) * 0.01 + 1.0);
    if m < 0.01 { f64::from(0.01f32) } else { m }
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

/// `0x4284e0`: grade of a track segment from the vertex heights (0.005 per height unit).
fn grade_ref(h: &dyn Fn(i32) -> i32, corners: [i32; 4], din: i32, dout: i32) -> f64 {
    let end = |d: i32| -> i32 {
        let k = d >> 1;
        if d & 1 != 0 {
            h(corners[((k + 1) & 3) as usize])
        } else {
            (h(corners[((k + 1) & 3) as usize]) + h(corners[k as usize])) >> 1
        }
    };
    let g = f64::from(end(dout) - end(din)) * 0.005;
    match (din & 1 != 0, dout & 1 != 0) {
        (false, false) => g,
        (true, true) => g * 0.7092,
        _ => g * 0.85,
    }
}

/// `0x4cfea0`: maximum speed (mph, 16.16) of engine `e` hauling `tons` on `grade`.
fn speed_ref(o: &Oracle, e: u32, tons: i32, grade: f32, curve: bool, age: i32) -> i32 {
    let eng = engine(o, e);
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
    let mult = if g > 0.0 {
        (g * 20.0 + 1.0).powf(2.3)
    } else {
        let g = if g < -0.03 { f64::from(-0.03f32) } else { g };
        g * 10.0 + 1.0
    };
    let mut eff = ftol(mult * f64::from(w)).wrapping_sub(eng.base_tons.wrapping_add(eng.free_tons));
    if eff < 0 {
        eff = 0;
    }
    let eff = ftol((af + 1.0) * f64::from(eff));
    let m1 = modifier(o, u32::from(eng.kind) + 0x2b);
    let m2 = modifier(o, 0x2a);
    let mut p = f32r(m2 * m1 * f64::from(eng.power as f32) * 0.92);
    if p < 1.0 {
        p = 1.0;
    }
    let mut s = ftol(0.98f64.powf(f64::from(eff) / p) * f64::from(eng.top_mph << 16));
    s = ftol(f64::from(s) * (1.0 - af));
    if curve {
        s = ftol(f64::from(s) * 0.6);
    }
    let m3 = modifier(o, u32::from(eng.kind) + 0x24);
    let m4 = modifier(o, 0x23);
    s = ftol(m4 * m3 * f64::from(s));
    s.max(0x10000)
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
    /// Weighted grade of the first two vehicles (0x4d00c0 / 0x4cf030 / 0x4cfd40).
    fn train_grade(&self, o: &Oracle, t: &Train) -> f64 {
        if !t.on_track {
            return 0.0;
        }
        let (mut sum, mut acc) = (0i32, 0f64);
        for v in t.vehs.iter().take(2) {
            if v.on_track {
                let w = self.veh_tons(o, v);
                sum += w;
                acc = f32r(self.veh_grade(v) * f64::from(w) + acc);
            }
        }
        if sum == 0 {
            0.0
        } else {
            f32r(acc / f64::from(sum))
        }
    }
}

/// `0x4d00c0`: target speed (16.16 mph) and the "restricted" flag (curve).
fn target_ref(o: &Oracle, w: &World, t: &Train) -> (i32, bool) {
    let curve = t
        .vehs
        .iter()
        .filter(|v| v.on_track)
        .any(|v| (i32::from(v.din) - i32::from(v.dout)) & 3 != 0);
    let mut g = w.train_grade(o, t);
    if f64::from(t.sand) <= 1.0 {
        g = f32r(g + g);
    }
    let age = i32::from(w.year) - i32::from(t.bought);
    let mut s = speed_ref(o, t.engine, t.tons, g as f32, curve, age);
    s = ftol(f64::from(t.throttle) * f64::from(1.0f32 / 85.0) * f64::from(s));
    if engine(o, t.engine).kind == 0 && f64::from(t.water) <= 1.0 {
        s = ftol(f64::from(s) * 0.5);
    }
    if w.double_speed {
        s = s.wrapping_mul(2);
    }
    (s, curve)
}

/// `0x4d0400`, moving branch: new speed after advancing to `now`.
fn accel_ref(o: &Oracle, w: &World, t: &Train, now: i32) -> i32 {
    let eng = engine(o, t.engine);
    let (target, flag) = target_ref(o, w, t);
    let mut tau = eng.accel.wrapping_mul(1440);
    if t.speed > target {
        tau >>= if flag { 4 } else { 1 };
    }
    tau -= tau >> 2;
    let m1 = modifier(o, u32::from(eng.kind) + 0x2f);
    let m2 = modifier(o, 0x2e);
    let mut m = m2 * m1;
    if m < 0.1 {
        m = f64::from(0.1f32);
    }
    let tau2 = ftol(f64::from(tau) / m);
    let mut elapsed = if t.last != 0 { now - t.last } else { 0 };
    elapsed = elapsed.min(tau2).max(0);
    let div = if target < 0 { tau2 / 2 } else { tau2 };
    let delta = ftol(f64::from(elapsed) / f64::from(div) * f64::from(target.wrapping_sub(t.speed)));
    t.speed.wrapping_add(delta)
}

/// `0x4cf030`: breakdown probability for `tiles` tiles travelled.
fn breakdown_ref(o: &Oracle, w: &World, t: &Train, tiles: i32, check_age: bool) -> f64 {
    let age = i32::from(w.year) - i32::from(t.bought);
    if check_age && age < 1 {
        return 0.0;
    }
    let eng = engine(o, t.engine);
    let rate = f64::from(rd_f32(o, RELIABILITY + 4 * eng.reliability as u32));
    let base = f32r(f64::from(tiles) * rate);
    let thr = f64::from(t.throttle);
    let x = if thr > 85.0 {
        // net_min_peer_version() = 9012 >= 0x232d in a local game
        ((thr - 85.0) * 3.0 * (1.0 / 15.0) + 1.0) * base
    } else {
        base * thr * f64::from(1.0f32 / 85.0)
    };
    let x = f32r(x);
    let mut m = modifier(o, 0x27);
    if m < 0.01 {
        m = f64::from(0.01f32);
    }
    let x = f32r(x / m);
    let g = w.train_grade(o, t);
    let mult = if g > 0.0 {
        (g * 20.0 + 1.0).powf(2.3)
    } else {
        let g = if g < -0.03 { f64::from(-0.03f32) } else { g };
        g * 10.0 + 1.0
    };
    let mut e =
        ftol(mult * f64::from(t.tons)).wrapping_sub(eng.base_tons.wrapping_add(eng.free_tons));
    if e < 0 {
        e = 0;
    }
    let mut y = (f64::from(e) + 150.0) * x * 0.005;
    if t.flags_bb & 2 != 0 {
        y *= 0.75;
    }
    y = y * f64::from(age + 30) * (1.0 / 30.0);
    if age <= 0 {
        y *= 0.3;
    } else if age <= 1 {
        y *= 0.6;
    }
    let oil = f64::from(t.oil);
    if oil < 1.0 {
        y *= 3.0;
    } else {
        y *= (100.0 - oil + 100.0) * 0.01;
    }
    let mut y = f32r(y * 0.7);
    if w.year >= 1970 {
        y = f32r(y * 0.7);
    } else if w.year >= 1920 {
        y = f32r(y * 0.85);
    }
    if w.nowreck {
        return y * 0.0001;
    }
    y
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
