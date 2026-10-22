//! Differential tests for track construction and station catchment (docs/spec/terrain-track.md):
//! the per-tile track cost `0x463520` and the building-in-catchment test `0x407030`. Each test
//! builds the inputs in the emulated memory, runs the original function and compares it with a
//! Rust reference written from the spec. Skipped when the executable is absent. The x87 control
//! word is pinned to 0x27f (53 bits).

use rt2_oracle::Oracle;
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

fn ftol(x: f64) -> i32 {
    (x.trunc() as i64) as i32
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
fn rd_i32(o: &Oracle, a: u32) -> i32 {
    o.read_u32(a).unwrap() as i32
}

/// Reads game data straight from the emulated `.data` (never typed in).
fn exe_f32(o: &Oracle, a: u32) -> f32 {
    f32::from_bits(o.read_u32(a).unwrap())
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

// ---- 1. per-tile track cost (0x463520) ---------------------------------------------------------

const COST_FN: u32 = 0x0046_3520; // fastcall(ECX = x, EDX = y, [d_in, d_out, mode, span, &cost, &cost2]) ret 0x18
const TILES: u32 = 0x0057_4c24;
const TILES_W: u32 = 0x0055_e7a4;
const TRACKS: u32 = 0x0057_4c84;
const G_SINGLE: u32 = 0x0055_34b0;
const G_BRIDGE: u32 = 0x0053_ce18;
const G_ELECTRIC: u32 = 0x0056_6fcc;
const G_COMPANY: u32 = 0x0056_6a50;
const G_ELEC_BASE: u32 = 0x0056_6a60;
const G_EDGE_BASE: u32 = 0x0056_6a64;
const G_ECON: u32 = 0x0057_0717;
const BRIDGE_TABLE: u32 = 0x0053_ce60; // i32[2 * type + double?0:1 ... see spec]
const TERRAIN_CLASS: u32 = 0x0058_1090; // i32[1024], filled at world load
const CLASS_TABLE: u32 = 0x0054_0aa0; // 0xc0-byte records, f32 cost multiplier at +0
const ECON_TRACK: u32 = 0x0055_04a8; // f32[5]
const EFFECT: u32 = 0x0057_07c6; // i16 percent per effect id
const MAP_W: u32 = 8;
const PIECE_SIZE: u32 = 0x24;

#[derive(Clone, Copy, Default)]
struct Piece {
    mask: u8,
    owner: u8,
    btype: u8,
    flags: u8, // +0x23: bit0 double, bit4 electrified
}

#[derive(Clone, Copy)]
struct Tile {
    terrain_id: u16,
    flags2: u8,
    b3: u8,
    track_id: u16,
}

struct CostIn {
    tile: Tile,
    piece: Piece,
    single: u32,
    btype: u32,
    electric: u32,
    company: u32,
    base: i32, // 0x566a64
    elec: i32, // 0x566a60
    econ: f32,
    terrain_mult: f32,
    bridge_tbl: [i32; 12],
    bridge_pct: i16,
    d1: i32,
    d2: i32,
    mode: u32,
    span: u32,
}

fn effect_factor(pct: i16) -> f32 {
    let p = if pct < -0x62 { -99 } else { i32::from(pct) };
    let v = (f64::from(p) * 0.01 + 1.0) as f32;
    if f64::from(v) < 0.01 { 0.01 } else { v }
}

fn tree_count(t: &Tile) -> i32 {
    match t.flags2 & 0xf {
        0 => 0,
        1 | 2 | 4 | 5 | 7 | 8 | 9 => i32::from(t.b3 & 0x1f),
        _ => 1,
    }
}

/// Rust reference for `0x463520`; returns (cost, cost2). `None` for cost2 means "not written".
fn cost_ref(i: &CostIn) -> (i32, Option<i32>) {
    let bridge = f64::from(effect_factor(i.bridge_pct));
    let has_piece = i.tile.track_id != 0;
    let piece = if has_piece { i.piece } else { Piece::default() };
    if i.span != 0 {
        let c = if i.tile.b3 & 0x40 != 0 {
            0
        } else {
            ftol(bridge * 120000.0)
        };
        return (c, None);
    }
    let mut single = i.single;
    let mut base = i.base;
    if i.mode == 0 {
        if i.tile.flags2 & 0x80 != 0 {
            single = if has_piece {
                u32::from(piece.flags & 1)
            } else {
                0
            };
        }
    } else {
        if i.single == 0 && i.btype == 1 {
            single = 1;
        }
        if matches!(i.mode, 2 | 4 | 7) {
            let mut c2 = 0;
            if i.tile.flags2 & 0x80 != 0 {
                if u32::from(piece.owner) != i.company {
                    return (0, Some(0));
                }
                let skip = if piece.flags & 1 == 0 && single == 0 {
                    false
                } else {
                    u32::from(piece.btype) == i.btype
                };
                if skip {
                    return (0, Some(0));
                }
                let idx = usize::from(piece.btype) * 2 + usize::from(piece.flags & 1);
                c2 = ftol(bridge * f64::from(i.bridge_tbl[idx]));
            }
            let idx = (single + i.btype * 2) as usize;
            return (ftol(bridge * f64::from(i.bridge_tbl[idx])), Some(c2));
        }
        base = 0;
    }
    let mut acc1: i32;
    let mut acc2: i32 = 0;
    if i.d1 != -1 {
        let bit = 1u32 << i.d1;
        let mask = if has_piece { u32::from(piece.mask) } else { 0 };
        let mut f = 0f32;
        let v = if mask & bit != 0 {
            if has_piece && piece.flags & 1 != 0 || single != 0 {
                0
            } else {
                f = 0.5;
                base
            }
        } else if single != 0 {
            base
        } else {
            ftol(f64::from(base) * 1.5)
        };
        if i.d1 & 1 != 0 {
            acc1 = ftol(f64::from(v) * 1.4);
            acc2 = ftol(f64::from(v) * f64::from(f) * 1.4);
        } else {
            acc1 = v;
            acc2 = ftol(f64::from(v) * f64::from(f));
        }
        if i.electric != 0 && !(has_piece && piece.flags & 0x10 != 0) {
            let e = if single != 0 && !(has_piece && piece.flags & 1 != 0) {
                i.elec
            } else {
                ftol(f64::from(i.elec) * 1.5)
            };
            if mask & bit != 0 {
                let n1 = ftol(f64::from(e) * 1.3 + f64::from(acc1));
                acc2 = ftol(f64::from(e) * 0.3 + f64::from(acc2));
                acc1 = n1;
            } else {
                acc1 += e;
            }
        }
    } else {
        acc1 = 0;
    }
    if i.d2 != -1 {
        let bit = 1u32 << i.d2;
        let mask = if has_piece { u32::from(piece.mask) } else { 0 };
        let mut f = 0f32;
        let v = if mask & bit != 0 {
            if has_piece && piece.flags & 1 != 0 || single != 0 {
                0
            } else {
                f = 0.5;
                base
            }
        } else if single != 0 {
            base
        } else {
            ftol(f64::from(base) * 1.5)
        };
        if i.d2 & 1 != 0 {
            acc1 = ftol(f64::from(v) * 1.4 + f64::from(acc1));
            acc2 = ftol(f64::from(v) * f64::from(f) * 1.4 + f64::from(acc2));
        } else {
            acc1 += v;
            acc2 = ftol(f64::from(v) * f64::from(f) + f64::from(acc2));
        }
    }
    if i.tile.flags2 & 0x20 == 0 {
        acc1 += 1000 * tree_count(&i.tile);
    }
    let t = ftol(f64::from(acc1) * f64::from(i.terrain_mult));
    (ftol(f64::from(t) * f64::from(i.econ)), Some(acc2))
}

#[test]
fn track_tile_cost_matches_original() {
    let Some(mut o) = oracle() else { return };
    o.set_instruction_limit(2_000_000);
    let w = MAP_W;
    let tiles = o.alloc(w * w * 15).unwrap();
    w32(&mut o, TILES, tiles);
    w32(&mut o, TILES_W, w);
    let pieces = make_list(&mut o, TRACKS, PIECE_SIZE, 4);
    // game data read from the executable
    let mut bridge_tbl = [0i32; 12];
    for (k, b) in bridge_tbl.iter_mut().enumerate() {
        *b = rd_i32(&o, BRIDGE_TABLE + 4 * k as u32);
    }
    let class_mult: Vec<f32> = (0..19)
        .map(|c| exe_f32(&o, CLASS_TABLE + c * 0xc0))
        .collect();
    let econ_tbl: Vec<f32> = (0..5).map(|k| exe_f32(&o, ECON_TRACK + 4 * k)).collect();
    w32(&mut o, G_COMPANY, 0);
    let mut rng = Xs(0x1357_9bdf);
    let mut ok = 0;
    for _ in 0..30000 {
        let (x, y) = (rng.below(w), rng.below(w));
        let has_piece = rng.chance(60);
        // flag 0x80 (bridge) without a track piece would dereference record 0 in the original
        let tile = Tile {
            terrain_id: rng.below(8) as u16,
            flags2: (rng.below(16) as u8)
                | if has_piece && rng.chance(40) { 0x80 } else { 0 }
                | if rng.chance(40) { 0x20 } else { 0 },
            b3: (rng.below(29) as u8) | if rng.chance(15) { 0x40 } else { 0 },
            track_id: u16::from(has_piece),
        };
        let piece = Piece {
            mask: rng.below(256) as u8,
            owner: u8::from(rng.chance(15)),
            btype: rng.below(6) as u8,
            flags: (rng.below(2) as u8) | if rng.chance(40) { 0x10 } else { 0 },
        };
        let class = rng.below(19);
        let pct = (rng.below(120) as i16) - 60;
        let inp = CostIn {
            tile,
            piece,
            single: rng.below(2),
            btype: 1 + rng.below(4),
            electric: rng.below(2),
            company: 0,
            base: 1000 * (1 + rng.below(8)) as i32,
            elec: 1000 * (1 + rng.below(8)) as i32,
            econ: econ_tbl[rng.below(5) as usize],
            terrain_mult: class_mult[class as usize],
            bridge_tbl,
            bridge_pct: pct,
            d1: if rng.chance(10) {
                -1
            } else {
                rng.below(8) as i32
            },
            d2: if rng.chance(10) {
                -1
            } else {
                rng.below(8) as i32
            },
            mode: if rng.chance(40) { 0 } else { rng.below(8) },
            span: if rng.chance(10) { 1 + rng.below(3) } else { 0 },
        };
        let ta = tiles + (y * w + x) * 15;
        o.write_mem(ta, &[0u8; 15]).unwrap();
        w16(&mut o, ta, inp.tile.terrain_id << 6);
        w8(&mut o, ta + 2, inp.tile.flags2);
        w8(&mut o, ta + 3, inp.tile.b3);
        w16(&mut o, ta + 7, inp.tile.track_id);
        w32(
            &mut o,
            TERRAIN_CLASS + 4 * u32::from(inp.tile.terrain_id),
            class,
        );
        let pa = pieces + PIECE_SIZE;
        o.write_mem(pa, &[0u8; PIECE_SIZE as usize]).unwrap();
        w8(&mut o, pa, inp.piece.mask);
        w8(&mut o, pa + 2, inp.piece.owner);
        w8(&mut o, pa + 0x11, inp.piece.btype);
        w8(&mut o, pa + 0x23, inp.piece.flags);
        w32(&mut o, G_SINGLE, inp.single);
        w32(&mut o, G_BRIDGE, inp.btype);
        w32(&mut o, G_ELECTRIC, inp.electric);
        w32(&mut o, G_EDGE_BASE, inp.base as u32);
        w32(&mut o, G_ELEC_BASE, inp.elec as u32);
        w32(&mut o, G_ECON, 0);
        // economy state index is random: write the factor through the index
        let econ_idx = econ_tbl.iter().position(|v| *v == inp.econ).unwrap() as u32;
        w32(&mut o, G_ECON, econ_idx);
        w16(&mut o, EFFECT + 2 * 0xe, inp.bridge_pct as u16);
        let out = o.alloc(8).unwrap();
        w32(&mut o, out, 0x7777_7777);
        w32(&mut o, out + 4, 0x5555_5555);
        fpu_reset(&mut o);
        let r = o.call_fastcall(
            COST_FN,
            x,
            y,
            &[
                inp.d1 as u32,
                inp.d2 as u32,
                inp.mode,
                inp.span,
                out,
                out + 4,
            ],
        );
        if r.is_err() {
            let pc = o.uc().pc_read().unwrap();
            panic!(
                "cost fn {r:?} pc={pc:#x} x={x} y={y} d1={} d2={} mode={} span={} flags2={:#x}",
                inp.d1, inp.d2, inp.mode, inp.span, inp.tile.flags2
            );
        }
        let got = (rd_i32(&o, out), rd_i32(&o, out + 4));
        let (c1, c2) = cost_ref(&inp);
        assert_eq!(got.0, c1, "cost mismatch");
        match c2 {
            Some(v) => assert_eq!(got.1, v, "cost2 mismatch"),
            None => assert_eq!(got.1 as u32, 0x5555_5555, "cost2 must stay untouched"),
        }
        ok += 1;
    }
    eprintln!("track_tile_cost: {ok} cases identical");
}

// ---- 2. catchment (0x407030) -------------------------------------------------------------------

const CATCH_FN: u32 = 0x0040_7030; // fastcall(ECX = building, EDX = station object, [&min_dist]) -> EAX
const STATIONS: u32 = 0x0057_4c60;
const BUILDING_TYPES: u32 = 0x0054_57f8; // 0x154-byte records
const FOOTPRINT: u32 = 0x0054_58c5; // cell (row, col) of type t at t*0x154 - row*20 - col*2
const RADIUS: u32 = 0x0054_fd9c; // u32[3] by station size
const STATION_STRIDE: u32 = 0x4c7;

fn dist(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    let dx = f64::from(x2 as f32) - f64::from(x1 as f32);
    let dy = f64::from(y2 as f32) - f64::from(y1 as f32);
    ftol((dx * dx + dy * dy).sqrt())
}

#[test]
fn station_catchment_matches_original() {
    let Some(mut o) = oracle() else { return };
    o.set_instruction_limit(2_000_000);
    let stations = make_list(&mut o, STATIONS, STATION_STRIDE, 4);
    let radius: Vec<i32> = (0..3).map(|k| rd_i32(&o, RADIUS + 4 * k)).collect();
    // footprint grids from the executable
    let grid = |o: &Oracle, t: u32| -> Vec<u8> {
        // 100 cells, byte 0 of each 2-byte cell, indexed [row * 10 + col]
        let mut g = vec![0u8; 100];
        for row in 0..10u32 {
            for col in 0..10u32 {
                let a = FOOTPRINT + t * 0x154 - row * 20 - col * 2;
                g[(row * 10 + col) as usize] = o.read_mem(a, 1).unwrap()[0];
            }
        }
        g
    };
    let building = o.alloc(0x100).unwrap();
    let station_obj = o.alloc(0x100).unwrap();
    let mut rng = Xs(0x2468_ace1);
    let (mut hit, mut miss) = (0, 0);
    for _ in 0..20000 {
        let size = rng.below(3);
        let btype = 24 + rng.below(40);
        let kind = o.read_mem(BUILDING_TYPES + btype * 0x154, 1).unwrap()[0];
        let (sx, sy) = (10 + rng.below(30) as i32, 10 + rng.below(30) as i32);
        let (ox, oy) = (
            sx + rng.below(21) as i32 - 10 + 5,
            sy + rng.below(21) as i32 - 10 + 5,
        );
        let (cx, cy) = (ox - rng.below(3) as i32, oy - rng.below(3) as i32);
        let srec = stations + STATION_STRIDE;
        o.write_mem(srec, &vec![0u8; STATION_STRIDE as usize])
            .unwrap();
        w8(&mut o, srec + 8, size as u8);
        w32(&mut o, srec + 10, sx as u32);
        w32(&mut o, srec + 0xe, sy as u32);
        o.write_mem(station_obj, &[0u8; 0x100]).unwrap();
        w16(&mut o, station_obj + 8, 1);
        o.write_mem(building, &[0u8; 0x100]).unwrap();
        w8(&mut o, building + 6, btype as u8);
        w8(&mut o, building + 7, kind.max(1));
        w16(&mut o, building + 0x1a, cx as u16);
        w16(&mut o, building + 0x1c, cy as u16);
        w16(&mut o, building + 0x1e, ox as u16);
        w16(&mut o, building + 0x20, oy as u16);
        let out = o.alloc(4).unwrap();
        w32(&mut o, out, 0xdead_beef);
        fpu_reset(&mut o);
        let eax = o
            .call_fastcall(CATCH_FN, building, station_obj, &[out])
            .expect("catchment");
        // reference
        let r = radius[size as usize];
        let want = if dist(sx, sy, cx, cy) <= r + 11 {
            let g = grid(&o, btype);
            let mut best = 9999;
            for row in 0..10i32 {
                for col in 0..10i32 {
                    if g[(row * 10 + col) as usize] & 0x43 != 0 {
                        best = best.min(dist(sx, sy, ox - col, oy - row));
                    }
                }
            }
            (best <= r).then_some(best)
        } else {
            None
        };
        match want {
            Some(d) => {
                assert_eq!(eax, 1);
                assert_eq!(rd_i32(&o, out), d);
                hit += 1;
            }
            None => {
                assert_eq!(eax, 0);
                assert_eq!(o.read_u32(out).unwrap(), 0xdead_beef);
                miss += 1;
            }
        }
    }
    eprintln!("catchment: {hit} linked, {miss} not linked");
    assert!(hit > 100 && miss > 100);
}
