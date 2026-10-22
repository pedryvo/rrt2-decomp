//! Differential tests for the pure pieces of the train step (docs/spec/trains.md section 10):
//! the segment length `0x4ca4e0`, the point on a segment `0x4ca550`, the ring test `0x4f0830` and
//! the CRT `crt_rand_range` `0x4f0790` that draws the smoke puffs. The whole train step is checked
//! against the original by the save oracle (`crates/rt2-sim/tests/worldloop.rs`). Skipped when
//! the executable is absent. The x87 control word is pinned to 0x27f (53 bits).

use rt2_oracle::Oracle;
use rt2_sim::data::DataImage;
use rt2_sim::exe::Exe;
use rt2_sim::rng::CrtRng;
use rt2_sim::trainstep::geom::{segment_length, track_point};
use rt2_sim::trainstep::motion::ring_ahead;
use unicorn_engine::RegisterX86;

fn exe_path() -> String {
    std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    })
}

fn setup() -> Option<(Oracle, DataImage)> {
    let path = exe_path();
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return None;
    }
    let o = Oracle::load_file(&path).expect("load exe");
    let img = DataImage::load(&Exe::from_bytes(std::fs::read(&path).unwrap()).unwrap()).unwrap();
    Some((o, img))
}

fn fpu_reset(o: &mut Oracle) {
    let uc = o.uc();
    uc.reg_write(RegisterX86::FPCW, 0x27f).unwrap();
    uc.reg_write(RegisterX86::FPSW, 0).unwrap();
    uc.reg_write(RegisterX86::FPTAG, 0xffff).unwrap();
}

#[test]
fn segment_length_matches_original() {
    let Some((mut o, img)) = setup() else { return };
    for din in 0..8u32 {
        for dout in 0..8u32 {
            for kind in 0..3u32 {
                let got = o.call_fastcall(0x004c_a4e0, din, dout, &[kind]).unwrap() as i32;
                assert_eq!(
                    segment_length(&img, din, dout, kind),
                    got,
                    "din {din} dout {dout} kind {kind}"
                );
            }
        }
    }
}

#[test]
fn track_point_matches_original() {
    let Some((mut o, img)) = setup() else { return };
    let out = o.alloc(12).unwrap();
    let mut n = 0;
    for din in 0..8u32 {
        for dout in 0..8u32 {
            for kind in 0..3u32 {
                for pos in -130..=230i32 {
                    fpu_reset(&mut o);
                    o.write_mem(out, &[0xAA; 12]).unwrap();
                    o.call_fastcall(
                        0x004c_a550,
                        din,
                        dout,
                        &[pos as u32, kind, out, out + 4, out + 8],
                    )
                    .unwrap();
                    let x = o.read_u32(out).unwrap() as i32;
                    let y = o.read_u32(out + 4).unwrap() as i32;
                    let d = o.read_u32(out + 8).unwrap();
                    assert_eq!(
                        track_point(&img, din, dout, pos, kind),
                        (x, y, d),
                        "din {din} dout {dout} pos {pos} kind {kind}"
                    );
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 64 * 3 * 361);
}

#[test]
fn ring_in_range_matches_original() {
    let Some((mut o, _)) = setup() else { return };
    let mut s = 0x1234_5678u32;
    for _ in 0..20_000 {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        let (a, b, head) = (
            (s % 32) as i32,
            ((s >> 8) % 32) as i32,
            ((s >> 16) % 32) as i32,
        );
        let count = ((s >> 24) % 33) as i32;
        let got = o
            .call_fastcall(
                0x004f_0830,
                a as u32,
                b as u32,
                &[head as u32, count as u32, 0x20],
            )
            .unwrap()
            & 0xff;
        assert_eq!(
            ring_ahead(a, b, head, count),
            got != 0,
            "a {a} b {b} head {head}"
        );
    }
}

#[test]
fn crt_rand_range_matches_original() {
    let Some((mut o, _)) = setup() else { return };
    let ranges = [
        (0, 3),
        (-2000, 2000),
        (0x19, 0x23),
        (0xdac, 5000),
        (0, 100),
        (0, 8),
        (8, 16),
        (5, 5),
        (9, 4),
        (0, 1_000_000),
    ];
    for seed in [1u32, 1234, 0xdead_beef] {
        o.write_u32(0x005_609f0, seed).unwrap();
        let mut crt = CrtRng::new(seed);
        for i in 0..2000 {
            let (lo, hi) = ranges[i % ranges.len()];
            let got = o
                .call_fastcall(0x004f_0790, lo as u32, hi as u32, &[])
                .unwrap() as i32;
            assert_eq!(
                crt.range(lo, hi),
                got,
                "seed {seed} draw {i} range {lo}..{hi}"
            );
            assert_eq!(crt.state(), o.read_u32(0x005_609f0).unwrap());
        }
    }
}
