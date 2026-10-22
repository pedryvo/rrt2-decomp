//! Differential tests for the simulation RNGs and the calendar math
//! (docs/spec/rng.md, docs/spec/time.md). Skipped when the executable is absent.

use rt2_oracle::Oracle;
use rt2_sim::calendar::{self, SpeedTable};
use rt2_sim::rng::{GameRng, hash_range};
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
}

// ---- game RNG (state at 0x55d2c0) ------------------------------------------------------------

const RNG_STATE: u32 = 0x0055_d2c0;
const RNG_RANGE: u32 = 0x004f_0660; // fastcall(ECX = lo, EDX = hi)
const RNG_SEED: u32 = 0x004f_06d0; // fastcall(ECX = seed)
const RNG_GET: u32 = 0x004f_06e0;
const RNG_HASH: u32 = 0x004f_06f0; // fastcall(ECX = seed, EDX = lo, [hi]) ret 4

#[test]
fn rng_step_range_matches_original() {
    let Some(mut o) = oracle() else { return };
    let mut xs = Xs(0x1234_5678);
    // implementation under test: rt2_sim::rng::GameRng
    let mut rng = GameRng::new(0);
    let mut state = rng.state();
    o.write_u32(RNG_STATE, state).unwrap();
    let mut checked = 0;
    for i in 0..30_000 {
        if i % 997 == 0 {
            // reseed through the original setter
            state = xs.next();
            rng.seed(state);
            o.call_fastcall(RNG_SEED, state, 0, &[]).unwrap();
            assert_eq!(o.read_u32(RNG_STATE).unwrap(), state);
            assert_eq!(o.call_fastcall(RNG_GET, 0, 0, &[]).unwrap(), state);
        }
        let (lo, hi) = match xs.next() % 6 {
            0 => (0, 99),
            1 => (1, 1 + (xs.next() % 40_000) as i32),
            2 => (-((xs.next() % 300) as i32), (xs.next() % 300) as i32),
            3 => (0, (xs.next() % 0x7fff) as i32),
            4 => (0x7000, 0x7000), // lo == hi: no state change
            _ => (
                (xs.next() % 2000) as i32 - 1000,
                (xs.next() % 200_000) as i32 - 1000,
            ),
        };
        let want = rng.range(lo, hi);
        let got = o
            .call_fastcall(RNG_RANGE, lo as u32, hi as u32, &[])
            .unwrap() as i32;
        assert_eq!(got, want, "rng_range({lo}, {hi}) call {i}");
        assert_eq!(
            o.read_u32(RNG_STATE).unwrap(),
            rng.state(),
            "state after {i}"
        );
        checked += 1;
    }
    assert_eq!(checked, 30_000);
}

#[test]
fn rng_hash_matches_original() {
    let Some(mut o) = oracle() else { return };
    let mut xs = Xs(0xdead_beef);
    for i in 0..20_000 {
        let seed = xs.next();
        let lo = (xs.next() % 2000) as i32 - 1000;
        let span = match xs.next() % 4 {
            0 => 0,
            1 => (xs.next() % 100) as i32,
            2 => (xs.next() % 0x7fff) as i32,
            _ => (xs.next() % 300_000) as i32,
        };
        let hi = lo + span;
        let want = hash_range(seed, lo, hi); // rt2-sim under test
        let got = o
            .call_fastcall(RNG_HASH, seed, lo as u32, &[hi as u32])
            .unwrap() as i32;
        assert_eq!(got, want, "rng_hash({seed:#x}, {lo}, {hi}) case {i}");
    }
}

// ---- CRT rand (state at 0x5609f0), used for cosmetic and session randomness ----------------------

const CRT_STATE: u32 = 0x0056_09f0;
const CRT_SRAND: u32 = 0x0051_c12d; // cdecl(seed)
const CRT_RAND: u32 = 0x0051_c137;
const CRT_RANGE: u32 = 0x004f_0790; // fastcall(ECX = lo, EDX = hi)

fn crt_rand(state: &mut u32) -> i32 {
    *state = state.wrapping_mul(214_013).wrapping_add(2_531_011);
    ((*state >> 16) & 0x7fff) as i32
}

fn crt_range(state: &mut u32, lo: i32, hi: i32) -> i32 {
    if hi == lo {
        return hi;
    }
    if hi < lo {
        return lo;
    }
    let span = hi - lo;
    if span > 0x7fff {
        let a = crt_rand(state) as u32;
        let b = crt_rand(state) as u32;
        return (((b & 0x7fff) | ((a & 0x7fff) << 15)) % (span as u32 + 1)).wrapping_add(lo as u32)
            as i32;
    }
    crt_rand(state) % (span + 1) + lo
}

#[test]
fn crt_rand_matches_original() {
    let Some(mut o) = oracle() else { return };
    let mut xs = Xs(77);
    let mut state = 0u32;
    for i in 0..20_000 {
        if i % 500 == 0 {
            state = xs.next();
            o.call_cdecl(CRT_SRAND, &[state]).unwrap();
            assert_eq!(o.read_u32(CRT_STATE).unwrap(), state);
        }
        if i % 3 == 0 {
            let want = crt_rand(&mut state);
            assert_eq!(o.call_cdecl(CRT_RAND, &[]).unwrap() as i32, want);
        } else {
            let lo = (xs.next() % 100) as i32 - 50;
            let hi = lo + (xs.next() % 100_000) as i32 - 10;
            let want = crt_range(&mut state, lo, hi);
            let got = o
                .call_fastcall(CRT_RANGE, lo as u32, hi as u32, &[])
                .unwrap() as i32;
            assert_eq!(got, want, "crt_range({lo}, {hi}) case {i}");
        }
        assert_eq!(o.read_u32(CRT_STATE).unwrap(), state, "state after {i}");
    }
}

// ---- calendar: 12 months x 28 days, minute resolution --------------------------------------

const DATE_PACK: u32 = 0x004a_4950; // fastcall(ECX = year, EDX = month, [day, hour, minute]) ret 12
const DATE_PACK_PACKED: u32 = 0x004a_4990; // stdcall(date_u32, time_u32)
const DATE_UNPACK: u32 = 0x004a_4a30; // thiscall(ECX = minutes, [out*]) writes year u16, month, day, hour, minute

#[test]
fn date_pack_and_unpack_match_original() {
    let Some(mut o) = oracle() else { return };
    let out = o.alloc(16).unwrap();
    let mut xs = Xs(4242);
    for i in 0..20_000 {
        let y = 1804 + (xs.next() % 400) as i32;
        let mo = 1 + (xs.next() % 12) as i32;
        let d = 1 + (xs.next() % 28) as i32;
        let h = (xs.next() % 24) as i32;
        let mi = (xs.next() % 60) as i32;
        let t = calendar::date_to_minutes(y, mo, d, h, mi); // rt2-sim under test

        let got = o
            .call_fastcall(
                DATE_PACK,
                y as u32,
                mo as u32,
                &[d as u32, h as u32, mi as u32],
            )
            .unwrap() as i32;
        assert_eq!(got, t, "pack {y}-{mo}-{d} {h}:{mi} case {i}");

        let date = (y as u32) | ((mo as u32) << 16) | ((d as u32) << 24);
        let time = (h as u32) | ((mi as u32) << 8);
        let got = o.call_stdcall(DATE_PACK_PACKED, &[date, time]).unwrap() as i32;
        assert_eq!(got, t, "pack_packed case {i}");

        // Unpack the packed value and also a raw minute count (not tied to a valid date).
        for v in [t, (xs.next() % 1_500_000_000) as i32] {
            o.call_thiscall(DATE_UNPACK, v as u32, &[out]).unwrap();
            let b = o.read_mem(out, 6).unwrap();
            let got = (u16::from_le_bytes([b[0], b[1]]), b[2], b[3], b[4], b[5]);
            let want = calendar::minutes_to_date(v);
            let want = (want.year, want.month, want.day, want.hour, want.minute);
            assert_eq!(got, want, "unpack {v} case {i}");
        }
        o.call_thiscall(DATE_UNPACK, t as u32, &[out]).unwrap();
        let b = o.read_mem(out, 6).unwrap();
        assert_eq!(
            (
                u16::from_le_bytes([b[0], b[1]]) as i32,
                b[2] as i32,
                b[3] as i32,
                b[4] as i32,
                b[5] as i32
            ),
            (y, mo, d, h, mi),
            "round trip"
        );
    }
}

// ---- game speed -> game minutes per frame --------------------------------------------------------

const SPEED_IDX: u32 = 0x0057_050e;
const SPEED_TABLE: u32 = 0x0053_6508; // f32[11]
const SPEED_SCALE: u32 = 0x0053_7578; // f64 = 3.2
const TICKS_FOR_MS: u32 = 0x0046_c650; // thiscall(ECX = elapsed ms) -> game minutes

fn set_fpcw(o: &mut Oracle, cw: u64) {
    o.uc().reg_write(RegisterX86::FPCW, cw).unwrap();
}

fn read_speed_table(o: &Oracle) -> Vec<f32> {
    o.read_mem(SPEED_TABLE, 44)
        .unwrap()
        .chunks(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

#[test]
fn speed_table_and_ticks_match_original() {
    let Some(mut o) = oracle() else { return };
    let tbl = read_speed_table(&o);
    assert_eq!(
        tbl,
        [0.0, 0.3, 0.6, 1.0, 1.7, 2.1, 3.0, 5.0, 8.0, 12.0, 18.0]
    );
    let scale = f64::from_le_bytes(o.read_mem(SPEED_SCALE, 8).unwrap().try_into().unwrap());
    assert_eq!(scale, 3.2);
    // MSVC CRT start-up leaves the x87 in 53-bit precision (CW 0x27f); that is the assumed
    // run-time state of the original (see docs/spec/time.md, "Em aberto").
    set_fpcw(&mut o, 0x27f);
    // implementation under test: rt2_sim::calendar::SpeedTable
    let sim_table = SpeedTable(tbl.clone().try_into().expect("11 entries"));
    assert_eq!(scale, calendar::SPEED_SCALE);
    for idx in 0..=10u32 {
        o.write_u32(SPEED_IDX, idx).unwrap();
        for ms in 0..=1500u32 {
            let want = sim_table.minutes_for_ms(idx as u8, ms);
            let got = o.call_thiscall(TICKS_FOR_MS, ms, &[]).unwrap() as i32;
            assert_eq!(got, want, "speed {idx}, {ms} ms");
        }
    }
}

/// With the x87 in 24-bit precision (CW 0x7f) a few results move by one minute. The simulation
/// is not bit-exact across precision settings, so the clone must pin one (docs/spec/time.md).
#[test]
fn speed_ticks_depend_on_x87_precision() {
    let Some(mut o) = oracle() else { return };
    let tbl = read_speed_table(&o);
    let mut diff = 0;
    for idx in 0..=10u32 {
        o.write_u32(SPEED_IDX, idx).unwrap();
        for ms in 0..=1500u32 {
            set_fpcw(&mut o, 0x27f);
            let a = o.call_thiscall(TICKS_FOR_MS, ms, &[]).unwrap();
            set_fpcw(&mut o, 0x7f);
            let b = o.call_thiscall(TICKS_FOR_MS, ms, &[]).unwrap();
            // single-precision reference: both products rounded to f32
            let t = tbl[idx as usize] * ms as f32;
            let want = ((f64::from(t) * 3.2) as f32) as i32;
            assert_eq!(b as i32, want, "24-bit: speed {idx}, {ms} ms");
            if a != b {
                diff += 1;
            }
        }
    }
    eprintln!("53-bit vs 24-bit differ in {diff} of {} cases", 11 * 1501);
    assert!(diff > 0);
}
