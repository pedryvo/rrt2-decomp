//! Differential tests for the world growth rules (docs/spec/world-growth.md): the per-place
//! growth step, the building type pick, the site search, the removal pick, the building
//! statistics, the site classes, the industry rating and the shape pick. Skipped when the
//! executable is absent. The x87 control word is pinned to 53-bit precision (CW 0x27f).
//!
//! The functions that call other game subsystems (building creation and demolition touch the
//! map, the UI and the station pool) are run with their callees replaced by small machine-code
//! stubs written over the callee's body in the emulated image: the stubs log their arguments in
//! emulated memory and return values taken from tables the test fills. The Rust side runs the
//! same scenario against equivalent Rust environments, and the return value, the RNG state, the
//! written fields and the call logs are compared.

use rt2_oracle::Oracle;
use rt2_sim::cargo::IndustryType;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::growth::*;
use rt2_sim::rng::GameRng;
use unicorn_engine::RegisterX86;

fn exe_path() -> String {
    std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    })
}

fn load_exe() -> Exe {
    Exe::from_bytes(std::fs::read(exe_path()).expect("read exe")).expect("parse exe")
}

fn oracle() -> Option<Oracle> {
    let path = exe_path();
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
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
    fn chance(&mut self, pct: u32) -> bool {
        self.next() % 100 < pct
    }
}

// ---- memory helpers ----------------------------------------------------------------------------

fn w8(o: &mut Oracle, a: u32, v: u8) {
    o.write_mem(a, &[v]).unwrap();
}
fn w16(o: &mut Oracle, a: u32, v: u16) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn w32(o: &mut Oracle, a: u32, v: u32) {
    o.write_u32(a, v).unwrap();
}
fn wf(o: &mut Oracle, a: u32, v: f32) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn r8(o: &Oracle, a: u32) -> u8 {
    o.read_mem(a, 1).unwrap()[0]
}
fn r16(o: &Oracle, a: u32) -> u16 {
    let b = o.read_mem(a, 2).unwrap();
    u16::from_le_bytes([b[0], b[1]])
}
fn r32(o: &Oracle, a: u32) -> u32 {
    o.read_u32(a).unwrap()
}

/// A list object as seen by `0x4f18b0`: record `id` lives at `base + id * stride`
/// (`+8` = stride, `+0x28` = base pointer); id 0 yields a null pointer.
#[derive(Clone, Copy)]
struct List {
    base: u32,
    stride: u32,
}
impl List {
    fn new(o: &mut Oracle, global: u32, stride: u32, n: u32) -> Self {
        let base = o.alloc(stride * (n + 1)).unwrap();
        let hdr = o.alloc(0x40).unwrap();
        w32(o, hdr + 8, stride);
        w32(o, hdr + 0x28, base);
        w32(o, global, hdr);
        List { base, stride }
    }
    fn rec(&self, id: u32) -> u32 {
        self.base + id * self.stride
    }
}

const RNG_STATE: u32 = 0x0055_d2c0;
const G_INDUSTRIES: u32 = 0x0057_4c74;
const G_INDUSTRY_SET: u32 = 0x0057_04ac;
const G_DATE: u32 = 0x0057_04f8;
const G_NOW: u32 = 0x0057_04f4;
const G_START_DATE: u32 = 0x0057_0508;
const G_LOCK: u32 = 0x0057_48f8;
const G_SANDBOX: u32 = 0x0057_0227;
const G_CARGO_ON: u32 = 0x0057_05d5;
const G_GROWTH: u32 = 0x0057_05d1;
const G_ECON: u32 = 0x0057_0717;
const G_VIAGRA: u32 = 0x0057_4c14;
const G_INDUSTRY_COUNT: u32 = 0x0057_0713;
const G_DELIVERIES: u32 = 0x0057_070f;
const G_TYPE_FLAGS: u32 = 0x0057_02cb; // 9-byte records: [disabled u8, scarcity i32, ...]

fn set_rng(o: &mut Oracle, s: u32) {
    w32(o, RNG_STATE, s);
}
fn get_rng(o: &Oracle) -> u32 {
    r32(o, RNG_STATE)
}

/// Tiny x86 assembler for the stubs: raw bytes plus short-jump labels.
#[derive(Default)]
struct Asm {
    buf: Vec<u8>,
    fixups: Vec<(usize, &'static str)>,
    labels: Vec<(&'static str, usize)>,
}
impl Asm {
    fn b(&mut self, bytes: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(bytes);
        self
    }
    fn d(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn jmp8(&mut self, opcode: u8, label: &'static str) -> &mut Self {
        self.buf.push(opcode);
        self.fixups.push((self.buf.len(), label));
        self.buf.push(0);
        self
    }
    fn label(&mut self, l: &'static str) -> &mut Self {
        self.labels.push((l, self.buf.len()));
        self
    }
    fn finish(mut self) -> Vec<u8> {
        for (at, l) in std::mem::take(&mut self.fixups) {
            let target = self.labels.iter().find(|(n, _)| *n == l).expect("label").1;
            let rel = target as i64 - (at as i64 + 1);
            assert!((-128..128).contains(&rel));
            self.buf[at] = rel as i8 as u8;
        }
        self.buf
    }
    // mov reg, [abs]
    fn mov_eax_abs(&mut self, a: u32) -> &mut Self {
        self.b(&[0xa1]).d(a)
    }
    fn mov_edx_abs(&mut self, a: u32) -> &mut Self {
        self.b(&[0x8b, 0x15]).d(a)
    }
    fn mov_abs_eax(&mut self, a: u32) -> &mut Self {
        self.b(&[0xa3]).d(a)
    }
    fn mov_abs_edx(&mut self, a: u32) -> &mut Self {
        self.b(&[0x89, 0x15]).d(a)
    }
    fn ret(&mut self, n: u16) -> &mut Self {
        self.b(&[0xc2, n as u8, (n >> 8) as u8])
    }
}

fn patch(o: &mut Oracle, at: u32, code: Vec<u8>) {
    o.write_mem(at, &code).unwrap();
}

// ---- 1. industry rating (0x40b450) ---------------------------------------------------------------

#[test]
fn industry_rating_matches_original() {
    let Some(mut o) = oracle() else { return };
    let exe = load_exe();
    let data = GameData::load(&exe).unwrap();
    let tab = GrowthTables::load(&exe).unwrap();
    let list = List::new(&mut o, G_INDUSTRIES, 0x9f, 2);
    let b = list.rec(1);
    let mut xs = Xs(0x1357_9bdf);
    w32(&mut o, G_LOCK, 0);
    for case in 0..8000 {
        let set = xs.below(2) as usize;
        let ty = if xs.chance(3) {
            0
        } else {
            xs.range(1, 52) as usize
        };
        let year = xs.range(1795, 2020);
        let sandbox = xs.chance(10);
        let on: Vec<bool> = (0..0x48).map(|_| !xs.chance(15)).collect();
        let shape = xs.below(0x7a) as usize;
        let received = if xs.chance(30) {
            0
        } else {
            xs.below(300) as u16
        };
        let mut delivered = [0i8; 4];
        let mut produced = [0i8; 4];
        for k in 0..4 {
            produced[k] = if xs.chance(30) {
                0
            } else {
                xs.range(0, 127) as i8
            };
            delivered[k] = if xs.chance(30) {
                0
            } else if xs.chance(10) {
                xs.range(-128, 127) as i8
            } else {
                xs.range(0, 127) as i8
            };
        }
        let old = if xs.chance(10) {
            0.0f32
        } else {
            xs.range(0, 2000) as f32 / 1000.0
        };
        // Building record.
        o.write_mem(b, &[0u8; 0x9f]).unwrap();
        w8(&mut o, b + 7, ty as u8);
        w8(&mut o, b + 6, shape as u8);
        w16(&mut o, b + 0x34, received);
        for k in 0..4 {
            w8(&mut o, b + 0x63 + k as u32, delivered[k] as u8);
            w8(&mut o, b + 0x67 + k as u32, produced[k] as u8);
        }
        wf(&mut o, b + 0x93, old);
        w16(&mut o, b + 0x4b, 0xbeef);
        w16(&mut o, b + 0x36, 0xbeef);
        w8(&mut o, G_INDUSTRY_SET, set as u8);
        w32(&mut o, G_DATE, year as u32 | 0x0501_0000);
        w8(&mut o, G_SANDBOX, sandbox as u8);
        for (c, v) in on.iter().enumerate() {
            w8(&mut o, G_CARGO_ON + c as u32, *v as u8);
        }
        o.call_fastcall(0x0040_b450, b, 0, &[]).unwrap();
        let got = (
            f32::from_bits(r32(&o, b + 0x93)),
            r16(&o, b + 0x4b),
            r16(&o, b + 0x36) as i16,
        );
        let want = if ty == 0 {
            (old, 0xbeef, 0xbeefu16 as i16)
        } else {
            let cargo_on = |c: u8| on.get(usize::from(c)).copied().unwrap_or(false);
            let it: &IndustryType = &data.industries[set][ty];
            let r = industry_rating(&RatingInput {
                ty: it,
                year,
                cargo_on: &cargo_on,
                sandbox,
                peer_version: 0x2334,
                received,
                delivered,
                produced,
                old,
                price: tab.shape_price[shape],
            });
            (r.smoothed, r.rating, r.profit_k)
        };
        assert_eq!(
            (got.0.to_bits(), got.1, got.2),
            (want.0.to_bits(), want.1, want.2),
            "case {case}: set {set} type {ty} year {year} recv {received} d {delivered:?} p {produced:?} old {old}"
        );
    }
}

// ---- 2. site classes (0x42fb10) --------------------------------------------------------------------

const G_TILES: u32 = 0x0057_4c24;
const G_ELEV: u32 = 0x0057_4c28;
const G_MAP_W: u32 = 0x0055_e7a4;
const G_MAP_H: u32 = 0x0055_e7a0;
const G_ELEV_W: u32 = 0x0056_09ac;
const G_CLASS_TABLE: u32 = 0x0058_1090;

#[test]
fn site_classes_match_original() {
    let Some(mut o) = oracle() else { return };
    let (w, h) = (24i32, 20i32);
    let tiles = o.alloc((w * h * 15) as u32).unwrap();
    let ew = w + 1;
    let elev = o.alloc((ew * (h + 1) * 3) as u32).unwrap();
    w32(&mut o, G_TILES, tiles);
    w32(&mut o, G_ELEV, elev);
    w32(&mut o, G_MAP_W, w as u32);
    w32(&mut o, G_MAP_H, h as u32);
    w32(&mut o, G_ELEV_W, ew as u32);
    let out = o.alloc(16).unwrap();
    let mut xs = Xs(0x0bad_cafe);
    for case in 0..3000 {
        // Random tile bytes (b2, b3, b6) and heights with mostly small steps.
        let mut tile_bytes = vec![(0u8, 0u8, 0u8); (w * h) as usize];
        for (i, t) in tile_bytes.iter_mut().enumerate() {
            let b2 = xs.below(16) as u8 | (xs.below(16) as u8) << 4;
            let b3 = if xs.chance(50) {
                0
            } else {
                xs.below(256) as u8
            };
            let b6 = xs.below(256) as u8;
            *t = (b2, b3, b6);
            let a = tiles + i as u32 * 15;
            w8(&mut o, a + 2, b2);
            w8(&mut o, a + 3, b3);
            w8(&mut o, a + 6, b6);
        }
        let mut heights = vec![0i32; (ew * (h + 1)) as usize];
        let spread = [2, 6, 40][xs.below(3) as usize];
        for (i, v) in heights.iter_mut().enumerate() {
            *v = xs.range(-spread, spread) * i32::from(!xs.chance(20));
            w16(&mut o, elev + i as u32 * 3, *v as i16 as u16);
        }
        let x = xs.range(0, w - 1);
        let y = xs.range(0, h - 1);
        o.call_fastcall(0x0042_fb10, x as u32, y as u32, &[out, out + 4])
            .unwrap();
        let got = (r32(&o, out) as i32, r32(&o, out + 4) as i32);
        let tile = |px: i32, py: i32| {
            if (0..w).contains(&px) && (0..h).contains(&py) {
                Some(tile_bytes[(py * w + px) as usize])
            } else {
                None
            }
        };
        let elev_at = |gx: i32, gy: i32| heights[(gy * ew + gx) as usize];
        let want = site_classes(&tile, &elev_at, x, y);
        assert_eq!(got, want, "case {case}: x {x} y {y}");
    }
}

// ---- 3. building statistics (0x430400) ---------------------------------------------------------------

#[test]
fn town_building_stats_matches_original() {
    let Some(mut o) = oracle() else { return };
    let exe = load_exe();
    let data = GameData::load(&exe).unwrap();
    let tab = GrowthTables::load(&exe).unwrap();
    let list = List::new(&mut o, G_INDUSTRIES, 0x9f, 40);
    let town = o.alloc(0xb4).unwrap();
    let out = o.alloc(16).unwrap();
    let mut xs = Xs(0x2468_ace1);
    for case in 0..4000 {
        let set = xs.below(2) as usize;
        let year = xs.range(1795, 2020);
        let n = xs.below(30) as usize;
        let mut chain = Vec::new();
        for _ in 0..n {
            let ty = if xs.chance(40) {
                0x12
            } else {
                xs.below(53) as u8
            };
            let stations = if xs.chance(50) {
                0
            } else {
                xs.range(1, 4) as i8
            };
            chain.push(BuildingRef { ty, stations });
        }
        o.write_mem(town, &[0u8; 0xb4]).unwrap();
        wf(&mut o, town + 0x12, 7.0);
        wf(&mut o, town + 0x3a, 7.0);
        for (i, b) in chain.iter().enumerate() {
            let rec = list.rec(1 + i as u32);
            o.write_mem(rec, &[0u8; 0x9f]).unwrap();
            w8(&mut o, rec + 7, b.ty);
            w8(&mut o, rec + 0x33, b.stations as u8);
            let next = if i + 1 < chain.len() { 2 + i as u32 } else { 0 };
            w16(&mut o, rec + 4, next as u16);
        }
        w16(&mut o, town + 2, if chain.is_empty() { 0 } else { 1 });
        w8(&mut o, G_INDUSTRY_SET, set as u8);
        w32(&mut o, G_DATE, year as u32 | 0x0501_0000);
        o.call_thiscall(0x0043_0400, town, &[out, out + 4, out + 8])
            .unwrap();
        let got = (
            r32(&o, out) as i32,
            r32(&o, out + 4) as i32,
            r32(&o, out + 8) as i32,
            r32(&o, town + 0x12),
            r32(&o, town + 0x3a),
            r8(&o, town + 0x48),
        );
        let want = town_building_stats(&chain, &tab, &data.industries[set], set == 1, year);
        let want = (
            want.stats.houses,
            want.stats.others,
            want.stats.size_sum,
            want.output_rate.to_bits(),
            want.served_rate.to_bits(),
            want.houses_u8,
        );
        assert_eq!(
            got, want,
            "case {case}: set {set} year {year} chain {chain:?}"
        );
    }
}

// ---- 4. per-place growth step (0x430540) ---------------------------------------------------------------

/// Scratch layout shared by the stubs: counts, the add/remove logs and the add budget.
const S_H: u32 = 0;
const S_O: u32 = 4;
const S_SIZE_SUM: u32 = 8;
const S_BUDGET: u32 = 0x10;
const S_UNIT: u32 = 0x14;
const S_ADD_N: u32 = 0x18;
const S_REM_N: u32 = 0x1c;
const S_ADD_LOG: u32 = 0x100;
const S_REM_LOG: u32 = 0x500;

/// Stubs over `0x430400` (stats), `0x42ffc0` (add) and `0x4302c0` (remove).
fn install_town_stubs(o: &mut Oracle, scr: u32) {
    let mut a = Asm::default();
    a.b(&[0x8b, 0x44, 0x24, 0x04])
        .mov_edx_abs(scr + S_H)
        .b(&[0x89, 0x10]);
    a.b(&[0x8b, 0x44, 0x24, 0x08])
        .mov_edx_abs(scr + S_O)
        .b(&[0x89, 0x10]);
    a.b(&[0x8b, 0x44, 0x24, 0x0c])
        .mov_edx_abs(scr + S_SIZE_SUM)
        .b(&[0x89, 0x10]);
    a.ret(12);
    patch(o, 0x0043_0400, a.finish());

    let mut a = Asm::default();
    a.mov_eax_abs(scr + S_BUDGET)
        .b(&[0x85, 0xc0])
        .jmp8(0x74, "fail");
    a.b(&[0x48]).mov_abs_eax(scr + S_BUDGET);
    a.b(&[0x8b, 0x4c, 0x24, 0x08]); // house flag
    a.mov_edx_abs(scr + S_ADD_N);
    a.b(&[0x89, 0x0c, 0x95]).d(scr + S_ADD_LOG);
    a.b(&[0x42]).mov_abs_edx(scr + S_ADD_N);
    a.b(&[0x85, 0xc9]).jmp8(0x74, "ind");
    a.b(&[0x83, 0x05]).d(scr + S_H).b(&[1]);
    a.jmp8(0xeb, "ok");
    a.label("ind");
    a.b(&[0x83, 0x05]).d(scr + S_O).b(&[1]);
    a.mov_eax_abs(scr + S_UNIT);
    a.b(&[0x01, 0x05]).d(scr + S_SIZE_SUM);
    a.label("ok");
    a.b(&[0xb8, 1, 0, 0, 0]).ret(8);
    a.label("fail");
    a.b(&[0x31, 0xc0]).ret(8);
    patch(o, 0x0042_ffc0, a.finish());

    let mut a = Asm::default();
    a.b(&[0x8b, 0x4c, 0x24, 0x04]);
    a.mov_edx_abs(scr + S_REM_N);
    a.b(&[0x89, 0x0c, 0x95]).d(scr + S_REM_LOG);
    a.b(&[0x42]).mov_abs_edx(scr + S_REM_N);
    a.b(&[0x85, 0xc9]).jmp8(0x74, "ind");
    a.b(&[0x83, 0x2d]).d(scr + S_H).b(&[1]);
    a.jmp8(0xeb, "done");
    a.label("ind");
    a.b(&[0x83, 0x2d]).d(scr + S_O).b(&[1]);
    a.mov_eax_abs(scr + S_UNIT);
    a.b(&[0x29, 0x05]).d(scr + S_SIZE_SUM);
    a.label("done");
    a.ret(4);
    patch(o, 0x0043_02c0, a.finish());
}

/// Rust twin of the three stubs.
struct StubTown {
    h: i32,
    o: i32,
    s: i32,
    budget: i32,
    unit: i32,
    adds: Vec<bool>,
    rems: Vec<bool>,
}
impl TownEnv for StubTown {
    fn stats(&mut self) -> BuildingStats {
        BuildingStats {
            houses: self.h,
            others: self.o,
            size_sum: self.s,
        }
    }
    fn add_building(&mut self, house: bool) -> bool {
        if self.budget == 0 {
            return false;
        }
        self.budget -= 1;
        self.adds.push(house);
        if house {
            self.h += 1;
        } else {
            self.o += 1;
            self.s += self.unit;
        }
        true
    }
    fn remove_worst(&mut self, house: bool) {
        self.rems.push(house);
        if house {
            self.h -= 1;
        } else {
            self.o -= 1;
            self.s -= self.unit;
        }
    }
}

#[test]
fn town_growth_step_matches_original() {
    let Some(mut o) = oracle() else { return };
    let exe = load_exe();
    let tab = GrowthTables::load(&exe).unwrap();
    let scr = o.alloc(0x1000).unwrap();
    install_town_stubs(&mut o, scr);
    let town = o.alloc(0xb4).unwrap();
    let mut xs = Xs(0xdead_beef);
    let mut grew = 0;
    let mut shrank = 0;
    let mut churned = 0;
    for case in 0..6000 {
        let kind = [0u8, 0, 0, 1, 1, 2][xs.below(6) as usize];
        let size_class = xs.below(5) as u8;
        let t0 = Town {
            kind,
            size_class,
            growth_index: xs.range(0, 400),
            land_tiles: xs.range(0, 6000),
            lots_in: if xs.chance(30) {
                0
            } else {
                xs.below(3000) as u16
            },
            lots_out: if xs.chance(30) {
                0
            } else {
                xs.below(3000) as u16
            },
            cooldown_until: if xs.chance(30) { 2_000_000 } else { 100 },
            intensity: if xs.chance(40) {
                xs.below(200) as u16
            } else {
                xs.range(80, 120) as u16
            },
            trend: if xs.chance(20) {
                xs.range(-5, 9) as i8
            } else {
                xs.range(-2, 6) as i8
            },
        };
        let years = xs.range(-1, 30);
        let w = GrowthWorld {
            growth: (xs.range(30, 800) as f32) / 100.0,
            econ_state: xs.below(5) as usize,
            viagra: xs.chance(15),
            industry_count: if xs.chance(10) { 0 } else { xs.range(1, 400) },
            deliveries_total: if xs.chance(20) { 0 } else { xs.range(1, 30000) },
            year: 1850 + years,
            start_year: 1850,
            now: 1_000_000,
            set1: xs.chance(20),
        };
        let st = StubTown {
            h: xs.range(0, 70),
            o: xs.range(0, 40),
            s: xs.range(0, 130),
            budget: if xs.chance(15) { 0 } else { xs.range(1, 12) },
            unit: xs.range(1, 4),
            adds: Vec::new(),
            rems: Vec::new(),
        };
        let dt = if xs.chance(60) {
            483_840
        } else {
            xs.range(1, 1_500_000)
        };
        let initial = xs.chance(20);
        let seed = xs.next();

        // Emulator.
        o.write_mem(town, &[0u8; 0xb4]).unwrap();
        w8(&mut o, town + 0x10, t0.kind);
        w8(&mut o, town + 0x2c, t0.size_class);
        w32(&mut o, town + 0x2e, t0.growth_index as u32);
        w32(&mut o, town + 0x32, t0.land_tiles as u32);
        w16(&mut o, town + 0x3e, t0.lots_in);
        w16(&mut o, town + 0x40, t0.lots_out);
        w32(&mut o, town + 0x42, t0.cooldown_until);
        w16(&mut o, town + 0x46, t0.intensity);
        w8(&mut o, town + 0x49, t0.trend as u8);
        wf(&mut o, G_GROWTH, w.growth);
        w32(&mut o, G_ECON, w.econ_state as u32);
        w32(&mut o, G_VIAGRA, w.viagra as u32);
        w32(&mut o, G_INDUSTRY_COUNT, w.industry_count as u32);
        w32(&mut o, G_DELIVERIES, w.deliveries_total as u32);
        w32(&mut o, G_DATE, w.year as u32 | 0x0501_0000);
        w32(&mut o, G_START_DATE, w.start_year as u32 | 0x0101_0000);
        w32(&mut o, G_NOW, w.now);
        w8(&mut o, G_INDUSTRY_SET, w.set1 as u8);
        w32(&mut o, scr + S_H, st.h as u32);
        w32(&mut o, scr + S_O, st.o as u32);
        w32(&mut o, scr + S_SIZE_SUM, st.s as u32);
        w32(&mut o, scr + S_BUDGET, st.budget as u32);
        w32(&mut o, scr + S_UNIT, st.unit as u32);
        w32(&mut o, scr + S_ADD_N, 0);
        w32(&mut o, scr + S_REM_N, 0);
        set_rng(&mut o, seed);
        o.call_thiscall(0x0043_0540, town, &[dt as u32, initial as u32])
            .unwrap();
        let add_n = r32(&o, scr + S_ADD_N) as usize;
        let rem_n = r32(&o, scr + S_REM_N) as usize;
        let adds: Vec<bool> = (0..add_n)
            .map(|i| r32(&o, scr + S_ADD_LOG + 4 * i as u32) != 0)
            .collect();
        let rems: Vec<bool> = (0..rem_n)
            .map(|i| r32(&o, scr + S_REM_LOG + 4 * i as u32) != 0)
            .collect();
        let got = (
            r16(&o, town + 0x46),
            r32(&o, town + 0x2e) as i32,
            r8(&o, town + 0x49) as i8,
            r32(&o, scr + S_H) as i32,
            r32(&o, scr + S_O) as i32,
            r32(&o, scr + S_SIZE_SUM) as i32,
            get_rng(&o),
            adds,
            rems,
        );

        // Rust.
        let mut t = t0;
        let mut env = st;
        let mut rng = GameRng::new(seed);
        town_growth_step(&mut t, &w, &tab, &mut env, &mut rng, dt, initial);
        let want = (
            t.intensity,
            t.growth_index,
            t.trend,
            env.h,
            env.o,
            env.s,
            rng.state(),
            env.adds,
            env.rems,
        );
        assert_eq!(
            got, want,
            "case {case}: {t0:?} world {w:?} dt {dt} initial {initial} seed {seed:#x}"
        );
        grew += usize::from(!want.7.is_empty());
        shrank += usize::from(!want.8.is_empty());
        churned += usize::from(want.8.len() > 1);
    }
    assert!(
        grew > 300 && shrank > 100,
        "coverage: grew {grew} shrank {shrank} ({churned})"
    );
}

// ---- 5. building type pick (0x42ffc0) -------------------------------------------------------------------

const S5_LOG_N: u32 = 0;
const S5_LOG: u32 = 0x100;
const S5_TAB: u32 = 0x800;

#[test]
fn town_add_building_matches_original() {
    let Some(mut o) = oracle() else { return };
    let exe = load_exe();
    let data = GameData::load(&exe).unwrap();
    let tab = GrowthTables::load(&exe).unwrap();
    let list = List::new(&mut o, G_INDUSTRIES, 0x9f, 40);
    let scr = o.alloc(0x1000).unwrap();
    // Stub over 0x42fcb0: log the type, succeed when table[type] != 0.
    let mut a = Asm::default();
    a.b(&[0x8b, 0x4c, 0x24, 0x04]);
    a.mov_edx_abs(scr + S5_LOG_N);
    a.b(&[0x89, 0x0c, 0x95]).d(scr + S5_LOG);
    a.b(&[0x42]).mov_abs_edx(scr + S5_LOG_N);
    a.b(&[0x0f, 0xb6, 0x81]).d(scr + S5_TAB);
    a.ret(4);
    patch(&mut o, 0x0042_fcb0, a.finish());
    let town = o.alloc(0xb4).unwrap();
    let mut xs = Xs(0x5eed_0005);
    let mut placed = 0;
    for case in 0..2500 {
        let set = xs.below(2) as usize;
        let year = xs.range(1795, 2000);
        let region = xs.chance(35);
        let house = xs.chance(20);
        let mut weights = [0u16; BUILDING_TYPES];
        let density = [5, 30, 90][xs.below(3) as usize];
        for w in weights.iter_mut() {
            if xs.chance(density) {
                *w = [1, 2, 10, 100, 100, 400, 1000][xs.below(7) as usize];
            }
        }
        let mut world = PickWorld {
            year,
            set1: set == 1,
            scarcity: [100; BUILDING_TYPES],
            disabled: [false; BUILDING_TYPES],
        };
        for t in 0..BUILDING_TYPES {
            world.scarcity[t] = [100, 100, 250, 166, 500][xs.below(5) as usize];
            world.disabled[t] = xs.chance(8);
        }
        let succ_p = [0, 5, 50, 100][xs.below(4) as usize];
        let succ: Vec<bool> = (0..BUILDING_TYPES).map(|_| xs.chance(succ_p)).collect();
        let n = xs.below(20) as usize;
        let chain: Vec<u8> = (0..n).map(|_| xs.below(53) as u8).collect();

        o.write_mem(town, &[0u8; 0xb4]).unwrap();
        w8(&mut o, town + 0x10, region as u8);
        w16(&mut o, town + 2, if n == 0 { 0 } else { 1 });
        for (t, w) in weights.iter().enumerate() {
            w16(&mut o, town + 0x4a + 2 * t as u32, *w);
        }
        for (i, ty) in chain.iter().enumerate() {
            let rec = list.rec(1 + i as u32);
            o.write_mem(rec, &[0u8; 0x9f]).unwrap();
            w8(&mut o, rec + 7, *ty);
            let next = if i + 1 < n { 2 + i as u32 } else { 0 };
            w16(&mut o, rec + 4, next as u16);
        }
        for t in 0..BUILDING_TYPES as u32 {
            w8(
                &mut o,
                G_TYPE_FLAGS + 9 * t,
                world.disabled[t as usize] as u8,
            );
            w32(
                &mut o,
                G_TYPE_FLAGS + 9 * t + 1,
                world.scarcity[t as usize] as u32,
            );
            w8(&mut o, scr + S5_TAB + t, succ[t as usize] as u8);
        }
        w32(&mut o, scr + S5_LOG_N, 0);
        w8(&mut o, G_INDUSTRY_SET, set as u8);
        w32(&mut o, G_DATE, year as u32 | 0x0501_0000);
        let seed = xs.next();
        set_rng(&mut o, seed);
        let ret = o
            .call_thiscall(0x0042_ffc0, town, &[0, house as u32])
            .unwrap();
        let n_log = r32(&o, scr + S5_LOG_N) as usize;
        let log: Vec<u32> = (0..n_log)
            .map(|i| r32(&o, scr + S5_LOG + 4 * i as u32))
            .collect();
        let got = (ret & 0xff, get_rng(&o), log);

        let mut counts = [0u8; BUILDING_TYPES];
        for ty in &chain {
            counts[usize::from(*ty)] += 1;
        }
        for (t, c) in counts.iter_mut().enumerate() {
            if data.industries[set][t].slots[0].mode != 0 {
                *c = 0;
            }
        }
        let mut rng = GameRng::new(seed);
        let mut want_log = Vec::new();
        let ok = town_add_building(
            &mut rng,
            &tab,
            &world,
            region,
            &weights,
            &counts,
            house,
            &mut |_, t| {
                want_log.push(t as u32);
                succ[t]
            },
        );
        placed += usize::from(ok);
        let want = (u32::from(ok), rng.state(), want_log);
        assert_eq!(
            got, want,
            "case {case}: set {set} year {year} region {region} house {house} seed {seed:#x}"
        );
    }
    assert!(placed > 200, "coverage: {placed}");
}

// ---- 6. site search (0x42fcb0) ----------------------------------------------------------------------------

const S6_CAN_N: u32 = 0;
const S6_CRE_N: u32 = 4;
const S6_CAN_LOG: u32 = 0x100;
const S6_CRE_LOG: u32 = 0x20_0100;
const SIDE: u32 = 256;

#[test]
fn town_place_building_matches_original() {
    let Some(mut o) = oracle() else { return };
    let exe = load_exe();
    let tab = GrowthTables::load(&exe).unwrap();
    let scr = o.alloc(0x21_0000).unwrap();
    let pa = o.alloc(SIDE * SIDE).unwrap();
    let sc = o.alloc(SIDE * SIDE).unwrap();
    let cp = o.alloc(SIDE * SIDE).unwrap();
    let tiles = o.alloc(SIDE * SIDE * 15).unwrap();
    w32(&mut o, G_TILES, tiles);
    w32(&mut o, G_MAP_W, SIDE);
    w32(&mut o, G_MAP_H, SIDE);

    // place_at: PA[(y << 8) + x]
    let mut a = Asm::default();
    a.b(&[0xc1, 0xe2, 0x08, 0x01, 0xca, 0x0f, 0xb6, 0x82])
        .d(pa)
        .ret(4);
    patch(&mut o, 0x0043_0c20, a.finish());
    // site classes: SC byte -> (b & 3, (b >> 2) & 3)
    let mut a = Asm::default();
    a.b(&[0xc1, 0xe2, 0x08, 0x01, 0xca, 0x0f, 0xb6, 0x82]).d(sc);
    a.b(&[
        0x89, 0xc1, 0x83, 0xe1, 0x03, 0x8b, 0x54, 0x24, 0x04, 0x89, 0x0a,
    ]);
    a.b(&[
        0xc1, 0xe8, 0x02, 0x83, 0xe0, 0x03, 0x8b, 0x54, 0x24, 0x08, 0x89, 0x02,
    ]);
    a.ret(8);
    patch(&mut o, 0x0042_fb10, a.finish());
    // can_place: log (x, y, shape, flag) and return CP[(y << 8) + x]
    let mut a = Asm::default();
    a.mov_eax_abs(scr + S6_CAN_N).b(&[0xc1, 0xe0, 0x04]);
    a.b(&[0x89, 0x88]).d(scr + S6_CAN_LOG);
    a.b(&[0x89, 0x90]).d(scr + S6_CAN_LOG + 4);
    a.b(&[0xc1, 0xe2, 0x08, 0x01, 0xca, 0x0f, 0xb6, 0x92]).d(cp);
    a.b(&[0x52, 0x8b, 0x4c, 0x24, 0x08]);
    a.mov_eax_abs(scr + S6_CAN_N).b(&[0xc1, 0xe0, 0x04]);
    a.b(&[0x89, 0x88]).d(scr + S6_CAN_LOG + 8);
    a.b(&[0x8b, 0x4c, 0x24, 0x24]);
    a.b(&[0x89, 0x88]).d(scr + S6_CAN_LOG + 12);
    a.b(&[0x83, 0x05]).d(scr + S6_CAN_N).b(&[1]);
    a.b(&[0x58]).ret(0x20);
    patch(&mut o, 0x0040_87a0, a.finish());
    // create: log (x, y, shape, town)
    let mut a = Asm::default();
    a.mov_eax_abs(scr + S6_CRE_N).b(&[0xc1, 0xe0, 0x04]);
    a.b(&[0x89, 0x88]).d(scr + S6_CRE_LOG);
    a.b(&[0x89, 0x90]).d(scr + S6_CRE_LOG + 4);
    a.b(&[0x8b, 0x4c, 0x24, 0x04, 0x89, 0x88])
        .d(scr + S6_CRE_LOG + 8);
    a.b(&[0x8b, 0x4c, 0x24, 0x10, 0x89, 0x88])
        .d(scr + S6_CRE_LOG + 12);
    a.b(&[0x83, 0x05]).d(scr + S6_CRE_N).b(&[1]);
    a.b(&[0xb8, 1, 0, 0, 0]).ret(0x14);
    patch(&mut o, 0x0040_9010, a.finish());

    struct Maps {
        pa: Vec<u8>,
        sc: Vec<u8>,
        cp: Vec<u8>,
        tile_id: Vec<u16>,
        classes: Vec<i32>,
        can: Vec<[i32; 4]>,
        created: Vec<[i32; 4]>,
        town_id: i32,
    }
    impl SiteEnv for Maps {
        fn place_at(&mut self, x: i32, y: i32) -> u16 {
            u16::from(self.pa[((y << 8) + x) as usize])
        }
        fn site_classes(&mut self, x: i32, y: i32) -> (i32, i32) {
            let b = i32::from(self.sc[((y << 8) + x) as usize]);
            (b & 3, (b >> 2) & 3)
        }
        fn ground_class(&mut self, x: i32, y: i32) -> i32 {
            self.classes[usize::from(self.tile_id[((y << 8) + x) as usize] >> 6)]
        }
        fn can_place(&mut self, x: i32, y: i32, shape: usize, early: bool) -> bool {
            self.can.push([x, y, shape as i32, i32::from(early)]);
            self.cp[((y << 8) + x) as usize] != 0
        }
        fn create(&mut self, x: i32, y: i32, shape: usize) {
            self.created.push([x, y, shape as i32, self.town_id]);
        }
    }

    let mut xs = Xs(0xa11c_e004);
    let mut maps = Maps {
        pa: vec![],
        sc: vec![],
        cp: vec![],
        tile_id: vec![],
        classes: vec![],
        can: vec![],
        created: vec![],
        town_id: 0,
    };
    let town = o.alloc(0xb4).unwrap();
    let mut done = 0;
    let mut snow = 0;
    for case in 0..3000 {
        if case % 60 == 0 {
            // New random maps.
            let own = [30, 70, 95][xs.below(3) as usize];
            maps.pa = (0..SIDE * SIDE)
                .map(|_| if xs.chance(own) { 1 } else { xs.below(4) as u8 })
                .collect();
            maps.sc = (0..SIDE * SIDE).map(|_| xs.below(16) as u8).collect();
            let cpp = [2, 20, 60][xs.below(3) as usize];
            maps.cp = (0..SIDE * SIDE).map(|_| xs.chance(cpp) as u8).collect();
            maps.tile_id = (0..SIDE * SIDE)
                .map(|_| (xs.below(1024) << 6) as u16 | xs.below(64) as u16)
                .collect();
            maps.classes = (0..1024).map(|_| xs.range(0, 18)).collect();
            o.write_mem(pa, &maps.pa).unwrap();
            o.write_mem(sc, &maps.sc).unwrap();
            o.write_mem(cp, &maps.cp).unwrap();
            let mut tb = vec![0u8; (SIDE * SIDE * 15) as usize];
            for (i, id) in maps.tile_id.iter().enumerate() {
                tb[i * 15..i * 15 + 2].copy_from_slice(&id.to_le_bytes());
            }
            o.write_mem(tiles, &tb).unwrap();
            let cls: Vec<u8> = maps.classes.iter().flat_map(|c| c.to_le_bytes()).collect();
            o.write_mem(G_CLASS_TABLE, &cls).unwrap();
        }
        let region = xs.chance(30);
        let ty = [
            0x12usize, 0x12, 0x1c, 0x27, 0x28, 0x29, 10, 11, 0x10, 0x1d, 0x21,
        ][xs.below(11) as usize];
        let ty = if xs.chance(50) {
            ty
        } else {
            xs.range(1, 52) as usize
        };
        let ty = if tab.shape_type.contains(&(ty as u8)) {
            ty
        } else {
            0x12
        };
        let geom = PlaceGeom {
            id: 1,
            region,
            size_class: xs.below(5) as u8,
            cx: xs.range(40, 215),
            cy: xs.range(40, 215),
            bbox: {
                let x0 = xs.range(20, 120);
                let y0 = xs.range(20, 120);
                [x0, y0, x0 + xs.range(0, 100), y0 + xs.range(0, 100)]
            },
        };
        o.write_mem(town, &[0u8; 0xb4]).unwrap();
        w16(&mut o, town, geom.id);
        w8(&mut o, town + 0x10, region as u8);
        w8(&mut o, town + 0x2c, geom.size_class);
        w16(&mut o, town + 4, geom.cx as u16);
        w16(&mut o, town + 6, geom.cy as u16);
        for (k, v) in geom.bbox.iter().enumerate() {
            w16(&mut o, town + 8 + 2 * k as u32, *v as u16);
        }
        w32(&mut o, scr + S6_CAN_N, 0);
        w32(&mut o, scr + S6_CRE_N, 0);
        let seed = xs.next();
        set_rng(&mut o, seed);
        let ret = o.call_thiscall(0x0042_fcb0, town, &[ty as u32]).unwrap();
        let rd = |o: &Oracle, base: u32, n: u32| -> Vec<[i32; 4]> {
            (0..n)
                .map(|i| {
                    let a = base + 16 * i;
                    [
                        r32(o, a) as i32,
                        r32(o, a + 4) as i32,
                        r32(o, a + 8) as i32,
                        r32(o, a + 12) as i32,
                    ]
                })
                .collect()
        };
        let got = (
            ret & 0xff,
            get_rng(&o),
            rd(&o, scr + S6_CAN_LOG, r32(&o, scr + S6_CAN_N)),
            rd(&o, scr + S6_CRE_LOG, r32(&o, scr + S6_CRE_N)),
        );
        maps.can.clear();
        maps.created.clear();
        maps.town_id = 1;
        let mut rng = GameRng::new(seed);
        let ok = town_place_building(&mut rng, &tab, &geom, ty, &mut maps);
        let want = (
            u32::from(ok),
            rng.state(),
            maps.can.clone(),
            maps.created.clone(),
        );
        let diff = |a: &[[i32; 4]], b: &[[i32; 4]]| {
            (a.len(), b.len(), a.iter().zip(b).position(|(x, y)| x != y))
        };
        assert!(
            got == want,
            "case {case}: type {ty:#x} {geom:?} seed {seed:#x}: ret {}/{} rng {:#x}/{:#x} can {:?} create {:?}",
            got.0,
            want.0,
            got.1,
            want.1,
            diff(&got.2, &want.2),
            diff(&got.3, &want.3)
        );
        done += usize::from(ok);
        snow += usize::from(maps.can.len() > 1);
    }
    assert!(
        done > 300 && snow > 100,
        "coverage: placed {done}, retried {snow}"
    );
}

// ---- 7. removal pick (0x4302c0) and shape pick (0x40ba00) ------------------------------------------------

#[test]
fn pick_worst_building_matches_original() {
    let Some(mut o) = oracle() else { return };
    let list = List::new(&mut o, G_INDUSTRIES, 0x9f, 40);
    let scr = o.alloc(0x100).unwrap();
    // Stub over 0x40adb0: log (this, param).
    let mut a = Asm::default();
    a.mov_eax_abs(scr)
        .b(&[0xc1, 0xe0, 0x03, 0x89, 0x88])
        .d(scr + 0x10);
    a.b(&[0x8b, 0x54, 0x24, 0x04, 0x89, 0x90]).d(scr + 0x14);
    a.b(&[0x83, 0x05]).d(scr).b(&[1]).ret(4);
    patch(&mut o, 0x0040_adb0, a.finish());
    let town = o.alloc(0xb4).unwrap();
    let mut xs = Xs(0x77aa_1234);
    let mut picked = 0;
    for case in 0..5000 {
        let n = xs.below(25) as usize;
        let set1 = xs.chance(10);
        let house = xs.chance(50);
        let cands: Vec<RemovalCandidate> = (0..n)
            .map(|_| RemovalCandidate {
                is_house: xs.chance(50),
                has_station: xs.chance(10),
                owned: xs.chance(10),
                rating: if xs.chance(20) {
                    0
                } else {
                    xs.below(300) as u16
                },
            })
            .collect();
        o.write_mem(town, &[0u8; 0xb4]).unwrap();
        w16(&mut o, town + 2, if n == 0 { 0 } else { 1 });
        for (i, c) in cands.iter().enumerate() {
            let rec = list.rec(1 + i as u32);
            o.write_mem(rec, &[0u8; 0x9f]).unwrap();
            w8(&mut o, rec + 7, if c.is_house { 0x12 } else { 5 });
            w16(&mut o, rec + 8, u16::from(c.has_station));
            w16(&mut o, rec + 0x97, u16::from(c.owned));
            w16(&mut o, rec + 0x4b, c.rating);
            let next = if i + 1 < n { 2 + i as u32 } else { 0 };
            w16(&mut o, rec + 4, next as u16);
        }
        w8(&mut o, G_INDUSTRY_SET, set1 as u8);
        w32(&mut o, scr, 0);
        let seed = xs.next();
        set_rng(&mut o, seed);
        o.call_thiscall(0x0043_02c0, town, &[house as u32]).unwrap();
        let calls = r32(&o, scr);
        let got = (
            get_rng(&o),
            calls,
            if calls > 0 { r32(&o, scr + 0x10) } else { 0 },
            if calls > 0 { r32(&o, scr + 0x14) } else { 0 },
        );
        let mut rng = GameRng::new(seed);
        let pick = pick_worst_building(&mut rng, &cands, house, set1);
        let want = (
            rng.state(),
            u32::from(pick.is_some()),
            pick.map_or(0, |i| list.rec(1 + i as u32)),
            pick.map_or(0, |_| 1),
        );
        assert_eq!(
            got, want,
            "case {case}: house {house} set1 {set1} {cands:?}"
        );
        picked += usize::from(pick.is_some());
    }
    assert!(picked > 1000, "coverage: {picked}");
}

#[test]
fn shape_pick_matches_original() {
    let Some(mut o) = oracle() else { return };
    let exe = load_exe();
    let tab = GrowthTables::load(&exe).unwrap();
    let mut xs = Xs(0x1111_2222);
    for case in 0..4000 {
        let ty = xs.range(0, 0x40) as usize;
        let eff = if ty > 0x30 { 0x12 } else { ty };
        if !tab.shape_type.contains(&(eff as u8)) {
            continue;
        }
        let randomize = xs.chance(60);
        let seed = xs.next();
        set_rng(&mut o, seed);
        let ret = o
            .call_fastcall(0x0040_ba00, ty as u32, randomize as u32, &[])
            .unwrap();
        let mut rng = GameRng::new(seed);
        let want = shape_pick(&mut rng, &tab.shape_type, ty, randomize);
        assert_eq!(
            (ret as usize, get_rng(&o)),
            (want, rng.state()),
            "case {case}: type {ty:#x} randomize {randomize}"
        );
    }
}

// ---- 8. monthly bounty offer (0x431660) ---------------------------------------------------------------------

/// A pool list with the linked-list header `0x4f19e0` walks: ids `1..=n` in order; `set_len`
/// re-links it for another length (up to the capacity).
struct Linked {
    list: List,
    hdr: u32,
    next: u32,
    prev: u32,
}
impl Linked {
    fn new(o: &mut Oracle, global: u32, stride: u32, cap: u32) -> Self {
        let list = List::new(o, global, stride, cap);
        let hdr = r32(o, global);
        let next = o.alloc(2 * (cap + 2)).unwrap();
        let prev = o.alloc(2 * (cap + 2)).unwrap();
        w32(o, hdr + 0x2c, next);
        w32(o, hdr + 0x30, prev);
        Linked {
            list,
            hdr,
            next,
            prev,
        }
    }
    fn set_len(&self, o: &mut Oracle, n: u32) {
        for i in 1..=n {
            w16(o, self.next + 2 * i, if i < n { i as u16 + 1 } else { 0 });
            w16(o, self.prev + 2 * i, (i - 1) as u16);
        }
        w32(o, self.hdr + 0x14, n);
        w16(o, self.hdr + 0x24, u16::from(n > 0));
        w16(o, self.hdr + 0x26, n as u16);
    }
    fn rec(&self, id: u32) -> u32 {
        self.list.rec(id)
    }
}

#[test]
fn town_bounty_offer_matches_original() {
    let Some(mut o) = oracle() else { return };
    let exe = load_exe();
    let tab = GrowthTables::load(&exe).unwrap();
    let mut xs = Xs(0x0ff3_7007);
    // The message helpers are not interesting here: return immediately.
    patch(&mut o, 0x0047_5da0, vec![0xc2, 0x0c, 0x00]);
    patch(&mut o, 0x0047_6910, vec![0xc3]);
    patch(&mut o, 0x0047_5fa0, vec![0xc2, 0x20, 0x00]);
    patch(&mut o, 0x0046_9320, vec![0xc2, 0x10, 0x00]);
    let mut offered = 0;
    let towns = Linked::new(&mut o, 0x0057_4c5c, 180, 25);
    let stations = Linked::new(&mut o, 0x0057_4c60, 0x4c7, 12);
    for case in 0..3000 {
        let np = xs.range(0, 25) as u32;
        let ns = xs.range(0, 12) as u32;
        towns.set_len(&mut o, np);
        stations.set_len(&mut o, ns);
        let mut places = Vec::new();
        for i in 0..np {
            let kind = [0u8, 0, 0, 1, 2][xs.below(5) as usize];
            let cx = xs.range(10, 240);
            let cy = xs.range(10, 240);
            let p = OfferPlace {
                kind,
                bounty: if xs.chance(30) {
                    xs.range(1, 500_000)
                } else {
                    0
                },
                size_class: xs.below(5) as u8,
                cx,
                cy,
                bbox: [
                    cx - xs.range(0, 20),
                    cy - xs.range(0, 20),
                    cx + xs.range(0, 20),
                    cy + xs.range(0, 20),
                ],
            };
            let rec = towns.rec(1 + i);
            o.write_mem(rec, &[0u8; 180]).unwrap();
            w8(&mut o, rec + 0x10, p.kind);
            w32(&mut o, rec + 0x36, p.bounty as u32);
            w8(&mut o, rec + 0x2c, p.size_class);
            w16(&mut o, rec + 4, p.cx as u16);
            w16(&mut o, rec + 6, p.cy as u16);
            for (k, v) in p.bbox.iter().enumerate() {
                w16(&mut o, rec + 8 + 2 * k as u32, *v as u16);
            }
            places.push(p);
        }
        let mut sts = Vec::new();
        for i in 0..ns {
            let near = places.get(xs.below(np.max(1)) as usize);
            let (sx, sy) = match near {
                Some(p) if xs.chance(70) => (p.cx + xs.range(-12, 12), p.cy + xs.range(-12, 12)),
                _ => (xs.range(0, 255), xs.range(0, 255)),
            };
            let rec = stations.rec(1 + i);
            o.write_mem(rec, &[0u8; 0x4c7]).unwrap();
            w32(&mut o, rec + 10, sx as u32);
            w32(&mut o, rec + 14, sy as u32);
            sts.push((sx, sy));
        }
        let seed = xs.next();
        set_rng(&mut o, seed);
        o.call_cdecl(0x0043_1660, &[]).unwrap();
        // Which place got a bounty (a place whose +0x36 changed from 0).
        let mut got = None;
        for (i, p) in places.iter().enumerate() {
            let b = r32(&o, towns.rec(1 + i as u32) + 0x36) as i32;
            if b != p.bounty {
                got = Some((i, b));
            }
        }
        let mut rng = GameRng::new(seed);
        let want = town_bounty_offer(&mut rng, &tab, &places, &sts);
        assert_eq!(
            (got, get_rng(&o)),
            (want, rng.state()),
            "case {case}: {places:?} stations {sts:?}"
        );
        offered += usize::from(want.is_some());
    }
    assert!(offered > 500, "coverage: {offered}");
}

// ---- 9. initial slot clocks of a new building (0x40bb40) ----------------------------------------------------

#[test]
fn initial_slot_clocks_match_original() {
    let Some(mut o) = oracle() else { return };
    let exe = load_exe();
    let data = GameData::load(&exe).unwrap();
    let list = List::new(&mut o, G_INDUSTRIES, 0x9f, 2);
    let b = list.rec(1);
    let mut xs = Xs(0x6c10_cc01);
    for case in 0..4000 {
        let set = xs.below(2) as usize;
        let ty = xs.below(53) as usize;
        let year = xs.range(1795, 2000);
        let now = xs.next() >> xs.below(8);
        let decorative = xs.chance(20);
        o.write_mem(b, &[0u8; 0x9f]).unwrap();
        w8(&mut o, b + 7, ty as u8);
        w8(&mut o, b + 0x2e, decorative as u8);
        w8(&mut o, G_INDUSTRY_SET, set as u8);
        w32(&mut o, G_DATE, year as u32 | 0x0501_0000);
        w32(&mut o, G_NOW, now);
        let seed = xs.next();
        set_rng(&mut o, seed);
        o.call_fastcall(0x0040_bb40, b, 0, &[]).unwrap();
        let got = ([0, 1, 2, 3].map(|k| r32(&o, b + 0x53 + 4 * k)), get_rng(&o));
        let mut rng = GameRng::new(seed);
        let clocks =
            initial_slot_clocks(&mut rng, &data.industries[set][ty], year, now, decorative);
        assert_eq!(
            got,
            (clocks, rng.state()),
            "case {case}: set {set} type {ty} year {year} now {now} decorative {decorative}"
        );
    }
}
