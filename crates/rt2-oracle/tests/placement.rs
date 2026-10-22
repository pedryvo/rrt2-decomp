//! Differential test of `building_can_place` (`0x4087a0`, docs/spec/world-growth.md 4.6) against
//! `rt2_sim::tilemark::can_place`: a random map in emulated memory (terrain ids, rivers, bit 3,
//! roads, bridges, tracks with overlay lists, buildings in the structure chains) and every town
//! building shape tried at random anchors, both values of the early flag. Skipped when the
//! executable is absent.

use rt2_formats::terrain;
use rt2_oracle::Oracle;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::tilemark::{PlaceMap, TileFacts, can_place};

fn exe_path() -> String {
    std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    })
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
}

const G_TILES: u32 = 0x0057_4c24;
const G_W: u32 = 0x0055_e7a4;
const G_H: u32 = 0x0055_e7a0;
const G_STRUCTS: u32 = 0x0057_4c54;
const G_BUILDINGS: u32 = 0x0057_4c74;
const CLASS_TABLE: u32 = 0x0058_1090;

struct TestMap {
    w: i32,
    h: i32,
    tiles: Vec<TileFacts>,
}
impl PlaceMap for TestMap {
    fn size(&self) -> (i32, i32) {
        (self.w, self.h)
    }
    fn tile(&self, x: i32, y: i32) -> TileFacts {
        self.tiles[(y * self.w + x) as usize]
    }
    fn class_of(&self, id: u16) -> i32 {
        i32::from(terrain::tables().class_of_id(id))
    }
}

#[test]
fn can_place_matches_original() {
    let path = exe_path();
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return;
    }
    let exe = Exe::from_bytes(std::fs::read(&path).unwrap()).unwrap();
    let data = GameData::load(&exe).unwrap();
    let mut o = Oracle::load_file(&path).unwrap();
    let (w, h) = (40i32, 40i32);
    let mut rng = Xs(0x1234_5678);

    // Terrain class table (`0x581090`, built at start-up by the original).
    let mut cls = Vec::new();
    for id in 0..0x400u32 {
        cls.extend_from_slice(&u32::from(terrain::tables().class_of_id(id as u16)).to_le_bytes());
    }
    o.write_mem(CLASS_TABLE, &cls).unwrap();

    // Pools as `0x4f18b0` sees them: header +8 stride, +0x28 base.
    let mk_list = |o: &mut Oracle, global: u32, stride: u32, n: u32| -> u32 {
        let base = o.alloc(stride * (n + 1)).unwrap();
        let hdr = o.alloc(0x40).unwrap();
        o.write_u32(hdr + 8, stride).unwrap();
        o.write_u32(hdr + 0x28, base).unwrap();
        o.write_u32(global, hdr).unwrap();
        base
    };
    let n_structs = 120u32;
    let sbase = mk_list(&mut o, G_STRUCTS, 7, n_structs);
    let _bbase = mk_list(&mut o, G_BUILDINGS, 0x9f, 30);

    let tiles_ptr = o.alloc((w * h * 15) as u32).unwrap();
    o.write_u32(G_TILES, tiles_ptr).unwrap();
    o.write_u32(G_W, w as u32).unwrap();
    o.write_u32(G_H, h as u32).unwrap();

    let grounds = [0u8, 0x12, 0x2d, 0x36, 0x24, 0x5b, 0x61, 0x62, 0x1b];
    let mut next_struct = 1u32;
    let mut facts = Vec::new();
    for i in 0..(w * h) as u32 {
        let var = (rng.next() & 0xff) as u8;
        // Rivers and bit 3 are rare so that most tiles are usable.
        let var = if rng.below(10) == 0 { var } else { var & 0xf6 };
        let ground = grounds[rng.below(grounds.len() as u32) as usize];
        let flags2 = (rng.next() & 0xff) as u8 & if rng.below(3) == 0 { 0xff } else { 0x4f };
        let flags2 = flags2 & !0x10;
        let trees = (rng.next() & 0xff) as u8 & if rng.below(3) == 0 { 0xff } else { 0x1f };
        let mut struct_id = 0u16;
        let mut has_building = false;
        if rng.below(12) == 0 && next_struct + 1 < n_structs {
            // A kind-2 record first, then (sometimes) a kind-3 one with a live building id.
            let kind3 = rng.below(2) == 0;
            let id3 = next_struct + 1;
            let a = sbase + next_struct * 7;
            o.write_mem(a, &[2, 0, 0, 0, 0]).unwrap();
            let next_field = if kind3 { id3 as u16 } else { 0 };
            o.write_mem(a + 5, &next_field.to_le_bytes()).unwrap();
            struct_id = next_struct as u16;
            next_struct += 1;
            if kind3 {
                // Chains are ordered by decreasing kind in the real game; here kind 3 follows
                // kind 2, which only changes how far `0x42e8f0` has to walk.
                let b = sbase + id3 * 7;
                let building = 1 + rng.below(25);
                let building = if rng.below(4) == 0 { 0 } else { building };
                o.write_mem(b, &[3]).unwrap();
                o.write_mem(b + 1, &building.to_le_bytes()).unwrap();
                o.write_mem(b + 5, &[0, 0]).unwrap();
                next_struct += 1;
                has_building = building != 0;
            }
        }
        let has_track = rng.below(8) == 0;
        let mut track_overlay = false;
        let mut overlay_ptr = 0u32;
        if rng.below(4) == 0 || has_track {
            // Overlay list of 1-2 nodes with random layers.
            let n = 1 + rng.below(2);
            let nodes = o.alloc(16).unwrap();
            for k in 0..n {
                let layer = rng.below(6) as u16;
                let word = (rng.below(200) as u16) << 6 | layer;
                let at = nodes + 8 * k;
                o.write_mem(at, &word.to_le_bytes()).unwrap();
                o.write_mem(at + 2, &[0, 0]).unwrap();
                let nx = if k + 1 < n { nodes + 8 * (k + 1) } else { 0 };
                o.write_u32(at + 4, nx).unwrap();
                if layer == 4 || layer == 5 {
                    track_overlay = true;
                }
            }
            overlay_ptr = nodes;
        }
        let at = tiles_ptr + i * 15;
        let t0 = u16::from(var) | (u16::from(ground) << 8);
        o.write_mem(at, &t0.to_le_bytes()).unwrap();
        o.write_mem(at + 2, &[flags2, trees]).unwrap();
        o.write_mem(at + 4, &struct_id.to_le_bytes()).unwrap();
        o.write_mem(at + 6, &[0]).unwrap();
        o.write_mem(at + 7, &[0, 0, 0]).unwrap();
        o.write_u32(at + 10, overlay_ptr).unwrap();
        facts.push(TileFacts {
            var,
            ground,
            flags2,
            trees,
            has_building,
            track_overlay,
        });
    }
    let map = TestMap { w, h, tiles: facts };

    // The footprint extents of the shape rows are computed at start-up by `0x471b10` (they are
    // 0 in the image): write them as the running game has them.
    for shape in 0..0x7a_usize {
        let e = rt2_sim::tilemark::shape_extents(&data.image, shape);
        let at = 0x0054_57ef + 12 + 0x154 * shape as u32;
        o.write_mem(at, &e.map(|v| v as u8)).unwrap();
    }
    let mut tested = 0usize;
    let mut accepted = 0usize;
    for shape in 0..0x7a_usize {
        let ty = rt2_sim::tilemark::shape_type(&data.image, shape);
        if ty == 0 {
            continue; // stations: not modelled
        }
        for _ in 0..400 {
            let x = rng.below((w + 4) as u32) as i32 - 2;
            let y = rng.below((h + 4) as u32) as i32 - 2;
            for early in [false, true] {
                let got = o
                    .call_fastcall(
                        0x4087a0,
                        x as u32,
                        y as u32,
                        &[shape as u32, 0, 0, 0, 1, 0, 0, u32::from(early)],
                    )
                    .unwrap();
                let want = can_place(&map, &data.image, x, y, shape, early);
                assert_eq!(
                    got & 0xff != 0,
                    want,
                    "shape {shape} at ({x},{y}) early {early}"
                );
                tested += 1;
                accepted += usize::from(want);
            }
        }
    }
    assert!(tested > 5000, "{tested}");
    assert!(accepted > 50, "too few accepted placements: {accepted}");
    eprintln!("can_place: {tested} cases, {accepted} accepted");
}
