//! Differential tests for the company finance functions (docs/spec/finance.md): prime rate,
//! trade price with trade pressure, fundamental share price, credit rating and the bond rate.
//! Each test lays a company record out in emulated memory, calls the original and compares with a
//! Rust reference written in this file. The x87 control word is pinned to 53-bit precision
//! (CW 0x27f). Skipped when the executable is absent.
//!
//! The code under test is `rt2_sim::finance` (the test only lays the record out and wraps it); the
//! company id is 0 throughout so that `effect_multiplier` and `effect_sum` only see the global
//! effect table (no entity lists are needed).

use rt2_oracle::Oracle;
use rt2_sim::exe::Exe;
use rt2_sim::finance::{self, Bond, Company, Ctx, FinanceTables};
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

// ---- globals and the company record ------------------------------------------------------------

const GAME_TIME: u32 = 0x0057_04f4;
const GAME_DATE: u32 = 0x0057_04f8; // u16 year, u8 month, u8 day
const ECON_STATE: u32 = 0x0057_0717; // 4 bytes
const PRIME_RATE: u32 = 0x0057_06d1; // f32
const STOCK_LIQUIDITY: u32 = 0x0057_06cd; // f32
const EFFECT_GLOBAL: u32 = 0x0057_07c6; // i16[effect]

const PRIME_FOR: u32 = 0x0046_bac0; // fastcall(ECX = company id) -> ST0
const TRADE_PRICE: u32 = 0x0043_b410; // thiscall(n, commit) -> ST0
const FUNDAMENTAL: u32 = 0x0043_aea0; // fastcall(ECX = company) -> ST0
const RATING: u32 = 0x0043_b650; // thiscall(X) -> ST0
const BOND_RATE: u32 = 0x0043_92e0; // thiscall(X, _) -> ST0
const LEDGER_CREDIT: u32 = 0x0043_9970; // thiscall(cat, f64 x), ret 0xc
const MOVE_ACCOUNT: u32 = 0x0043_98d0; // thiscall(account, f64 x), ret 0xc
const ANNUALIZED: u32 = 0x0043_a440; // thiscall(i) -> ST0
const MAX_DIVIDEND: u32 = 0x0043_edf0; // thiscall() -> ST0
const BANKRUPTCY: u32 = 0x0043_bc30; // thiscall()
const CO_YEAR_CATS: u32 = 0x46fe;
const CO_SHARES_AT: u32 = CO_SHARES;

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

/// Game tables read from the executable (never hard-coded in the simulation).
fn tables() -> FinanceTables {
    let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    });
    FinanceTables::load(&Exe::from_bytes(std::fs::read(path).expect("read exe")).expect("pe"))
        .expect("finance tables")
}

impl Co {
    /// The same record as the simulation's [`Company`].
    fn to_sim(&self) -> Company {
        Company {
            shares: self.shares,
            dividend: self.div,
            pressure: self.pressure,
            init_price: self.init_price,
            founded: self.founded,
            bankrupt_year: self.bankrupt_year,
            created: self.created,
            cats: self.cats,
            arch_cats: self.arch_cats,
            acct: self.acct,
            arch_metrics: self.arch,
            bonds: self
                .bonds
                .iter()
                .map(|&(principal, maturity_year, rate)| Bond {
                    principal,
                    maturity_year,
                    rate,
                })
                .collect(),
            ..Company::default()
        }
    }
}

fn ctx<'a>(w: &World, t: &'a FinanceTables) -> Ctx<'a> {
    Ctx {
        tables: t,
        year: w.year,
        month: w.month,
        day: w.day,
        minutes: w.minutes,
        econ_state: w.state,
        prime_rate: w.prime_rate,
        liquidity: w.liquidity,
        price_effect: rt2_sim::effects::global_modifier(w.fx_price),
        prime_effect_pct: w.fx_prime,
        rating_effect: w.fx_rating,
    }
}

// ---- 1. prime rate (0x46bac0) -----------------------------------------------------------------

fn prime_ref(w: &World) -> f64 {
    finance::prime_rate_for(w.prime_rate, w.fx_prime)
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
fn trade_price_ref(t: &FinanceTables, w: &World, c: &Co, pf: f64, n: i32) -> (i32, f32) {
    let mut co = c.to_sim();
    co.price_stamp = w.minutes;
    co.price_cache = pf;
    let price = co.trade_price(&ctx(w, t), n, true);
    (price, co.pressure)
}

#[test]
fn trade_price_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let t = tables();
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
        let (want, press) = trade_price_ref(&t, &w, &c, pf, n);
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

fn fundamental_ref(t: &FinanceTables, w: &World, c: &Co) -> (f64, f32) {
    let mut co = c.to_sim();
    let p = co.fundamental_price(&ctx(w, t));
    assert_eq!(co.price_stamp, w.minutes);
    assert_eq!(co.price_cache, p);
    (p, co.no_div_price)
}

#[test]
fn fundamental_price_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let t = tables();
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
        let (want, nodiv) = fundamental_ref(&t, &w, &c);
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

fn rating_ref(t: &FinanceTables, w: &World, c: &Co, x: i32) -> f64 {
    c.to_sim().credit_rating(&ctx(w, t), x)
}

#[test]
fn credit_rating_and_bond_rate_match_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let t = tables();
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
        let want = rating_ref(&t, &w, &c, x);
        o.call_thiscall(RATING, base, &[x as u32]).unwrap();
        assert_eq!(o.st0().unwrap(), want, "rating case {case} x {x}");
        seen[want as usize] += 1;
        let bond = c.to_sim().bond_rate(&ctx(&w, &t), x);
        o.call_thiscall(BOND_RATE, base, &[x as u32, 0]).unwrap();
        assert_eq!(o.st0().unwrap(), bond, "bond rate case {case}");
    }
    // The generator must exercise the whole scale, not just the clamps.
    assert!(
        seen.iter().filter(|n| **n > 0).count() >= 7,
        "ratings seen: {seen:?}"
    );
}

// ---- 5. ledger entries (0x439970, 0x4398d0) ----------------------------------------------------

fn read_f64s<const N: usize>(o: &Oracle, a: u32) -> [f64; N] {
    let mut v = [0.0; N];
    for (i, x) in v.iter_mut().enumerate() {
        *x = rf64(o, a + 8 * i as u32);
    }
    v
}

fn f64_args(x: f64) -> [u32; 2] {
    let b = x.to_bits();
    [b as u32, (b >> 32) as u32]
}

#[test]
fn ledger_entries_match_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let mut xs = Xs(0x1ed9_e121);
    for case in 0..1500 {
        let w = rand_world(&mut xs);
        let c = rand_company(&mut xs, &w);
        let mut want = c.to_sim();
        want.year_cats = [0.0; 13];
        write_company(&mut o, base, &c);
        for (i, v) in want.year_cats.iter().enumerate() {
            wf64(
                &mut o,
                base + CO_YEAR_CATS + 8 * i as u32,
                *v + 0.25 * i as f64,
            );
        }
        for (i, v) in want.year_cats.iter_mut().enumerate() {
            *v += 0.25 * i as f64;
        }
        w32(&mut o, base + CO_STAMP, 777);
        want.price_stamp = 777;
        let x = match xs.below(3) {
            0 => xs.real(-1.0e6, 1.0e6).floor(),
            1 => xs.real(-1.0e6, 1.0e6),
            _ => xs.real(-50.0, 50.0) + 0.1,
        };
        let k = xs.below(13) as usize;
        let [lo, hi] = f64_args(x);
        if xs.chance(50) {
            o.call_thiscall(LEDGER_CREDIT, base, &[k as u32, lo, hi])
                .unwrap();
            want.credit(k, x);
        } else {
            let acct = xs.below(8) as usize;
            o.call_thiscall(MOVE_ACCOUNT, base, &[acct as u32, lo, hi])
                .unwrap();
            want.move_account(acct, x);
        }
        assert_eq!(o.read_u32(base + CO_STAMP).unwrap(), 0, "stamp {case}");
        assert_eq!(want.price_stamp, 0);
        assert_eq!(
            read_f64s::<13>(&o, base + CO_CATS),
            want.cats,
            "cats {case}"
        );
        assert_eq!(
            read_f64s::<13>(&o, base + CO_YEAR_CATS),
            want.year_cats,
            "year {case}"
        );
        assert_eq!(read_f64s::<8>(&o, base + CO_ACCT), want.acct, "acct {case}");
    }
}

// ---- 6. annualized totals (0x43a440) -------------------------------------------------------------

#[test]
fn annualized_totals_match_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let t = tables();
    let mut xs = Xs(0xa22a_11ed);
    for case in 0..3000 {
        let mut w = rand_world(&mut xs);
        if case % 7 == 0 {
            w.month = 12;
            w.day = 28;
        }
        write_world(&mut o, &w);
        let c = rand_company(&mut xs, &w);
        write_company(&mut o, base, &c);
        let i = xs.below(16);
        o.call_thiscall(ANNUALIZED, base, &[i]).unwrap();
        let want = c.to_sim().annualized(&ctx(&w, &t), i as usize);
        assert_eq!(o.st0().unwrap(), want, "case {case} i {i}");
    }
}

// ---- 7. board dividend cap (0x43edf0) ------------------------------------------------------------

#[test]
fn max_dividend_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let t = tables();
    let mut xs = Xs(0xd1d1_d1d1);
    let mut positive = 0;
    for case in 0..4000 {
        let mut w = rand_world(&mut xs);
        if case % 4 == 0 {
            w.month = 12;
            w.day = 28;
        }
        write_world(&mut o, &w);
        let mut c = rand_company(&mut xs, &w);
        // Mostly healthy companies so that the cap is positive and the branches differ.
        if xs.chance(70) {
            c.acct[0] = xs.real(0.0, 6.0e6).floor();
        }
        write_company(&mut o, base, &c);
        o.call_thiscall(MAX_DIVIDEND, base, &[]).unwrap();
        let want = c.to_sim().max_dividend(&ctx(&w, &t));
        assert_eq!(
            o.st0().unwrap(),
            want,
            "case {case} age {} month {}",
            w.year - c.founded,
            w.month
        );
        if want > 0.0 {
            positive += 1;
        }
    }
    assert!(positive > 500, "too few positive caps: {positive}");
}

// ---- 8. bankruptcy (0x43bc30) ----------------------------------------------------------------------

#[test]
fn bankruptcy_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let base = o.alloc(CO_SIZE).unwrap();
    let t = tables();
    let mut xs = Xs(0xba4c_0ff5);
    for case in 0..1500 {
        let w = rand_world(&mut xs);
        write_world(&mut o, &w);
        let mut c = rand_company(&mut xs, &w);
        if xs.chance(30) {
            c.acct[0] = -xs.real(0.0, 2.0e6);
        }
        // Odd principals exercise the truncating halves.
        for b in &mut c.bonds {
            b.0 += xs.range(0, 3) * 1001;
        }
        write_company(&mut o, base, &c);
        let mut want = c.to_sim();
        want.declare_bankruptcy(&ctx(&w, &t));
        o.call_thiscall(BANKRUPTCY, base, &[]).unwrap();
        assert_eq!(read_f64s::<8>(&o, base + CO_ACCT), want.acct, "acct {case}");
        assert_eq!(o.read_u32(base + CO_SHARES_AT).unwrap() as i32, want.shares);
        assert_eq!(
            o.read_u32(base + CO_BANKRUPT_YEAR).unwrap() as i32,
            want.bankrupt_year
        );
        for (i, b) in want.bonds.iter().enumerate() {
            let a = base + CO_BONDS + 12 * i as u32;
            assert_eq!(o.read_u32(a).unwrap() as i32, b.principal, "bond {case}");
        }
        // The closing quote refreshes the fundamental-price cache.
        assert_eq!(
            o.read_u32(base + CO_STAMP).unwrap() as i32,
            want.price_stamp,
            "stamp {case}"
        );
        assert_eq!(
            rf64(&o, base + CO_PRICE_CACHE),
            want.price_cache,
            "cache {case}"
        );
    }
}
