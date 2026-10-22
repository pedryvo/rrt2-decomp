//! Differential tests for the pure helpers of the computer opponents (docs/spec/ai.md):
//! `ai_pick_engine` (0x4d86f0) and `ai_town_value` (0x4313f0, company 0).
//! The reference implementations live in this file (rt2-sim is not touched by this spec).
//! Skipped when the executable is absent.

use rt2_oracle::Oracle;
use rt2_sim::rng::GameRng;
use unicorn_engine::RegisterX86;

fn oracle() -> Option<Oracle> {
    let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    });
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return None;
    }
    let mut o = Oracle::load_file(&path).expect("load exe");
    o.uc().reg_write(RegisterX86::FPCW, 0x27f).unwrap();
    Some(o)
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
}

const RNG_STATE: u32 = 0x0055_d2c0;
const GAME_DATE: u32 = 0x0057_04f8; // u16 year, u8 month, u8 day
const AI_COMPANY_ID: u32 = 0x0056_5144;
const EFFECT_GLOBAL: u32 = 0x0057_07c6; // i16 per effect id
const ENGINE_ALLOWED: u32 = 0x0057_0667; // u8 per engine
const ENGINE_TABLE: u32 = 0x0053_cec0; // 66 records of 0x3a bytes
const VEHICLE_TABLE: u32 = 0x0053_ddb8; // records of 0x39 bytes
const N_ENGINES: u32 = 66;

const AI_PICK_ENGINE: u32 = 0x004d_86f0; // fastcall(ECX = mode)
const AI_TOWN_VALUE: u32 = 0x0043_13f0; // thiscall(ECX = town, company) ret 4

/// Snapshot of the memory read by `ai_pick_engine`.
struct EngineMem {
    table: Vec<u8>,
    vehicles: Vec<u8>,
    allowed: Vec<u8>,
    effects: Vec<u8>,
    year: u16,
    month: u8,
}

impl EngineMem {
    fn read(o: &Oracle) -> Self {
        let date = o.read_u32(GAME_DATE).unwrap();
        Self {
            table: o
                .read_mem(ENGINE_TABLE, (N_ENGINES * 0x3a) as usize)
                .unwrap(),
            vehicles: o.read_mem(VEHICLE_TABLE, 256 * 0x39).unwrap(),
            allowed: o.read_mem(ENGINE_ALLOWED, N_ENGINES as usize).unwrap(),
            effects: o.read_mem(EFFECT_GLOBAL, 0x200 * 2).unwrap(),
            year: date as u16,
            month: (date >> 16) as u8,
        }
    }
    fn eng_u8(&self, e: usize, off: usize) -> u8 {
        self.table[e * 0x3a + off]
    }
    fn eng_i32(&self, e: usize, off: usize) -> i32 {
        let p = e * 0x3a + off;
        i32::from_le_bytes(self.table[p..p + 4].try_into().unwrap())
    }
    fn veh_u16(&self, v: usize, off: usize) -> u16 {
        let p = v * 0x39 + off;
        u16::from_le_bytes(self.vehicles[p..p + 2].try_into().unwrap())
    }
    /// Loco vehicle years (+0x1c first, +0x1e last) of engine `e` (engine +0x2c).
    fn years(&self, e: usize) -> (u16, u16) {
        let v = self.eng_u8(e, 0x2c) as usize;
        (self.veh_u16(v, 0x1c), self.veh_u16(v, 0x1e))
    }
    /// `engine_available` (0x4683a0) with company 0 (only global effects).
    fn available(&self, e: usize) -> bool {
        let id = if e < 0x3e { e + 0x7f } else { e + 0xf3 };
        let m = i16::from_le_bytes([self.effects[id * 2], self.effects[id * 2 + 1]]);
        if m < 0 {
            return false;
        }
        if m > 0 {
            return true;
        }
        if self.eng_u8(e, 0) == 0 || self.allowed[e] == 0 {
            return false;
        }
        let (first, last) = self.years(e);
        (first < self.year || (self.year == first && self.month > 5)) && self.year <= last
    }
}

/// Reference `ai_pick_engine` (docs/spec/ai.md, section 4).
fn pick_engine(m: &EngineMem, mode: u32, rng: &mut GameRng) -> i32 {
    let mut best = -1i32;
    let mut best_score = -1i32;
    for pass in 0..3 {
        if best != -1 {
            continue;
        }
        for e in 0..N_ENGINES as usize {
            if m.eng_u8(e, 0x28) == 2 && pass < 1 {
                continue;
            }
            let ok = m.available(e) || {
                let (first, last) = m.years(e);
                pass >= 2 && first < m.year && m.year <= last
            };
            if !ok {
                continue;
            }
            let speed = m.eng_i32(e, 0x14);
            let attr = if mode != 0 {
                speed
            } else {
                speed.wrapping_mul(m.eng_i32(e, 0x18))
            };
            let r = rng.range(80, 120);
            let score = (f64::from(r) * 0.01 * f64::from(attr)) as i32;
            if score > best_score {
                best_score = score;
                best = e as i32;
            }
        }
    }
    if best < 1 { 0 } else { best }
}

/// What the world load (`0x471b10`) fills in at run time: engine `+0x2c` = its loco vehicle
/// type, and the loco vehicle years `+0x1c/+0x1e` = engine years `+0x01/+0x03`.
fn prepare_engines(o: &mut Oracle) {
    for v in 0..175u32 {
        let rec = o.read_mem(VEHICLE_TABLE + v * 0x39, 0x39).unwrap();
        if rec[0x2e] != 0 {
            continue;
        }
        let e = u32::from(rec[0x2f]);
        let eng = o.read_mem(ENGINE_TABLE + e * 0x3a, 0x3a).unwrap();
        o.write_mem(ENGINE_TABLE + e * 0x3a + 0x2c, &[v as u8])
            .unwrap();
        o.write_mem(VEHICLE_TABLE + v * 0x39 + 0x1c, &eng[1..5])
            .unwrap();
    }
}

#[test]
fn ai_pick_engine_matches_original() {
    let Some(mut o) = oracle() else { return };
    let mut xs = Xs(0x00a1_e5e1);
    let mut picked = std::collections::BTreeSet::new();
    let mut total_draws = 0u64;
    for case in 0..1500 {
        o.restore().unwrap();
        o.uc().reg_write(RegisterX86::FPCW, 0x27f).unwrap();
        prepare_engines(&mut o);
        let year = 1800 + (xs.next() % 260) as u16;
        let month = (xs.next() % 12) as u8 + 1;
        let day = (xs.next() % 28) as u8 + 1;
        let date = u32::from(year) | (u32::from(month) << 16) | (u32::from(day) << 24);
        o.write_u32(GAME_DATE, date).unwrap();
        o.write_u32(AI_COMPANY_ID, 0).unwrap();
        // random engine_allowed holes and a few forcing/forbidding scenario effects
        for e in 0..N_ENGINES {
            let a = u8::from(!xs.next().is_multiple_of(8));
            o.write_mem(ENGINE_ALLOWED + e, &[a]).unwrap();
        }
        if case % 3 == 0 {
            for _ in 0..4 {
                let e = xs.next() % N_ENGINES;
                let id = if e < 0x3e { e + 0x7f } else { e + 0xf3 };
                let v: i16 = if xs.next().is_multiple_of(2) { 25 } else { -25 };
                o.write_mem(EFFECT_GLOBAL + id * 2, &v.to_le_bytes())
                    .unwrap();
            }
        }
        let seed = xs.next();
        o.write_u32(RNG_STATE, seed).unwrap();
        let mode = if case % 4 == 3 { 0 } else { 1 };
        let mem = EngineMem::read(&o);
        let mut rng = GameRng::new(seed);
        let want = pick_engine(&mem, mode, &mut rng);
        let got = o.call_fastcall(AI_PICK_ENGINE, mode, 0, &[]).unwrap() as i32;
        assert_eq!(
            got, want,
            "case {case}: year {year} month {month} mode {mode}"
        );
        assert_eq!(
            o.read_u32(RNG_STATE).unwrap(),
            rng.state(),
            "case {case}: RNG state (draws per candidate)"
        );
        if rng.state() != seed {
            total_draws += 1;
        }
        picked.insert(got);
    }
    assert!(picked.len() >= 20, "only {} distinct engines", picked.len());
    assert!(total_draws > 1000);
}

/// Reference `ai_town_value(town, 0)` (docs/spec/ai.md, section 5.1).
fn town_value_company0(kind: u8, a: f32, b: f32, size: u8) -> i32 {
    if kind != 0 {
        return 0;
    }
    let v = ((f64::from(a) - f64::from(b)) * 60000.0) as i32;
    if size >= 8 {
        v.wrapping_mul(2)
    } else if size >= 4 {
        (f64::from(v) * 1.7) as i32
    } else {
        v
    }
}

#[test]
fn ai_town_value_company0_matches_original() {
    let Some(mut o) = oracle() else { return };
    let mut xs = Xs(0x7031_00f0);
    let town = o.alloc(0xb4).unwrap();
    for case in 0..5000 {
        let kind = if xs.next().is_multiple_of(5) {
            (xs.next() % 3) as u8
        } else {
            0
        };
        let a = (xs.next() % 400_000) as f32 / 1000.0;
        let b = (xs.next() % 400_000) as f32 / 1000.0;
        let size = (xs.next() % 12) as u8;
        let mut rec = vec![0u8; 0xb4];
        rec[0x10] = kind;
        rec[0x12..0x16].copy_from_slice(&a.to_le_bytes());
        rec[0x3a..0x3e].copy_from_slice(&b.to_le_bytes());
        rec[0x48] = size;
        o.write_mem(town, &rec).unwrap();
        let got = o.call_thiscall(AI_TOWN_VALUE, town, &[0]).unwrap() as i32;
        let want = town_value_company0(kind, a, b, size);
        assert_eq!(
            got, want,
            "case {case}: kind {kind} a {a} b {b} size {size}"
        );
    }
}

/// `distance_int` (`0x4f0cd0`, fastcall(x1, y1, x2, y2)): the distance every AI rule uses
/// (`rt2_sim::ai::Ai::dist`): `ftol(sqrt(dx*dx + dy*dy))` on the integer sum of squares.
#[test]
fn ai_int_dist_matches_original() {
    let Some(mut o) = oracle() else { return };
    let mut xs = Xs(0x0d15_7a11);
    for case in 0..20_000 {
        let r = |xs: &mut Xs| (xs.next() % 2049) as i32 - 1024;
        let (x1, y1, x2, y2) = (r(&mut xs), r(&mut xs), r(&mut xs), r(&mut xs));
        let got = o
            .call_fastcall(0x004f_0cd0, x1 as u32, y1 as u32, &[x2 as u32, y2 as u32])
            .unwrap() as i32;
        assert_eq!(
            rt2_sim::ai::Ai::dist(x1, y1, x2, y2),
            got,
            "case {case}: ({x1},{y1})-({x2},{y2})"
        );
    }
}
