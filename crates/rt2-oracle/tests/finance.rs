//! Differential tests for the company finance functions (docs/spec/finance.md): prime rate,
//! trade price with trade pressure, fundamental share price, credit rating and the bond rate.
//! Each test lays a company record out in emulated memory, calls the original and compares with a
//! Rust reference written in this file. The x87 control word is pinned to 53-bit precision
//! (CW 0x27f). Skipped when the executable is absent.
//!
//! The reference functions are the executable documentation of the formulas; the company id is 0
//! throughout so that `effect_multiplier` and `effect_sum` only see the global effect table (no
//! entity lists are needed).

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

fn set_fpcw(o: &mut Oracle, cw: u64) {
    o.uc().reg_write(RegisterX86::FPCW, cw).unwrap();
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

fn ftol(x: f64) -> i32 {
    (x.trunc() as i64) as i32
}

// ---- globals and the company record ------------------------------------------------------------

const GAME_TIME: u32 = 0x0057_04f4;
const GAME_DATE: u32 = 0x0057_04f8; // u16 year, u8 month, u8 day
const ECON_STATE: u32 = 0x0057_0717; // 4 bytes
const PRIME_RATE: u32 = 0x0057_06d1; // f32
const STOCK_LIQUIDITY: u32 = 0x0057_06cd; // f32
const EFFECT_GLOBAL: u32 = 0x0057_07c6; // i16[effect]
const ECON_STOCK_MULT: [f32; 5] = [0.8, 0.9, 1.0, 1.1, 1.2];

const PRIME_FOR: u32 = 0x0046_bac0; // fastcall(ECX = company id) -> ST0
const TRADE_PRICE: u32 = 0x0043_b410; // thiscall(n, commit) -> ST0
const FUNDAMENTAL: u32 = 0x0043_aea0; // fastcall(ECX = company) -> ST0
const RATING: u32 = 0x0043_b650; // thiscall(X) -> ST0
const BOND_RATE: u32 = 0x0043_92e0; // thiscall(X, _) -> ST0

const CO_SIZE: u32 = 0x4e86;
const CO_ACTIVE: u32 = 0x16e;
const CO_SHARES: u32 = 0x2c;
const CO_DIV: u32 = 0x30;
const CO_PRESSURE: u32 = 0x34;
const CO_INIT_PRICE: u32 = 0x3c;
const CO_BOND_N: u32 = 0x40;
const CO_BONDS: u32 = 0x44;
const CO_FOUNDED: u32 = 0x148;
const CO_BANKRUPT_YEAR: u32 = 0x158;
const CO_STAMP: u32 = 0x4250;
const CO_PRICE_CACHE: u32 = 0x4254;
const CO_CREATED: u32 = 0x425c;
const CO_NO_DIV_PRICE: u32 = 0x427a;
const CO_LAST_PRICE: u32 = 0x427e;
const CO_CATS: u32 = 0x4286;
const CO_ARCH_CATS: u32 = 0x42ee; // [10 years][13]
const CO_ACCT: u32 = 0x4766; // [8]
const CO_ARCH_METRICS: u32 = 0x49b6; // [k][14], k = years ago (0x70 bytes)

fn w32(o: &mut Oracle, a: u32, v: u32) {
    o.write_u32(a, v).unwrap();
}
fn wi16(o: &mut Oracle, a: u32, v: i16) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn wf32(o: &mut Oracle, a: u32, v: f32) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn wf64(o: &mut Oracle, a: u32, v: f64) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn rf32(o: &Oracle, a: u32) -> f32 {
    f32::from_le_bytes(o.read_mem(a, 4).unwrap().try_into().unwrap())
}
fn rf64(o: &Oracle, a: u32) -> f64 {
    f64::from_le_bytes(o.read_mem(a, 8).unwrap().try_into().unwrap())
}

/// Calendar and economy globals shared by all the functions under test.
#[derive(Clone, Copy)]
struct World {
    year: i32,
    month: i32,
    day: i32,
    minutes: i32,
    state: usize,
    prime_rate: f32,
    fx_prime: i16,  // effect 0x20, percent points
    fx_rating: i16, // effect 0x1f, rating levels
    fx_price: i16,  // effect 0x1e, percent
    liquidity: f32,
}

fn rand_world(xs: &mut Xs) -> World {
    let state = xs.below(5) as usize;
    World {
        year: xs.range(1850, 1990),
        month: xs.range(1, 12),
        day: xs.range(1, 28),
        minutes: xs.range(1_000_000, 900_000_000),
        state,
        prime_rate: (0.09 - state as f64 * 0.01) as f32,
        fx_prime: if xs.chance(50) {
            0
        } else {
            xs.range(-4, 6) as i16
        },
        fx_rating: if xs.chance(60) {
            0
        } else {
            xs.range(-3, 3) as i16
        },
        fx_price: if xs.chance(50) {
            0
        } else {
            xs.range(-40, 80) as i16
        },
        liquidity: if xs.chance(60) {
            100.0
        } else {
            xs.real(40.0, 250.0) as f32
        },
    }
}

fn write_world(o: &mut Oracle, w: &World) {
    w32(
        o,
        GAME_DATE,
        w.year as u32 | (w.month as u32) << 16 | (w.day as u32) << 24,
    );
    w32(o, GAME_TIME, w.minutes as u32);
    w32(o, ECON_STATE, w.state as u32);
    wf32(o, PRIME_RATE, w.prime_rate);
    wf32(o, STOCK_LIQUIDITY, w.liquidity);
    for e in 0..0x30u32 {
        wi16(o, EFFECT_GLOBAL + 2 * e, 0);
    }
    wi16(o, EFFECT_GLOBAL + 2 * 0x20, w.fx_prime);
    wi16(o, EFFECT_GLOBAL + 2 * 0x1f, w.fx_rating);
    wi16(o, EFFECT_GLOBAL + 2 * 0x1e, w.fx_price);
}

#[derive(Clone)]
struct Co {
    shares: i32,
    div: f32,
    pressure: f32,
    init_price: f32,
    founded: i32,
    bankrupt_year: i32,
    created: i32,
    cats: [f64; 13],
    arch_cats: [[f64; 13]; 10],
    acct: [f64; 8],
    /// `arch[k]`, k = 1..=10 years ago (index 0 unused); 14 metrics each.
    arch: [[f64; 14]; 11],
    /// `(principal, maturity year, rate)`.
    bonds: Vec<(i32, i32, f32)>,
}

fn rand_cats(xs: &mut Xs, scale: f64) -> [f64; 13] {
    let mut c = [0.0; 13];
    for (i, v) in c.iter_mut().enumerate() {
        let mag = if xs.chance(15) {
            0.0
        } else {
            (xs.real(0.0, scale)).floor()
        };
        *v = if i < 5 { mag } else { -mag * 0.6 };
        if xs.chance(10) {
            *v += 0.5;
        }
    }
    c
}

fn rand_company(xs: &mut Xs, w: &World) -> Co {
    let scale = [2.0e4, 3.0e5, 2.0e6][xs.below(3) as usize];
    let age = if xs.chance(30) {
        xs.range(0, 5)
    } else {
        xs.range(0, 14)
    };
    let mut arch_cats = [[0.0; 13]; 10];
    for y in &mut arch_cats {
        *y = rand_cats(xs, scale);
    }
    let mut arch = [[0.0; 14]; 11];
    for y in arch.iter_mut().skip(1) {
        y[0] = xs.real(2.0, 250.0).floor();
        y[1] = xs.real(-5.0, 120.0);
        y[2] = xs.real(0.0, 80.0);
        y[3] = xs.real(-6.0, 25.0);
        y[4] = if xs.chance(30) {
            0.0
        } else {
            f64::from(xs.below(80)) * 0.1
        };
        for m in y.iter_mut().skip(5) {
            *m = xs.real(-10.0, 10.0);
        }
    }
    let mut acct = [0.0; 8];
    acct[0] = xs.real(-3.0e6, 6.0e6).floor();
    for a in acct.iter_mut().take(6).skip(1) {
        *a = xs.real(0.0, 4.0e6).floor();
    }
    let mut bonds = Vec::new();
    let nb = if xs.chance(25) {
        0
    } else {
        xs.below(6) as usize
    };
    let mut debt = 0.0;
    for _ in 0..nb {
        let p = 500_000 * xs.range(1, 4);
        bonds.push((
            p,
            w.year + xs.range(1, 30),
            (xs.real(0.05, 0.16) * 100.0).floor() as f32 / 100.0,
        ));
        debt += f64::from(p);
    }
    acct[6] = -debt;
    acct[7] = if xs.chance(20) {
        xs.real(-5.0e5, 0.0).floor()
    } else {
        -xs.real(0.0, 9.0e6).floor()
    };
    Co {
        shares: 1000 * xs.range(100, 3000),
        div: if xs.chance(40) {
            0.0
        } else {
            xs.real(0.0, 6.0) as f32
        },
        pressure: xs.real(-2.0e5, 2.0e5) as f32,
        init_price: xs.real(5.0, 100.0) as f32,
        founded: w.year - age,
        bankrupt_year: if xs.chance(60) {
            0
        } else {
            w.year - xs.range(0, 20)
        },
        created: w.minutes - xs.range(0, 60 * 40320),
        cats: rand_cats(xs, scale),
        arch_cats,
        acct,
        arch,
        bonds,
    }
}

fn write_company(o: &mut Oracle, base: u32, c: &Co) {
    o.write_mem(base, &vec![0u8; CO_SIZE as usize]).unwrap();
    w32(o, base + CO_ACTIVE, 1);
    w32(o, base + CO_SHARES, c.shares as u32);
    wf32(o, base + CO_DIV, c.div);
    wf32(o, base + CO_PRESSURE, c.pressure);
    wf32(o, base + CO_INIT_PRICE, c.init_price);
    w32(o, base + CO_FOUNDED, c.founded as u32);
    w32(o, base + CO_BANKRUPT_YEAR, c.bankrupt_year as u32);
    w32(o, base + CO_CREATED, c.created as u32);
    w32(o, base + CO_STAMP, 0);
    for (i, v) in c.cats.iter().enumerate() {
        wf64(o, base + CO_CATS + 8 * i as u32, *v);
    }
    for (k, y) in c.arch_cats.iter().enumerate() {
        for (i, v) in y.iter().enumerate() {
            wf64(o, base + CO_ARCH_CATS + 0x68 * k as u32 + 8 * i as u32, *v);
        }
    }
    for (i, v) in c.acct.iter().enumerate() {
        wf64(o, base + CO_ACCT + 8 * i as u32, *v);
    }
    for (k, y) in c.arch.iter().enumerate().skip(1) {
        for (i, v) in y.iter().enumerate() {
            wf64(
                o,
                base + CO_ARCH_METRICS + 0x70 * k as u32 + 8 * i as u32,
                *v,
            );
        }
    }
    w32(o, base + CO_BOND_N, c.bonds.len() as u32);
    for (i, (p, y, r)) in c.bonds.iter().enumerate() {
        let a = base + CO_BONDS + 12 * i as u32;
        w32(o, a, *p as u32);
        w32(o, a + 4, *y as u32);
        wf32(o, a + 8, *r);
    }
}

fn effect_mult(g: i16) -> f64 {
    let v = ((f64::from(g) * 0.01) + 1.0) as f32;
    if v >= 0.01 { f64::from(v) } else { 0.1 }
}

fn econ(w: &World) -> f64 {
    f64::from(ECON_STOCK_MULT[w.state])
}

// ---- 1. prime rate (0x46bac0) -----------------------------------------------------------------

fn prime_ref(w: &World) -> f64 {
    let v = f64::from(w.fx_prime) * 0.01 + f64::from(w.prime_rate) + 0.001;
    f64::from(ftol(v * 100.0)) * 0.01
}

#[test]
fn prime_rate_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let mut xs = Xs(0x1234_5678);
    for case in 0..600 {
        let mut w = rand_world(&mut xs);
        if case % 3 == 0 {
            w.prime_rate = xs.real(0.0, 0.2) as f32;
        }
        write_world(&mut o, &w);
        o.call_fastcall(PRIME_FOR, 0, 0, &[]).unwrap();
        assert_eq!(o.st0().unwrap(), prime_ref(&w), "case {case}");
    }
}

// ---- 2. trade price (0x43b410) ----------------------------------------------------------------

/// `0x43b410`: returns `(price, new_pressure)`; `pf` is the fundamental price already cached.
fn trade_price_ref(w: &World, c: &Co, pf: f64, n: i32) -> (i32, f32) {
    let s = f64::from(c.shares);
    let mut pf = pf;
    if f64::from(c.init_price) < pf && w.year - c.founded <= 5 {
        let mut m = (w.minutes - c.created) / 40320;
        if m >= 50 {
            m = 50;
        }
        pf = (f64::from(50 - m) * f64::from(c.init_price) + f64::from(50 + m) * pf) * 0.01;
    }
    let mut x = f64::from(n) / s;
    if x > 0.2 {
        x = f64::from(0.2f32);
    }
    if x < -0.2 {
        x = f64::from(-0.2f32);
    }
    let press = if n != 0 {
        (1.0 - x) * f64::from(c.pressure) + f64::from(n)
    } else {
        f64::from(c.pressure)
    };
    let p = rt2_sim::fp::pow(s * 5e-5, 0.33);
    let mut r = ((p * (100.0 / f64::from(w.liquidity))) * press * 1.4 + s) / s;
    if r < 0.6 {
        r = (r - 0.6) * 0.5 + 0.6;
    }
    if r < 0.45 {
        r = (r - 0.45) * 0.5 + 0.45;
    }
    if r < 0.3 {
        r = 0.3;
    } else {
        if r > 1.7 {
            r = (r - 1.7) * 0.5 + 1.7;
        }
        if r > 2.5 {
            r = (r - 2.5) * 0.5 + 2.5;
        }
        if r > 4.0 {
            r = (r - 4.0) * 0.5 + 4.0;
        }
        if r > 6.0 {
            r = 6.0;
        }
    }
    (ftol(r * pf + 0.5), press as f32)
}

#[test]
fn trade_price_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let mut xs = Xs(0x0bad_cafe);
    for case in 0..4000 {
        let w = rand_world(&mut xs);
        write_world(&mut o, &w);
        let c = rand_company(&mut xs, &w);
        write_company(&mut o, base, &c);
        // The fundamental price is served from the per-minute cache.
        let pf = xs.real(1.0, 400.0);
        w32(&mut o, base + CO_STAMP, w.minutes as u32);
        wf64(&mut o, base + CO_PRICE_CACHE, pf);
        let n = match xs.below(4) {
            0 => 0,
            1 => xs.range(-300, 300) * 1000,
            2 => xs.range(-(c.shares / 3), c.shares / 3),
            _ => xs.range(-(c.shares), c.shares),
        };
        let commit = xs.chance(50);
        let (want, press) = trade_price_ref(&w, &c, pf, n);
        o.call_thiscall(TRADE_PRICE, base, &[n as u32, u32::from(commit)])
            .unwrap();
        let got = o.st0().unwrap();
        assert_eq!(got, f64::from(want), "case {case}: n {n}");
        if commit {
            assert_eq!(
                rf32(&o, base + CO_LAST_PRICE),
                want as f32,
                "last price {case}"
            );
            if n != 0 {
                assert_eq!(rf32(&o, base + CO_PRESSURE), press, "pressure {case}");
            }
        } else {
            assert_eq!(rf32(&o, base + CO_PRESSURE), c.pressure, "no commit {case}");
        }
    }
}

// ---- 3. fundamental price (0x43aea0) ----------------------------------------------------------

/// Annualized current-year total (0x43a440), `i` = 13 (profit) or 14 (revenue).
fn annualized(w: &World, c: &Co, i: usize) -> f64 {
    let sum = |cats: &[f64; 13]| -> f64 {
        let n = if i == 13 { 13 } else { 5 };
        cats.iter().take(n).fold(0.0, |a, b| a + b)
    };
    let raw = sum(&c.cats);
    if w.month == 12 && w.day == 28 {
        return raw;
    }
    let mut f = (f64::from(w.day + 28 * w.month - 28) * 0.002_976_190_476_190_476_4) as f32;
    if f64::from(f) < 0.05 {
        f = 0.05f32;
    }
    if f64::from(f) > 0.95 {
        f = 0.95f32;
    }
    let f = f64::from(f);
    if w.year <= c.founded {
        return raw / f;
    }
    let last = &c.arch_cats[0];
    let raw = raw + f * last[3];
    f * raw + (1.0 - f) * sum(last)
}

/// Live metrics 1..4 (0x43ad60).
fn live_metric(w: &World, c: &Co, m: usize) -> f64 {
    let s = f64::from(c.shares);
    match m {
        1 => -c.acct[7] / s,
        2 => annualized(w, c, 14) / s,
        3 => annualized(w, c, 13) / s,
        _ => f64::from(c.div),
    }
}

fn fundamental_ref(w: &World, c: &Co) -> (f64, f32) {
    const BASE_W: [f64; 4] = [40.0, 10.0, 20.0, 30.0];
    const AGE_W: [[f64; 4]; 5] = [
        [0.0, 0.0, 0.0, 100.0],
        [25.0, 25.0, 35.0, 100.0],
        [50.0, 50.0, 65.0, 100.0],
        [75.0, 75.0, 85.0, 100.0],
        [100.0, 100.0, 100.0, 100.0],
    ];
    let metric = |k: usize, m: usize| -> f64 {
        if k == 0 {
            live_metric(w, c, m)
        } else {
            c.arch[k][m]
        }
    };
    let (mut wsum, mut acc_e, mut acc_r) = (0i32, 0.0f64, 0.0f64);
    for i in 0..5usize {
        if w.year - i as i32 >= c.founded {
            let wi = if i == 0 {
                w.month * 5 - 5
            } else {
                (5 - i as i32) * 12
            };
            wsum += wi;
            acc_e += f64::from(wi) * metric(i, 3);
            acc_r += f64::from(wi) * metric(i, 2);
        }
    }
    let (mut vsum, mut cont, mut dsum) = (0i32, 0.0f64, 0.0f64);
    for i in 1..10usize {
        if w.year - i as i32 >= c.founded {
            let v = 10 - i as i32;
            vsum += v;
            dsum += f64::from(v) * c.arch[i][4];
            if w.year - i as i32 > c.founded {
                cont += f64::from(v);
            }
        }
    }
    let mut crit = [0.0f64; 4];
    if wsum != 0 {
        crit[0] = (acc_e / f64::from(wsum)) * 10.0;
        crit[1] = acc_r / f64::from(wsum);
    }
    if vsum != 0 {
        let mut d = (dsum / f64::from(vsum)) as f32;
        let t = ((c.arch[1][4] + c.arch[1][4]) + f64::from(d)) * f64::from(0.333_333_34f32);
        if f64::from(d) >= t {
            d = t as f32;
        }
        crit[2] = (((cont / f64::from(vsum)) * 0.5) + 0.6) * f64::from(d) * 30.0;
    }
    crit[3] = live_metric(w, c, 1);
    let mut m = 1.0f64;
    for (k, v) in crit.iter().enumerate() {
        if k != 2 && *v >= m {
            m = *v;
        }
    }
    let m = m * 1.13;
    if crit[2] >= m {
        crit[2] = m;
    }
    for v in &mut crit {
        if *v < 0.0 {
            *v = 0.0;
        }
    }
    let mut wt = BASE_W;
    let age = w.year - c.founded;
    if age < 5 {
        for j in 0..4 {
            wt[j] *= AGE_W[age as usize][j];
        }
    }
    let (mut imax, mut imin) = (0usize, 0usize);
    for k in 0..4 {
        if crit[k] > crit[imax] {
            imax = k;
        }
        if crit[k] < crit[imin] {
            imin = k;
        }
    }
    wt[imax] *= 1.25;
    wt[imin] *= 0.8;
    let (mut x, mut wx, mut y, mut wy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for k in 0..4 {
        let t = crit[k] * wt[k];
        x += t;
        wx += wt[k];
        if k != 2 {
            y += t;
            wy += wt[k];
        }
    }
    let e = econ(w);
    let mut nodiv = ((y / wy) * (e + 1.0) * 0.5) as f32;
    if f64::from(nodiv) < 0.5 {
        nodiv = 0.5;
    }
    let mut p = effect_mult(w.fx_price) * ((x / wx) * e);
    let a2 = w.year - c.bankrupt_year;
    if a2 < 10 {
        p *= f64::from(a2) * 0.025 + 0.75;
    }
    if p < 1.0 {
        p = 1.0;
    }
    (p, nodiv)
}

#[test]
fn fundamental_price_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let mut xs = Xs(0x5eed_f00d);
    for case in 0..3000 {
        let mut w = rand_world(&mut xs);
        if case % 7 == 0 {
            w.month = 12;
            w.day = 28;
        }
        w.fx_price = w.fx_price.max(-50);
        write_world(&mut o, &w);
        let c = rand_company(&mut xs, &w);
        write_company(&mut o, base, &c);
        let (want, nodiv) = fundamental_ref(&w, &c);
        o.call_fastcall(FUNDAMENTAL, base, 0, &[]).unwrap();
        let got = o.st0().unwrap();
        assert_eq!(
            got,
            want,
            "case {case} age {} y {}",
            w.year - c.founded,
            w.year
        );
        assert_eq!(rf64(&o, base + CO_PRICE_CACHE), want, "cache {case}");
        assert_eq!(
            rf32(&o, base + CO_NO_DIV_PRICE),
            nodiv,
            "no-dividend price {case}"
        );
        assert_eq!(o.read_u32(base + CO_STAMP).unwrap(), w.minutes as u32);
    }
}

// ---- 4. credit rating (0x43b650) and bond rate (0x4392e0) --------------------------------------

fn rating_ref(w: &World, c: &Co, x: i32) -> f64 {
    // Weighted average of (profit - interest) over the last four years.
    let (mut acc, mut ebx) = (0.0f64, 0i32);
    for i in 1..=4i32 {
        ebx += 5 - i;
        if w.year - i >= c.founded {
            let cats = &c.arch_cats[(i - 1) as usize];
            let ni = cats.iter().fold(0.0, |a, b| a + b);
            acc += (ni - cats[9]) * f64::from(5 - i);
        }
    }
    let a = acc / f64::from(ebx);
    let avg_rate = if c.bonds.is_empty() {
        0.0
    } else {
        let mut sum = 0.0f64;
        let mut tot = 0i32;
        for (p, _, r) in &c.bonds {
            tot += p;
            sum += f64::from(*p) * f64::from(*r);
        }
        sum / f64::from(tot)
    };
    let mut est = (c.acct[6] + f64::from(x)) * avg_rate;
    let cash = c.acct[0];
    if cash < 0.0 {
        est += (prime_ref(w) + 0.03) * cash;
    }
    let cov = if est == 0.0 { 10.0 } else { a / (-est) };
    let mut s1 = if cov >= 5.0 {
        10.0
    } else if cov > 2.0 {
        10.0 - (5.0 - cov)
    } else if cov > 1.0 {
        10.0 - ((2.0 - cov) * 2.0 + 3.0)
    } else if cov > 0.0 {
        10.0 - ((1.0 - cov) * 4.0 + 5.0)
    } else {
        0.0
    };
    let age = w.year - c.founded;
    if age < 5 {
        s1 += f64::from(5 - age) * 0.1 * (10.0 - s1);
    }
    let total = ftol(c.acct.iter().take(6).fold(0.0, |a, b| a + b));
    let ratio = if total > 0 {
        (-c.acct[6] + f64::from(x)) / f64::from(total)
    } else {
        1.0
    };
    let s2 = if ratio <= 0.1 {
        10.0
    } else if ratio < 0.25 {
        10.0 - ratio * 4.0
    } else if ratio < 0.5 {
        10.0 - ((ratio - 0.25) * 8.0 + 1.0)
    } else if ratio < 0.75 {
        10.0 - ((ratio - 0.5) * 12.0 + 3.0)
    } else if ratio < 1.0 {
        10.0 - ((ratio - 0.75) * 16.0 + 6.0)
    } else {
        0.0
    };
    let p = (s1 * s2 * 0.1) * 2.0;
    let q = p + (s2 + s1) * 0.5;
    let mut r = q * 0.333_333_333_333_333_3 * econ(w);
    let eq = -c.acct[7];
    if eq < 1_000_000.0 {
        r -= 10.0 - eq * f64::from(f32::from_bits(0x3727_c5ac));
    }
    if age < 5 {
        let a = age.max(1);
        r -= f64::from(5 - a) * 0.5;
        let cap = f64::from(a + 5);
        if r >= cap {
            r = cap;
        }
    }
    let ba = w.year - c.bankrupt_year;
    if ba < 15 {
        r = f64::from(ba) * r * 0.0666;
    }
    let v = f64::from(ftol(r + 0.5)) + f64::from(w.fx_rating);
    v.clamp(0.0, 10.0)
}

#[test]
fn credit_rating_and_bond_rate_match_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let mut xs = Xs(0xdead_beef);
    let mut seen = [0u32; 11];
    for case in 0..4000 {
        let w = rand_world(&mut xs);
        write_world(&mut o, &w);
        let c = rand_company(&mut xs, &w);
        write_company(&mut o, base, &c);
        let x = if xs.chance(50) {
            0
        } else {
            500_000 * xs.range(1, 6)
        };
        let want = rating_ref(&w, &c, x);
        o.call_thiscall(RATING, base, &[x as u32]).unwrap();
        assert_eq!(o.st0().unwrap(), want, "rating case {case} x {x}");
        seen[want as usize] += 1;
        let bond = prime_ref(&w) + (10.0 - want) * 0.01;
        o.call_thiscall(BOND_RATE, base, &[x as u32, 0]).unwrap();
        assert_eq!(o.st0().unwrap(), bond, "bond rate case {case}");
    }
    // The generator must exercise the whole scale, not just the clamps.
    assert!(
        seen.iter().filter(|n| **n > 0).count() >= 7,
        "ratings seen: {seen:?}"
    );
}
