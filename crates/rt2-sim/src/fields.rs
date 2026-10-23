//! Field tiles of new buildings (`building_construct` `0x409a30`, world-growth.md 4.5.1): the
//! cells of the shape with flag `0x40` keep a copy of the tile in `pool_88` (`0x409120`) and get
//! the building's field look: a vegetation class with random crop rows (`0x429fc0`) for types
//! 10, 0x1d and 0x1e, or a forced terrain class with a random variant (`0x42a3e0`) for types 0xb,
//! 0x10 and 0x21, whose box is then re-blended (`0x42a7d0`). The demolition gives the tiles back
//! (`0x4091f0`, [`crate::tilemark::restore_saved_tiles`]) and re-blends the same types.

use rt2_formats::map::{Save, Tile};
use rt2_formats::terrain::{self, Node, TerrainGrid, TileBytes};

use crate::pool;
use crate::rng::GameRng;
use crate::tilemark::{self, DirtyRect};

fn in_map(s: &Save, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && (x as usize) < s.map.width && (y as usize) < s.map.height
}

fn tile_mut(s: &mut Save, x: i32, y: i32) -> &mut Tile {
    let w = s.map.width;
    &mut s.map.tiles[y as usize * w + x as usize]
}

fn terrain_class(t: &Tile) -> usize {
    let id = u16::from(t.var) | (u16::from(t.ground) << 8);
    usize::from(terrain::tables().class_of_id(id >> 6))
}

/// What a field shape writes: `Veg(class)` is `0x429fc0` with the vegetation class kept in the
/// global `0x53a0a4` (12, 10, 11 for types 10, 0x1d, 0x1e); `Ground(class)` is `0x42a3e0` with a
/// forced terrain class (10, 9, 11 for types 0xb, 0x10, 0x21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Veg(u8),
    Ground(usize),
}

/// The `switch` of `0x409a30` on the building type.
pub fn field_kind(building_type: u8) -> Option<FieldKind> {
    Some(match building_type {
        10 => FieldKind::Veg(0xc),
        0x1d => FieldKind::Veg(10),
        0x1e => FieldKind::Veg(0xb),
        0xb => FieldKind::Ground(10),
        0x10 => FieldKind::Ground(9),
        0x21 => FieldKind::Ground(0xb),
        _ => return None,
    })
}

/// `0x409120(building, dx, dy)`: pushes the tile at `origin + (dx, dy)` onto the building's
/// `pool_88` chain (`+0x50`) as `{i8 dx, i8 dy, trees << 4 | flags2 & 0xf, u16 id << 2 | turns,
/// u16 next}`. The original reads the tile without a bounds check; outside the map the record
/// is still written (with zero tile bytes here). Returns the new head.
pub fn save_tile(s: &mut Save, head: u16, ox: i32, oy: i32, dx: i32, dy: i32) -> u16 {
    let (x, y) = (ox + dx, oy + dy);
    let (b2, v) = if in_map(s, x, y) {
        let t = &s.map.tiles[y as usize * s.map.width + x as usize];
        (
            (t.trees << 4) | (t.flags2 & 0xf),
            u16::from(t.var) | (u16::from(t.ground) << 8),
        )
    } else {
        (0, 0)
    };
    let saved = ((v & 0xffc0) >> 4) | ((v >> 4) & 3);
    let mut d = vec![0u8; s.pool_88.record_size.max(7)];
    d[0] = dx as u8;
    d[1] = dy as u8;
    d[2] = b2;
    d[3..5].copy_from_slice(&saved.to_le_bytes());
    d[5..7].copy_from_slice(&head.to_le_bytes());
    d.truncate(s.pool_88.record_size);
    pool::alloc(&mut s.pool_88, d).map_or(head, |slot| slot as u16)
}

/// `0x429fc0(x, y, radius 0, strength 1, flags 1)` with vegetation class `class` (`0x53a0a4`):
/// on a land tile that is not beach (class 0) nor road, one `rng(0, 100)` (the chance table
/// gives 0 for the field classes, so the tile always changes), the vegetation nibble becomes
/// `class` (a bare tile also loses its trees), then `rng(0, 8)` is drawn into the tree bits until
/// the value is not 3..=6; finally the terrain flags of the 3x3 box (`0x424c10`).
pub fn paint_vegetation(s: &mut Save, rng: &mut GameRng, x: i32, y: i32, class: u8) {
    if in_map(s, x, y) {
        let t = &s.map.tiles[y as usize * s.map.width + x as usize];
        let c = terrain_class(t);
        if c != 0x12 && (c != 0 || class == 4) && t.trees & 0x20 == 0 {
            // Chance table `local_38`: classes 10..=13 are 0, so `rng >= 0` always passes.
            const CHANCE: [i32; 14] = [0, 60, 60, 60, 70, 75, 80, 92, 60, 60, 0, 0, 0, 0];
            let r = rng.range(0, 100);
            if CHANCE[usize::from(class)] <= r {
                let t = tile_mut(s, x, y);
                if t.flags2 & 0xf == 0 {
                    t.flags2 = (t.flags2 & 0xf0) | class;
                    t.trees &= 0xe0;
                } else {
                    t.flags2 = (t.flags2 & 0xf0) | class;
                }
                debug_assert!(matches!(class, 10..=12));
                loop {
                    let v = rng.range(0, 8) as u8;
                    let t = tile_mut(s, x, y);
                    t.trees = (t.trees & 0xe0) | (v & 0x1f);
                    if !matches!(v & 0x1f, 3..=6) {
                        break;
                    }
                }
            }
        }
    }
    tilemark::recompute_flags(s, x - 1, y - 1, x + 1, y + 1);
}

/// `0x42a3e0(x, y, 1, 1, class, repaint 0, _, forced 1, 0, 0)`: a tile whose terrain id is
/// outside the variant range of the forced `class` ([`terrain::variant_range`]) gets
/// `rng(lo, hi)`; the turn bits are cleared. Returns whether the id was drawn. (The redraw
/// rectangle `0x417770` only affects the flat bit, recomputed from unchanged corners.)
pub fn force_ground(
    s: &mut Save,
    dirty: &mut DirtyRect,
    rng: &mut GameRng,
    x: i32,
    y: i32,
    class: usize,
) -> bool {
    if !in_map(s, x, y) {
        return false;
    }
    dirty.add(x - 1, y - 1, x - 1, y - 1);
    let (lo, hi) = terrain::variant_range(class, x, y);
    let t = tile_mut(s, x, y);
    let mut v = u16::from(t.var) | (u16::from(t.ground) << 8);
    let id = i32::from(v >> 6);
    let mut drawn = false;
    if id < lo || hi < id {
        let n = rng.range(lo, hi);
        v = ((n as u16) << 6) | (v & 0x3f);
        drawn = true;
    }
    if v & 0x30 != 0 {
        v &= 0xffcf;
    }
    let t = tile_mut(s, x, y);
    t.var = v as u8;
    t.ground = (v >> 8) as u8;
    drawn
}

/// Where the drawn track of `0x4cb090` (overlay layers 4 and 5) is, for a rectangle of tiles
/// (`build::Ai::track_overlay`); outside it, none.
#[derive(Debug, Clone, Default)]
pub struct TrackMask {
    pub x0: i32,
    pub y0: i32,
    pub w: i32,
    pub h: i32,
    pub bits: Vec<bool>,
}

impl TrackMask {
    pub fn get(&self, x: i32, y: i32) -> bool {
        let (dx, dy) = (x - self.x0, y - self.y0);
        dx >= 0 && dy >= 0 && dx < self.w && dy < self.h && self.bits[(dy * self.w + dx) as usize]
    }
}

/// What the field passes need besides the save: the redraw rectangle, the overlay state of the
/// tiles (runtime, rebuilt by the loader's passes) and the track overlays.
pub struct FieldCtx<'a> {
    pub dirty: &'a mut DirtyRect,
    /// Per tile (`y * W + x`): its overlay list holds ground blends (layer 0). Only the
    /// presence matters to `0x42a7d0`; the other layers are derived from the tile.
    pub ground: &'a mut [bool],
    pub track: &'a TrackMask,
}

/// The ground-layer state the loader's passes leave (`0x4294d0`, `0x428fe0`, `0x42a7d0` on the
/// whole map, with the track overlays present): per tile, whether a ground blend node is in its
/// list. `rng` only feeds the passes' draws (their effect on the ids is already in the tiles).
pub fn loader_ground_layer(s: &Save, track: &TrackMask, rng: u32) -> Vec<bool> {
    let mwu = s.map.width;
    let tiles = &s.map.tiles;
    let mut g = TerrainGrid::from_new_ids(mwu, s.map.height, |tx, ty| {
        let t = &tiles[ty * mwu + tx];
        TileBytes {
            b0: t.var,
            b1: t.ground,
            b2: t.flags2,
            b3: t.trees,
            b6: t.flags6,
        }
    });
    for (i, c) in g.cells.iter_mut().enumerate() {
        if track.get((i % mwu) as i32, (i / mwu) as i32) {
            c.nodes.push(Node {
                word: 4,
                turns: 0,
                shade: 0,
            });
        }
    }
    g.build_overlays(&mut terrain::GameRng(rng));
    ground_layer_of(&g)
}

/// Per tile: the cell holds a ground (layer 0) node.
pub fn ground_layer_of(g: &TerrainGrid) -> Vec<bool> {
    g.cells
        .iter()
        .map(|c| c.nodes.iter().any(|n| n.layer() == terrain::LAYER_GROUND))
        .collect()
}

/// `0x42a7d0(x, y, w, h)` on the save's tiles (position and size; clamped like the original),
/// with its tail: the territory borders (`0x42bf90`) and the terrain flags (`0x424c10`) of
/// `(x0 - 1, y0 - 1, x0 + w + 1, y0 + h + 1)` from the clamped corner, and the redraw
/// rectangle (`0x417770`).
///
/// The overlay lists of the tiles it touches are rebuilt first: the ground blends from
/// [`FieldCtx::ground`], rivers (`tile & 1`, layer 2), roads (`trees & 0x20`, layer 3) and track
/// (layers 4/5) from the tile, and the territory borders by `0x42bf90` itself (with the saved
/// flag bytes kept: `tile+6 & 0x80` can be stale in a map file, and `0x4258e0` only runs when
/// a list changes). Only the presence of the nodes matters to the pass: whether a blend
/// replaces the base id, and whether `0x4258e0` runs after the ground layer is cleared.
pub fn ground_rect(
    s: &mut Save,
    ctx: &mut FieldCtx<'_>,
    rng: &mut GameRng,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    ground_rect_with(s, ctx, rng, x, y, w, h, None);
}

/// Per tile (`y * W + x`): the territory borders `0x42bf90` gives the tile (layer 1) over the
/// current tiles of the whole map.
pub fn border_layer(s: &Save) -> Vec<bool> {
    let mwu = s.map.width;
    let tiles = &s.map.tiles;
    let mut g = TerrainGrid::from_new_ids(mwu, s.map.height, |tx, ty| {
        let t = &tiles[ty * mwu + tx];
        TileBytes {
            b0: t.var,
            b1: t.ground,
            b2: t.flags2,
            b3: t.trees,
            b6: t.flags6,
        }
    });
    let region: Vec<u8> = tiles.iter().map(|t| t.region).collect();
    let terr = &s.territories;
    let shown = |r: u8| {
        terr.get(usize::from(r))
            .is_some_and(|t| t.data.get(0x24).is_some_and(|&b| b != 0))
    };
    g.build_territory_borders(&region, shown);
    g.cells
        .iter()
        .map(|c| {
            c.nodes
                .iter()
                .any(|n| n.layer() == terrain::LAYER_TERRITORY)
        })
        .collect()
}

/// [`ground_rect`] with the territory borders the tiles' lists hold before the pass given
/// (`borders`, per tile), when the tiles changed since the borders were last built (the water
/// fill of a flood); without it they are rebuilt from the current tiles.
#[allow(clippy::too_many_arguments)]
pub fn ground_rect_with(
    s: &mut Save,
    ctx: &mut FieldCtx<'_>,
    rng: &mut GameRng,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    borders: Option<&[bool]>,
) {
    let (mw, mh) = (s.map.width as i32, s.map.height as i32);
    let (x0, y0) = (x.max(0), y.max(0));
    let x1 = (x + w - 1).min(mw - 1);
    let y1 = (y + h - 1).min(mh - 1);
    if x0 > x1 || y0 > y1 {
        return;
    }
    let (bx0, by0, bx1, by1) = (x0 - 1, y0 - 1, x0 + w + 1, y0 + h + 1);
    let (rx0, ry0) = (bx0.max(0), by0.max(0));
    let (rx1, ry1) = (bx1.min(mw - 1), by1.min(mh - 1));
    let mwu = s.map.width;
    let tiles = &s.map.tiles;
    let mut g = TerrainGrid::from_new_ids(mwu, s.map.height, |tx, ty| {
        let t = &tiles[ty * mwu + tx];
        TileBytes {
            b0: t.var,
            b1: t.ground,
            b2: t.flags2,
            b3: t.trees,
            b6: t.flags6,
        }
    });
    let region: Vec<u8> = s.map.tiles.iter().map(|t| t.region).collect();
    let terr = &s.territories;
    let shown = |r: u8| {
        terr.get(usize::from(r))
            .is_some_and(|t| t.data.get(0x24).is_some_and(|&b| b != 0))
    };
    let node = |layer: u8| Node {
        word: u16::from(layer),
        turns: 0,
        shade: 0,
    };
    for ty in ry0..=ry1 {
        for tx in rx0..=rx1 {
            let i = ty as usize * mwu + tx as usize;
            let t = &s.map.tiles[i];
            let c = &mut g.cells[i];
            if ctx.ground[i] {
                c.nodes.push(node(terrain::LAYER_GROUND));
            }
            if t.var & 1 != 0 {
                c.nodes.insert(0, node(terrain::LAYER_RIVER));
            }
            if t.trees & 0x20 != 0 {
                c.nodes.insert(0, node(terrain::LAYER_ROAD));
            }
            if ctx.track.get(tx, ty) {
                c.nodes.insert(0, node(4));
            }
        }
    }
    match borders {
        Some(b) => {
            for ty in ry0..=ry1 {
                for tx in rx0..=rx1 {
                    let i = ty as usize * mwu + tx as usize;
                    if b[i] {
                        g.cells[i].nodes.push(node(terrain::LAYER_TERRITORY));
                    }
                }
            }
        }
        None => g.build_territory_borders_rect(rx0, ry0, rx1, ry1, &region, shown),
    }
    for ty in ry0..=ry1 {
        for tx in rx0..=rx1 {
            let i = ty as usize * mwu + tx as usize;
            let (f2, f6) = (s.map.tiles[i].flags2, s.map.tiles[i].flags6);
            g.cells[i].flags2 = f2;
            g.cells[i].flags6 = f6;
        }
    }
    let mut trng = terrain::GameRng(rng.state());
    g.build_ground_rect(
        x0 as usize,
        y0 as usize,
        (x1 - x0 + 1) as usize,
        (y1 - y0 + 1) as usize,
        &mut trng,
    );
    *rng = GameRng::new(trng.0);
    g.build_territory_borders_rect(bx0, by0, bx1, by1, &region, shown);
    for ty in ry0..=ry1 {
        for tx in rx0..=rx1 {
            let i = ty as usize * mwu + tx as usize;
            let c = &g.cells[i];
            ctx.ground[i] = c.nodes.iter().any(|n| n.layer() == terrain::LAYER_GROUND);
            let t = tile_mut(s, tx, ty);
            t.var = c.v as u8;
            t.ground = (c.v >> 8) as u8;
            t.flags2 = c.flags2;
            t.flags6 = c.flags6;
        }
    }
    tilemark::recompute_flags(s, bx0, by0, bx1, by1);
    ctx.dirty.add(x0, y0, x1, y1);
}

/// The field pass of `0x409a30` (after `0x409770` and `0x409580`): for each shape cell with flag
/// `0x40` (`lin` outer, `col` inner) the tile is saved (`0x409120`) and painted; the forced
/// ground types then re-blend their box (`0x42a7d0(xmin - 1, ymin - 1, w + 3, h + 3)`). Returns
/// the new `+0x50` head.
#[allow(clippy::too_many_arguments)]
pub fn paint_fields(
    s: &mut Save,
    ctx: &mut FieldCtx<'_>,
    img: &crate::data::DataImage,
    rng: &mut GameRng,
    shape: usize,
    building_type: u8,
    origin: (i32, i32),
    mut head: u16,
) -> u16 {
    let kind = field_kind(building_type);
    let (ox, oy) = origin;
    let mut bbox: Option<[i32; 4]> = None;
    for (lin, col) in tilemark::cells() {
        if tilemark::shape_cell(img, shape, lin, col) & 0x40 == 0 {
            continue;
        }
        head = save_tile(s, head, ox, oy, -col, -lin);
        let (x, y) = (ox - col, oy - lin);
        match kind {
            Some(FieldKind::Veg(c)) => paint_vegetation(s, rng, x, y, c),
            Some(FieldKind::Ground(c)) => {
                force_ground(s, ctx.dirty, rng, x, y, c);
                let b = bbox.get_or_insert([x, y, x, y]);
                b[0] = b[0].min(x);
                b[1] = b[1].min(y);
                b[2] = b[2].max(x);
                b[3] = b[3].max(y);
            }
            None => {}
        }
    }
    if let Some([x0, y0, x1, y1]) = bbox {
        ground_rect(s, ctx, rng, x0 - 1, y0 - 1, x1 - x0 + 3, y1 - y0 + 3);
    }
    head
}

/// The tail of `0x4091f0` for types 0xb, 0x10 and 0x21: `0x42a7d0(xmin - 1, ymin - 1, w + 3,
/// h + 3)` over the building's box `+0x26..+0x2c`.
pub fn repaint_after_demolition(
    s: &mut Save,
    ctx: &mut FieldCtx<'_>,
    rng: &mut GameRng,
    bbox: [i16; 4],
) {
    let b = bbox.map(i32::from);
    ground_rect(
        s,
        ctx,
        rng,
        b[0] - 1,
        b[1] - 1,
        b[2] - b[0] + 3,
        b[3] - b[1] + 3,
    );
}
