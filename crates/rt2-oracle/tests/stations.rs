//! Differential tests for station improvements, asset sales and the track-block priority rule
//! (docs/spec/terrain-track.md sections 3.4, 5, 6 and 10): the improvement table in `.data`
//! (`0x5500c8`), the availability filter `0x468310`, the purchase of improvements `0x46aef0`
//! (cumulative `ftol` cost and ledger entries), the write-off of a removed station `0x4bf4e0`,
//! the industry purchase/sale `0x43eb60` and the tile-conflict/priority test `0x416050`.
//!
//! Each test lays the inputs out in the emulated memory, runs the original and compares it with a
//! Rust reference written from the spec in this file. Skipped when the executable is absent. The
//! x87 control word is pinned to 0x27f (53 bits). Calls that only repaint, relink or reroute
//! (`0x406f60`, `0x42e410`, `0x46a370`) are patched to a bare `ret` in the emulated copy, and
//! the train-stop function `0x4d3c80` is replaced by a recorder, so the tests see only the
//! economic and priority behaviour.

use rt2_oracle::Oracle;
use rt2_sim::effects::{EffectLevels, effect_multiplier};
use rt2_sim::exe::Exe;
use unicorn_engine::RegisterX86;

fn exe_path() -> String {
    std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    })
}

fn oracle() -> Option<Oracle> {
    let path = exe_path();
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return None;
    }
    Some(Oracle::load_file(&path).expect("load exe"))
}

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
    fn chance(&mut self, pct: u32) -> bool {
        self.next() % 100 < pct
    }
}

fn fpu_reset(o: &mut Oracle) {
    let uc = o.uc();
    uc.reg_write(RegisterX86::FPCW, 0x27f).unwrap();
    uc.reg_write(RegisterX86::FPSW, 0).unwrap();
    uc.reg_write(RegisterX86::FPTAG, 0xffff).unwrap();
}

fn w32(o: &mut Oracle, a: u32, v: u32) {
    o.write_u32(a, v).unwrap();
}
fn w16(o: &mut Oracle, a: u32, v: u16) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn w8(o: &mut Oracle, a: u32, v: u8) {
    o.write_mem(a, &[v]).unwrap();
}
fn wf64(o: &mut Oracle, a: u32, v: f64) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn rd_i32(o: &Oracle, a: u32) -> i32 {
    o.read_u32(a).unwrap() as i32
}
fn rf64(o: &Oracle, a: u32) -> f64 {
    f64::from_le_bytes(o.read_mem(a, 8).unwrap().try_into().unwrap())
}

/// A list object as seen by `0x4f18b0`: record `id` at `base + id * stride`.
fn make_list(o: &mut Oracle, global: u32, stride: u32, n: u32) -> u32 {
    let base = o.alloc(stride * (n + 1)).unwrap();
    let hdr = o.alloc(0x40).unwrap();
    w32(o, hdr + 8, stride);
    w32(o, hdr + 0x28, base);
    w32(o, global, hdr);
    base
}

// ---- globals and layouts -------------------------------------------------------------------------

const GAME_DATE: u32 = 0x0057_04f8; // u16 year, u8 month, u8 day
const EXPANSION: u32 = 0x0057_04ac; // byte: 0 = base industry set, 1 = expansion set
const SCENARIO: u32 = 0x0057_4bc8; // pointer to the scenario options (+0x60 = ignore dates)
const EFFECT: u32 = 0x0057_07c6; // i16 percent per effect id
const LIST_STATIONS: u32 = 0x0057_4c60;
const LIST_TRAINS: u32 = 0x0057_4c64;
const LIST_COMPANIES: u32 = 0x0057_4c6c;
const LIST_OBJECTS: u32 = 0x0057_4c74;
const LIST_PIECES: u32 = 0x0057_4c84;
const LIST_TERRITORIES: u32 = 0x0057_4ca8;
const TILES: u32 = 0x0057_4c24;
const TILES_W: u32 = 0x0055_e7a4;
const NO_UI: u32 = 0x0057_b3f0; // nonzero: skip the window refreshes

/// `.data` improvement table: 30 rows of 0x1b bytes starting at `0x5500c8`.
const IMPROVEMENTS: u32 = 0x0055_00c8;
const IMPROVEMENT_ROW: u32 = 0x1b;
const IMPROVEMENT_ROWS: usize = 30;
/// Building table cost column (`0x5457ef + type * 0x154`, `docs/spec/terrain-track.md` 3.1).
const BUILDING_COST: u32 = 0x0054_57ef;
const BUILDING_STRIDE: u32 = 0x154;

const CO_SIZE: u32 = 0x4c00;
const CO_EFFECT: u32 = 0x1ae; // i16 per effect id
const CO_PERSON: u32 = 0x23;
const CO_STATIONS: u32 = 0x172; // station count
const CO_STAMP: u32 = 0x4250;
const CO_CATS: u32 = 0x4286; // f64[13]
const CO_YEAR: u32 = 0x46fe; // f64[13]
const CO_ACCT: u32 = 0x4766; // f64[8]
const TERR_EFFECT: u32 = 0x58;
const TERR_SIZE: u32 = 0x300;

const ST_SIZE: u32 = 0x500;
const OBJ_SIZE: u32 = 0xa4;

/// One row of the improvement table (`docs/spec/terrain-track.md` 3.4).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    /// Key stored at the start of the row (`"STATNA"`, `"WATER"`, ...; blank for unused rows).
    name: String,
    cost: i32,
    first_year: i32,
    last_year: i32,
    enabled: bool,
    excludes: u32,
}

/// Reads the table from the executable file (not from the emulator).
fn table_from_exe(exe: &Exe) -> Vec<Row> {
    let mut rows = Vec::new();
    for i in 0..IMPROVEMENT_ROWS as u32 {
        let a = IMPROVEMENTS + i * IMPROVEMENT_ROW;
        let r = exe.read(a, IMPROVEMENT_ROW as usize).unwrap();
        let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let name_bytes = &r[0..7];
        let name = String::from_utf8_lossy(name_bytes)
            .trim_end_matches(['\0', ' '])
            .to_string();
        rows.push(Row {
            name,
            cost: i32_at(7),
            first_year: i32_at(11),
            last_year: i32_at(15),
            enabled: i32_at(19) != 0,
            excludes: i32_at(23) as u32,
        });
    }
    rows
}

fn exe() -> Option<Exe> {
    let path = exe_path();
    std::fs::read(&path)
        .ok()
        .map(|b| Exe::from_bytes(b).expect("pe"))
}

// ---- 1. the table ---------------------------------------------------------------------------------

#[test]
fn improvement_table_matches_emulated_memory_and_spec() {
    let (Some(o), Some(exe)) = (oracle(), exe()) else {
        return;
    };
    let rows = table_from_exe(&exe);
    for (i, r) in rows.iter().enumerate() {
        let a = IMPROVEMENTS + i as u32 * IMPROVEMENT_ROW;
        assert_eq!(r.cost, rd_i32(&o, a + 7), "cost {i}");
        assert_eq!(r.first_year, rd_i32(&o, a + 11), "first {i}");
        assert_eq!(r.last_year, rd_i32(&o, a + 15), "last {i}");
        assert_eq!(r.enabled, rd_i32(&o, a + 19) != 0, "flag {i}");
        assert_eq!(r.excludes, o.read_u32(a + 23).unwrap(), "excl {i}");
    }
    // (id, key, cost, first year, last year, excludes) of the 19 purchasable rows (spec 3.4).
    let want: [(usize, &str, i32, i32, i32, u32); 19] = [
        (0, "STATNA", 50_000, 1800, 9999, 0x6),
        (1, "STATNB", 100_000, 1800, 9999, 0x4),
        (2, "STATNC", 200_000, 1800, 9999, 0x0),
        (3, "TELEP", 80_000, 1905, 9999, 0x0),
        (4, "TELEGR", 40_000, 1837, 1910, 0x8),
        (5, "RESTS", 50_000, 1800, 9999, 0x40),
        (6, "RESTL", 100_000, 1850, 9999, 0x0),
        (7, "SALOO", 16_000, 1800, 9999, 0x0),
        (8, "POST", 35_000, 1840, 9999, 0x0),
        (9, "CUSTOM", 25_000, 1800, 9999, 0x0),
        (10, "SAND", 30_000, 1800, 9999, 0x0),
        (11, "WATER", 25_000, 1800, 9999, 0x0),
        (15, "HOTELL", 175_000, 1878, 9999, 0x0),
        (16, "HOTELS", 65_000, 1800, 9999, 0x8000),
        (17, "LIQUID", 50_000, 1885, 9999, 0x0),
        (18, "REFRIG", 50_000, 1860, 9999, 0x0),
        (19, "GRAIN", 35_000, 1873, 9999, 0x0),
        (20, "WAREH", 25_000, 1800, 9999, 0x0),
        (21, "ROUND", 120_000, 1800, 9999, 0x0),
    ];
    for (id, key, cost, y0, y1, ex) in want {
        // the key of row `id` is stored as the 7 bytes that precede its cost
        let k = exe
            .read(IMPROVEMENTS + id as u32 * IMPROVEMENT_ROW, 7)
            .unwrap();
        let k = String::from_utf8_lossy(k)
            .trim_end_matches(['\0', ' '])
            .to_string();
        assert_eq!(k, key, "key {id}");
        let r = &rows[id];
        assert_eq!(
            (r.cost, r.first_year, r.last_year, r.excludes),
            (cost, y0, y1, ex)
        );
        assert!(r.enabled, "row {id} enabled");
    }
    // everything else is switched off (turntable 12, second/main track line 13/14, engine
    // shop 22, and the unused rows 23..29)
    let enabled: Vec<usize> = (0..IMPROVEMENT_ROWS).filter(|&i| rows[i].enabled).collect();
    assert_eq!(
        enabled,
        vec![
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 18, 19, 20, 21
        ]
    );
    assert_eq!(rows[0].name, "STATNA");
}

// ---- 2. availability filter (0x468310) --------------------------------------------------------

fn available_ref(
    rows: &[Row],
    i: usize,
    year: i32,
    month: u8,
    expansion: bool,
    ignore: bool,
) -> bool {
    if expansion && !matches!(i, 0..=4 | 10 | 11 | 0x15) {
        return false;
    }
    let r = &rows[i];
    let started = year > r.first_year || (year == r.first_year && month >= 9);
    let in_window = (started && year <= r.last_year) || ignore;
    in_window && r.enabled
}

#[test]
fn improvement_availability_matches_original() {
    let (Some(mut o), Some(exe)) = (oracle(), exe()) else {
        return;
    };
    let rows = table_from_exe(&exe);
    let scen = o.alloc(0x100).unwrap();
    w32(&mut o, SCENARIO, scen);
    let mut rng = Xs(0x5eed_1234);
    let mut positives = 0;
    for _ in 0..6000 {
        let i = rng.below(IMPROVEMENT_ROWS as u32) as usize;
        let year = match rng.below(3) {
            0 => 1790 + rng.below(30) as i32,
            1 => 1830 + rng.below(90) as i32,
            _ => 1900 + rng.below(110) as i32,
        };
        // exact-boundary years for the dated rows
        let year = if rng.chance(25) {
            [1837, 1840, 1850, 1860, 1873, 1878, 1885, 1905, 1910, 1911][rng.below(10) as usize]
        } else {
            year
        };
        let month = 1 + rng.below(12) as u8;
        let expansion = rng.chance(30);
        let ignore = rng.chance(20);
        w32(
            &mut o,
            GAME_DATE,
            year as u32 | (u32::from(month) << 16) | (1 << 24),
        );
        w8(&mut o, EXPANSION, u8::from(expansion));
        w32(&mut o, scen + 0x60, u32::from(ignore));
        fpu_reset(&mut o);
        let got = o.call_fastcall(0x0046_8310, i as u32, 0, &[]).unwrap();
        let want = available_ref(&rows, i, year, month, expansion, ignore);
        assert_eq!(
            got & 0xff != 0,
            want,
            "row {i} {year}-{month} exp={expansion} ign={ignore}"
        );
        positives += usize::from(want);
    }
    assert!(positives > 500, "too few available cases: {positives}");
}

// ---- helpers for the company ledger ------------------------------------------------------------

/// The company ledger fields the original touches.
#[derive(Clone, Debug, PartialEq)]
struct Ledger {
    cats: [f64; 13],
    year: [f64; 13],
    acct: [f64; 8],
}

impl Ledger {
    /// `0x4398d0(account, x)`: asset or cash movement.
    fn account(&mut self, k: usize, x: f64) {
        if k == 0 {
            self.acct[0] += x;
            self.acct[7] -= x;
        } else {
            self.acct[k] += x;
            self.acct[0] -= x;
        }
    }
    /// `0x439970(category, x)`: income or expense.
    fn credit(&mut self, k: usize, x: f64) {
        self.cats[k] += x;
        self.year[k] += x;
        self.acct[0] += x;
        self.acct[7] -= x;
    }
}

fn read_f64s<const N: usize>(o: &Oracle, a: u32) -> [f64; N] {
    let mut v = [0.0; N];
    for (i, x) in v.iter_mut().enumerate() {
        *x = rf64(o, a + 8 * i as u32);
    }
    v
}

fn write_ledger(o: &mut Oracle, co: u32, l: &Ledger) {
    for i in 0..13 {
        wf64(o, co + CO_CATS + 8 * i as u32, l.cats[i]);
        wf64(o, co + CO_YEAR + 8 * i as u32, l.year[i]);
    }
    for i in 0..8 {
        wf64(o, co + CO_ACCT + 8 * i as u32, l.acct[i]);
    }
    w32(o, co + CO_STAMP, 777);
}

fn read_ledger(o: &Oracle, co: u32) -> Ledger {
    Ledger {
        cats: read_f64s::<13>(o, co + CO_CATS),
        year: read_f64s::<13>(o, co + CO_YEAR),
        acct: read_f64s::<8>(o, co + CO_ACCT),
    }
}

fn rand_ledger(rng: &mut Xs) -> Ledger {
    let mut l = Ledger {
        cats: [0.0; 13],
        year: [0.0; 13],
        acct: [0.0; 8],
    };
    for v in l
        .cats
        .iter_mut()
        .chain(l.year.iter_mut())
        .chain(l.acct.iter_mut())
    {
        *v = f64::from(rng.below(2_000_000)) - 1_000_000.0;
    }
    l
}

/// `ftol(mult * cost + acc)` with the multiplier an `f32` kept at extended precision.
fn ftol_mul_add(mult: f32, cost: i32, acc: i32) -> i32 {
    (f64::from(mult) * f64::from(cost) + f64::from(acc)) as i64 as i32
}

/// Effect percentages of one effect id at every level (global is shared by all entities).
struct EffectSetup {
    global: i16,
    company: i16,
    territory: i16,
}

fn effect_ref(e: &EffectSetup, with_territory: bool) -> f32 {
    effect_multiplier(EffectLevels {
        global: e.global,
        company: Some(e.company),
        person: None,
        territory: with_territory.then_some(e.territory),
    })
}

// ---- 3. buying improvements (0x46aef0, non-size bits) -------------------------------------------

#[test]
fn improvement_purchase_matches_original() {
    let (Some(mut o), Some(exe)) = (oracle(), exe()) else {
        return;
    };
    let rows = table_from_exe(&exe);
    let cos = make_list(&mut o, LIST_COMPANIES, CO_SIZE, 4);
    let sts = make_list(&mut o, LIST_STATIONS, ST_SIZE, 4);
    let objs = make_list(&mut o, LIST_OBJECTS, OBJ_SIZE, 4);
    let terrs = make_list(&mut o, LIST_TERRITORIES, TERR_SIZE, 4);
    let mut rng = Xs(0x0ddb_a115);
    for case in 0..1500 {
        let co = 1 + rng.below(3);
        let terr = rng.below(4); // 0 = no territory
        let e = EffectSetup {
            global: (rng.below(140) as i16) - 70,
            company: (rng.below(140) as i16) - 70,
            territory: (rng.below(140) as i16) - 70,
        };
        o.write_mem(EFFECT + 16, &e.global.to_le_bytes()).unwrap();
        let cbase = cos + co * CO_SIZE;
        w32(&mut o, cbase, co);
        w32(&mut o, cbase + CO_PERSON, 0);
        w16(&mut o, cbase + CO_EFFECT + 16, e.company as u16);
        if terr != 0 {
            w16(
                &mut o,
                terrs + terr * TERR_SIZE + TERR_EFFECT + 16,
                e.territory as u16,
            );
        }
        let ledger = rand_ledger(&mut rng);
        write_ledger(&mut o, cbase, &ledger);
        // station 1, object 1
        let st = sts + ST_SIZE;
        let obj = objs + OBJ_SIZE;
        let had = rng.below(1 << 22) & !0x7;
        let paid = rng.below(900_000) as i32;
        w32(&mut o, st, 1);
        w32(&mut o, st + 4, 1);
        w8(&mut o, st + 8, rng.below(3) as u8);
        w32(&mut o, st + 0x39, co);
        w32(&mut o, st + 0x45, 0);
        w32(&mut o, st + 0x49, had | 1);
        w32(&mut o, st + 0x4ad, terr);
        w32(&mut o, obj + 0x99, paid as u32);
        // a random purchase set among the non-size bits 3..29
        let mut mask = 0u32;
        for b in 3..30 {
            if rng.chance(25) {
                mask |= 1 << b;
            }
        }
        fpu_reset(&mut o);
        o.call_fastcall(0x0046_aef0, 1, mask, &[]).unwrap();

        // reference
        let m = effect_ref(&e, terr != 0);
        let mut total = 0;
        for (b, row) in rows.iter().enumerate().take(30) {
            if mask >> b & 1 != 0 {
                total = ftol_mul_add(m, row.cost, total);
            }
        }
        let mut want = ledger.clone();
        want.account(1, f64::from(total));
        want.credit(4, 0.0); // the old size's cost is 0 here: a zero entry, but the stamp is cleared
        assert_eq!(read_ledger(&o, cbase), want, "ledger {case}");
        assert_eq!(rd_i32(&o, cbase + CO_STAMP), 0, "stamp {case}");
        assert_eq!(rd_i32(&o, obj + 0x99), paid + total, "value {case}");
        assert_eq!(
            o.read_u32(st + 0x49).unwrap(),
            (had | 1) | mask,
            "mask {case}"
        );
    }
}

// ---- 4. removing a station (0x4bf4e0): no refund, the paid value is written off ---------------

/// Patches the function entry at `addr` to a bare `ret`.
fn stub_ret(o: &mut Oracle, addr: u32) {
    o.write_mem(addr, &[0xc3]).unwrap();
}

#[test]
fn station_removal_writes_off_value_matches_original() {
    let Some(mut o) = oracle() else { return };
    let cos = make_list(&mut o, LIST_COMPANIES, CO_SIZE, 4);
    let sts = make_list(&mut o, LIST_STATIONS, ST_SIZE, 4);
    let objs = make_list(&mut o, LIST_OBJECTS, OBJ_SIZE, 4);
    let tiles = o.alloc(8 * 8 * 15).unwrap();
    w32(&mut o, TILES, tiles);
    w32(&mut o, TILES_W, 8);
    w32(&mut o, NO_UI, 1);
    for f in [0x0040_6f60, 0x0042_e410, 0x0046_a370] {
        stub_ret(&mut o, f);
    }
    let mut rng = Xs(0x7a11_0ff5);
    for case in 0..800 {
        let co = rng.below(3); // 0 = unowned station
        let ghost = rng.chance(15);
        let cbase = cos + co.max(1) * CO_SIZE;
        let ledger = rand_ledger(&mut rng);
        write_ledger(&mut o, cbase, &ledger);
        w32(&mut o, cbase, co.max(1));
        let count = 1 + rng.below(30);
        w32(&mut o, cbase + CO_STATIONS, count);
        let st = sts + ST_SIZE;
        let obj = objs + OBJ_SIZE;
        let value = rng.below(1_500_000) as i32;
        w32(&mut o, st, 1);
        w32(&mut o, st + 4, 1);
        w32(&mut o, st + 0x39, co);
        w32(&mut o, st + 0x45, 0);
        w32(&mut o, st + 0xa, rng.below(8));
        w32(&mut o, st + 0xe, rng.below(8));
        w8(&mut o, obj + 0x2e, u8::from(ghost));
        w32(&mut o, obj + 0x99, value as u32);
        fpu_reset(&mut o);
        o.call_thiscall(0x004b_f4e0, st, &[0]).unwrap();

        let mut want = ledger.clone();
        let mut want_count = count;
        if !ghost && co != 0 {
            want_count -= 1;
            want.account(1, f64::from(-value));
            want.credit(5, f64::from(-value));
        }
        if ghost || co == 0 {
            // untouched, including the price-cache stamp
            assert_eq!(rd_i32(&o, cbase + CO_STAMP), 777, "stamp {case}");
        } else {
            assert_eq!(rd_i32(&o, cbase + CO_STAMP), 0, "stamp {case}");
        }
        assert_eq!(read_ledger(&o, cbase), want, "ledger {case}");
        assert_eq!(
            o.read_u32(cbase + CO_STATIONS).unwrap(),
            want_count,
            "count {case}"
        );
        assert_eq!(o.read_u32(st + 0x45).unwrap(), 1, "removed flag {case}");
        // the net cash effect of removing a station is zero
        if !ghost && co != 0 {
            assert_eq!(read_ledger(&o, cbase).acct[0], ledger.acct[0]);
        }
    }
}

// ---- 5. industry purchase and sale (0x43eb60, price 0x40bae0) ---------------------------------

#[test]
fn industry_purchase_and_sale_match_original() {
    let (Some(mut o), Some(exe)) = (oracle(), exe()) else {
        return;
    };
    let cos = make_list(&mut o, LIST_COMPANIES, CO_SIZE, 4);
    let objs = make_list(&mut o, LIST_OBJECTS, OBJ_SIZE, 4);
    let terrs = make_list(&mut o, LIST_TERRITORIES, TERR_SIZE, 4);
    let mut rng = Xs(0x1d05_71a1);
    for case in 0..1500 {
        let co = 1 + rng.below(3);
        let btype = rng.below(70);
        let terr = rng.below(4);
        let e = EffectSetup {
            global: (rng.below(140) as i16) - 70,
            company: (rng.below(140) as i16) - 70,
            territory: (rng.below(140) as i16) - 70,
        };
        // effect 0xd is the industry purchase price (spec 3.4 / "refunds")
        o.write_mem(EFFECT + 2 * 0xd, &e.global.to_le_bytes())
            .unwrap();
        let cbase = cos + co * CO_SIZE;
        w32(&mut o, cbase, co);
        w32(&mut o, cbase + CO_PERSON, 0);
        w16(&mut o, cbase + CO_EFFECT + 2 * 0xd, e.company as u16);
        if terr != 0 {
            w16(
                &mut o,
                terrs + terr * TERR_SIZE + TERR_EFFECT + 2 * 0xd,
                e.territory as u16,
            );
        }
        let ledger = rand_ledger(&mut rng);
        write_ledger(&mut o, cbase, &ledger);
        let obj = objs + OBJ_SIZE;
        for (a, v) in [(0x26u32, 1i16), (0x28, 1), (0x2a, 1), (0x2c, 1)] {
            w16(&mut o, obj + a, v as u16);
        }
        w8(&mut o, obj + 6, btype as u8);
        w8(&mut o, obj + 0x9d, terr as u8);
        w16(&mut o, obj + 0x97, 0);
        fpu_reset(&mut o);
        // purchase: ECX = company record, args (object id, 1)
        o.call_thiscall(0x0043_eb60, cbase, &[1, 1]).unwrap();
        let table_cost = exe.u32_at(BUILDING_COST + btype * BUILDING_STRIDE).unwrap() as i32;
        let price = ftol_mul_add(effect_ref(&e, terr != 0), table_cost, 0);
        let mut want = ledger.clone();
        want.acct[5] += f64::from(price);
        want.acct[0] -= f64::from(price);
        let got = read_ledger(&o, cbase);
        assert_eq!(got.acct, want.acct, "buy accounts {case}");
        assert_eq!(got.cats, want.cats, "buy cats {case}");
        assert_eq!(rd_i32(&o, obj + 0x99), price, "buy value {case}");
        assert_eq!(
            o.read_mem(obj + 0x97, 2).unwrap(),
            (co as u16).to_le_bytes()
        );
        assert_eq!(rd_i32(&o, cbase + CO_STAMP), 0);
        // `0x40bae0` on an owned building returns the value stored at +0x99 (no recomputation)
        fpu_reset(&mut o);
        let owned = o.call_fastcall(0x0040_bae0, 1, co, &[]).unwrap();
        assert_eq!(owned as i32, price, "owned value {case}");

        // sale: the owner gets back price - (price >> 1) in cash; the loss goes to category 3
        let before = read_ledger(&o, cbase);
        w32(&mut o, cbase + CO_STAMP, 555);
        fpu_reset(&mut o);
        o.call_thiscall(0x0043_eb60, cbase, &[1, 0]).unwrap();
        let half = f64::from(price >> 1);
        let mut want = before.clone();
        want.acct[5] += f64::from(-price);
        want.acct[0] += f64::from(price); // first block moves the whole value to cash
        want.cats[3] += -half;
        want.year[3] += -half;
        want.acct[0] += -half;
        want.acct[7] -= -half;
        assert_eq!(read_ledger(&o, cbase), want, "sale {case}");
        assert_eq!(
            want.acct[0] - before.acct[0],
            f64::from(price - (price >> 1)),
            "net refund {case}"
        );
        assert_eq!(o.read_mem(obj + 0x97, 2).unwrap(), [0, 0], "owner cleared");
    }
}

// ---- 6. tile conflict with train priorities (0x416050) ----------------------------------------

const PIECE_SIZE: u32 = 0x24;
const TRAIN_SIZE: u32 = 0x200;

/// Replaces `0x4d3c80` (train stop/reserve, `ret 0xc`) with a recorder: for every call it stores
/// ECX and the three stack arguments in four arrays and counts the calls.
fn install_reserve_recorder(o: &mut Oracle) -> u32 {
    let buf = o.alloc(0x100).unwrap();
    let (cnt, ecx, a1, a2, a3) = (buf, buf + 0x10, buf + 0x20, buf + 0x30, buf + 0x40);
    let mut c = vec![0xA1];
    c.extend(cnt.to_le_bytes());
    for (opcode_reg, arr, esp_off) in [
        (0x0Cu8, ecx, None),
        (0x14, a1, Some(4u8)),
        (0x14, a2, Some(8)),
        (0x14, a3, Some(12)),
    ] {
        if let Some(off) = esp_off {
            c.extend([0x8B, 0x54, 0x24, off]);
        }
        c.extend([0x89, opcode_reg, 0x85]);
        c.extend(arr.to_le_bytes());
    }
    c.push(0x40);
    c.push(0xA3);
    c.extend(cnt.to_le_bytes());
    c.extend([0xC2, 0x0C, 0x00]);
    o.write_mem(0x004d_3c80, &c).unwrap();
    buf
}

#[derive(Clone, Copy, Default, Debug)]
struct PieceRef {
    mask: u8,
    double: bool,
    owner: [u16; 2],
    entry: [u8; 2],
    exit: [u8; 2],
}

/// Spec 10.4: `tile_conflict(piece, mover, a, b)`; `prio[id]` is `train +0xa8`. Returns whether
/// the move is blocked and the owners that were forced to stop (in order).
fn conflict_ref(p: &PieceRef, mover: u16, a: u32, b: u32, prio: &[u8]) -> (bool, Vec<u16>) {
    let mp = i32::from(prio[usize::from(mover)]) - 1;
    let mut stopped = Vec::new();
    for slot in 0..2usize {
        if slot == 1 && !p.double {
            continue;
        }
        let h = p.owner[slot];
        if h == 0 || h == mover {
            continue;
        }
        let resolve = |stopped: &mut Vec<u16>| -> bool {
            if mp < 1 || i32::from(prio[usize::from(h)]) > mp {
                return true;
            }
            stopped.push(h);
            false
        };
        if p.double {
            let d = u32::from(p.exit[slot]).wrapping_sub(b) & 7;
            if (2..7).contains(&d) {
                if ((1u32 << a | 1u32 << b) & u32::from(p.mask)) == u32::from(p.mask) {
                    continue;
                }
                let ia = a * 2;
                let ib = b * 2 + 1;
                let hi = u32::from(p.exit[slot]) * 2 + 1;
                let lo = u32::from(p.entry[slot]) * 2;
                let (ina, inb) = if lo < hi {
                    (lo < ia && ia < hi, lo < ib && ib < hi)
                } else {
                    (hi < ia && ia < lo, hi < ib && ib < lo)
                };
                if ina != inb && resolve(&mut stopped) {
                    return (true, stopped);
                }
                continue;
            }
        }
        if resolve(&mut stopped) {
            return (true, stopped);
        }
    }
    (false, stopped)
}

#[test]
fn tile_conflict_priority_matches_original() {
    let Some(mut o) = oracle() else { return };
    let pieces = make_list(&mut o, LIST_PIECES, PIECE_SIZE, 4);
    let trains = make_list(&mut o, LIST_TRAINS, TRAIN_SIZE, 8);
    let rec = install_reserve_recorder(&mut o);
    let tile = o.alloc(0x20).unwrap();
    w16(&mut o, tile + 7, 1);
    let mut rng = Xs(0x5160_0c0d);
    let mut blocked_n = 0;
    let mut stopped_n = 0;
    for case in 0..20000 {
        let p = PieceRef {
            mask: rng.below(256) as u8,
            double: rng.chance(50),
            owner: [
                if rng.chance(70) {
                    1 + rng.below(6) as u16
                } else {
                    0
                },
                if rng.chance(60) {
                    1 + rng.below(6) as u16
                } else {
                    0
                },
            ],
            entry: [rng.below(8) as u8, rng.below(8) as u8],
            exit: [rng.below(8) as u8, rng.below(8) as u8],
        };
        let pc = pieces + PIECE_SIZE;
        w8(&mut o, pc, p.mask);
        w16(&mut o, pc + 7, p.owner[0]);
        w16(&mut o, pc + 9, p.owner[1]);
        for s in 0..2 {
            w8(&mut o, pc + 0xb + s, p.entry[s as usize]);
            w8(&mut o, pc + 0xd + s, p.exit[s as usize]);
        }
        w8(&mut o, pc + 0x23, u8::from(p.double));
        let mut prio = [0u8; 8];
        for (id, v) in prio.iter_mut().enumerate().skip(1) {
            *v = match rng.below(5) {
                0 => 0,
                1 => 1,
                2 => 2 + rng.below(2) as u8,
                3 => 4 + rng.below(4) as u8,
                _ => 20,
            };
            w8(&mut o, trains + id as u32 * TRAIN_SIZE + 0xa8, *v);
        }
        let mover = 1 + rng.below(6) as u16;
        let (a, b) = (rng.below(8), rng.below(8));
        w32(&mut o, rec, 0);
        fpu_reset(&mut o);
        let r = o
            .call_thiscall(0x0041_6050, tile, &[0, u32::from(mover), a, b])
            .unwrap();
        let (want_blocked, want_stops) = conflict_ref(&p, mover, a, b, &prio);
        assert_eq!(
            r & 0xff,
            u32::from(want_blocked),
            "blocked {case} {p:?} mover {mover} {a}->{b}"
        );
        let n = o.read_u32(rec).unwrap() as usize;
        assert_eq!(n, want_stops.len(), "stops {case}");
        for (k, h) in want_stops.iter().enumerate() {
            let ecx = o.read_u32(rec + 0x10 + 4 * k as u32).unwrap();
            assert_eq!(ecx, trains + u32::from(*h) * TRAIN_SIZE, "who {case}");
            for off in [0x20u32, 0x30, 0x40] {
                assert_eq!(
                    o.read_u32(rec + off + 4 * k as u32).unwrap(),
                    1,
                    "args {case}"
                );
            }
        }
        blocked_n += usize::from(want_blocked);
        stopped_n += want_stops.len();
    }
    assert!(
        blocked_n > 2000 && stopped_n > 500,
        "{blocked_n} {stopped_n}"
    );
}
