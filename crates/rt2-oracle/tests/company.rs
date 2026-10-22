//! Differential tests for the company and person rules of docs/spec/finance.md sections 13..17:
//! the manager candidate pick (`0x438450`), the investor-confidence multiplier (`0x4a8150`), the
//! founding-capital sliders (`0x4bc990`, `0x4bc7d0`) and the annual company review (`0x43c760`).
//! Each test lays the records out in emulated memory, calls the original and compares with a Rust
//! reference written in this file. The x87 control word is the one the Oracle starts with (CW
//! 0x27f, 53-bit precision). Skipped when the executable is absent.
//!
//! The functions that call into the UI or into other subsystems are run with their callees
//! replaced by small machine-code stubs written over the callee's body in the emulated image: the
//! stubs count their calls and log their arguments in emulated memory.

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
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
    fn chance(&mut self, pct: u32) -> bool {
        self.next() % 100 < pct
    }
    /// Uniform in [lo, hi).
    fn real(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * f64::from(self.next() >> 8) / f64::from(1u32 << 24)
    }
}

// ---- memory helpers ----------------------------------------------------------------------------

fn w32(o: &mut Oracle, a: u32, v: u32) {
    o.write_u32(a, v).unwrap();
}
fn w16(o: &mut Oracle, a: u32, v: u16) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn w8(o: &mut Oracle, a: u32, v: u8) {
    o.write_mem(a, &[v]).unwrap();
}
fn wf32(o: &mut Oracle, a: u32, v: f32) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn wf64(o: &mut Oracle, a: u32, v: f64) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn r32(o: &Oracle, a: u32) -> u32 {
    o.read_u32(a).unwrap()
}
fn ri32(o: &Oracle, a: u32) -> i32 {
    o.read_u32(a).unwrap() as i32
}
fn ri16(o: &Oracle, a: u32) -> i16 {
    i16::from_le_bytes(o.read_mem(a, 2).unwrap().try_into().unwrap())
}
fn rf32(o: &Oracle, a: u32) -> f32 {
    f32::from_le_bytes(o.read_mem(a, 4).unwrap().try_into().unwrap())
}
fn rf64(o: &Oracle, a: u32) -> f64 {
    f64::from_le_bytes(o.read_mem(a, 8).unwrap().try_into().unwrap())
}

/// A pool as seen by `0x4f18b0` (record `id` at `base + id * stride`) and by the linked-list
/// walkers `0x4f19e0` / `0x4f1960` (ids `1..=n` in order).
#[derive(Clone, Copy)]
struct Pool {
    hdr: u32,
    base: u32,
    stride: u32,
    next: u32,
    prev: u32,
}
impl Pool {
    fn new(o: &mut Oracle, global: u32, stride: u32, cap: u32) -> Self {
        let base = o.alloc(stride * (cap + 1)).unwrap();
        let hdr = o.alloc(0x40).unwrap();
        let next = o.alloc(2 * (cap + 2)).unwrap();
        let prev = o.alloc(2 * (cap + 2)).unwrap();
        o.write_mem(base, &vec![0u8; (stride * (cap + 1)) as usize])
            .unwrap();
        o.write_mem(hdr, &[0u8; 0x40]).unwrap();
        w32(o, hdr + 8, stride);
        w32(o, hdr + 0x28, base);
        w32(o, hdr + 0x2c, next);
        w32(o, hdr + 0x30, prev);
        w32(o, global, hdr);
        Pool {
            hdr,
            base,
            stride,
            next,
            prev,
        }
    }
    fn rec(&self, id: u32) -> u32 {
        self.base + id * self.stride
    }
    /// Links the records `1..=n` and writes each record's own id at `+0`.
    fn set_len(&self, o: &mut Oracle, n: u32) {
        for i in 1..=n {
            w16(o, self.next + 2 * i, if i < n { i as u16 + 1 } else { 0 });
            w16(o, self.prev + 2 * i, (i - 1) as u16);
            w32(o, self.rec(i), i);
        }
        w32(o, self.hdr + 0x14, n);
        w16(o, self.hdr + 0x24, u16::from(n > 0));
        w16(o, self.hdr + 0x26, n as u16);
    }
}

// ---- globals -----------------------------------------------------------------------------------

const RNG_STATE: u32 = 0x0055_d2c0;
const GAME_DATE: u32 = 0x0057_04f8; // u16 year, u8 month, u8 day
const G_LOCK: u32 = 0x0057_48f8;
const G_EDITOR: u32 = 0x0057_3d7a;
const G_CHAIRMAN_FIRE: u32 = 0x0057_06ba; // restriction "Being fired as chairman"
const G_FIN_ADVANCED: u32 = 0x0057_3d75; // financial model >= advanced
const G_OPTIONS: u32 = 0x0057_4872; // bits 3..5: financial model
const G_REF_SPEED: u32 = 0x0057_3e08; // u8
const G_MANAGER_USE: u32 = 0x0057_3e09; // u8[40]: company id that employs manager i
const G_MANAGER_AVAIL: u32 = 0x0057_0638; // u8[40]
const G_MANAGER_CURSOR: u32 = 0x0056_5154;
const MANAGER_TABLE: u32 = 0x0054_fdad; // 40 x 0x11 bytes, +0 salary i32
const G_LOCAL_PERSON: u32 = 0x0056_6ca4;
const G_LOCAL_COMPANY: u32 = 0x0057_4414;
const G_NOEDIT_FLAG: u32 = 0x0057_0237;
const G_PERSONS: u32 = 0x0057_4c68;
const G_COMPANIES: u32 = 0x0057_4c6c;
const G_STATIONS: u32 = 0x0057_4c60;
const G_SET_COUNT: u32 = 0x0057_0729;
const G_SET_IDS: u32 = 0x0057_072d; // u16[36]
const G_NET_OPTIONS: u32 = 0x0057_4bc8;

const PICK_MANAGER: u32 = 0x0043_8450; // thiscall(company, forced) ret 4
const CREDIT_MULT: u32 = 0x004a_8150; // fastcall(ECX = person) -> ST0
const SLIDER_PLAYER: u32 = 0x004b_c990; // fastcall(ECX = reset)
const SLIDER_OUTSIDE: u32 = 0x004b_c7d0; // fastcall(ECX = reset)
const ANNUAL_REVIEW: u32 = 0x0043_c760; // fastcall(ECX = company)
const METRIC: u32 = 0x0043_ad60; // thiscall(i) -> ST0

fn set_rng(o: &mut Oracle, s: u32) {
    w32(o, RNG_STATE, s);
}
fn get_rng(o: &Oracle) -> u32 {
    r32(o, RNG_STATE)
}

fn patch(o: &mut Oracle, at: u32, code: &[u8]) {
    o.write_mem(at, code).unwrap();
}

fn write_date(o: &mut Oracle, year: i32, month: i32, day: i32) {
    w32(
        o,
        GAME_DATE,
        year as u32 | (month as u32) << 16 | (day as u32) << 24,
    );
}

/// C-style signed division and remainder (truncating toward zero).
fn cdiv(a: i32, b: i32) -> i32 {
    a.wrapping_div(b)
}
fn crem(a: i32, b: i32) -> i32 {
    a.wrapping_rem(b)
}
/// `__ftol` (truncation toward zero).
fn ftol(x: f64) -> i32 {
    x as i64 as i32
}

// ---- 1. manager candidate pick (0x438450) ------------------------------------------------------

const CO_SIZE: u32 = 0x4e86;
const CO_ID: u32 = 0x00;
const CO_CHAIRMAN: u32 = 0x23;
const CO_SHARES: u32 = 0x2c;
const CO_INIT_PRICE: u32 = 0x3c;
const CO_SALARY: u32 = 0x134;
const CO_CAND0: u32 = 0x140;
const CO_CAND1: u32 = 0x144;
const CO_FOUNDED: u32 = 0x148;
const CO_SPEED_N: u32 = 0x196;
const CO_SPEED_SUM: u32 = 0x19a;
const CO_ACTIVE: u32 = 0x16e;
const CO_REP: u32 = 0x46a;
const CO_CHAIR_SINCE: u32 = 0x470; // i16
const CO_LAST_BONUS: u32 = 0x46e; // i16
const CO_WARN_YEAR: u32 = 0x472; // i16
const CO_BONUS: u32 = 0x474;
const CO_STANDING: u32 = 0x47a; // i16 per territory, stride 0x1ed
const CO_STANDING_ALL: u32 = 0x4229;
const CO_REV_MULT: u32 = 0x4248; // f32
const CO_CATS: u32 = 0x4286;
const CO_ARCH_CATS: u32 = 0x42ee; // [10 years][13]
const CO_LIFE_CATS: u32 = 0x46fe;
const CO_ACCT: u32 = 0x4766; // [8]
const CO_ARCH_ACCT: u32 = 0x47a6; // [10 years][8]
const CO_ARCH_METRICS: u32 = 0x49b6; // [k][14], k = years ago (0x70 bytes)

struct PickIn {
    year: i32,
    month: i32,
    founded: i32,
    cats: [f64; 13],
    arch: [[f64; 13]; 10],
    c140: i32,
    c144: i32,
    uses: [u8; 40],
    avail: [u8; 40],
    sal: [i32; 40],
    lock: bool,
}

/// Reference of `0x438450`: returns the chosen manager index. `cursor` is `[0x565154]`.
fn pick_ref(p: &PickIn, forced: i32, rng: &mut GameRng, cursor: &mut i32) -> i32 {
    let y = p.year - i32::from(p.month != 12);
    let mut acc = 0.0f64;
    let mut cnt = 0i32;
    for i in 0..3 {
        if p.founded <= y {
            let mut rev = 0.0f64;
            for k in 0..5 {
                rev += if y == p.year {
                    p.cats[k]
                } else {
                    p.arch[(p.year - y - 1) as usize][k]
                };
            }
            cnt += 3 - i;
            acc += f64::from(3 - i) * rev;
        }
    }
    let avg = if cnt != 0 { acc / f64::from(cnt) } else { acc };
    let r = rng.range(25, 180);
    let mut v = ((avg * 1.4) * 0.01) * f64::from(r) * 0.01;
    if v < 8000.0 {
        v = f64::from(rng.range(1000, 12000));
    }
    let mut c;
    if p.lock {
        *cursor = (*cursor + 1) % 40;
        c = *cursor;
    } else {
        c = rng.range(0, 39);
    }
    if forced != -1 {
        c = forced;
    }
    let mut attempts = 4000i32;
    let (mut f1, mut f2) = (true, true);
    let (mut lo, mut hi) = (0.8f32, f32::from_bits(0x3fa6_6666));
    loop {
        let cu = c as usize;
        let mut reject = p.uses[cu] != 0 || c == p.c140 || c == p.c144;
        if !reject && f2 {
            let ratio = f64::from(p.sal[cu]) / v;
            if ratio <= f64::from(lo) || ratio >= f64::from(hi) {
                reject = true;
            }
        }
        if !reject && f1 && p.avail[cu] == 0 {
            reject = true;
        }
        if !reject {
            return c;
        }
        if attempts & 0xf == 0xf {
            lo = (f64::from(lo) * 0.96) as f32;
            hi = (f64::from(hi) * 1.04) as f32;
            if attempts <= 1000 {
                f2 = false;
            }
            if attempts <= 500 {
                f1 = false;
            }
        }
        if p.lock {
            *cursor = (*cursor + 1) % 40;
            c = *cursor;
        } else if attempts > 60 {
            c = rng.range(0, 39);
        } else {
            c = (c + 1) % 40;
        }
        attempts -= 1;
    }
}

fn manager_salaries(o: &Oracle) -> [i32; 40] {
    let mut s = [0i32; 40];
    for (i, v) in s.iter_mut().enumerate() {
        *v = ri32(o, MANAGER_TABLE + 0x11 * i as u32);
    }
    s
}

fn rand_cats(xs: &mut Xs, scale: f64) -> [f64; 13] {
    let mut c = [0.0; 13];
    for (i, v) in c.iter_mut().enumerate() {
        let mag = if xs.chance(15) {
            0.0
        } else {
            xs.real(0.0, scale).floor()
        };
        *v = if i < 5 { mag } else { -mag * 0.6 };
    }
    c
}

#[test]
fn manager_pick_matches_original() {
    let Some(mut o) = oracle() else { return };
    let co = o.alloc(CO_SIZE).unwrap();
    let sal = manager_salaries(&o);
    assert!(sal.iter().all(|&s| s > 0));
    let mut xs = Xs(0x0ddb_a115);
    let mut picked = [0u32; 40];
    for case in 0..6000 {
        let year = xs.range(1850, 2000);
        let month = if xs.chance(50) { 12 } else { xs.range(1, 11) };
        let founded = year - if xs.chance(30) { 0 } else { xs.range(0, 12) };
        let scale = [1.0e4, 1.0e5, 1.0e6, 1.0e7][xs.below(4) as usize];
        let mut arch = [[0.0; 13]; 10];
        for y in &mut arch {
            *y = rand_cats(&mut xs, scale);
        }
        let mut uses = [0u8; 40];
        let busy = xs.below(36);
        for _ in 0..busy {
            uses[xs.below(40) as usize] = 1 + xs.below(20) as u8;
        }
        let mut avail = [1u8; 40];
        if xs.chance(50) {
            for a in &mut avail {
                *a = u8::from(!xs.chance(40));
            }
        }
        // Keep a few free managers so that the search ends.
        for _ in 0..4 {
            let f = xs.below(40) as usize;
            uses[f] = 0;
            avail[f] = 1;
        }
        let mut c140 = if xs.chance(30) { -1 } else { xs.range(0, 39) };
        let mut c144 = if xs.chance(30) { -1 } else { xs.range(0, 39) };
        let eligible = (0..40)
            .filter(|&i| uses[i] == 0 && avail[i] == 1 && i as i32 != c140 && i as i32 != c144)
            .count();
        if eligible == 0 {
            c140 = -1;
            c144 = -1;
        }
        let inp = PickIn {
            year,
            month,
            founded,
            cats: rand_cats(&mut xs, scale),
            arch,
            c140,
            c144,
            uses,
            avail,
            sal,
            lock: xs.chance(15),
        };
        let forced = if xs.chance(20) { xs.range(0, 39) } else { -1 };
        let cursor0 = xs.range(0, 39);
        let seed = xs.next();

        write_date(&mut o, year, month, 28);
        w32(&mut o, G_LOCK, u32::from(inp.lock));
        w32(&mut o, G_MANAGER_CURSOR, cursor0 as u32);
        o.write_mem(co, &vec![0u8; CO_SIZE as usize]).unwrap();
        w32(&mut o, co + CO_FOUNDED, founded as u32);
        w32(&mut o, co + CO_CAND0, inp.c140 as u32);
        w32(&mut o, co + CO_CAND1, inp.c144 as u32);
        for (i, v) in inp.cats.iter().enumerate() {
            wf64(&mut o, co + CO_CATS + 8 * i as u32, *v);
        }
        for (k, y) in inp.arch.iter().enumerate() {
            for (i, v) in y.iter().enumerate() {
                wf64(
                    &mut o,
                    co + CO_ARCH_CATS + 0x68 * k as u32 + 8 * i as u32,
                    *v,
                );
            }
        }
        o.write_mem(G_MANAGER_USE, &inp.uses).unwrap();
        o.write_mem(G_MANAGER_AVAIL, &inp.avail).unwrap();
        set_rng(&mut o, seed);

        let mut rng = GameRng::new(seed);
        let mut cursor = cursor0;
        let want = pick_ref(&inp, forced, &mut rng, &mut cursor);
        let got = o.call_thiscall(PICK_MANAGER, co, &[forced as u32]).unwrap() as i32;
        assert_eq!(
            got, want,
            "case {case} (lock {}, forced {forced})",
            inp.lock
        );
        assert_eq!(get_rng(&o), rng.state(), "rng {case}");
        assert_eq!(ri32(&o, G_MANAGER_CURSOR), cursor, "cursor {case}");
        picked[got as usize] += 1;
    }
    assert!(picked.iter().filter(|&&n| n > 0).count() >= 30);
}

// ---- 2. investor-confidence multiplier (0x4a8150) ----------------------------------------------

const PERSON_SIZE: u32 = 0x576;
const PE_ACTIVE: u32 = 0x04;
const PE_HUMAN: u32 = 0x122;
const PE_CASH: u32 = 0x12a;
const PE_HOLDINGS: u32 = 0x1d3; // i32[36]
const PE_CHAIR_OF: u32 = 0x263;
const PE_REP: u32 = 0x52b;

fn credit_mult_ref(human: bool, rep: i32, chair_of: i32, since: i32, year: i32) -> f64 {
    if !human {
        return 10.0;
    }
    let pen = if chair_of == 0 {
        0
    } else {
        let t = (year - since).clamp(0, 10);
        (t - 10) * 7
    };
    // Both clamps use `<=`: the original replaces a value equal to the bound by the bound.
    (f64::from(rep + 100 + pen) * 0.1).clamp(0.5, 15.0)
}

#[test]
fn credit_multiplier_matches_original() {
    let Some(mut o) = oracle() else { return };
    let persons = Pool::new(&mut o, G_PERSONS, PERSON_SIZE, 4);
    let companies = Pool::new(&mut o, G_COMPANIES, CO_SIZE, 4);
    let mut xs = Xs(0xc0ff_ee11);
    let mut distinct = std::collections::BTreeSet::new();
    for case in 0..3000 {
        let year = xs.range(1850, 2000);
        write_date(&mut o, year, xs.range(1, 12), 28);
        let human = xs.chance(80);
        let rep = if xs.chance(10) {
            xs.range(-500, 500)
        } else {
            xs.range(-99, 120)
        };
        let chair_of = if xs.chance(35) { 0 } else { xs.range(1, 3) };
        let since = year - xs.range(-1, 14);
        let p = persons.rec(1);
        o.write_mem(p, &vec![0u8; PERSON_SIZE as usize]).unwrap();
        w32(&mut o, p + PE_HUMAN, u32::from(human));
        w32(&mut o, p + PE_REP, rep as u32);
        w32(&mut o, p + PE_CHAIR_OF, chair_of as u32);
        if chair_of != 0 {
            w16(
                &mut o,
                companies.rec(chair_of as u32) + CO_CHAIR_SINCE,
                since as u16,
            );
        }
        let want = credit_mult_ref(human, rep, chair_of, since, year);
        o.call_fastcall(CREDIT_MULT, p, 0, &[]).unwrap();
        let got = o.st0().unwrap();
        assert_eq!(
            got, want,
            "case {case}: human {human} rep {rep} since {since} y {year}"
        );
        distinct.insert(want.to_bits());
    }
    assert!(distinct.len() > 40);
}

// ---- 3. founding-capital sliders (0x4bc990, 0x4bc7d0) ------------------------------------------

const SL_MIN_PLAYER: u32 = 0x0057_9454;
const SL_MAX_PLAYER: u32 = 0x0057_9424;
const SL_N_PLAYER: u32 = 0x0055_b1a8;
const SL_MIN_OUTSIDE: u32 = 0x0057_9450;
const SL_MAX_OUTSIDE: u32 = 0x0057_9458;
const SL_N_OUTSIDE: u32 = 0x0055_b1ac;
const SL_PLAYER_INV: u32 = 0x0057_9428;
const SL_OUTSIDE_INV: u32 = 0x0057_945c;
const SL_COMPANY: u32 = 0x0057_9420;
const WIDGET_SET: u32 = 0x0050_73b0; // thiscall(widget, id, code, value, 0) ret 0x10
const LOG_PTR: u32 = 0x0057_a000;
const LOG_BASE: u32 = 0x0057_a100;

/// Replaces `0x5073b0` by a stub that appends `(id, code, value)` to a log at `LOG_BASE`.
fn stub_widget_log(o: &mut Oracle) {
    let mut c: Vec<u8> = vec![0xa1];
    c.extend_from_slice(&LOG_PTR.to_le_bytes()); // mov eax,[LOG_PTR]
    c.extend_from_slice(&[0x8b, 0x54, 0x24, 0x08, 0x89, 0x10]); // edx=[esp+8]; [eax]=edx
    c.extend_from_slice(&[0x8b, 0x54, 0x24, 0x0c, 0x89, 0x50, 0x04]); // edx=[esp+12]; [eax+4]=edx
    c.extend_from_slice(&[0x8b, 0x54, 0x24, 0x04, 0x89, 0x50, 0x08]); // edx=[esp+4]; [eax+8]=edx
    c.extend_from_slice(&[0x83, 0x05]);
    c.extend_from_slice(&LOG_PTR.to_le_bytes());
    c.push(12); // add dword [LOG_PTR], 12
    c.extend_from_slice(&[0xc2, 0x10, 0x00]); // ret 0x10
    patch(o, WIDGET_SET, &c);
}

/// `(op, widget, value)` triples in call order.
fn widget_log(o: &Oracle) -> Vec<(u32, u32, i32)> {
    let end = r32(o, LOG_PTR);
    (LOG_BASE..end)
        .step_by(12)
        .map(|a| (r32(o, a + 8), r32(o, a), ri32(o, a + 4)))
        .collect()
}

struct PlayerSlider {
    min: i32,
    max: i32,
    n: i32,
}

fn player_slider_ref(w: i32) -> PlayerSlider {
    let (step, min) = if w < 1_000_000 {
        (10_000, 100_000)
    } else if w < 10_000_000 {
        (100_000, 100_000)
    } else {
        let s = if w > 99_999_999 {
            10_000_000
        } else {
            1_000_000
        };
        (s, s)
    };
    let max = w - crem(w, step);
    let n = (cdiv(max - min, step) + 1).max(1);
    PlayerSlider { min, max, n }
}

struct OutsideSlider {
    min: i32,
    max: i32,
    n: i32,
}

fn outside_slider_ref(m: f64, p: i32) -> OutsideSlider {
    let mp = m * f64::from(p);
    let mut c = if mp >= 1.0e9 { 1_000_000_000 } else { ftol(mp) };
    if c <= 200_000 - p {
        c = 200_000 - p;
    }
    let step;
    if c <= 50_000 {
        c = 50_000;
        step = 1000;
    } else if c >= 4_000_000 {
        c = 4_000_000;
        step = 100_000;
    } else if c < 100_000 {
        step = 1000;
    } else if c < 1_000_000 {
        step = 10_000;
    } else {
        step = 100_000;
    }
    let half = cdiv(c, 2);
    let mut min = half.min(500_000 - p).max(0);
    if crem(min, step) != 0 {
        min += step - crem(min, step);
    }
    let mut max = c - crem(c, step);
    if max <= min + step {
        max = min + step;
    }
    let n = (cdiv(max - min, step) + 1).max(3);
    OutsideSlider { min, max, n }
}

#[test]
fn founding_sliders_match_original() {
    let Some(mut o) = oracle() else { return };
    stub_widget_log(&mut o);
    let persons = Pool::new(&mut o, G_PERSONS, PERSON_SIZE, 4);
    let companies = Pool::new(&mut o, G_COMPANIES, CO_SIZE, 6);
    w32(&mut o, G_LOCAL_PERSON, 1);
    w32(&mut o, SL_COMPANY, 0);
    let mut xs = Xs(0x5110_de55);
    let mut seen_n = std::collections::BTreeSet::new();
    for case in 0..3000 {
        let year = xs.range(1850, 2000);
        write_date(&mut o, year, xs.range(1, 12), 28);
        // Person: cash across the slider brackets and a few positions in the stock market.
        let p = persons.rec(1);
        o.write_mem(p, &vec![0u8; PERSON_SIZE as usize]).unwrap();
        let cash = match xs.below(6) {
            0 => xs.real(-50_000.0, 400_000.0),
            1 => xs.real(100_000.0, 1_000_000.0),
            2 => xs.real(1_000_000.0, 10_000_000.0),
            3 => xs.real(10_000_000.0, 99_999_999.0),
            4 => xs.real(99_000_000.0, 300_000_000.0),
            _ => xs.real(0.0, 3_000_000.0),
        }
        .floor();
        wf64(&mut o, p + PE_CASH, cash);
        let human = xs.chance(85);
        w32(&mut o, p + PE_HUMAN, u32::from(human));
        w32(&mut o, p + PE_REP, xs.range(-120, 130) as u32);
        let chair_of = if xs.chance(50) { 0 } else { xs.range(1, 3) };
        w32(&mut o, p + PE_CHAIR_OF, chair_of as u32);
        let since = year - xs.range(0, 14);
        if chair_of != 0 {
            w16(
                &mut o,
                companies.rec(chair_of as u32) + CO_CHAIR_SINCE,
                since as u16,
            );
        }
        // Company set with three companies and their last prices.
        w32(&mut o, G_SET_COUNT, 3);
        let mut stock = 0.0f64;
        for i in 0..36u32 {
            let c = 1 + (i % 3);
            w16(&mut o, G_SET_IDS + 2 * i, c as u16);
            let h = if i < 3 && xs.chance(60) {
                xs.range(-20, 40) * 1000
            } else {
                0
            };
            w32(&mut o, p + PE_HOLDINGS + 4 * i, h as u32);
            if h != 0 {
                let price = xs.real(2.0, 150.0) as f32;
                // Holdings may share a company: the last price written wins.
                wf32(&mut o, companies.rec(c) + 0x427e, price);
            }
        }
        // Recompute the stock value with the prices actually stored.
        for i in 0..36u32 {
            let h = ri32(&o, p + PE_HOLDINGS + 4 * i);
            if h != 0 {
                let c = 1 + (i % 3);
                let v = f64::from(rf32(&o, companies.rec(c) + 0x427e)) * f64::from(h);
                stock += if v < 0.0 { v * 0.5 + v } else { v * 0.5 };
            }
        }
        let w_money = if stock <= 0.0 { cash + stock } else { cash };
        let wi = ftol(w_money.min(1.0e8));

        let reset = xs.below(2);
        w32(&mut o, LOG_PTR, LOG_BASE);
        o.call_fastcall(SLIDER_PLAYER, reset, 0, &[]).unwrap();
        let ps = player_slider_ref(wi);
        assert_eq!(ri32(&o, SL_MIN_PLAYER), ps.min, "player min {case} w {wi}");
        assert_eq!(ri32(&o, SL_MAX_PLAYER), ps.max, "player max {case} w {wi}");
        assert_eq!(ri32(&o, SL_N_PLAYER), ps.n, "player n {case} w {wi}");
        let log = widget_log(&o);
        let mut want_log = vec![(0x77, 0x8b12, ps.n)];
        if reset != 0 {
            want_log.push((0x66, 0x8b12, (ps.n - 1) / 2));
        }
        assert_eq!(log, want_log, "player widget log {case}");

        // Outside investment slider for a random player investment.
        let pinv = if xs.chance(20) {
            0
        } else {
            xs.range(0, 400) * 1000
        };
        w32(&mut o, SL_PLAYER_INV, pinv as u32);
        let prev = if xs.chance(30) {
            -1
        } else {
            xs.range(0, 3_000_000)
        };
        w32(&mut o, SL_OUTSIDE_INV, prev as u32);
        let m = credit_mult_ref(human, ri32(&o, p + PE_REP), chair_of, since, year);
        let reset = xs.below(2);
        w32(&mut o, LOG_PTR, LOG_BASE);
        o.call_fastcall(SLIDER_OUTSIDE, reset, 0, &[]).unwrap();
        let os = outside_slider_ref(m, pinv);
        assert_eq!(
            ri32(&o, SL_MIN_OUTSIDE),
            os.min,
            "outside min {case} m {m} p {pinv}"
        );
        assert_eq!(
            ri32(&o, SL_MAX_OUTSIDE),
            os.max,
            "outside max {case} m {m} p {pinv}"
        );
        assert_eq!(
            ri32(&o, SL_N_OUTSIDE),
            os.n,
            "outside n {case} m {m} p {pinv}"
        );
        let pos = if prev == -1 || reset != 0 {
            (os.n - 1) / 2
        } else {
            let k = os.n - 1;
            let q = cdiv(prev - os.min, cdiv(os.max - os.min, k));
            q.max(0).min(k)
        };
        let want_log = vec![(0x77, 0x8b16, os.n), (0x66, 0x8b16, pos)];
        assert_eq!(widget_log(&o), want_log, "outside widget log {case}");
        seen_n.insert(os.n);
    }
    assert!(seen_n.len() > 10);
}

// ---- 4. annual company review (0x43c760) ---------------------------------------------------------

const N_TERR: usize = 32;
const TERR_STRIDE: u32 = 0x1ed;
const STATION_SIZE: u32 = 0x4c7;
const ST_COMPANY: u32 = 0x39;
const ST_TERRITORY: u32 = 0x4ad;
const ST_GENERATED: u32 = 0x4b5;
const ST_SHIPPED: u32 = 0x4b7;
const ST_RECEIVED: u32 = 0x4b9;

// stub logs (emulated memory)
const M_FIRE: u32 = 0x0057_a800; // [count, arg]
const M_LIQ: u32 = 0x0057_a810; // [count, arg]
const M_POPUP: u32 = 0x0057_a820;
const M_DIALOG: u32 = 0x0057_a824;
const M_EMPTY: u32 = 0x0057_a8f0; // an empty string
const MSG_BUFFER: u32 = 0x0057_96f4;
const M_MERGE: u32 = 0x0057_a830; // [count, this, target id, price]
const M_ELECT: u32 = 0x0057_a840; // [count, this, challenger id]
const M_DELETE: u32 = 0x0057_a850; // [merger proposals, election proposals]
const G_MERGERS: u32 = 0x0057_4ca0;
const G_ELECTIONS: u32 = 0x0057_4ca4;

#[derive(Clone)]
struct Station {
    company: i32,
    territory: i32,
    generated: u16,
    shipped: u16,
    received: u16,
}

#[derive(Clone)]
struct Holder {
    active: bool,
    human: bool,
    holding: i32,
}

#[derive(Clone)]
struct Review {
    year: i32,
    month: i32,
    day: i32,
    cid: i32,
    founded: i32,
    shares: i32,
    init_price: f32,
    cats: [f64; 13],
    arch_cats: [[f64; 13]; 10],
    acct: [f64; 8],
    arch_acct: [[f64; 8]; 10],
    /// `arch[k]`, k = 1..=10 years ago (index 0 unused).
    arch: [[f64; 14]; 11],
    rep: i32,
    salary: i32,
    last_bonus: i32,
    chair_since: i32,
    warn_year: i32,
    standing: [i16; N_TERR],
    standing_all: i32,
    cand: [i32; 2],
    chairman: i32,
    chair_rep: i32,
    chair_human: bool,
    chair_cash: f64,
    speed_n: i32,
    speed_sum: i32,
    options: u8,
    ref_speed: u8,
    fin_adv: bool,
    gate: bool,
    editor: bool,
    uses: [u8; 40],
    avail: [u8; 40],
    sal: [i32; 40],
    stations: Vec<Station>,
    holders: Vec<Holder>,
    set: Vec<u16>,
    local_person: i32,
    local_company: i32,
    net_flag: bool,
    /// `[0x570237] == 0`: the warning popups are shown.
    show_msgs: bool,
    /// Lifetime copy of category 11 (`+0x4756`).
    life11: f64,
}

/// What the review leaves behind (and the stub logs).
#[derive(Clone, Debug, PartialEq)]
struct ReviewOut {
    rep: i32,
    salary: i32,
    bonus: i32,
    last_bonus: i16,
    standing: Vec<i16>,
    standing_all: i32,
    rev_mult: f32,
    cand: [i32; 2],
    cat11: f64,
    life11: f64,
    acct0: f64,
    acct7: f64,
    chair_rep: i32,
    chair_cash: f64,
    fire: (u32, u32),
    liq: (u32, u32),
    popups: u32,
    warn_year: i16,
    rng: u32,
}

/// Live metrics the review asks the company for (served by the verified functions of
/// finance.md 1): price `0x43ad60(0)` and profit per share `0x43ad60(3)`.
struct Live {
    price: f64,
    eps: f64,
}

fn sum_cats(c: &[f64; 13], n: usize) -> f64 {
    let mut s = 0.0;
    for v in &c[..n] {
        s += *v;
    }
    s
}

fn review_ref(r: &Review, live: &Live, rng: &mut GameRng) -> ReviewOut {
    let year = r.year;
    let age = year - r.founded;
    // -- performance of the chairman: price growth and profit-per-share growth
    let price = live.price;
    let prev = if r.founded == year {
        f64::from(r.init_price)
    } else {
        r.arch[1][0]
    };
    let prev = if prev <= 0.01 { 0.01 } else { prev };
    let mut g1 = (price / prev - 1.0) as f32;
    if f64::from(g1) > 1.0 {
        g1 = f64::from(g1).sqrt() as f32;
    }
    if f64::from(g1) < -1.0 {
        g1 = (-(-f64::from(g1)).sqrt()) as f32;
    }
    if f64::from(g1) > 2.0 {
        g1 = 2.0;
    }
    if f64::from(g1) < -2.0 {
        g1 = -2.0;
    }
    let mut a = live.eps;
    let mut b = if r.founded == year { 1.0 } else { r.arch[1][3] };
    if a == 0.0 {
        a = 0.01;
    }
    if b == 0.0 {
        b = 0.01;
    }
    let squash = |mut x: f64| {
        if x > 1.0 {
            x = x.sqrt();
        }
        if x < -1.0 {
            x = -(-x).sqrt();
        }
        x.clamp(-2.0, 2.0)
    };
    let g2 = if a > 0.0 && b > 0.0 {
        squash(a / (b + 1.0) - 1.0)
    } else if a < 0.0 && b < 0.0 {
        squash(b / (a - 1.0) - 1.0)
    } else if a > 0.0 && b < 0.0 {
        squash(a / (-b + 1.0) - 1.0)
    } else if a < 0.0 && b > 0.0 {
        -1.0
    } else {
        0.0
    };
    let h2 = g2 * 0.5;
    let g1d = f64::from(g1);
    let mix = if h2 <= g1d {
        g1d * 6.0 + h2 * 4.0
    } else {
        g1d * 4.0 + h2 * 6.0
    };
    let v = mix * 0.05 * 100.0;
    let v = if v < 0.0 {
        -((-v).sqrt() * 7.07)
    } else if v > 0.0 {
        v.sqrt() * 7.07
    } else {
        v
    };
    let perf = v * 1.8;
    let w = if age < 3 {
        f64::from(age) * 0.15 * 0.45 + 0.225
    } else {
        0.45
    };
    let mut perf32 = perf as f32;
    if (0.5..10.0).contains(&price) && perf32 > 0.0 {
        perf32 = (f64::from(perf32) * price * 0.1) as f32;
    }
    let rep = ftol((1.0 - w) * f64::from(r.rep) + f64::from(perf32) * w);
    let chair_rep = if r.chairman != 0 {
        ftol(f64::from(r.chair_rep) * 0.55 + f64::from(perf32) * 0.45)
    } else {
        r.chair_rep
    };

    // -- chairman salary
    let rev = sum_cats(&r.cats, 5);
    let target = ftol((rev.sqrt() + 200.0) * f64::from((rep + 40) * 3) * 0.1);
    let mut sal = r.salary;
    match r.options & 0x38 {
        0 => sal = ftol(f64::from(sal) * 1.5),
        8 => sal = ftol(f64::from(sal) * 1.25),
        _ => {}
    }
    let sal2 = ftol(f64::from(sal) * 0.65 + f64::from(target) * 0.35);
    let salary = ((sal2 + 500) / 1000 * 1000).max(1000);

    // -- bonus (RNG only when the conditions hold)
    let mut out_cat11 = r.cats[11];
    let mut out_life11 = r.life11;
    let mut acct0 = r.acct[0];
    let mut acct7 = r.acct[7];
    let mut chair_cash = r.chair_cash;
    let mut last_bonus = r.last_bonus as i16;
    let mut bonus = 0;
    if year - r.last_bonus > 1 && r.chairman != 0 {
        let roll = rng.range(20, 110);
        if roll < rep {
            let mut b = cdiv(cdiv(rep.wrapping_mul(salary), 100), 1000) + 1000;
            if b < 5000 {
                b = 5000;
            }
            last_bonus = year as i16;
            chair_cash += f64::from(b);
            let d = f64::from(-b);
            out_cat11 += d;
            out_life11 += d;
            acct0 += d;
            acct7 -= d;
            bonus = b;
        }
    }

    // -- standing with the territories
    let mut base = 5000;
    if r.ref_speed != 0 {
        let speed = cdiv(r.speed_sum, r.speed_n.max(1));
        let mut ratio = f64::from(speed) / f64::from(r.ref_speed);
        if ratio > f64::from(2.0f32) {
            ratio = f64::from(2.0f32);
        }
        base = ftol((ratio - 1.0) * 2000.0 + 5000.0);
    }
    let mut tot_a = [0.0f32; N_TERR];
    let mut tot_b = [0.0f32; N_TERR];
    for s in &r.stations {
        if s.company == r.cid {
            let t = s.territory as usize;
            tot_a[t] += (u32::from(s.received) + u32::from(s.shipped)) as f32;
            tot_b[t] += f32::from(s.generated);
        }
    }
    let mut standing = r.standing;
    let (mut ta, mut tb) = (0i32, 0i32);
    for t in 0..N_TERR {
        ta = ftol(f64::from(ta) + f64::from(tot_a[t]));
        tb = ftol(f64::from(tb) + f64::from(tot_b[t]));
        if tot_b[t] > 3.0 {
            let score =
                ftol((f64::from(tot_a[t]) / f64::from(tot_b[t]) - 1.0) * 4000.0 + f64::from(base));
            standing[t] =
                ftol(f64::from(score) * 0.3 + f64::from(i32::from(standing[t])) * 0.7) as i16;
        }
    }
    let d = tb.max(1);
    let rnd0 = rng.range(-200, 200);
    let score = ftol((f64::from(ta) / f64::from(d) - 1.0) * 4000.0 + f64::from(base));
    let v = rnd0.wrapping_add(score);
    let mut standing_all = ftol(f64::from(r.standing_all) * 0.7 + f64::from(v) * 0.3);
    for s in &mut standing {
        let x = rng.range(-200, 200);
        *s = (x as i16).wrapping_add(*s).clamp(-10000, 10000);
    }
    standing_all = standing_all.clamp(0, 10000);

    // -- revenue multiplier
    // The bonus is posted to the ledger first, so the profit includes it.
    let mut cats_after = r.cats;
    cats_after[11] = out_cat11;
    let mut rv = rev;
    let mut pf = sum_cats(&cats_after, 13);
    if age > 0 {
        rv += sum_cats(&r.arch_cats[0], 5) * 0.7;
        pf += sum_cats(&r.arch_cats[0], 13) * 0.7;
    }
    if age > 1 {
        rv += sum_cats(&r.arch_cats[1], 5) * 0.4;
        pf += sum_cats(&r.arch_cats[1], 13) * 0.4;
    }
    if rv < 1.0 {
        rv = 1.0;
    }
    let m = 1.0 - (pf / rv - 0.2) * 0.333_333_333_333_333_3;
    let mut m32 = m as f32;
    if m < 0.8 {
        m32 = f32::from_bits(0x3f4c_cccd);
    } else if m > 1.2 {
        m32 = f32::from_bits(0x3f99_999a);
    }
    if age < 5 {
        m32 = ((f64::from(age) * f64::from(m32) + f64::from(5 - age)) * 0.2) as f32;
    }
    if r.chairman > 0 && r.chair_human {
        m32 = ((f64::from(m32) + 1.0) * 0.5) as f32;
    }

    // -- manager candidates (odd years)
    let mut cand = r.cand;
    if year & 1 != 0 {
        let mut uses = r.uses;
        for &c in &cand {
            if c >= 0 {
                uses[c as usize] = uses[c as usize].wrapping_add(1);
            }
        }
        let mut inp = PickIn {
            year,
            month: r.month,
            founded: r.founded,
            cats: r.cats,
            arch: r.arch_cats,
            c140: cand[0],
            c144: cand[1],
            uses,
            avail: r.avail,
            sal: r.sal,
            lock: false,
        };
        let mut cursor = 0;
        cand[0] = pick_ref(&inp, -1, rng, &mut cursor);
        inp.c140 = cand[0];
        cand[1] = pick_ref(&inp, -1, rng, &mut cursor);
    }

    // -- chairman dismissal and liquidation
    let mut fire = (0u32, 0u32);
    let mut liq = (0u32, 0u32);
    let mut popups = 0u32;
    let mut warn_year = r.warn_year as i16;
    if !r.editor && r.gate {
        let held = |hs: &[Holder]| -> i32 {
            hs.iter()
                .filter(|h| h.active && h.holding > 0)
                .map(|h| h.holding)
                .sum()
        };
        if r.fin_adv && r.chairman != 0 && r.shares - held(&r.holders) > (r.shares >> 1) {
            let mut n = 0i32;
            let mut t = -15i32;
            loop {
                let y = year - n;
                if y < r.founded || y < r.chair_since {
                    break;
                }
                let rp = if n == 0 {
                    f64::from(rep)
                } else {
                    r.arch[n as usize][12]
                };
                if rp >= f64::from(t) {
                    break;
                }
                let e = if n == 0 {
                    live.eps
                } else {
                    r.arch[n as usize][3]
                };
                if e >= 1.0 {
                    break;
                }
                n += 1;
                t += 5;
                if t >= 0 {
                    break;
                }
            }
            if n > 1 {
                let mut pct = (f64::from(-rep) * 1.75) as f32;
                if f64::from(-rep) * 1.75 > 100.0 {
                    pct = 100.0;
                }
                let mut votes =
                    ftol(f64::from(pct) * 0.01 * f64::from(r.shares - held(&r.holders)));
                if pct > 50.0 {
                    for (i, h) in r.holders.iter().enumerate() {
                        if h.holding > 0 && !h.human && (i as i32 + 1) != r.chairman {
                            votes += h.holding;
                        }
                    }
                }
                if f64::from(votes) > f64::from(r.shares) * 0.5 {
                    if r.warn_year < year - 1 {
                        if r.chairman == r.local_person && r.show_msgs {
                            popups += 1;
                        }
                        warn_year = year as i16;
                    } else {
                        fire = (1, 0);
                    }
                }
            }
        }
        let mut n = 0i32;
        let mut thr = 5i32;
        loop {
            let y = year - n;
            if y < r.founded {
                break;
            }
            let p = if n == 0 {
                live.price
            } else {
                r.arch[n as usize][0]
            };
            if p >= f64::from(thr) {
                break;
            }
            let equity = if n == 0 {
                -r.acct[7]
            } else {
                let a = &r.arch_acct[n as usize - 1];
                let mut s = 0.0;
                for v in &a[..6] {
                    s += *v;
                }
                s + a[6]
            };
            if equity >= 0.0 {
                break;
            }
            let income = if n == 0 {
                sum_cats(&cats_after, 13)
            } else {
                sum_cats(&r.arch_cats[n as usize - 1], 13)
            };
            if income >= -10000.0 {
                break;
            }
            let cash = if n == 0 {
                r.acct[0]
            } else {
                r.arch_acct[n as usize - 1][0]
            };
            if cash >= 0.0 {
                break;
            }
            n += 1;
            thr += 2;
            if thr >= 11 {
                break;
            }
        }
        if n == 2 {
            if r.local_company == r.cid && r.show_msgs {
                popups += 1;
            }
        } else if n == 3 {
            liq = (1, u32::from(r.net_flag));
        }
    }

    ReviewOut {
        rep,
        salary,
        bonus,
        last_bonus,
        standing: standing.to_vec(),
        standing_all,
        rev_mult: m32,
        cand,
        cat11: out_cat11,
        life11: out_life11,
        acct0,
        acct7,
        chair_rep,
        chair_cash,
        fire,
        liq,
        popups,
        warn_year,
        rng: rng.state(),
    }
}

fn install_review_stubs(o: &mut Oracle) {
    let counter = |log: u32, ret: u16| -> Vec<u8> {
        let mut c = vec![0xff, 0x05];
        c.extend_from_slice(&log.to_le_bytes()); // inc dword [log]
        c.extend_from_slice(&[0x8b, 0x44, 0x24, 0x04, 0xa3]); // mov eax,[esp+4]; mov [log+4],eax
        c.extend_from_slice(&(log + 4).to_le_bytes());
        c.extend_from_slice(&[0xc2, ret as u8, (ret >> 8) as u8]);
        c
    };
    patch(o, 0x0043_8900, &counter(M_FIRE, 4)); // chairman change (thiscall(new chairman))
    patch(o, 0x0043_8170, &counter(M_LIQ, 4)); // liquidation (thiscall(flag))
    let mut popup = vec![0xff, 0x05];
    popup.extend_from_slice(&M_POPUP.to_le_bytes());
    popup.extend_from_slice(&[0xc2, 0x10, 0x00]);
    patch(o, 0x0046_9320, &popup); // blocking message
    patch(o, 0x004a_42f0, &[0xc2, 0x04, 0x00]); // message log
    patch(o, 0x0047_5fa0, &[0xc2, 0x20, 0x00]); // sprintf-like
    // LNG lookup: every id yields the empty string at `M_EMPTY`
    let mut lng = vec![0xb8];
    lng.extend_from_slice(&M_EMPTY.to_le_bytes());
    lng.push(0xc3);
    patch(o, 0x0047_6910, &lng);
    w32(o, M_EMPTY, 0);
    patch(o, 0x0049_d920, &[0xb8, 0x30, 0x23, 0x00, 0x00, 0xc3]); // net_min_peer_version
    patch(o, 0x0046_ae50, &[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]); // net_player_count
    // vote helpers: the result dialog, the merger, the chairmanship change and the deletion of
    // the proposal records
    patch(o, 0x0045_10a0, &count_stub(M_DIALOG, 0x24));
    patch(
        o,
        0x0047_5980,
        &[0xb8, 0xf4, 0x96, 0x57, 0x00, 0xc2, 0x10, 0x00],
    ); // number text
    let mut merge = vec![0xff, 0x05];
    merge.extend_from_slice(&M_MERGE.to_le_bytes()); // inc [M_MERGE]
    merge.extend_from_slice(&[0x89, 0x0d]);
    merge.extend_from_slice(&(M_MERGE + 4).to_le_bytes()); // [M+4] = ecx
    merge.extend_from_slice(&[0x8b, 0x44, 0x24, 0x04, 0xa3]);
    merge.extend_from_slice(&(M_MERGE + 8).to_le_bytes()); // [M+8] = target id
    merge.extend_from_slice(&[0x8b, 0x44, 0x24, 0x08, 0xa3]);
    merge.extend_from_slice(&(M_MERGE + 12).to_le_bytes()); // [M+12] = price
    merge.extend_from_slice(&[0xc2, 0x08, 0x00]);
    patch(o, 0x0043_c3c0, &merge);
    let mut elect = vec![0xff, 0x05];
    elect.extend_from_slice(&M_ELECT.to_le_bytes());
    elect.extend_from_slice(&[0x89, 0x0d]);
    elect.extend_from_slice(&(M_ELECT + 4).to_le_bytes());
    elect.extend_from_slice(&[0x8b, 0x44, 0x24, 0x04, 0xa3]);
    elect.extend_from_slice(&(M_ELECT + 8).to_le_bytes());
    elect.extend_from_slice(&[0xc2, 0x04, 0x00]);
    patch(o, 0x0043_c050, &elect);
    for (at, log) in [(0x0046_6cc0, M_DELETE), (0x0046_6cf0, M_DELETE + 4)] {
        let mut c = vec![0xff, 0x05];
        c.extend_from_slice(&log.to_le_bytes());
        c.push(0xc3);
        patch(o, at, &c);
    }
}

/// `inc dword [log]; ret n`
fn count_stub(log: u32, ret: u16) -> Vec<u8> {
    let mut c = vec![0xff, 0x05];
    c.extend_from_slice(&log.to_le_bytes());
    c.extend_from_slice(&[0xc2, ret as u8, (ret >> 8) as u8]);
    c
}

struct ReviewEnv {
    persons: Pool,
    companies: Pool,
    stations: Pool,
    net: u32,
}

fn review_env(o: &mut Oracle) -> ReviewEnv {
    install_review_stubs(o);
    let persons = Pool::new(o, G_PERSONS, PERSON_SIZE, 5);
    let companies = Pool::new(o, G_COMPANIES, CO_SIZE, 4);
    let stations = Pool::new(o, G_STATIONS, STATION_SIZE, 16);
    let net = o.alloc(0x400).unwrap();
    w32(o, G_NET_OPTIONS, net);
    ReviewEnv {
        persons,
        companies,
        stations,
        net,
    }
}

/// Lays the company record of `r` out at `co` (the pools and globals are written elsewhere).
fn write_company_record(o: &mut Oracle, co: u32, r: &Review) {
    o.write_mem(co, &vec![0u8; CO_SIZE as usize]).unwrap();
    w32(o, co + CO_ID, r.cid as u32);
    w32(o, co + CO_CHAIRMAN, r.chairman as u32);
    w32(o, co + CO_SHARES, r.shares as u32);
    wf32(o, co + CO_INIT_PRICE, r.init_price);
    w32(o, co + CO_SALARY, r.salary as u32);
    w32(o, co + CO_CAND0, r.cand[0] as u32);
    w32(o, co + CO_CAND1, r.cand[1] as u32);
    w32(o, co + CO_FOUNDED, r.founded as u32);
    w32(o, co + CO_SPEED_N, r.speed_n as u32);
    w32(o, co + CO_SPEED_SUM, r.speed_sum as u32);
    w32(o, co + CO_ACTIVE, 1);
    w32(o, co + CO_REP, r.rep as u32);
    w16(o, co + CO_CHAIR_SINCE, r.chair_since as u16);
    w16(o, co + CO_LAST_BONUS, r.last_bonus as u16);
    w16(o, co + CO_WARN_YEAR, r.warn_year as u16);
    for (t, v) in r.standing.iter().enumerate() {
        w16(o, co + CO_STANDING + TERR_STRIDE * t as u32, *v as u16);
    }
    w32(o, co + CO_STANDING_ALL, r.standing_all as u32);
    w32(o, co + 0x425c, 590_000_000); // creation minute
    for (i, v) in r.cats.iter().enumerate() {
        wf64(o, co + CO_CATS + 8 * i as u32, *v);
    }
    for (k, y) in r.arch_cats.iter().enumerate() {
        for (i, v) in y.iter().enumerate() {
            wf64(o, co + CO_ARCH_CATS + 0x68 * k as u32 + 8 * i as u32, *v);
        }
    }
    wf64(o, co + CO_LIFE_CATS + 8 * 11, r.life11);
    for (i, v) in r.acct.iter().enumerate() {
        wf64(o, co + CO_ACCT + 8 * i as u32, *v);
    }
    for (k, y) in r.arch_acct.iter().enumerate() {
        for (i, v) in y.iter().enumerate() {
            wf64(o, co + CO_ARCH_ACCT + 0x40 * k as u32 + 8 * i as u32, *v);
        }
    }
    for (k, y) in r.arch.iter().enumerate().skip(1) {
        for (i, v) in y.iter().enumerate() {
            wf64(o, co + CO_ARCH_METRICS + 0x70 * k as u32 + 8 * i as u32, *v);
        }
    }
}

fn write_review(o: &mut Oracle, env: &ReviewEnv, r: &Review) -> u32 {
    write_date(o, r.year, r.month, r.day);
    w32(o, 0x0057_04f4, 600_000_000); // game_time_minutes
    w32(o, 0x0057_0717, 2); // economy state
    wf32(o, 0x0057_06d1, 0.08); // prime rate
    wf32(o, 0x0057_06cd, 100.0); // stock liquidity
    for e in 0..0x30u32 {
        w16(o, 0x0057_07c6 + 2 * e, 0);
    }
    w32(o, G_LOCK, 0);
    w32(o, G_EDITOR, u32::from(r.editor));
    w8(o, G_CHAIRMAN_FIRE, u8::from(r.gate));
    w8(o, G_FIN_ADVANCED, u8::from(r.fin_adv));
    w8(o, G_OPTIONS, r.options);
    w8(o, G_REF_SPEED, r.ref_speed);
    o.write_mem(G_MANAGER_USE, &r.uses).unwrap();
    o.write_mem(G_MANAGER_AVAIL, &r.avail).unwrap();
    w32(o, G_LOCAL_PERSON, r.local_person as u32);
    w32(o, G_LOCAL_COMPANY, r.local_company as u32);
    w8(o, G_NOEDIT_FLAG, u8::from(!r.show_msgs));
    w8(o, env.net + 0x240, u8::from(r.net_flag));

    // company set
    w32(o, G_SET_COUNT, r.set.len() as u32);
    for (i, c) in r.set.iter().enumerate() {
        w16(o, G_SET_IDS + 2 * i as u32, *c);
    }
    env.companies.set_len(o, 3);
    let co = env.companies.rec(r.cid as u32);
    write_company_record(o, co, r);

    // persons: id i+1 holds `holding` shares in the company's position of the set
    let pos = r.set.iter().position(|&c| i32::from(c) == r.cid).unwrap() as u32;
    for (i, h) in r.holders.iter().enumerate() {
        let p = env.persons.rec(i as u32 + 1);
        o.write_mem(p, &vec![0u8; PERSON_SIZE as usize]).unwrap();
        w32(o, p + PE_ACTIVE, u32::from(h.active));
        w32(o, p + PE_HUMAN, u32::from(h.human));
        w32(o, p + PE_HOLDINGS + 4 * pos, h.holding as u32);
        if i as i32 + 1 == r.chairman {
            w32(o, p + PE_REP, r.chair_rep as u32);
            wf64(o, p + PE_CASH, r.chair_cash);
            w32(o, p + PE_CHAIR_OF, r.cid as u32);
        }
    }
    env.persons.set_len(o, r.holders.len() as u32);
    // stations
    for (i, s) in r.stations.iter().enumerate() {
        let a = env.stations.rec(i as u32 + 1);
        o.write_mem(a, &vec![0u8; STATION_SIZE as usize]).unwrap();
        w32(o, a + ST_COMPANY, s.company as u32);
        w32(o, a + ST_TERRITORY, s.territory as u32);
        w16(o, a + ST_GENERATED, s.generated);
        w16(o, a + ST_SHIPPED, s.shipped);
        w16(o, a + ST_RECEIVED, s.received);
    }
    env.stations.set_len(o, r.stations.len() as u32);
    for a in [M_FIRE, M_FIRE + 4, M_LIQ, M_LIQ + 4, M_POPUP] {
        w32(o, a, 0);
    }
    co
}

fn read_review(o: &Oracle, env: &ReviewEnv, r: &Review, co: u32) -> ReviewOut {
    let standing = (0..N_TERR as u32)
        .map(|t| ri16(o, co + CO_STANDING + TERR_STRIDE * t))
        .collect();
    let chair = if r.chairman != 0 {
        Some(env.persons.rec(r.chairman as u32))
    } else {
        None
    };
    ReviewOut {
        rep: ri32(o, co + CO_REP),
        salary: ri32(o, co + CO_SALARY),
        bonus: ri32(o, co + CO_BONUS),
        last_bonus: ri16(o, co + CO_LAST_BONUS),
        standing,
        standing_all: ri32(o, co + CO_STANDING_ALL),
        rev_mult: rf32(o, co + CO_REV_MULT),
        cand: [ri32(o, co + CO_CAND0), ri32(o, co + CO_CAND1)],
        cat11: rf64(o, co + CO_CATS + 8 * 11),
        life11: rf64(o, co + CO_LIFE_CATS + 8 * 11),
        acct0: rf64(o, co + CO_ACCT),
        acct7: rf64(o, co + CO_ACCT + 8 * 7),
        chair_rep: chair.map_or(r.chair_rep, |p| ri32(o, p + PE_REP)),
        chair_cash: chair.map_or(r.chair_cash, |p| rf64(o, p + PE_CASH)),
        fire: (r32(o, M_FIRE), r32(o, M_FIRE + 4)),
        liq: (r32(o, M_LIQ), r32(o, M_LIQ + 4)),
        popups: r32(o, M_POPUP),
        warn_year: ri16(o, co + CO_WARN_YEAR),
        rng: get_rng(o),
    }
}

/// A random company in the state the review sees on 28 December (or on a random day).
/// `distress`: the shape that triggers the dismissal and liquidation tests.
fn rand_review(xs: &mut Xs, sal: &[i32; 40], distress: bool) -> Review {
    let year = xs.range(1850, 1990);
    let age = if xs.chance(35) {
        xs.range(0, 5)
    } else {
        xs.range(0, 14)
    };
    let (month, day) = if xs.chance(75) {
        (12, 28)
    } else {
        (xs.range(1, 12), xs.range(1, 28))
    };
    let scale = if distress {
        3.0e4
    } else {
        [2.0e4, 3.0e5, 2.0e6][xs.below(3) as usize]
    };
    let mut cats = rand_cats(xs, scale);
    let mut arch_cats = [[0.0; 13]; 10];
    for y in &mut arch_cats {
        *y = rand_cats(xs, scale);
    }
    let shares = if distress {
        1000 * xs.range(1000, 3000)
    } else {
        1000 * xs.range(100, 2000)
    };
    let mut arch = [[0.0; 14]; 11];
    for y in arch.iter_mut().skip(1) {
        y[0] = xs.real(1.0, 120.0);
        y[1] = xs.real(-5.0, 100.0);
        y[2] = xs.real(0.0, 50.0);
        y[3] = xs.real(-6.0, 12.0);
        y[4] = f64::from(xs.below(60)) * 0.1;
        for m in y.iter_mut().skip(5) {
            *m = xs.real(-30.0, 30.0);
        }
    }
    let mut acct = [0.0; 8];
    acct[0] = xs.real(-2.0e6, 4.0e6).floor();
    for a in acct.iter_mut().take(6).skip(1) {
        *a = xs.real(0.0, 3.0e6).floor();
    }
    acct[6] = -xs.real(0.0, 3.0e6).floor();
    acct[7] = -xs.real(0.0, 9.0e6).floor();
    let mut arch_acct = [[0.0; 8]; 10];
    for y in &mut arch_acct {
        y[0] = xs.real(-2.0e6, 4.0e6).floor();
        for a in y.iter_mut().take(6).skip(1) {
            *a = xs.real(0.0, 3.0e6).floor();
        }
        y[6] = -xs.real(0.0, 3.0e6).floor();
        y[7] = -xs.real(0.0, 9.0e6).floor();
    }
    let gate = if distress { true } else { xs.chance(30) };
    let fin_adv = !xs.chance(25);
    let editor = xs.chance(5);
    let n_hold = xs.range(1, 4) as usize;
    let chairman = if xs.chance(15) {
        0
    } else {
        xs.range(1, n_hold as i32)
    };
    let mut holders: Vec<Holder> = (0..n_hold)
        .map(|_| Holder {
            active: !xs.chance(15),
            human: xs.chance(30),
            holding: match xs.below(4) {
                0 => 0,
                1 => -1000 * xs.range(1, 10),
                _ => (f64::from(shares) * xs.real(0.0, 0.5)) as i32 / 1000 * 1000,
            },
        })
        .collect();
    if distress && xs.chance(50) {
        // keep the public float large so that the shareholders can revolt
        for h in &mut holders {
            h.holding = (h.holding / 4).max(0);
        }
    }
    let cid = xs.range(1, 3);
    let mut set: Vec<u16> = (1..=3).filter(|&c| c != cid).map(|c| c as u16).collect();
    for i in (1..set.len()).rev() {
        set.swap(i, xs.below(i as u32 + 1) as usize);
    }
    set.truncate(xs.below(3) as usize);
    let at = xs.below(set.len() as u32 + 1) as usize;
    set.insert(at, cid as u16);
    let chair_human = chairman != 0 && holders[chairman as usize - 1].human;
    let mut last_bonus = year - xs.range(0, 5);
    if gate && !editor {
        last_bonus = year; // no bonus: the ledger stays as it is for the liquidation test
    }
    let chair_since = year - xs.range(0, 8);
    let warn_year = year - xs.range(0, 4);
    let mut uses = [0u8; 40];
    for _ in 0..xs.below(30) {
        uses[xs.below(40) as usize] = 1 + xs.below(9) as u8;
    }
    let mut avail = [1u8; 40];
    if xs.chance(40) {
        for a in &mut avail {
            *a = u8::from(!xs.chance(35));
        }
    }
    for _ in 0..6 {
        let f = xs.below(40) as usize;
        uses[f] = 0;
        avail[f] = 1;
    }
    let mut cand = [xs.range(0, 39), xs.range(0, 39)];
    let eligible = (0..40)
        .filter(|&i| uses[i] == 0 && avail[i] == 1 && i as i32 != cand[0] && i as i32 != cand[1])
        .count();
    if eligible == 0 {
        cand = [-1, -1];
    }
    let n_st = xs.below(13) as usize;
    let stations = (0..n_st)
        .map(|_| Station {
            company: if xs.chance(80) { cid } else { 9 },
            territory: xs.range(0, 5),
            generated: xs.below(9) as u16,
            shipped: xs.below(12) as u16,
            received: xs.below(12) as u16,
        })
        .collect();
    let mut standing = [0i16; N_TERR];
    for s in &mut standing {
        *s = xs.range(-10000, 10000) as i16;
    }
    let speed_n = xs.range(0, 60);
    if distress {
        // losses, negative equity and cash for the years the test wants to exercise
        for v in cats.iter_mut().take(5) {
            *v = (*v * 0.05).floor();
        }
        cats[9] = -xs.real(50_000.0, 400_000.0).floor();
        acct[0] = -xs.real(10_000.0, 3.0e6).floor();
        acct[7] = xs.real(1.0, 5.0e6).floor();
        for k in 0..xs.below(4) as usize {
            let y = &mut arch_acct[k];
            y[0] = -xs.real(10_000.0, 3.0e6).floor();
            y[6] = -xs.real(2.0e6, 9.0e6).floor();
            arch_cats[k][9] = -xs.real(50_000.0, 400_000.0).floor();
            arch[k + 1][0] = xs.real(1.0, 4.5);
        }
        for k in 0..3 {
            arch[k + 1][12] = xs.real(-40.0, -3.0);
            arch[k + 1][3] = xs.real(-8.0, 0.9);
        }
    }
    Review {
        year,
        month,
        day,
        cid,
        founded: year - age,
        shares,
        init_price: xs.real(5.0, 100.0) as f32,
        cats,
        arch_cats,
        acct,
        arch_acct,
        arch,
        rep: if distress {
            xs.range(-90, -10)
        } else {
            xs.range(-100, 110)
        },
        salary: xs.range(1, 300) * if xs.chance(70) { 1000 } else { 777 },
        last_bonus,
        chair_since,
        warn_year,
        standing,
        standing_all: xs.range(0, 10000),
        cand,
        chairman,
        chair_rep: xs.range(-99, 99),
        chair_human,
        chair_cash: xs.real(-1.0e5, 5.0e6).floor(),
        speed_n,
        speed_sum: speed_n * xs.range(5, 90),
        options: [0u8, 8, 0x10, 0x38, 0x41][xs.below(5) as usize],
        ref_speed: if xs.chance(25) {
            0
        } else {
            xs.range(4, 200) as u8
        },
        fin_adv,
        gate,
        editor,
        uses,
        avail,
        sal: *sal,
        stations,
        holders,
        set,
        local_person: if xs.chance(50) {
            chairman
        } else {
            xs.range(1, 4)
        },
        local_company: if xs.chance(50) { cid } else { 3 },
        net_flag: xs.chance(50),
        show_msgs: !xs.chance(25),
        life11: -xs.real(0.0, 1.0e7).floor(),
    }
}

#[test]
fn annual_review_matches_original() {
    let Some(mut o) = oracle() else { return };
    let env = review_env(&mut o);
    let sal = manager_salaries(&o);
    let mut xs = Xs(0xa11a_c0de);
    let mut seen = (0u32, 0u32, 0u32, 0u32, 0u32);
    for case in 0..8000 {
        let distress = case % 3 == 0;
        let r = rand_review(&mut xs, &sal, distress);
        let co = write_review(&mut o, &env, &r);
        // Live metrics from the verified functions (finance.md 1, 3.1).
        o.call_thiscall(METRIC, co, &[0]).unwrap();
        let price = o.st0().unwrap();
        o.call_thiscall(METRIC, co, &[3]).unwrap();
        let eps = o.st0().unwrap();
        let live = Live { price, eps };
        let seed = xs.next();
        set_rng(&mut o, seed);
        let mut rng = GameRng::new(seed);
        let want = review_ref(&r, &live, &mut rng);
        o.call_fastcall(ANNUAL_REVIEW, co, 0, &[]).unwrap();
        let got = read_review(&o, &env, &r, co);
        assert_eq!(
            got,
            want,
            "case {case} (distress {distress}, age {}, price {price}, eps {eps})",
            r.year - r.founded
        );
        seen.0 += u32::from(want.bonus != 0);
        seen.1 += want.fire.0;
        seen.2 += want.liq.0;
        seen.3 += u32::from(want.warn_year != r.warn_year as i16);
        seen.4 += want.popups;
    }
    eprintln!(
        "bonus {} fire {} liquidation {} warn {} popups {}",
        seen.0, seen.1, seen.2, seen.3, seen.4
    );
    assert!(seen.0 > 100 && seen.1 > 20 && seen.2 > 10 && seen.3 > 20 && seen.4 > 20);
}

// ---- 5. merger vote (0x48b5d0, called through 0x4015a0) -------------------------------------------

struct Voter {
    active: bool,
    human: bool,
    holding: i32,
    chair_of: i32,
    answered: bool,
    vote: u8,
    rep: i32,
}

fn rand_voters(xs: &mut Xs, shares: i32, a: i32, b: i32) -> Vec<Voter> {
    let n = xs.range(1, 5);
    (0..n)
        .map(|_| Voter {
            active: !xs.chance(15),
            human: xs.chance(30),
            holding: match xs.below(5) {
                0 => 0,
                1 => -1000 * xs.range(1, 20),
                _ => (f64::from(shares) * xs.real(0.0, 0.45)) as i32 / 1000 * 1000,
            },
            chair_of: [0, 0, a, b][xs.below(4) as usize],
            answered: !xs.chance(8),
            vote: u8::from(xs.chance(50)),
            rep: xs.range(-100, 100),
        })
        .collect()
}

fn write_voters(o: &mut Oracle, env: &ReviewEnv, voters: &[Voter], pos: u32) {
    for (i, v) in voters.iter().enumerate() {
        let p = env.persons.rec(i as u32 + 1);
        o.write_mem(p, &vec![0u8; PERSON_SIZE as usize]).unwrap();
        w32(o, p + PE_ACTIVE, u32::from(v.active));
        w32(o, p + PE_HUMAN, u32::from(v.human));
        w32(o, p + PE_HOLDINGS + 4 * pos, v.holding as u32);
        w32(o, p + PE_CHAIR_OF, v.chair_of as u32);
        w32(o, p + PE_REP, v.rep as u32);
    }
    env.persons.set_len(o, voters.len() as u32);
}

/// The two companies of a vote on a common calendar: `(proposer, target)` with ids 1 and 2 or 2
/// and 1, both in the company set.
fn rand_pair(xs: &mut Xs, sal: &[i32; 40]) -> (Review, Review) {
    let mut t = rand_review(xs, sal, false);
    let mut p = rand_review(xs, sal, false);
    t.cid = xs.range(1, 3);
    p.cid = 1 + t.cid % 3;
    p.year = t.year;
    p.month = t.month;
    p.day = t.day;
    p.founded = t.year - xs.range(0, 14);
    t.set = vec![p.cid as u16, t.cid as u16];
    p.set = t.set.clone();
    (p, t)
}

#[derive(Debug, PartialEq)]
struct MergeOut {
    votes: Vec<u8>,
    merge: (u32, u32, u32, u32),
    dialogs: u32,
    deleted: u32,
    retry_at: u32,
    rng: u32,
}

#[test]
fn merger_vote_matches_original() {
    let Some(mut o) = oracle() else { return };
    let env = review_env(&mut o);
    let proposals = Pool::new(&mut o, G_MERGERS, 0x48, 3);
    let sal = manager_salaries(&o);
    let mut xs = Xs(0x3e46_e757);
    let (mut passed, mut failed, mut waited, mut skipped) = (0, 0, 0, 0);
    for case in 0..6000 {
        let (rp, rt) = rand_pair(&mut xs, &sal);
        let voters = rand_voters(&mut xs, rt.shares, rp.cid, rt.cid);
        let price = xs.range(1, 400) as u16;
        let both_active = !xs.chance(5);
        // Calendar, pools and the two company records.
        write_review(&mut o, &env, &rp);
        let cp = write_review(&mut o, &env, &rt);
        let ct = env.companies.rec(rt.cid as u32);
        assert_eq!(cp, ct);
        let cpro = env.companies.rec(rp.cid as u32);
        write_company_record(&mut o, cpro, &rp);
        w32(&mut o, cpro + CO_ACTIVE, 1);
        // Both companies in the set; the stale persons of write_review are replaced.
        w32(&mut o, G_SET_COUNT, 2);
        w16(&mut o, G_SET_IDS, rp.cid as u16);
        w16(&mut o, G_SET_IDS + 2, rt.cid as u16);
        let pos = 1; // the target is the second company of the set
        write_voters(&mut o, &env, &voters, pos);
        w32(&mut o, ct + CO_ACTIVE, u32::from(both_active));
        let prop = proposals.rec(1);
        o.write_mem(prop, &[0u8; 0x48]).unwrap();
        w32(&mut o, prop, 1);
        w16(&mut o, prop + 4, rp.cid as u16);
        w16(&mut o, prop + 6, rt.cid as u16);
        w16(&mut o, prop + 8, price);
        for (i, v) in voters.iter().enumerate() {
            w8(&mut o, prop + 10 + i as u32, u8::from(v.answered));
            w8(&mut o, prop + 0x2a + i as u32, v.vote);
        }
        proposals.set_len(&mut o, 1);
        for a in [
            M_DIALOG,
            M_MERGE,
            M_MERGE + 4,
            M_MERGE + 8,
            M_MERGE + 12,
            M_DELETE,
        ] {
            w32(&mut o, a, 0);
        }
        w32(&mut o, cpro + 0x14c, 0);
        w32(&mut o, 0x0057_04f4, 600_000_000);
        w8(&mut o, MSG_BUFFER, 0);

        // Live values from the verified functions of finance.md.
        o.call_thiscall(0x0043_b410, ct, &[0, 0]).unwrap();
        let pq = o.st0().unwrap() as f32;
        o.call_fastcall(0x0043_aea0, ct, 0, &[]).unwrap();
        let pf = o.st0().unwrap();
        o.call_fastcall(0x0046_b480, 0x22, rp.cid as u32, &[0, 0])
            .unwrap();
        let eff = o.st0().unwrap();
        o.call_thiscall(METRIC, cpro, &[5]).unwrap();
        let r5 = o.st0().unwrap();
        let free = o.call_fastcall(0x0043_e580, ct, 0, &[]).unwrap() as i32;

        let seed = xs.next();
        set_rng(&mut o, seed);
        let mut rng = GameRng::new(seed);
        let mut votes: Vec<u8> = voters.iter().map(|v| v.vote).collect();
        let mut want = MergeOut {
            votes: votes.clone(),
            merge: (0, 0, 0, 0),
            dialogs: 0,
            deleted: 0,
            retry_at: 0,
            rng: seed,
        };
        if voters.iter().all(|v| v.answered) && both_active {
            let mut v = ((pf + f64::from(pq)) * f64::from(0.5f32)) as f32;
            if v < 1.0 {
                v = 1.0;
            }
            let a = (((f64::from(price) / eff) / f64::from(v) - 1.0) * 0.7 + 0.1) as f32;
            let b = ((r5 - 0.1) * 0.333_333_333_333_333_3 + f64::from(a)) as f32;
            let rnd = rng.range(-10, 10);
            let mut c = (f64::from(rnd) * 0.01 + f64::from(b)) as f32;
            let mut yes = 0i32;
            for (i, vt) in voters.iter().enumerate() {
                if !vt.human && vt.holding > 0 {
                    let mut q = (f64::from(c) - 0.1) as f32;
                    if vt.chair_of == rt.cid {
                        q = (f64::from(q) - 0.45) as f32;
                    }
                    let d = rng.range(-20, 20);
                    q = (f64::from(d) * 0.01 + f64::from(q)) as f32;
                    let roll = rng.range(0, 100);
                    votes[i] = u8::from(f64::from(q) * 100.0 > f64::from(roll));
                }
                if votes[i] != 0 && vt.holding > 0 {
                    yes += vt.holding;
                }
            }
            if c < 0.0 {
                c = 0.0;
            }
            if f64::from(c) > 1.0 {
                c = 1.0;
            }
            yes += ftol(f64::from(free) * f64::from(c)) / 1000 * 1000;
            let pass = yes > rt.shares / 2;
            want.dialogs = 1;
            want.deleted = 1;
            if pass {
                want.merge = (1, rp.cid as u32, rt.cid as u32, u32::from(price));
                passed += 1;
            } else {
                want.retry_at = 600_000_000;
                failed += 1;
            }
            want.votes = votes;
        } else if !voters.iter().all(|v| v.answered) {
            waited += 1;
        } else {
            skipped += 1;
        }
        want.rng = rng.state();

        if let Err(e) = o.call_fastcall(0x0048_b5d0, 1, 0, &[]) {
            let pc = o.uc().pc_read().unwrap();
            panic!("case {case}: {e:?} at pc {pc:#x}");
        }
        let got = MergeOut {
            votes: (0..voters.len() as u32)
                .map(|i| o.read_mem(prop + 0x2a + i, 1).unwrap()[0])
                .collect(),
            merge: (
                r32(&o, M_MERGE),
                r32(&o, M_MERGE + 4),
                r32(&o, M_MERGE + 8),
                r32(&o, M_MERGE + 12),
            ),
            dialogs: r32(&o, M_DIALOG),
            deleted: r32(&o, M_DELETE),
            retry_at: r32(&o, cpro + 0x14c),
            rng: get_rng(&o),
        };
        let mut want_cmp = want;
        // the stub logs `this` as an address
        if want_cmp.merge.0 != 0 {
            want_cmp.merge.1 = cpro;
            want_cmp.merge.2 = rt.cid as u32;
        }
        assert_eq!(got, want_cmp, "case {case}");
    }
    eprintln!("merger: {passed} pass, {failed} fail, {waited} waiting, {skipped} inactive");
    assert!(passed > 100 && failed > 100 && waited > 100);
}

// ---- 6. chairmanship vote (0x42ee00) ---------------------------------------------------------------

#[derive(Debug, PartialEq)]
struct ElectOut {
    votes: Vec<u8>,
    elect: (u32, u32, u32),
    dialogs: u32,
    deleted: u32,
    stamp: u32,
    rng: u32,
}

#[test]
fn chairmanship_vote_matches_original() {
    let Some(mut o) = oracle() else { return };
    let env = review_env(&mut o);
    let proposals = Pool::new(&mut o, G_ELECTIONS, 0x48, 3);
    let sal = manager_salaries(&o);
    let mut xs = Xs(0xe1ec_7001);
    let (mut passed, mut failed, mut waited) = (0, 0, 0);
    for case in 0..6000 {
        let mut r = rand_review(&mut xs, &sal, false);
        r.set = vec![r.cid as u16];
        let mut voters = rand_voters(&mut xs, r.shares, 0, r.cid);
        // the challenger is person 1..=n
        let challenger = xs.range(1, voters.len() as i32);
        // the incumbent chairman votes no; persons that chair another company count as `0`
        for v in &mut voters {
            if v.chair_of != 0 && v.chair_of != r.cid {
                v.chair_of = 0;
            }
        }
        write_review(&mut o, &env, &r);
        let co = env.companies.rec(r.cid as u32);
        write_voters(&mut o, &env, &voters, 0);
        w32(&mut o, co + CO_ACTIVE, u32::from(!xs.chance(5)));
        let active = r32(&o, co + CO_ACTIVE) != 0;
        let prop = proposals.rec(1);
        o.write_mem(prop, &[0u8; 0x48]).unwrap();
        w32(&mut o, prop, 1);
        w16(&mut o, prop + 4, r.cid as u16);
        w16(&mut o, prop + 6, challenger as u16);
        for (i, v) in voters.iter().enumerate() {
            w8(&mut o, prop + 8 + i as u32, u8::from(v.answered));
            w8(&mut o, prop + 0x28 + i as u32, v.vote);
        }
        proposals.set_len(&mut o, 1);
        for a in [M_DIALOG, M_ELECT, M_ELECT + 4, M_ELECT + 8, M_DELETE + 4] {
            w32(&mut o, a, 0);
        }
        w32(&mut o, 0x0057_04f4, 600_000_000);
        let chal = env.persons.rec(challenger as u32);
        w32(&mut o, chal + 0x527, 0);
        w8(&mut o, MSG_BUFFER, 0);

        let free = o.call_fastcall(0x0043_e580, co, 0, &[]).unwrap() as i32;
        let seed = xs.next();
        set_rng(&mut o, seed);
        let mut rng = GameRng::new(seed);
        let mut votes: Vec<u8> = voters.iter().map(|v| v.vote).collect();
        let mut want = ElectOut {
            votes: votes.clone(),
            elect: (0, 0, 0),
            dialogs: 0,
            deleted: 0,
            stamp: 0,
            rng: seed,
        };
        if voters.iter().all(|v| v.answered) && active {
            let chal_rep = voters[challenger as usize - 1].rep;
            let p0 = (0.2 - f64::from(r.rep) * 0.016_666_666_666_666_666
                + f64::from(chal_rep) * 0.0125) as f32;
            let rnd = rng.range(-10, 10);
            let ext = f64::from(rnd) * 0.01 + f64::from(p0);
            let p1 = ext as f32;
            let p = if ext > 1.0 {
                1.0
            } else if p1 < 0.0 {
                0.0
            } else {
                p1
            };
            let mut yes = ftol(f64::from(free) * f64::from(p)) / 1000 * 1000;
            for (i, vt) in voters.iter().enumerate() {
                if !vt.human && vt.holding > 0 {
                    if vt.chair_of != r.cid {
                        let d = rng.range(-20, 20);
                        let q = (f64::from(d) * 0.01 + f64::from(p)) as f32;
                        let roll = rng.range(0, 100);
                        votes[i] = u8::from(f64::from(q) * 100.0 > f64::from(roll));
                    } else {
                        votes[i] = 0;
                    }
                }
                if votes[i] != 0 && vt.holding > 0 {
                    yes += vt.holding;
                }
            }
            want.dialogs = 1;
            want.deleted = 1;
            if yes > r.shares / 2 {
                want.elect = (1, co, challenger as u32);
                passed += 1;
            } else {
                // a failed challenge is stamped on the challenger (one year of waiting)
                want.stamp = 600_000_000;
                failed += 1;
            }
            want.votes = votes;
        } else if !voters.iter().all(|v| v.answered) {
            waited += 1;
        }
        want.rng = rng.state();

        o.call_fastcall(0x0042_ee00, 1, 0, &[]).unwrap();
        let got = ElectOut {
            votes: (0..voters.len() as u32)
                .map(|i| o.read_mem(prop + 0x28 + i, 1).unwrap()[0])
                .collect(),
            elect: (r32(&o, M_ELECT), r32(&o, M_ELECT + 4), r32(&o, M_ELECT + 8)),
            dialogs: r32(&o, M_DIALOG),
            deleted: r32(&o, M_DELETE + 4),
            stamp: r32(&o, chal + 0x527),
            rng: get_rng(&o),
        };
        assert_eq!(got, want, "case {case}");
    }
    eprintln!("election: {passed} pass, {failed} fail, {waited} waiting");
    assert!(passed > 100 && failed > 100 && waited > 100);
}

// ---- 7. new company (0x438a10): founding capital, share count, initial price ----------------------

const COMPANY_INIT: u32 = 0x0043_8a10; // thiscall(id, chairman, color, name, capital) ret 0x14
const G_TEMP_NAME: u32 = 0x0057_07b9; // counter into TEMP_COMPANY_NAMES

/// Initial price and share count of a company founded with `capital` dollars (finance.md 17).
fn founding_ref(capital: i32) -> (f32, i32, i32) {
    let k = if capital == -1 { 500_000 } else { capital };
    let mut price = cdiv(k, 10_000) as f32;
    while price >= 100.0 {
        price *= 0.5;
    }
    let shares = ftol(f64::from(k) / f64::from(price) * f64::from(0.001f32)) * 1000;
    let cash = ftol(f64::from(price)).wrapping_mul(shares);
    (price, shares, cash)
}

#[test]
fn company_init_matches_original() {
    let Some(mut o) = oracle() else { return };
    let env = review_env(&mut o);
    let sal = manager_salaries(&o);
    // set_manager: marks the manager as employed by the new company
    patch(
        &mut o,
        0x0043_8780,
        &[
            0x8b, 0x44, 0x24, 0x04, // mov eax,[esp+4]
            0x8b, 0x11, // mov edx,[ecx]
            0x88, 0x90, 0x09, 0x3e, 0x57, 0x00, // mov [eax+0x573e09],dl
            0x89, 0x81, 0x3c, 0x01, 0x00, 0x00, // mov [ecx+0x13c],eax
            0xc2, 0x08, 0x00,
        ],
    );
    patch(&mut o, 0x0043_ddb0, &[0xc3]); // territory rights and standing (finance.md 17)
    let date_slot: u32 = 0x0057_a900;
    let mut d = vec![0xb8];
    d.extend_from_slice(&date_slot.to_le_bytes());
    d.extend_from_slice(&[0xc2, 0x10, 0x00]);
    patch(&mut o, 0x004a_49f0, &d); // date of the year 1800
    let mut xs = Xs(0xf0f0_5eed);
    let mut capitals = std::collections::BTreeSet::new();
    for case in 0..4000 {
        let year = xs.range(1850, 1990);
        let month = xs.range(1, 12);
        write_date(&mut o, year, month, xs.range(1, 28));
        w32(&mut o, 0x0057_04f4, 600_000_000);
        w32(&mut o, G_LOCK, 0);
        // companies in the pool: the new one is the last
        let n_total = xs.range(1, 4) as u32;
        let new_id = n_total;
        env.companies.set_len(&mut o, n_total);
        let mut used = [false; 41];
        for id in 1..=n_total {
            let c = env.companies.rec(id);
            o.write_mem(c, &vec![0u8; CO_SIZE as usize]).unwrap();
            w32(&mut o, c, id);
            if id != new_id {
                let color = xs.below(41) as u8;
                let active = xs.chance(80);
                let scenery = xs.chance(10);
                w32(&mut o, c + CO_ACTIVE, u32::from(active));
                w8(&mut o, c + 0x478, u8::from(scenery));
                w8(&mut o, c + 0x2b, color);
                if active && !scenery {
                    used[color as usize] = true;
                }
            }
        }
        used[0] = true; // the new company counts itself: active, `+0x478 == 0`, colour 0
        if used.iter().all(|&u| u) {
            used[40] = false;
            for id in 1..new_id {
                let c = env.companies.rec(id);
                if ri32(&o, c + CO_ACTIVE) != 0 && o.read_mem(c + 0x2b, 1).unwrap()[0] == 40 {
                    w8(&mut o, c + 0x2b, 39);
                    used[39] = true;
                }
            }
        }
        let explicit = xs.chance(40);
        let color_arg = if explicit { xs.range(0, 40) } else { -1 };
        let chairman = if xs.chance(30) { 0 } else { xs.range(1, 3) };
        let capital = match xs.below(5) {
            0 => -1,
            1 => 1000 * xs.range(50, 3000),
            2 => 10_000 * xs.range(1, 40),
            3 => xs.range(100_000, 9_000_000),
            _ => 500_000 + 1000 * xs.range(0, 1200),
        };
        let named = xs.chance(30);
        let name_ptr = if named { M_EMPTY } else { 0 };
        for i in 1..=3u32 {
            let p = env.persons.rec(i);
            o.write_mem(p, &vec![0u8; PERSON_SIZE as usize]).unwrap();
            w32(&mut o, p + PE_HUMAN, u32::from(xs.chance(50)));
        }
        env.persons.set_len(&mut o, 3);
        let mut uses = [0u8; 40];
        for _ in 0..xs.below(36) {
            uses[xs.below(40) as usize] = 1 + xs.below(9) as u8;
        }
        let mut avail = [1u8; 40];
        if xs.chance(40) {
            for a in &mut avail {
                *a = u8::from(!xs.chance(35));
            }
        }
        for _ in 0..6 {
            let f = xs.below(40) as usize;
            uses[f] = 0;
            avail[f] = 1;
        }
        o.write_mem(G_MANAGER_USE, &uses).unwrap();
        o.write_mem(G_MANAGER_AVAIL, &avail).unwrap();
        w8(&mut o, G_REF_SPEED, 0);
        let others = xs.below(40);
        w32(&mut o, G_TEMP_NAME, others);
        w32(&mut o, G_SET_COUNT, new_id - 1);
        w8(&mut o, 0x0057_0717, 2);

        let seed = xs.next();
        set_rng(&mut o, seed);
        let mut rng = GameRng::new(seed);
        // reference: colour, manager and two candidates, then the capital
        let color = if color_arg == -1 {
            let free = used.iter().filter(|&&u| !u).count() as i32;
            let r = rng.range(0, free - 1) + 1;
            let mut left = r;
            let mut pick = 0;
            for (i, &u) in used.iter().enumerate() {
                if !u {
                    left -= 1;
                }
                if left == 0 {
                    pick = i as i32;
                    break;
                }
            }
            pick
        } else {
            color_arg
        };
        let pick_in = |uses: [u8; 40], c140: i32, c144: i32| PickIn {
            year,
            month,
            founded: year,
            cats: [0.0; 13],
            arch: [[0.0; 13]; 10],
            c140,
            c144,
            uses,
            avail,
            sal,
            lock: false,
        };
        let mut cursor = 0;
        let g0 = pick_ref(&pick_in(uses, -1, -1), -1, &mut rng, &mut cursor);
        let mut uses2 = uses;
        uses2[g0 as usize] = new_id as u8;
        let g1 = pick_ref(&pick_in(uses2, -1, -1), -1, &mut rng, &mut cursor);
        let g2 = pick_ref(&pick_in(uses2, g1, -1), -1, &mut rng, &mut cursor);
        let (price, shares, cash) = founding_ref(capital);

        let co = env.companies.rec(new_id);
        o.call_thiscall(
            COMPANY_INIT,
            co,
            &[
                new_id,
                chairman as u32,
                color_arg as u32,
                name_ptr,
                capital as u32,
            ],
        )
        .unwrap();
        assert_eq!(
            ri32(&o, co + CO_SHARES),
            shares,
            "shares {case} capital {capital}"
        );
        assert_eq!(
            rf32(&o, co + CO_INIT_PRICE),
            price,
            "price {case} capital {capital}"
        );
        assert_eq!(rf64(&o, co + CO_ACCT), f64::from(cash), "cash {case}");
        assert_eq!(
            rf64(&o, co + CO_ACCT + 56),
            -f64::from(cash),
            "equity {case}"
        );
        assert_eq!(rf32(&o, co + 0x30), 0.5, "dividend {case}");
        assert_eq!(ri32(&o, co + 0x138), 6000, "overhead {case}");
        assert_eq!(ri32(&o, co + CO_SALARY), 10_000, "salary {case}");
        assert_eq!(ri32(&o, co + CO_FOUNDED), year, "founded {case}");
        assert_eq!(
            ri16(&o, co + CO_CHAIR_SINCE),
            year as i16,
            "chair since {case}"
        );
        assert_eq!(ri32(&o, co + 0x425c), 600_000_000, "created {case}");
        assert_eq!(ri32(&o, co + 0x17e), 599_999_999, "{case}");
        assert_eq!(
            o.read_mem(co + 0x2b, 1).unwrap()[0] as i32,
            color,
            "colour {case}"
        );
        assert_eq!(
            [
                ri32(&o, co + 0x13c),
                ri32(&o, co + CO_CAND0),
                ri32(&o, co + CO_CAND1)
            ],
            [g0, g1, g2],
            "manager and candidates {case}"
        );
        assert_eq!(r32(&o, G_SET_COUNT), new_id, "set {case}");
        assert_eq!(
            r16(&o, G_SET_IDS + 2 * (new_id - 1)),
            new_id as u16,
            "set id {case}"
        );
        let counter = if named { others } else { others + 1 };
        assert_eq!(
            r32(&o, G_TEMP_NAME),
            counter,
            "temporary name counter {case}"
        );
        assert_eq!(get_rng(&o), rng.state(), "rng {case}");
        capitals.insert((price.to_bits(), shares));
    }
    assert!(capitals.len() > 100);
}

fn r16(o: &Oracle, a: u32) -> u16 {
    u16::from_le_bytes(o.read_mem(a, 2).unwrap().try_into().unwrap())
}
