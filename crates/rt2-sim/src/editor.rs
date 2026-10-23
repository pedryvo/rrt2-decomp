//! The map and scenario editor's world rules (docs/PLAN.md phase 5; docs/spec/commands.md 3.1,
//! docs/spec/world-growth.md 2.1 and 4, docs/spec/scenarios.md 2).
//!
//! Every tool is the port of the routine the original's map click handler (`map_tool_click`
//! `0x48eb30`) calls, over the raw [`Save`] like the rest of the simulation:
//!
//! | Tool | Original | Here |
//! |---|---|---|
//! | 1 raise/lower (drag) | `0x426460` -> `0x426480` mode 0 | [`elevation_brush`] with [`Elevation::Raise`] |
//! | 2 smooth / flatten | `0x426430` -> `0x426480` mode 2 / 3 | [`Elevation::Smooth`], [`Elevation::Flatten`] |
//! | 3 lower to level 0 | `0x426410` -> `0x426480` mode 1 | [`Elevation::Lower`] |
//! | 5, 6 add / remove trees | `0x429fc0` | [`veg_brush`] |
//! | 9 paint terrain | `0x42a3e0` + `0x426d30` | [`paint_brush`] |
//! | 0xc mixed paint | `0x48cc00` | [`paint_mixed`] |
//! | 10, 0xb fill (adjacent, all) | `0x490490`, `0x490bd0` | [`fill`] |
//! | 0x12 territory paint | `0x42c320` | [`region_paint`] |
//! | 0x10 reserved cells | `0x428630` | [`reserve_cells`] |
//! | 0xf river | `0x429ea0` | [`lay_river`] |
//! | 0x13 road | `0x429e30` | [`lay_road`] |
//! | 0xd place (city, region, label) | `0x42f630`, `0x42f6f0`, `0x42f850` | [`World::editor_place_add`] ... |
//! | 7 building | command `0x17` | [`World::editor_build`] |
//! | 8 bulldozer | command `0x0a` with owner 0 | [`World::editor_bulldoze`] |
//!
//! None of them posts a ledger entry or checks cash: the editor has no company (`0x5748fc != 0`).
//! The RNG is the game's ([`GameRng`]), drawn in the original's order, so a tool applied to the
//! same map and seed gives the same tiles.
//!
//! What is not drawn here (overlay lists of rivers, roads, ground blends and territory borders)
//! is derived by the renderer from the tile bytes; the ground blends are also baked into the
//! saved terrain ids by [`World::editor_blend`] (`0x42a7d0`), as the original's saved `.MP2`
//! files keep them.

use rt2_formats::map::{Map, Pool, Save, Tile};
use rt2_formats::terrain::{self, WATER_CLASS};

use crate::pool;
use crate::rng::GameRng;
use crate::tilemark;
use crate::track::tile_distance;
use crate::world::World;

/// Brush radius per brush size: raise/lower tools (`0x536544`).
pub const RAISE_RADIUS: [i32; 5] = [1, 2, 5, 11, 20];
/// Tree brushes (`0x536558`).
pub const TREE_RADIUS: [i32; 5] = [0, 2, 5, 11, 20];
/// Paint and mixed-paint brushes (`0x536580`).
pub const PAINT_RADIUS: [i32; 5] = [0, 1, 3, 7, 20];
/// Bulldozer and reserved-cells brushes (`0x53656c`).
pub const DOZER_RADIUS: [i32; 5] = [0, 2, 5, 11, 20];
/// Strength of the elevation tools: smooth (`0x2d`), flatten (`0xf`), lower to level (`0x14`).
pub const SMOOTH_AMOUNT: i32 = 0x2d;
pub const FLATTEN_AMOUNT: i32 = 0xf;
pub const LOWER_AMOUNT: i32 = 0x14;

/// Terrain classes of the solid paint palette (`bppaintt.win`, handler `0x494f40`: slot `s` is
/// class `s` up to 8, then `s + 3`): beach to snow, then mountain and the five extra classes,
/// ocean last.
pub const SOLID_CLASSES: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 12, 13, 14, 15, 16, 17, 18];
/// Vegetation classes of the tree palette (`bpaddtre.win`: buttons `0x83c..0x843` give
/// `1, 2, 4, 5, 6, 7, 8, 9`) written to `0x53a0a4`.
pub const TREE_CLASSES: [u8; 8] = [1, 2, 4, 5, 6, 7, 8, 9];
/// Number of mixed-paint palette entries (the 18 columns of [`MIX_WEIGHTS`]).
pub const MIX_COUNT: usize = 18;

/// Weights (percent, each mix sums to 100) of the classes each mixed-paint entry draws from:
/// `MIX_WEIGHTS[mix][class]`, the table built on the stack of `0x48cc00` (stride 18 per class).
pub const MIX_WEIGHTS: [[u8; 19]; MIX_COUNT] = {
    let mut t = [[0u8; 19]; MIX_COUNT];
    // (mix, class, weight)
    let rows: [(usize, usize, u8); 40] = [
        (0, 1, 70),
        (0, 2, 30),
        (1, 1, 30),
        (1, 2, 70),
        (2, 2, 50),
        (2, 3, 40),
        (2, 4, 10),
        (3, 2, 25),
        (3, 3, 55),
        (3, 4, 20),
        (4, 2, 15),
        (4, 3, 40),
        (4, 4, 45),
        (5, 3, 45),
        (5, 4, 55),
        (6, 3, 35),
        (6, 4, 65),
        (7, 3, 30),
        (7, 4, 60),
        (7, 5, 10),
        (8, 3, 25),
        (8, 4, 50),
        (8, 5, 25),
        (9, 3, 15),
        (9, 4, 40),
        (9, 5, 45),
        (10, 4, 30),
        (10, 5, 55),
        (10, 6, 15),
        (11, 4, 15),
        (11, 5, 60),
        (11, 6, 25),
        (12, 4, 15),
        (12, 5, 45),
        (12, 6, 40),
        (13, 4, 10),
        (13, 5, 35),
        (13, 6, 55),
        (14, 5, 20),
        (14, 6, 60),
    ];
    let mut i = 0;
    while i < rows.len() {
        t[rows[i].0][rows[i].1] = rows[i].2;
        i += 1;
    }
    let rest: [(usize, usize, u8); 6] = [
        (14, 7, 20),
        (15, 5, 10),
        (15, 6, 60),
        (15, 7, 30),
        (16, 6, 35),
        (16, 7, 65),
    ];
    let mut j = 0;
    while j < rest.len() {
        t[rest[j].0][rest[j].1] = rest[j].2;
        j += 1;
    }
    t[17][6] = 15;
    t[17][7] = 85;
    t
};

/// Which elevation tool (`0x426480` mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// Mode 0: add `((r - d) * amount + r / 2) / r` to each corner within the radius.
    Raise,
    /// Mode 1 with level 0 (tool 3): pull the corners down to level 0.
    Lower,
    /// Mode 2 (tool 2): blend each ring of corners towards the mean of its four neighbours.
    Smooth,
    /// Mode 3 (tool 2 with a mouse button held): pull every corner towards the height under the
    /// click.
    Flatten,
}

type Rect = [i32; 4];

fn dims(s: &Save) -> (i32, i32) {
    (s.map.width as i32, s.map.height as i32)
}

fn in_map(s: &Save, x: i32, y: i32) -> bool {
    let (w, h) = dims(s);
    x >= 0 && y >= 0 && x < w && y < h
}

fn tile_ref(s: &Save, x: i32, y: i32) -> &Tile {
    &s.map.tiles[y as usize * s.map.width + x as usize]
}

fn tile_mut(s: &mut Save, x: i32, y: i32) -> &mut Tile {
    let w = s.map.width;
    &mut s.map.tiles[y as usize * w + x as usize]
}

fn terrain_id(t: &Tile) -> u16 {
    (u16::from(t.var) | (u16::from(t.ground) << 8)) >> 6
}

/// Terrain class of a tile (`0x581090[id]`), 18 = water.
pub fn class_at(s: &Save, x: i32, y: i32) -> u8 {
    terrain::tables().class_of_id(terrain_id(tile_ref(s, x, y)))
}

fn set_id(t: &mut Tile, id: i32) {
    let v = ((id as u16) << 6) | (u16::from(t.var) & 0x3f);
    t.var = v as u8;
    t.ground = (v >> 8) as u8;
}

fn corner(s: &Save, x: i32, y: i32) -> i32 {
    i32::from(s.map.elevation[(y * (s.map.width as i32 + 1) + x) as usize])
}

/// `0x4260c0` / `0x426000`: a corner height limited to `-255..=510`.
fn set_corner(s: &mut Save, x: i32, y: i32, h: i32) {
    let i = (y * (s.map.width as i32 + 1) + x) as usize;
    s.map.elevation[i] = h.clamp(-0xff, 0x1fe) as i16;
}

/// The redraw the original defers to `0x417c40` after a tool: the flat bit (`0x417880`), the
/// light pass (`0x416bf0`, `0x416d40`) and the terrain flags (`0x424c10`) of the rectangle.
pub fn redraw(s: &mut Save, r: Rect) {
    let [x0, y0, x1, y1] = r;
    tilemark::recompute_flat(s, x0, y0, x1, y1);
    tilemark::recompute_flags(s, x0, y0, x1, y1);
    tilemark::recompute_light(s, x0 - 1, y0 - 1, x1 + 1, y1 + 1);
}

fn clamp_rect(s: &Save, r: Rect) -> Rect {
    let (w, h) = dims(s);
    [r[0].max(0), r[1].max(0), r[2].min(w - 1), r[3].min(h - 1)]
}

// ---- elevation (0x426480) -------------------------------------------------------------------

/// `0x426480(x, y, mode, radius, amount)`: raises, lowers or smooths the corners within `radius`
/// of the corner `(x, y)`. The corner grid has `(w + 1) * (h + 1)` entries. Returns the redraw
/// rectangle.
pub fn elevation_brush(
    s: &mut Save,
    tool: Elevation,
    x: i32,
    y: i32,
    radius: i32,
    amount: i32,
) -> Rect {
    let (w, h) = dims(s);
    let stride = w + 1;
    let r = radius;
    let (mode, level) = match tool {
        Elevation::Raise => (0, 0),
        Elevation::Lower => (1, 0),
        Elevation::Smooth => (2, 0),
        Elevation::Flatten => {
            let under = if x >= 0 && y >= 0 && x <= w && y <= h {
                corner(s, x, y)
            } else {
                0
            };
            (1, under)
        }
    };
    let (x_lo, x_hi) = ((x - r - 1).max(0), (x + 2 + r).min(w + 1));
    let (y_lo, y_hi) = ((y - r - 1).max(0), (y + 2 + r).min(h + 1));
    let passes = if mode == 2 { r } else { 1 };
    for pass in 0..passes {
        let snap = (mode == 2).then(|| s.map.elevation.clone());
        for iy in y_lo..y_hi {
            for ix in x_lo..x_hi {
                let d = tile_distance(ix, iy, x, y);
                if d >= r {
                    continue;
                }
                let old = corner(s, ix, iy);
                let new = match mode {
                    0 => ((r - d) * amount + (r >> 1)) / r + old,
                    1 => {
                        let delta = ((r - d) * amount + (r >> 1)) / r;
                        if delta + level < old {
                            old - delta
                        } else if old < level - delta {
                            old + delta
                        } else {
                            level
                        }
                    }
                    _ => {
                        if pass != d || ix == 0 || iy == 0 || ix >= w - 1 || iy >= h - 1 {
                            continue;
                        }
                        let k = ((r - d) * amount * 6) / (r << 3);
                        let e = |ex: i32, ey: i32| {
                            i32::from(snap.as_ref().expect("smooth")[(ey * stride + ex) as usize])
                        };
                        let avg =
                            (e(ix, iy - 1) + e(ix, iy + 1) + e(ix - 1, iy) + 2 + e(ix + 1, iy))
                                >> 2;
                        let mut n = (old * (0x40 - k) + 0x20 + avg * k) >> 6;
                        if (avg - n).abs() < 3 {
                            n = avg;
                        }
                        n
                    }
                };
                set_corner(s, ix, iy, new);
            }
        }
    }
    clamp_rect(s, [x - r - 1, y - r - 1, x + r + 1, y + r + 1])
}

// ---- vegetation and trees (0x429fc0) --------------------------------------------------------

/// `0x429fc0(x, y, radius, strength, 1)` with the vegetation class `veg` kept in `0x53a0a4`:
/// adds (`strength > 0`) or removes trees and ground cover in the circle. A tile that is water,
/// beach (unless `veg` is 4) or road (when adding) is skipped; below `|strength| >= 20` the
/// per-class chance table decides each tile with one `rng(0, 100)`.
pub fn veg_brush(
    s: &mut Save,
    rng: &mut GameRng,
    x: i32,
    y: i32,
    radius: i32,
    strength: i32,
    veg: u8,
) -> Rect {
    const CHANCE: [i32; 14] = [0, 60, 60, 60, 70, 75, 80, 92, 60, 60, 0, 0, 0, 0];
    const DX4: [i32; 4] = [0, 1, 0, -1];
    const DY4: [i32; 4] = [-1, 0, 1, 0];
    let (w, h) = dims(s);
    let veg = veg & 0xf;
    for ix in x - radius..=x + radius {
        for iy in y - radius..=y + radius {
            if ix < 0 || ix > w - 1 || iy < 0 || iy > h - 1 {
                continue;
            }
            if tile_distance(ix, iy, x, y) > radius {
                continue;
            }
            let class = class_at(s, ix, iy);
            if class == WATER_CLASS
                || (class == 0 && veg != 4)
                || (tile_ref(s, ix, iy).trees & 0x20 != 0 && strength >= 1)
            {
                continue;
            }
            if strength.abs() < 20 && CHANCE[usize::from(veg).min(13)] > rng.range(0, 100) {
                continue;
            }
            let t = tile_mut(s, ix, iy);
            if t.flags2 & 0xf == 0 {
                t.flags2 = (t.flags2 & 0xf0) | veg;
                t.trees &= 0xe0;
            } else if strength > 0 {
                t.flags2 = (t.flags2 & 0xf0) | veg;
            }
            let nib = t.flags2 & 0xf;
            if (nib == 3 || nib == 0xd) && strength < 0 {
                t.flags2 &= 0xf0;
            } else if veg == 3 {
                if strength < 0 {
                    t.flags2 &= 0xf0;
                } else {
                    let v = rng.range(0, 0x1f) as u8;
                    t.trees = (t.trees & 0xe0) | (v & 0x1f);
                }
            } else if veg == 0xd {
                if strength < 0 {
                    t.flags2 &= 0xf0;
                    continue;
                }
                t.flags2 &= 0xf0;
                let road_here = tile_ref(s, ix, iy).trees & 0x20 != 0;
                if ix != 0 && ix != w - 1 && iy != 0 && iy != h - 1 && road_here {
                    let mut pick;
                    loop {
                        pick = rng.range(0, 5);
                        if pick != 3 {
                            break;
                        }
                    }
                    let mut mask = 0u32;
                    for k in 0..4 {
                        let (nx, ny) = (ix + DX4[k], iy + DY4[k]);
                        if tile_ref(s, nx, ny).trees & 0x20 != 0 {
                            mask |= 1 << k;
                        }
                    }
                    if mask != 0 && mask != 0xf {
                        let start = rng.range(0, 3);
                        for k in 0..4 {
                            if mask & (1 << ((start + k) & 3)) == 0 {
                                let t = tile_mut(s, ix, iy);
                                t.flags2 = (t.flags2 & 0xfd) | 0xd;
                                let v = (((start + k - 2) & 3) + pick * 4) as u8;
                                t.trees = (t.trees & 0xe0) | (v & 0x1f);
                                break;
                            }
                        }
                    }
                }
            } else if matches!(veg, 10..=12) {
                if strength < 0 {
                    t.flags2 &= 0xf0;
                } else {
                    loop {
                        let v = rng.range(0, 8) as u8;
                        let t = tile_mut(s, ix, iy);
                        t.trees = (t.trees & 0xe0) | (v & 0x1f);
                        if !matches!(v & 0x1f, 3..=6) {
                            break;
                        }
                    }
                }
            } else {
                let n = i32::from(t.trees & 0x1f) + strength;
                if n < 9 {
                    if n < 1 {
                        t.flags2 &= 0xf0;
                        continue;
                    }
                    t.trees = (t.trees & 0xe0) | (n as u8 & 0x1f);
                } else {
                    t.trees = (t.trees & 0xe0) | 8;
                }
            }
        }
    }
    clamp_rect(
        s,
        [
            x - radius - 1,
            y - radius - 1,
            x + radius + 1,
            y + radius + 1,
        ],
    )
}

// ---- terrain paint (0x42a3e0, 0x426d30) -----------------------------------------------------

/// `0x42a3e0` for one tile with the class forced: a terrain id outside the class's variant range
/// (`terrain::variant_range`) is drawn again with `rng(lo, hi)`; the turn bits are cleared.
/// Returns whether the id was drawn.
pub fn paint_tile(s: &mut Save, rng: &mut GameRng, x: i32, y: i32, class: usize) -> bool {
    if !in_map(s, x, y) {
        return false;
    }
    let (lo, hi) = terrain::variant_range(class, x, y);
    let t = tile_mut(s, x, y);
    let id = i32::from(terrain_id(t));
    let mut drawn = false;
    if id < lo || hi < id {
        set_id(t, rng.range(lo, hi));
        drawn = true;
    }
    if t.var & 0x30 != 0 {
        t.var &= 0xcf;
    }
    drawn
}

/// `0x42a3e0(x, y, w, h, class, ...)` forced over a rectangle (`y` outer, `x` inner, like the
/// original). Returns the clamped rectangle and whether any id was drawn.
pub fn paint_rect(
    s: &mut Save,
    rng: &mut GameRng,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    class: usize,
) -> (Rect, bool) {
    let r = clamp_rect(s, [x, y, x + w - 1, y + h - 1]);
    let mut any = false;
    if r[0] <= r[2] && r[1] <= r[3] {
        for ty in r[1]..=r[3] {
            for tx in r[0]..=r[2] {
                any |= paint_tile(s, rng, tx, ty, class);
            }
        }
    }
    (r, any)
}

/// `0x426d30(x0, y0, x1, y1)`: every land tile (not beach, not water) that touches water in the
/// rectangle becomes beach and loses its ground cover and trees.
pub fn coast_cleanup(s: &mut Save, rng: &mut GameRng, x0: i32, y0: i32, x1: i32, y1: i32) {
    let r = clamp_rect(s, [x0, y0, x1, y1]);
    if r[0] > r[2] || r[1] > r[3] {
        return;
    }
    let (rw, rh) = (r[2] - r[0] + 1, r[3] - r[1] + 1);
    let mut count = vec![0u8; (rw * rh) as usize];
    for y in r[1]..=r[3] {
        for x in r[0]..=r[2] {
            if class_at(s, x, y) != WATER_CLASS {
                continue;
            }
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx != 0 || dy != 0) && nx >= r[0] && nx <= r[2] && ny >= r[1] && ny <= r[3]
                    {
                        count[((ny - r[1]) * rw + nx - r[0]) as usize] += 1;
                    }
                }
            }
        }
    }
    for y in r[1]..=r[3] {
        for x in r[0]..=r[2] {
            if count[((y - r[1]) * rw + x - r[0]) as usize] == 0 {
                continue;
            }
            let c = class_at(s, x, y);
            if c != WATER_CLASS && c != 0 {
                paint_tile(s, rng, x, y, 0);
                let t = tile_mut(s, x, y);
                t.flags2 &= 0xf0;
                t.trees &= 0xe0;
            }
        }
    }
}

/// Tool 9: `0x42a3e0(x - r, y - r, 2r + 1, 2r + 1, class, ..)` with the coast cleanup. Returns
/// the redraw rectangle and the `(x, y, w, h)` whose ground blends must be baked again (`0x42a7d0`).
pub fn paint_brush(
    s: &mut Save,
    rng: &mut GameRng,
    x: i32,
    y: i32,
    radius: i32,
    class: usize,
) -> (Rect, Rect) {
    let w = 2 * radius + 1;
    let (r, _) = paint_rect(s, rng, x - radius, y - radius, w, w, class);
    coast_cleanup(s, rng, r[0] - 1, r[1] - 1, r[0] + w + 1, r[1] + w + 1);
    let dirty = clamp_rect(s, [r[0] - 1, r[1] - 1, r[2] - 1, r[3] - 1]);
    let blend = [r[0] - 2, r[1] - 2, w + 4, w + 4];
    (dirty, blend)
}

/// Tool 0xc, `0x48cc00(x, y, w, h, mix)`: scatters `w * h / 12` random seeds, each one of the
/// classes of the mix by weight, and paints every tile of classes 1 to 7 with the class of the
/// nearest seed. Returns the rectangle.
pub fn paint_mixed(
    s: &mut Save,
    rng: &mut GameRng,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    mix: usize,
) -> Rect {
    let n = ((w * h) / 12).max(1) as usize;
    let weights = &MIX_WEIGHTS[mix.min(MIX_COUNT - 1)];
    let mut seeds: Vec<(i32, i32, usize)> = Vec::with_capacity(n);
    for _ in 0..n {
        let sx = rng.range(0, w - 1);
        let sy = rng.range(0, h - 1);
        let mut roll = rng.range(0, 99);
        let mut c = 0;
        while c < 19 {
            let wt = i32::from(weights[c]);
            if wt != 0 {
                roll -= wt;
                if roll < 1 {
                    break;
                }
            }
            c += 1;
        }
        seeds.push((sx, sy, c.min(18)));
    }
    for row in 0..h {
        let ty = y + row;
        if ty < 0 || ty >= s.map.height as i32 {
            continue;
        }
        for col in 0..w {
            let tx = x + col;
            if tx < 0 || tx >= s.map.width as i32 {
                continue;
            }
            if !(1..=7).contains(&class_at(s, tx, ty)) {
                continue;
            }
            let (mut best, mut bd) = (0usize, 9999.0f32);
            for (i, &(sx, sy, _)) in seeds.iter().enumerate() {
                let dx = (col - sx) as f32;
                let dy = ((row - sy) * (row - sy)) as f32;
                let d = (dx * dx + dy).sqrt();
                if d < bd {
                    bd = d;
                    best = i;
                }
            }
            paint_tile(s, rng, tx, ty, seeds[best].2);
        }
    }
    clamp_rect(s, [x - 1, y - 1, x + w, y + h])
}

// ---- fill (0x490490, 0x490bd0) --------------------------------------------------------------

/// The comparison of a fill (`DAT_005534c4`): corners at least as high as the start tile, at
/// most as high, or the same terrain class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillMode {
    /// Mode 0: the sum of the four corners is `>=` the start tile's.
    Above,
    /// Mode 1: `<=`.
    Below,
    /// Mode 2: same class as the start tile.
    Match,
}

fn corner_sum(s: &Save, x: i32, y: i32) -> i32 {
    corner(s, x, y) + corner(s, x + 1, y) + corner(s, x, y + 1) + corner(s, x + 1, y + 1)
}

/// `0x490490` (`adjacent`, flood from `(x, y)` over 4-neighbours) and `0x490bd0` (every tile of
/// the map): paints the matching tiles with `class`; a fill with water (18) also sets the four
/// corners of each tile to the start tile's mean height (`level`, or `level` when given).
/// Returns the redraw rectangle and the `(x, y, w, h)` of the ground blends to bake, `None` if
/// nothing matched.
#[allow(clippy::too_many_arguments)]
pub fn fill(
    s: &mut Save,
    rng: &mut GameRng,
    x: i32,
    y: i32,
    class: usize,
    mode: FillMode,
    adjacent: bool,
    level: Option<i32>,
) -> Option<(Rect, Rect)> {
    if !in_map(s, x, y) {
        return None;
    }
    let (w, h) = dims(s);
    let start_sum = corner_sum(s, x, y);
    let mean = level.unwrap_or(start_sum >> 2);
    let start_class = i32::from(class_at(s, x, y));
    let pass = |s: &Save, tx: i32, ty: i32| match mode {
        FillMode::Above => corner_sum(s, tx, ty) >= start_sum,
        FillMode::Below => corner_sum(s, tx, ty) <= start_sum,
        FillMode::Match => i32::from(class_at(s, tx, ty)) == start_class,
    };
    let mut marked = vec![false; (w * h) as usize];
    if adjacent {
        let mut seen = vec![false; (w * h) as usize];
        let mut stack = vec![(x, y)];
        marked[(y * w + x) as usize] = true;
        seen[(y * w + x) as usize] = true;
        while let Some((cx, cy)) = stack.pop() {
            for (nx, ny) in [(cx - 1, cy), (cx, cy - 1), (cx + 1, cy), (cx, cy + 1)] {
                if nx < 0 || ny < 0 || nx >= w || ny >= h || seen[(ny * w + nx) as usize] {
                    continue;
                }
                seen[(ny * w + nx) as usize] = true;
                if pass(s, nx, ny) {
                    marked[(ny * w + nx) as usize] = true;
                    stack.push((nx, ny));
                }
            }
        }
    } else {
        for ty in 0..h {
            for tx in 0..w {
                if pass(s, tx, ty) {
                    marked[(ty * w + tx) as usize] = true;
                }
            }
        }
    }
    let mut bbox: Option<Rect> = None;
    for ty in 0..h {
        for tx in 0..w {
            if !marked[(ty * w + tx) as usize] {
                continue;
            }
            let b = bbox.get_or_insert([tx, ty, tx, ty]);
            b[0] = b[0].min(tx);
            b[1] = b[1].min(ty);
            b[2] = b[2].max(tx);
            b[3] = b[3].max(ty);
            if i32::from(class_at(s, tx, ty)) != class as i32 {
                paint_tile(s, rng, tx, ty, class);
            }
            if class == usize::from(WATER_CLASS) {
                for (cx, cy) in [(tx, ty), (tx + 1, ty), (tx + 1, ty + 1), (tx, ty + 1)] {
                    set_corner(s, cx, cy, mean);
                }
            }
        }
    }
    let b = bbox?;
    if adjacent {
        coast_cleanup(s, rng, b[0] - 1, b[1] - 1, b[2] + 1, b[3] + 1);
        Some((
            clamp_rect(s, [b[0] - 3, b[1] - 3, b[2] + 3, b[3] + 3]),
            [b[0] - 2, b[1] - 2, b[2] - b[0] + 5, b[3] - b[1] + 5],
        ))
    } else {
        coast_cleanup(s, rng, 0, 0, w - 1, h - 1);
        Some(([0, 0, w - 1, h - 1], [0, 0, w, h]))
    }
}

// ---- territories, reserved cells, roads, rivers ---------------------------------------------

/// `0x42c320(x, y, w, h, region)`: tile byte 9 of the rectangle becomes `region` (the territory
/// number, 1-based). Returns the clamped rectangle.
pub fn region_paint(s: &mut Save, x: i32, y: i32, w: i32, h: i32, region: u8) -> Rect {
    let r = clamp_rect(s, [x, y, x + w - 1, y + h - 1]);
    for ty in r[1]..=r[3] {
        for tx in r[0]..=r[2] {
            tile_mut(s, tx, ty).region = region;
        }
    }
    r
}

/// `0x428630(x, y, radius)`: toggles bit 3 of tile byte 0 (the "reserved cell" marker the
/// editor draws, `map-gam.md` 7) in the square of side `2r + 1`.
pub fn reserve_cells(s: &mut Save, x: i32, y: i32, radius: i32) {
    for tx in x - radius..=x + radius {
        for ty in y - radius..=y + radius {
            if in_map(s, tx, ty) {
                tile_mut(s, tx, ty).var ^= 8;
            }
        }
    }
}

/// `0x429e30(x, y, on)`: the road bit (tile byte 3, `0x20`) and the terrain flags around.
pub fn lay_road(s: &mut Save, x: i32, y: i32, on: bool) {
    if !in_map(s, x, y) {
        return;
    }
    let t = tile_mut(s, x, y);
    if (t.trees & 0x20 != 0) != on {
        t.trees = (t.trees & 0xdf) | (u8::from(on) << 5);
        tilemark::recompute_flags(s, x, y, x, y);
    }
}

/// `0x429920(x, y)`: which of the four neighbours (N, E, S, W) a river tile can flow to (a river
/// tile or the map edge) and how many.
fn river_links(s: &Save, x: i32, y: i32) -> ([bool; 4], i32) {
    const D: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    let mut f = [false; 4];
    let mut n = 0;
    for (i, (dx, dy)) in D.iter().enumerate() {
        let (nx, ny) = (x + dx, y + dy);
        if !in_map(s, nx, ny) || tile_ref(s, nx, ny).var & 1 != 0 {
            f[i] = true;
            n += 1;
        }
    }
    if n == 1
        && let Some(i) = f.iter().position(|&b| b)
    {
        n = 2;
        f[(i + 2) & 3] = true;
    }
    if n == 0 {
        n = 1;
        f[0] = true;
    }
    (f, n)
}

fn river_dir(t: &Tile) -> usize {
    usize::from((t.var >> 1) & 3)
}

fn river_turn(t: &mut Tile) {
    let d = (river_dir(t) + 1) & 3;
    t.var = (t.var & 0xf9) | ((d as u8) << 1);
}

/// `0x429ce0(x, y, extra)`: turns the flow direction until it points at a possible neighbour;
/// with `extra` one more step first.
pub(crate) fn river_orient(s: &mut Save, x: i32, y: i32, extra: bool) {
    let (f, _) = river_links(s, x, y);
    while !f[river_dir(tile_ref(s, x, y))] {
        river_turn(tile_mut(s, x, y));
    }
    if extra {
        river_turn(tile_mut(s, x, y));
        while !f[river_dir(tile_ref(s, x, y))] {
            river_turn(tile_mut(s, x, y));
        }
    }
}

/// `0x4299e0(x, y)`: propagates the flow direction through the river tiles connected to
/// `(x, y)`; returns the box of the tiles it touched.
fn river_flow(s: &mut Save, x: i32, y: i32) -> Rect {
    const D: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    let (w, h) = dims(s);
    let mut seen = vec![false; (w * h) as usize];
    seen[(y * w + x) as usize] = true;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (x, x, y, y);
    loop {
        let (lo_x, hi_x) = ((min_x - 1).max(0), (max_x + 1).min(w - 1));
        let (lo_y, hi_y) = ((min_y - 1).max(0), (max_y + 1).min(h - 1));
        let mut changed = false;
        for iy in lo_y..=hi_y {
            for ix in lo_x..=hi_x {
                let i = (iy * w + ix) as usize;
                if seen[i] {
                    continue;
                }
                if tile_ref(s, ix, iy).var & 1 == 0 {
                    seen[i] = true;
                    continue;
                }
                for (dir, (dx, dy)) in D.iter().enumerate() {
                    let (nx, ny) = (ix + dx, iy + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    if !seen[(ny * w + nx) as usize] || tile_ref(s, nx, ny).var & 1 == 0 {
                        continue;
                    }
                    let (f, n) = river_links(s, ix, iy);
                    if river_dir(tile_ref(s, nx, ny)) == (dir + 2) & 3 && n != 1 {
                        let mut v = dir;
                        if n == 2 || river_dir(tile_ref(s, ix, iy)) == dir {
                            loop {
                                loop {
                                    v = (v + 1) & 3;
                                    if f[v] {
                                        break;
                                    }
                                }
                                if v != dir {
                                    break;
                                }
                            }
                            let t = tile_mut(s, ix, iy);
                            t.var = (t.var & 0xf9) | ((v as u8) << 1);
                        }
                    } else {
                        let t = tile_mut(s, ix, iy);
                        t.var = (t.var & 0xf9) | (((dir & 3) as u8) << 1);
                    }
                    seen[i] = true;
                    changed = true;
                    min_x = min_x.min(ix);
                    min_y = min_y.min(iy);
                    max_x = max_x.max(ix);
                    max_y = max_y.max(iy);
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
    [min_x, min_y, max_x, max_y]
}

/// `0x429ea0(x, y, on, redraw)`: sets or clears the river bit of a tile and re-orients the river
/// tiles around it. Returns the redraw rectangle.
pub fn lay_river(s: &mut Save, x: i32, y: i32, on: bool) -> Rect {
    if !in_map(s, x, y) {
        return [x, y, x, y];
    }
    if (tile_ref(s, x, y).var & 1 != 0) == on {
        if !on {
            return [x, y, x, y];
        }
        river_orient(s, x, y, true);
    } else {
        let t = tile_mut(s, x, y);
        t.var = (t.var & 0xfe) | u8::from(on);
        for ny in y - 1..=y + 1 {
            for nx in x - 1..=x + 1 {
                if in_map(s, nx, ny) && tile_ref(s, nx, ny).var & 1 != 0 {
                    river_orient(s, nx, ny, false);
                }
            }
        }
    }
    tilemark::recompute_flags(s, x, y, x, y);
    let b = river_flow(s, x, y);
    clamp_rect(s, [b[0] - 1, b[1] - 1, b[2] + 1, b[3] + 1])
}

// ---- places (0x42f630, 0x42f6f0, 0x42f850) --------------------------------------------------

/// Kind of a place (`+0x10`): a city grows houses and businesses, a region only businesses, a
/// label only names an area (LNG 1146..1148).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceKind {
    City = 0,
    Region = 1,
    Label = 2,
}

/// Number of industry weights of a place (`+0x4a`, `u16[53]`).
pub const PLACE_WEIGHTS: usize = 53;
/// Offsets of a place record (world-growth.md 2.1).
pub mod place {
    pub const ID: usize = 0;
    pub const FIRST_BUILDING: usize = 2;
    pub const X: usize = 4;
    pub const Y: usize = 6;
    pub const BOX: usize = 8;
    pub const KIND: usize = 0x10;
    pub const STYLE: usize = 0x11;
    pub const NAME: usize = 0x16;
    pub const NAME_LEN: usize = 0x16;
    pub const SIZE: usize = 0x2c;
    pub const GROWTH: usize = 0x2e;
    pub const INTENSITY: usize = 0x46;
    pub const TREND: usize = 0x49;
    pub const WEIGHTS: usize = 0x4a;
}

/// The record `0x42f6f0` initialises for a place at tile `(x, y)` with slot `id`: whole-map box,
/// kind city, size class 2 (City), growth 100 %, intensity `rng(80, 120)` %, every weight 100,
/// and the default name.
#[allow(clippy::too_many_arguments)]
pub fn new_place_record(
    size: usize,
    id: u16,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rng: &mut GameRng,
    name: &str,
) -> Vec<u8> {
    let mut d = vec![0u8; size.max(0xb4)];
    pool::put_u16(&mut d, place::ID, id);
    pool::put_u16(&mut d, place::X, x as u16);
    pool::put_u16(&mut d, place::Y, y as u16);
    pool::put_u16(&mut d, place::BOX + 4, (w - 1) as u16);
    pool::put_u16(&mut d, place::BOX + 6, (h - 1) as u16);
    d[place::STYLE] = 1;
    d[place::KIND] = 0;
    d[place::SIZE] = 2;
    pool::put_u32(&mut d, place::GROWTH, 100);
    let intensity = rng.range(0x50, 0x78);
    pool::put_u16(&mut d, place::INTENSITY, intensity as u16);
    for t in 0..PLACE_WEIGHTS {
        pool::put_u16(&mut d, place::WEIGHTS + 2 * t, 100);
    }
    set_place_name(&mut d, name);
    d
}

/// Writes the NUL-terminated name of a place record (`+0x16`, 22 bytes up to the size class).
pub fn set_place_name(d: &mut [u8], name: &str) {
    let b = name.as_bytes();
    let n = b.len().min(place::NAME_LEN - 1);
    d[place::NAME..place::NAME + place::NAME_LEN].fill(0);
    d[place::NAME..place::NAME + n].copy_from_slice(&b[..n]);
}

/// The name of a place record.
pub fn place_name(d: &[u8]) -> String {
    let b = &d[place::NAME..place::NAME + place::NAME_LEN];
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// The place whose marker sits on tile `(x, y)` (structure kind 5 of the tile, `0x42f5f0`), 0 if
/// none.
pub fn place_on_tile(s: &Save, x: i32, y: i32) -> u16 {
    if !in_map(s, x, y) {
        return 0;
    }
    let t = tile_ref(s, x, y);
    let mut cur = u16::from(t.struct_id) | (u16::from(t.reserved5) << 8);
    let mut guard = 0;
    while cur != 0 && guard < 64 {
        guard += 1;
        let Some(r) = pool::record(&s.structures, usize::from(cur)) else {
            break;
        };
        let d = &r.data;
        if d[0] == PLACE_MARKER {
            return u16::from_le_bytes([d[1], d[2]]);
        }
        cur = u16::from_le_bytes([d[5], d[6]]);
    }
    0
}

/// Structure kind of the place marker on a tile (`0x42e850(tile, 5, id)`).
pub const PLACE_MARKER: u8 = 5;

// ---- territories ----------------------------------------------------------------------------

/// Offsets of a territory record (0x314 bytes in a `0x2348` file).
pub mod territory {
    pub const ID: usize = 0;
    pub const NAME: usize = 4;
    pub const NAME_LEN: usize = 0x20;
    /// Border shown on the map (`+0x24`).
    pub const SHOWN: usize = 0x24;
    pub const REGION: usize = 0x2a;
}

/// The name of a territory record.
pub fn territory_name(d: &[u8]) -> String {
    let b = &d[territory::NAME..territory::NAME + territory::NAME_LEN];
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// Writes a territory name (`+4`, NUL-terminated, 31 bytes at most).
pub fn set_territory_name(d: &mut [u8], name: &str) {
    let b = name.as_bytes();
    let n = b.len().min(territory::NAME_LEN - 1);
    d[territory::NAME..territory::NAME + territory::NAME_LEN].fill(0);
    d[territory::NAME..territory::NAME + n].copy_from_slice(&b[..n]);
}

// ---- map header (name, description, size, preview) ------------------------------------------

/// Offsets in the header block of the file (map-gam.md 1).
pub mod header {
    pub const NAME: usize = 0x4;
    pub const NAME_LEN: usize = 0x12d;
    pub const WIDTH: usize = 0x231;
    pub const HEIGHT: usize = 0x233;
    pub const DESCRIPTION: usize = 0x25a;
    pub const DESCRIPTION_LEN: usize = 0x12d;
    pub const PREVIEW: usize = 0x387;
    pub const PREVIEW_SIDE: usize = 170;
}

fn read_cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn write_cstr(b: &mut [u8], text: &str) {
    b.fill(0);
    let n = text.len().min(b.len() - 1);
    b[..n].copy_from_slice(&text.as_bytes()[..n]);
}

/// The short name of the map (header `+0x4`).
pub fn map_name(s: &Save) -> String {
    read_cstr(&s.header[header::NAME..header::NAME + header::NAME_LEN])
}

pub fn set_map_name(s: &mut Save, name: &str) {
    write_cstr(
        &mut s.header[header::NAME..header::NAME + header::NAME_LEN],
        name,
    );
}

/// The scenario description (header `+0x25a`).
pub fn map_description(s: &Save) -> String {
    read_cstr(&s.header[header::DESCRIPTION..header::DESCRIPTION + header::DESCRIPTION_LEN])
}

pub fn set_map_description(s: &mut Save, text: &str) {
    write_cstr(
        &mut s.header[header::DESCRIPTION..header::DESCRIPTION + header::DESCRIPTION_LEN],
        text,
    );
}

/// The saver's overview (`0x477040`, the branch for maps): 170 x 170 bytes, 0 empty, 1 coast and
/// territory lines and the map edge, 2 water, 3 land, the map centred and scaled to fit. Water
/// and land fill the even checkerboard cells only.
pub fn preview(s: &Save) -> Vec<u8> {
    const N: i32 = header::PREVIEW_SIDE as i32;
    let (w, h) = dims(s);
    let mut out = vec![0u8; (N * N) as usize];
    let m = w.max(h);
    let scale = 0x00aa_0000 / m;
    let off_y = (((m - h) >> 1) * scale) >> 16;
    let off_x = (((m - w) >> 1) * scale) >> 16;
    for row in 0..N {
        let ty = (row * 0x10000 - off_y * 0x10000) / scale;
        if !(0..h).contains(&ty) {
            continue;
        }
        for col in 0..N {
            if (row + col) & 1 != 0 {
                continue;
            }
            let tx = (col * 0x10000 - off_x * 0x10000) / scale;
            if (0..w).contains(&tx) {
                out[(row * N + col) as usize] = if class_at(s, tx, ty) == WATER_CLASS {
                    2
                } else {
                    3
                };
            }
        }
    }
    // Lines: shores (a water tile with a ground blend) and territory borders.
    let grid = {
        let mut g = crate::editor::terrain_grid(&s.map);
        let region: Vec<u8> = s.map.tiles.iter().map(|t| t.region).collect();
        let terr = &s.territories;
        let shown = |r: u8| {
            terr.get(usize::from(r))
                .is_some_and(|t| t.data.get(territory::SHOWN).is_some_and(|&b| b != 0))
        };
        g.build_territory_borders(&region, shown);
        g
    };
    for ty in 0..h {
        for tx in 0..w {
            let c = grid.cell(tx as usize, ty as usize);
            let mut mark = false;
            for n in &c.nodes {
                match n.layer() {
                    0 if class_at(s, tx, ty) == WATER_CLASS => mark = true,
                    1 => mark = true,
                    _ => {}
                }
            }
            if tx == 0 || ty == 0 || tx == w - 1 || ty == h - 1 {
                mark = true;
            }
            if mark {
                let py = off_y + ((ty * scale) >> 16);
                let px = off_x + ((tx * scale) >> 16);
                if (0..N).contains(&py) && (0..N).contains(&px) {
                    out[(py * N + px) as usize] = 1;
                }
            }
        }
    }
    out
}

/// The terrain passes of the loader over a map (ids, rivers, roads, ground blends and shores).
pub fn terrain_grid(map: &Map) -> terrain::TerrainGrid {
    let tile = |x: usize, y: usize| {
        let t = &map.tiles[y * map.width + x];
        terrain::TileBytes {
            b0: t.var,
            b1: t.ground,
            b2: t.flags2,
            b3: t.trees,
            b6: t.flags6,
        }
    };
    let mut rng = terrain::GameRng(0x5254_3200);
    let mut g = if map.version < 0x233d {
        terrain::TerrainGrid::from_old_ids(map.width, map.height, tile, &mut rng)
    } else {
        terrain::TerrainGrid::from_new_ids(map.width, map.height, tile)
    };
    g.build_overlays(&mut rng);
    g
}

/// Writes the overview into the header of the save (the saver does this at every save).
pub fn refresh_preview(s: &mut Save) {
    let p = preview(s);
    let o = header::PREVIEW;
    s.header[o..o + p.len()].copy_from_slice(&p);
}

// ---- a new map ------------------------------------------------------------------------------

/// Elevation a new land map starts at.
pub const NEW_LAND_LEVEL: i16 = 60;

/// A blank map `width` x `height` of terrain class `class` made from a shipped scenario with no
/// companies (`template`): its option blocks, players and the single territory are kept as the
/// defaults; the tiles, elevation, places, buildings, stations, trains, track and rules are
/// emptied. The original's File > New Map does the same from its initial globals; here the
/// defaults come from the template because they are not decoded (docs/formats/map-gam.md 1).
pub fn new_map(template: &Save, width: usize, height: usize, class: usize, name: &str) -> Save {
    let mut s = template.clone();
    s.map.width = width;
    s.map.height = height;
    let rng_class = class.min(usize::from(WATER_CLASS));
    s.map.tiles = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (lo, _) = terrain::variant_range(rng_class, x as i32, y as i32);
            let v = (lo as u16) << 6;
            Tile {
                var: v as u8,
                ground: (v >> 8) as u8,
                flags2: 0,
                trees: 0,
                struct_id: 0,
                reserved5: 0,
                flags6: 1,
                track_id: 0,
                reserved8: 0,
                region: 1,
            }
        })
        .collect();
    let level = if class == usize::from(WATER_CLASS) {
        0
    } else {
        NEW_LAND_LEVEL
    };
    s.map.elevation = vec![level; (width + 1) * (height + 1)];
    for pool_ref in [
        &mut s.structures,
        &mut s.stations,
        &mut s.pool_70,
        &mut s.objects,
        &mut s.pool_78,
        &mut s.pool_58,
        &mut s.pool_7c,
        &mut s.pool_80,
        &mut s.track_pieces,
        &mut s.trains,
        &mut s.places,
        &mut s.pool_88,
    ] {
        empty_pool(pool_ref);
    }
    s.train_extras.clear();
    s.rules.clear();
    // Industry counter and "first of type seen" flags (state `0x570713`, `0x570603`).
    for o in 0x22B..0x22F {
        s.state[o] = 0;
    }
    for t in 0..53 {
        s.state[0x11B + t] = 0;
    }
    // One territory covering the map; the template's own territories are dropped.
    keep_first_territory(&mut s.territories);
    s.header[header::WIDTH..header::WIDTH + 2].copy_from_slice(&(width as u16).to_le_bytes());
    s.header[header::HEIGHT..header::HEIGHT + 2].copy_from_slice(&(height as u16).to_le_bytes());
    set_map_name(&mut s, name);
    set_map_description(&mut s, "");
    s.header[header::PREVIEW..header::PREVIEW + header::PREVIEW_SIDE * header::PREVIEW_SIDE]
        .fill(0);
    let r = [0, 0, width as i32 - 1, height as i32 - 1];
    redraw(&mut s, r);
    refresh_preview(&mut s);
    s
}

/// A pool with no live record (bitmap all free), keeping its sizes.
pub fn empty_pool(p: &mut Pool) {
    p.records.clear();
    p.order.clear();
    p.count = 0;
    p.head = 0;
    p.tail = 0;
    p.field_04 = 0;
    p.bitmap.fill(0xff);
    for v in p.next.iter_mut().chain(p.prev.iter_mut()) {
        *v = 0;
    }
}

fn keep_first_territory(p: &mut Pool) {
    let keep: Vec<_> = p.ordered().take(1).map(|r| r.slot).collect();
    let mut data = None;
    if let Some(&slot) = keep.first() {
        data = pool::record(p, slot).map(|r| r.data.clone());
    }
    empty_pool(p);
    if let Some(mut d) = data {
        pool::put_u32(&mut d, territory::ID, 1);
        d[territory::REGION] = 1;
        if let Ok(slot) = pool::alloc(p, d) {
            debug_assert_eq!(slot, 1);
        }
    }
}

// ---- the World side -------------------------------------------------------------------------

/// A tool's effect for the front end: what to redraw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Touched {
    /// Terrain or elevation changed: the map mesh must be rebuilt.
    pub terrain: bool,
    /// Buildings, places or other objects changed: the sprites must be rebuilt.
    pub objects: bool,
}

impl World {
    fn ed_parts(&mut self) -> Option<(&mut Save, &mut GameRng)> {
        let sim = self.sim.as_mut()?;
        Some((&mut sim.save, &mut self.rng))
    }

    /// Replaces the edited map (undo, a map loaded into the same world). The world keeps its
    /// tables and clock.
    pub fn editor_restore(&mut self, save: Save) {
        if let Some(sim) = self.sim.as_mut() {
            sim.save = save;
            sim.ground_layer = None;
            sim.track_cache = None;
        }
    }

    /// Elevation tool at corner `(x, y)` (`0x426480`).
    pub fn editor_elevation(&mut self, tool: Elevation, x: i32, y: i32, radius: i32, amount: i32) {
        if let Some((s, _)) = self.ed_parts() {
            let r = elevation_brush(s, tool, x, y, radius, amount);
            redraw(s, r);
            if let Some(sim) = self.sim.as_mut() {
                sim.dirty.add(r[0], r[1], r[2], r[3]);
            }
        }
    }

    /// Add (`strength` 1) or remove (`-1`) trees with the vegetation class `veg` (`0x429fc0`).
    pub fn editor_trees(&mut self, x: i32, y: i32, radius: i32, strength: i32, veg: u8) {
        if let Some((s, rng)) = self.ed_parts() {
            let r = veg_brush(s, rng, x, y, radius, strength, veg);
            redraw(s, r);
        }
    }

    /// Paints a square brush of `class` and bakes the ground blends (tool 9).
    pub fn editor_paint(&mut self, x: i32, y: i32, radius: i32, class: usize) {
        let Some((s, rng)) = self.ed_parts() else {
            return;
        };
        let (dirty, blend) = paint_brush(s, rng, x, y, radius, class);
        redraw(s, dirty);
        self.editor_blend(blend);
    }

    /// Mixed paint of a `size`-square around `(x, y)` (tool 0xc).
    pub fn editor_paint_mixed(&mut self, x: i32, y: i32, radius: i32, mix: usize) {
        let Some((s, rng)) = self.ed_parts() else {
            return;
        };
        let side = 2 * radius + 1;
        let r = paint_mixed(s, rng, x - radius, y - radius, side, side, mix);
        redraw(s, r);
        self.editor_blend([x - radius - 1, y - radius - 1, side + 2, side + 2]);
    }

    /// Fill (tools 10 and 0xb). Returns false when nothing matched.
    pub fn editor_fill(
        &mut self,
        x: i32,
        y: i32,
        class: usize,
        mode: FillMode,
        adjacent: bool,
    ) -> bool {
        let Some((s, rng)) = self.ed_parts() else {
            return false;
        };
        let Some((dirty, blend)) = fill(s, rng, x, y, class, mode, adjacent, None) else {
            return false;
        };
        redraw(s, dirty);
        self.editor_blend(blend);
        true
    }

    /// Territory paint of a square brush with territory slot `territory` (tool 0x12).
    pub fn editor_territory_paint(&mut self, x: i32, y: i32, radius: i32, territory: u8) {
        if let Some((s, _)) = self.ed_parts() {
            let side = 2 * radius + 1;
            region_paint(s, x - radius, y - radius, side, side, territory);
        }
    }

    pub fn editor_reserve(&mut self, x: i32, y: i32, radius: i32) {
        if let Some((s, _)) = self.ed_parts() {
            reserve_cells(s, x, y, radius);
        }
    }

    pub fn editor_road(&mut self, x: i32, y: i32, on: bool) {
        if let Some((s, _)) = self.ed_parts() {
            lay_road(s, x, y, on);
        }
    }

    pub fn editor_river(&mut self, x: i32, y: i32, on: bool) {
        if let Some((s, _)) = self.ed_parts() {
            let r = lay_river(s, x, y, on);
            redraw(s, r);
        }
    }

    /// The edited map, if the world has one.
    pub fn editor_save(&self) -> Option<&Save> {
        self.save_ref()
    }

    /// Mutable access to the edited save for editor panels that write plain fields (names,
    /// options, rules).
    pub fn editor_save_mut(&mut self) -> Option<&mut Save> {
        self.sim.as_mut().map(|s| &mut s.save)
    }
}

// ---- places and territories on the World ----------------------------------------------------

/// Why an editor operation was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorError {
    /// The world has no map (not built from a save).
    NoMap,
    /// The shape does not fit there (`0x4087a0`, LNG 0x216/0x217).
    InvalidPlacement,
    /// A place already sits on the tile (`0x42f5f0`, LNG 0x18b).
    PlaceHere,
    NoSuchPlace,
    /// 31 territories exist (`0x494f40`: the list takes at most 30 more).
    TooManyTerritories,
    NoSuchTerritory,
    /// A construction or bulldozer refusal of the shared primitives.
    Command(crate::build::cmd::CmdError),
}

/// The editable fields of a place (`cityset.win`: General and Industry tabs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceInfo {
    pub id: u16,
    pub x: u16,
    pub y: u16,
    pub kind: u8,
    pub name: String,
    /// Size class 0..4 (`+0x2c`: Village .. Megalopolis).
    pub size: u8,
    /// Architecture byte (`+0x11`, 0 Victorian ... 3 Tudor, 5 Kyoto; the dialog's list is
    /// `0x8163`).
    pub style: u8,
    /// Region trend (`+0x49`, -2..6, `0x8151`).
    pub trend: i8,
    /// Weight of each of the 53 building types (`+0x4a`), the editor allows 0..200.
    pub weights: Vec<u16>,
}

/// Territory Setup (`0x4c6e70`): the fields of a territory record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerritoryInfo {
    pub id: u8,
    pub name: String,
    /// Default goodwill (`+0x4e`, i16 in hundredths).
    pub goodwill: i16,
    /// The border is drawn and the territory counts on its own (`+0x24`).
    pub visible: bool,
    /// `+0x25` (shown when the territory is not visible).
    pub merged_flag: bool,
    /// Territory the map shows this one as (`+0x2a`) when it is not visible.
    pub parent: u8,
    /// `+0x43`: the four rights below differ per territory.
    pub separate_rights: bool,
    /// `+0x26..+0x29` and `+0x44..+0x47`: per right (trains, track, buildings, stations).
    pub rights: [bool; 4],
    pub for_sale: [bool; 4],
}

impl World {
    /// The place records, in list order.
    pub fn editor_places(&self) -> Vec<PlaceInfo> {
        let Some(s) = self.save_ref() else {
            return Vec::new();
        };
        s.places
            .ordered()
            .map(|r| place_info(r.slot, &r.data))
            .collect()
    }

    pub fn editor_place(&self, id: u16) -> Option<PlaceInfo> {
        let r = pool::record(&self.save_ref()?.places, usize::from(id))?;
        Some(place_info(r.slot, &r.data))
    }

    /// Tool 0xd: a new place on the tile (`0x42f630`: the record from `0x42f6f0` and the
    /// marker structure on the tile). Refused when a place already sits there.
    pub fn editor_place_add(
        &mut self,
        x: i32,
        y: i32,
        kind: PlaceKind,
        name: &str,
    ) -> Result<u16, EditorError> {
        let (s, rng) = self.ed_parts().ok_or(EditorError::NoMap)?;
        if !in_map(s, x, y) {
            return Err(EditorError::InvalidPlacement);
        }
        if place_on_tile(s, x, y) != 0 {
            return Err(EditorError::PlaceHere);
        }
        let size = s.places.record_size;
        let slot = pool::alloc(&mut s.places, vec![0u8; size.max(0xb4)])
            .map_err(|_| EditorError::NoSuchPlace)?;
        let (w, h) = dims(s);
        let mut rec = new_place_record(size, slot as u16, x, y, w, h, rng, name);
        rec[place::KIND] = kind as u8;
        if let Some(r) = pool::record_mut(&mut s.places, slot) {
            r.data = rec;
        }
        tilemark::struct_add(s, x, y, PLACE_MARKER, slot as u32);
        Ok(slot as u16)
    }

    /// `0x42f850`: the buildings of the place lose it, the marker leaves its tile and the record
    /// is freed.
    pub fn editor_place_remove(&mut self, id: u16) -> Result<(), EditorError> {
        let (s, _) = self.ed_parts().ok_or(EditorError::NoMap)?;
        let Some(r) = pool::record(&s.places, usize::from(id)) else {
            return Err(EditorError::NoSuchPlace);
        };
        let (px, py) = (
            i32::from(pool::i16_at(&r.data, place::X)),
            i32::from(pool::i16_at(&r.data, place::Y)),
        );
        let mut b = usize::from(pool::u16_at(&r.data, place::FIRST_BUILDING));
        let mut guard = 0;
        while b != 0 && guard <= s.objects.records.len() {
            guard += 1;
            let Some(br) = pool::record_mut(&mut s.objects, b) else {
                break;
            };
            let next = usize::from(pool::u16_at(&br.data, 4));
            pool::put_u16(&mut br.data, 2, 0);
            pool::put_u16(&mut br.data, 4, 0);
            b = next;
        }
        if in_map(s, px, py) {
            tilemark::struct_remove(s, px, py, PLACE_MARKER, u32::from(id));
        }
        pool::free(&mut s.places, usize::from(id));
        Ok(())
    }

    /// Writes the edited fields of a place (the OK of the dialog).
    pub fn editor_place_set(&mut self, p: &PlaceInfo) -> Result<(), EditorError> {
        let (s, _) = self.ed_parts().ok_or(EditorError::NoMap)?;
        let r =
            pool::record_mut(&mut s.places, usize::from(p.id)).ok_or(EditorError::NoSuchPlace)?;
        let d = &mut r.data;
        set_place_name(d, &p.name);
        d[place::KIND] = p.kind.min(2);
        d[place::SIZE] = p.size.min(4);
        d[place::STYLE] = p.style;
        d[place::TREND] = p.trend as u8;
        for (t, &wt) in p.weights.iter().take(PLACE_WEIGHTS).enumerate() {
            pool::put_u16(d, place::WEIGHTS + 2 * t, wt.min(200));
        }
        Ok(())
    }

    /// The territory records, in list order.
    pub fn editor_territories(&self) -> Vec<TerritoryInfo> {
        let Some(s) = self.save_ref() else {
            return Vec::new();
        };
        s.territories
            .ordered()
            .map(|r| territory_info(&r.data))
            .collect()
    }

    /// A new territory (`0x4c8440` + `0x4c8470`): the next free slot is its number.
    pub fn editor_territory_add(&mut self, name: &str) -> Result<u8, EditorError> {
        let (s, _) = self.ed_parts().ok_or(EditorError::NoMap)?;
        if s.territories.count >= 0x1f {
            return Err(EditorError::TooManyTerritories);
        }
        let size = s.territories.record_size.max(0x314);
        let slot = pool::alloc(&mut s.territories, vec![0u8; size])
            .map_err(|_| EditorError::NoSuchTerritory)?;
        let mut d = vec![0u8; size];
        pool::put_u32(&mut d, territory::ID, slot as u32);
        set_territory_name(&mut d, name);
        d[territory::REGION] = slot as u8;
        d[0x23] = 1;
        d[territory::SHOWN] = 1;
        d[0x25] = 1;
        for i in 0..4 {
            d[0x26 + i] = 1;
            d[0x44 + i] = 1;
            pool::put_u16(&mut d, 0x3b + 2 * i, 0x32);
        }
        if let Some(r) = pool::record_mut(&mut s.territories, slot) {
            r.data = d;
        }
        Ok(slot as u8)
    }

    /// Writes the edited fields of a territory.
    pub fn editor_territory_set(&mut self, t: &TerritoryInfo) -> Result<(), EditorError> {
        let (s, _) = self.ed_parts().ok_or(EditorError::NoMap)?;
        let r = pool::record_mut(&mut s.territories, usize::from(t.id))
            .ok_or(EditorError::NoSuchTerritory)?;
        let d = &mut r.data;
        set_territory_name(d, &t.name);
        pool::put_i16(d, 0x4e, t.goodwill);
        d[territory::SHOWN] = u8::from(t.visible);
        d[0x25] = u8::from(t.merged_flag);
        d[territory::REGION] = t.parent;
        d[0x43] = u8::from(t.separate_rights);
        for i in 0..4 {
            d[0x26 + i] = u8::from(t.rights[i]);
            d[0x44 + i] = u8::from(t.for_sale[i]);
        }
        Ok(())
    }
}

fn place_info(slot: usize, d: &[u8]) -> PlaceInfo {
    PlaceInfo {
        id: slot as u16,
        x: pool::u16_at(d, place::X),
        y: pool::u16_at(d, place::Y),
        kind: d[place::KIND],
        name: place_name(d),
        size: d[place::SIZE],
        style: d[place::STYLE],
        trend: d[place::TREND] as i8,
        weights: (0..PLACE_WEIGHTS)
            .map(|t| pool::u16_at(d, place::WEIGHTS + 2 * t))
            .collect(),
    }
}

fn territory_info(d: &[u8]) -> TerritoryInfo {
    TerritoryInfo {
        id: pool::u32_at(d, territory::ID) as u8,
        name: territory_name(d),
        goodwill: pool::i16_at(d, 0x4e),
        visible: d[territory::SHOWN] != 0,
        merged_flag: d[0x25] != 0,
        parent: d[territory::REGION],
        separate_rights: d[0x43] != 0,
        rights: [0, 1, 2, 3].map(|i| d[0x26 + i] != 0),
        for_sale: [0, 1, 2, 3].map(|i| d[0x44 + i] != 0),
    }
}

// ---- scenario options (the byte maps of the Map Options sections) ---------------------------

/// Number of general restrictions (`0x5706a9 + i`, `docs/spec/finance.md` 13.9).
pub const PERMISSION_COUNT: usize = 22;
const ST_PERMISSIONS: usize = 0x5706a9 - 0x5704e8;
/// `0x5704ac`: the industry set (0 original, 1 expansion), a byte of the header block.
const HDR_INDUSTRY_SET: usize = 0x8974;
/// Option byte `0x574872` (`+0x32` of the `0x574840` block): bits 0..2 industrial model, bits 3..5
/// financial model.
const TR_MODELS: usize = 0x32;
const TR_START_YEAR: usize = 0x22;
/// The person's cash (`+0x12a`, f64), docs/spec/scenarios.md 3.3.
const PERSON_CASH: usize = 0x12a;

/// General restriction `i` (1 = allowed).
pub fn permission(s: &Save, i: usize) -> bool {
    s.state.get(ST_PERMISSIONS + i).is_some_and(|&b| b != 0)
}

pub fn set_permission(s: &mut Save, i: usize, on: bool) {
    if let Some(b) = s.state.get_mut(ST_PERMISSIONS + i) {
        *b = u8::from(on);
    }
}

/// The industry set: 0 the original industries, 1 the expansion's.
pub fn industry_set(s: &Save) -> u8 {
    s.header.get(HDR_INDUSTRY_SET).copied().unwrap_or(0)
}

pub fn set_industry_set(s: &mut Save, set: u8) {
    if let Some(b) = s.header.get_mut(HDR_INDUSTRY_SET) {
        *b = set;
    }
}

/// Financial model 0 Basic, 1 Advanced, 2 Expert (`+0x32` bits 3..5 hold 0, 8, 0x10).
pub fn financial_model(s: &Save) -> u8 {
    (s.trailer[TR_MODELS] >> 3) & 7
}

pub fn set_financial_model(s: &mut Save, m: u8) {
    s.trailer[TR_MODELS] = (s.trailer[TR_MODELS] & 0xc7) | ((m.min(2)) << 3);
}

/// Industrial model 0 Basic, 1 Advanced, 2 Expert (bits 0..2).
pub fn industrial_model(s: &Save) -> u8 {
    s.trailer[TR_MODELS] & 7
}

pub fn set_industrial_model(s: &mut Save, m: u8) {
    s.trailer[TR_MODELS] = (s.trailer[TR_MODELS] & 0xf8) | m.min(2);
}

/// The scenario's start year (the date a new game opens at).
pub fn start_year(s: &Save) -> u16 {
    s.date.year
}

/// Sets the start year: the file date (January 1, 00:00), its minute count and the option
/// block's year (`+0x22`).
pub fn set_start_year(s: &mut Save, year: u16) {
    s.date = rt2_formats::map::GameDate {
        year,
        month: 1,
        day: 1,
    };
    s.hour = 0;
    s.minute = 0;
    s.game_time_minutes = crate::calendar::Date::new(year, 1, 1, 0, 0).to_minutes();
    s.trailer[TR_START_YEAR..TR_START_YEAR + 2].copy_from_slice(&year.to_le_bytes());
}

/// Cash of the first player persona (all of them are set together by [`set_player_cash`]).
pub fn player_cash(s: &Save) -> f64 {
    s.persons
        .ordered()
        .next()
        .map_or(0.0, |r| pool::f64_at(&r.data, PERSON_CASH))
}

/// Gives every player persona this much cash (`+0x12a`).
pub fn set_player_cash(s: &mut Save, cash: f64) {
    for r in &mut s.persons.records {
        if r.data.len() >= PERSON_CASH + 8 {
            pool::put_f64(&mut r.data, PERSON_CASH, cash);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_paint_weights_sum_to_one_hundred() {
        for (i, m) in MIX_WEIGHTS.iter().enumerate() {
            let sum: u32 = m.iter().map(|&w| u32::from(w)).sum();
            assert_eq!(sum, 100, "mix {i}");
        }
    }

    #[test]
    fn palettes_have_the_original_sizes() {
        assert_eq!(SOLID_CLASSES.len(), 16);
        assert_eq!(*SOLID_CLASSES.last().unwrap(), WATER_CLASS);
        assert_eq!(TREE_CLASSES.len(), 8);
    }
}
