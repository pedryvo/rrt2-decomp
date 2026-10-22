//! Differential tests of the pure helpers of the construction primitives
//! (`rt2_sim::build`, docs/spec/terrain-track.md 10): the tile light level `0x416bf0`
//! (`tile_shade_from`), which decides the uniform-shade bit `tile+6 & 0x20` that the map
//! repaint writes after every construction. Skipped when the executable is absent.

use rt2_oracle::Oracle;
use unicorn_engine::RegisterX86;

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
}

const G_TILES: u32 = 0x0057_4c24;
const G_ELEV: u32 = 0x0057_4c28;
const G_W: u32 = 0x0055_e7a4;
const G_H: u32 = 0x0055_e7a0;
const G_STRIDE: u32 = 0x0056_09ac;
const G_STRIDE2: u32 = 0x0056_09b0;

#[test]
fn tile_shade_matches_original() {
    let path = exe_path();
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return;
    }
    let mut o = Oracle::load_file(&path).unwrap();
    let (w, h) = (6u32, 6u32);
    let tiles = o.alloc(w * h * 15).unwrap();
    let elev = o.alloc((w + 1) * (h + 1) * 3).unwrap();
    o.write_u32(G_TILES, tiles).unwrap();
    o.write_u32(G_ELEV, elev).unwrap();
    o.write_u32(G_W, w).unwrap();
    o.write_u32(G_H, h).unwrap();
    o.write_u32(G_STRIDE, w + 1).unwrap();
    o.write_u32(G_STRIDE2, w + 2).unwrap();
    let mut rng = Xs(0x5eed_0416);
    let mut n = 0;
    for case in 0..20_000 {
        // Mostly gentle slopes (the common case), sometimes extreme ones.
        let span = if case % 5 == 0 { 765 } else { 40 };
        let base = (rng.next() % 300) as i32 - 100;
        let hs: [i32; 4] = std::array::from_fn(|_| {
            (base + (rng.next() % span) as i32 - (span / 2) as i32).clamp(-255, 510)
        });
        let (x, y) = (2u32, 3u32);
        let s = w + 1;
        let v = |cx: u32, cy: u32| elev + (cy * s + cx) * 3;
        for (k, (cx, cy)) in [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)]
            .into_iter()
            .enumerate()
        {
            o.write_mem(v(cx, cy), &(hs[k] as i16).to_le_bytes())
                .unwrap();
        }
        {
            let uc = o.uc();
            uc.reg_write(RegisterX86::FPCW, 0x27f).unwrap();
            uc.reg_write(RegisterX86::FPSW, 0).unwrap();
            uc.reg_write(RegisterX86::FPTAG, 0xffff).unwrap();
        }
        o.call_fastcall(0x0041_6bf0, x, y, &[]).unwrap();
        let got = o.read_mem(tiles + (y * w + x) * 15 + 0xe, 1).unwrap()[0];
        let want = rt2_sim::build::tiles::tile_shade_from(hs) as u8;
        assert_eq!(got, want, "corners {hs:?}");
        n += 1;
    }
    eprintln!("{n} tile shades match");
}
