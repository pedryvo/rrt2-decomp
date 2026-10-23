//! Tile marking of buildings: what `building_construct` (`0x409a30`) and `building_destroy`
//! (`0x40adb0`) write into the map, docs/spec/world-growth.md 4.5.
//!
//! * the structure chain of each tile (`0x574c54`, `0x42e850` add and `0x42e320` remove);
//! * the footprint geometry of the building record (`0x409770`);
//! * the terrain levelling under the footprint (`0x409580`, corner elevations);
//! * the terrain flags of a region (`0x424c10`: buildable bit, trees, border bit) and the flat
//!   tile bit (`tile+6 & 1`, set by the deferred redraw `0x417880`).
//!
//! Everything works on the raw [`Save`] like the original works on its memory, so the pool slot
//! allocation order (and with it the ids that train markers get later) is the original's.

use rt2_formats::map::Save;
use rt2_formats::terrain;

use crate::data::DataImage;
use crate::pool;

/// `building_shape_table` rows are `0x154` bytes from `0x5457ef`; the 10x10 flag grid (u16 per
/// cell, read backwards) ends at `0x5458c5`.
const SHAPE_BASE: u32 = 0x0054_57ef;
const MASK_BASE: u32 = 0x0054_58c5;
const SHAPE_SIZE: u32 = 0x154;

pub const STRUCT_BUILDING: u8 = 3;

/// Flag word of the footprint cell `(lin, col)` of `shape`: tile `(cx - col, cy - lin)`.
pub fn shape_cell(img: &DataImage, shape: usize, lin: i32, col: i32) -> u16 {
    let a = i64::from(MASK_BASE) + i64::from(SHAPE_SIZE) * shape as i64
        - 20 * i64::from(lin)
        - 2 * i64::from(col);
    img.u16(a as u32)
}

/// Tile type of the shape (`0x5457f8`).
pub fn shape_type(img: &DataImage, shape: usize) -> u8 {
    img.u8(SHAPE_BASE + 9 + SHAPE_SIZE * shape as u32)
}

/// The footprint extents that `0x471b10` computes at start-up into the shape rows (they are 0
/// in the executable image): `+12` width and `+13` height of the non-empty cells, `+14`/`+15`
/// column/line of the centre cell (the first `& 3` cell probed around the middle of the solid
/// cells' box).
pub fn shape_extents(img: &DataImage, shape: usize) -> [i32; 4] {
    let cell = |lin: i32, col: i32| shape_cell(img, shape, lin, col);
    let (mut w, mut h) = (0, 0);
    let (mut cmin, mut lmin, mut cmax, mut lmax) = (999, 999, 0, 0);
    for col in 0..10 {
        for lin in 0..10 {
            let v = cell(lin, col);
            if v != 0 {
                w = w.max(col + 1);
                h = h.max(lin + 1);
            }
            if v & 3 != 0 {
                cmin = cmin.min(col);
                lmin = lmin.min(lin);
                cmax = cmax.max(col);
                lmax = lmax.max(lin);
            }
        }
    }
    let bw = cmax - cmin + 1;
    let bh = lmax - lmin + 1;
    let (mut c, mut l) = (0, 0);
    let mut cur = bw;
    'outer: for _ in 0..2 {
        for k in 0..4 {
            let (cc, ll) = match k {
                0 => (bw, bh),
                1 => (cur, bh),
                2 => (bw, bh - bw + cur),
                _ => (cur, bh - bw + cur),
            };
            c = cc / 2 + cmin;
            l = ll / 2 + lmin;
            if cell(l, c) & 3 != 0 {
                break 'outer;
            }
        }
        cur -= 1;
    }
    [w, h, c, l]
}

fn shape_byte(img: &DataImage, shape: usize, off: u32) -> i32 {
    if (12..=15).contains(&off) {
        return shape_extents(img, shape)[(off - 12) as usize];
    }
    i32::from(img.u8(SHAPE_BASE + off + SHAPE_SIZE * shape as u32))
}

/// Cells `(lin, col)` of the footprint in the order of the original loops (`lin` outer).
pub fn cells() -> impl Iterator<Item = (i32, i32)> {
    (0..10).flat_map(|lin| (0..10).map(move |col| (lin, col)))
}

fn in_map(s: &Save, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < s.map.width as i32 && y < s.map.height as i32
}

fn tile_at(s: &Save, x: i32, y: i32) -> &rt2_formats::map::Tile {
    &s.map.tiles[(y as usize) * s.map.width + x as usize]
}

fn tile_at_mut(s: &mut Save, x: i32, y: i32) -> &mut rt2_formats::map::Tile {
    let w = s.map.width;
    &mut s.map.tiles[(y as usize) * w + x as usize]
}

fn struct_id(s: &Save, x: i32, y: i32) -> u16 {
    let t = tile_at(s, x, y);
    u16::from(t.struct_id) | (u16::from(t.reserved5) << 8)
}

fn set_struct_id(s: &mut Save, x: i32, y: i32, v: u16) {
    let t = tile_at_mut(s, x, y);
    t.struct_id = v as u8;
    t.reserved5 = (v >> 8) as u8;
}

fn st_next(s: &Save, id: u16) -> u16 {
    pool::record(&s.structures, usize::from(id))
        .map_or(0, |r| u16::from_le_bytes([r.data[5], r.data[6]]))
}

fn st_kind(s: &Save, id: u16) -> u8 {
    pool::record(&s.structures, usize::from(id)).map_or(0, |r| r.data[0])
}

fn st_data(s: &Save, id: u16) -> u32 {
    pool::record(&s.structures, usize::from(id)).map_or(0, |r| {
        u32::from_le_bytes([r.data[1], r.data[2], r.data[3], r.data[4]])
    })
}

fn set_st_next(s: &mut Save, id: u16, next: u16) {
    if let Some(r) = pool::record_mut(&mut s.structures, usize::from(id)) {
        r.data[5..7].copy_from_slice(&next.to_le_bytes());
    }
}

/// `tile_struct_add` (`0x42e850`): the chain of a tile is ordered by decreasing kind and the new
/// record goes after the last one with `kind >= new`; a building (3) sets `tile+2 |= 0x10`.
pub fn struct_add(s: &mut Save, x: i32, y: i32, kind: u8, data: u32) -> u16 {
    let mut cur = struct_id(s, x, y);
    let mut prev = 0u16;
    while cur != 0 {
        if st_kind(s, cur) < kind {
            break;
        }
        prev = cur;
        cur = st_next(s, cur);
    }
    let mut rec = vec![kind];
    rec.extend_from_slice(&data.to_le_bytes());
    rec.extend_from_slice(&cur.to_le_bytes());
    let new = pool::alloc(&mut s.structures, rec).expect("pool grows") as u16;
    if prev == 0 {
        set_struct_id(s, x, y, new);
    } else {
        set_st_next(s, prev, new);
    }
    if kind == STRUCT_BUILDING {
        tile_at_mut(s, x, y).flags2 |= 0x10;
    }
    new
}

/// `tile_struct_remove` (`0x42e320`): frees the first `(kind, data)` record of the chain, then
/// `tile+2 & 0x10` follows whether a kind-3 record is left.
pub fn struct_remove(s: &mut Save, x: i32, y: i32, kind: u8, data: u32) {
    let head = struct_id(s, x, y);
    if head == 0 {
        return;
    }
    let mut link: Option<u16> = None;
    let mut cur = head;
    loop {
        if st_data(s, cur) == data && st_kind(s, cur) == kind {
            break;
        }
        let nx = st_next(s, cur);
        if nx == 0 {
            return;
        }
        link = Some(cur);
        cur = nx;
    }
    let nx = st_next(s, cur);
    pool::free(&mut s.structures, usize::from(cur));
    match link {
        None => set_struct_id(s, x, y, nx),
        Some(p) => set_st_next(s, p, nx),
    }
    let mut c = struct_id(s, x, y);
    while c != 0 {
        if st_kind(s, c) == STRUCT_BUILDING {
            // `0x4667b0(c) != 0`: the record of a chain member always exists.
            tile_at_mut(s, x, y).flags2 |= 0x10;
            return;
        }
        c = st_next(s, c);
    }
    tile_at_mut(s, x, y).flags2 &= 0xef;
}

/// `0x408fd0(x, y)`: the building record of the kind-3 structure of the tile, if live.
fn building_on(s: &Save, x: i32, y: i32) -> bool {
    let mut c = struct_id(s, x, y);
    while c != 0 {
        if st_kind(s, c) == STRUCT_BUILDING {
            return pool::record(&s.objects, st_data(s, c) as usize).is_some();
        }
        c = st_next(s, c);
    }
    false
}

/// Geometry fields of a new building record (`0x409a30` and `0x409770`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    /// `+0x0a`: x of the extreme cell for the 4 view rotations, then `+0x12`: y.
    pub corner_x: [i16; 4],
    pub corner_y: [i16; 4],
    /// `+0x1a`, `+0x1c`: the anchor given to `building_create`.
    pub anchor: (i16, i16),
    /// `+0x1e`, `+0x20`: the origin cell (cell `(lin, col)` is the tile `(x - col, y - lin)`).
    pub origin: (i16, i16),
    /// `+0x22`, `+0x24`: origin minus the shape bytes `+0xe`, `+0xf` (region lookup tile).
    pub region_tile: (i16, i16),
    /// `+0x26`, `+0x28`, `+0x2a`, `+0x2c`: bounding box of the cells with flags `& 0xc3`.
    pub bbox: [i16; 4],
}

/// Footprint geometry for a building of `shape` anchored at `(x, y)` on a map of `w` x `h`.
pub fn geometry(img: &DataImage, shape: usize, x: i32, y: i32, w: i32, h: i32) -> Geometry {
    let cx = x + (shape_byte(img, shape, 12) >> 1);
    let cy = y + (shape_byte(img, shape, 13) >> 1);
    let mut bbox = [9999i16, 9999, 0, 0];
    let mut corner_x = [0i16; 4];
    let mut corner_y = [0i16; 4];
    // The four rotations pick the cell that maximises a weighted key; the first maximum wins.
    for k in 0..4usize {
        let (mut best, mut bx, mut by) = (-1i32, -1i32, -1i32);
        for (lin, col) in cells() {
            let cell = shape_cell(img, shape, lin, col);
            if cell & 0xc3 == 0 {
                continue;
            }
            let (tx, ty) = (cx - col, cy - lin);
            if k == 0 {
                bbox[0] = bbox[0].min(tx as i16);
                bbox[1] = bbox[1].min(ty as i16);
                bbox[2] = bbox[2].max(tx as i16);
                bbox[3] = bbox[3].max(ty as i16);
            }
            if cell & 3 != 0 {
                let key = match k {
                    0 => tx * 0x3e9 + ty * 1000,
                    1 => ty * 0x3e9 + (w - tx) * 1000,
                    2 => (w - tx) * 0x3e9 + (h - ty) * 1000,
                    _ => (h - ty) * 0x3e9 + tx * 1000,
                };
                if best < key {
                    best = key;
                    bx = tx;
                    by = ty;
                }
            }
        }
        corner_x[k] = bx as i16;
        corner_y[k] = by as i16;
    }
    Geometry {
        corner_x,
        corner_y,
        anchor: (x as i16, y as i16),
        origin: (cx as i16, cy as i16),
        region_tile: (
            (cx - shape_byte(img, shape, 14)) as i16,
            (cy - shape_byte(img, shape, 15)) as i16,
        ),
        bbox,
    }
}

/// End of `0x409770`: a kind-3 structure with the building id on every footprint cell (flags
/// `& 0xc3`) inside the map, `lin` outer and `col` inner.
pub fn mark_building(s: &mut Save, img: &DataImage, shape: usize, id: u16, cx: i32, cy: i32) {
    for (lin, col) in cells() {
        if shape_cell(img, shape, lin, col) & 0xc3 == 0 {
            continue;
        }
        let (tx, ty) = (cx - col, cy - lin);
        if in_map(s, tx, ty) {
            struct_add(s, tx, ty, STRUCT_BUILDING, u32::from(id));
        }
    }
}

/// `0x409580`: the corners under the footprint (cells with flags `& 0xc1`) are pulled halfway to
/// their mean height: `h' = (h + mean + 1) >> 1`, clamped to `-255..=510` (`0x426000`).
pub fn level_terrain(s: &mut Save, img: &DataImage, shape: usize, cx: i32, cy: i32) {
    let stride = s.map.width as i32 + 1;
    let mut verts: Vec<i32> = Vec::new();
    for (lin, col) in cells() {
        if shape_cell(img, shape, lin, col) & 0xc1 == 0 {
            continue;
        }
        let (tx, ty) = (cx - col, cy - lin);
        if !in_map(s, tx, ty) {
            continue;
        }
        // The corner offsets are `0x580fd8`: 0, 1, width + 2, width + 1.
        for off in [0, 1, stride + 1, stride] {
            let v = stride * ty + tx + off;
            if !verts.contains(&v) {
                verts.push(v);
            }
        }
    }
    if verts.is_empty() {
        return;
    }
    let n = verts.len() as i32;
    let sum: i32 = verts
        .iter()
        .map(|&v| i32::from(s.map.elevation[v as usize]))
        .sum();
    let mean = ((n >> 1) + sum) / n;
    for &v in &verts {
        let h = i32::from(s.map.elevation[v as usize]);
        s.map.elevation[v as usize] = ((h + mean + 1) >> 1).clamp(-0xff, 0x1fe) as i16;
    }
}

fn class_of(t: &rt2_formats::map::Tile) -> i32 {
    let id = u16::from(t.var) | (u16::from(t.ground) << 8);
    i32::from(terrain::tables().class_of_id(id >> 6))
}

/// `0x424c10(x0, y0, x1, y1)`: terrain flags of a region. Pass 1 sets `tile+6 & 0x10` (not
/// buildable: water, coast, fields, rivers, track, or a building on a flat-ish nibble), drops
/// the vegetation nibble of bare tiles and sets `tile+6 & 4`; pass 2 sets `tile+6 & 8` (border)
/// for the tiles whose neighbour is not buildable.
pub fn recompute_flags(s: &mut Save, x0: i32, y0: i32, x1: i32, y1: i32) {
    let (w, h) = (s.map.width as i32, s.map.height as i32);
    let (x0, y0) = (x0.max(0), y0.max(0));
    let (x1, y1) = (x1.min(w - 1), y1.min(h - 1));
    if x0 > x1 || y0 > y1 {
        return;
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let bld = building_on(s, x, y);
            let t = tile_at_mut(s, x, y);
            let c = class_of(t);
            let nib = t.flags2 & 0xf;
            let unbuildable = c == 0x12
                || (c == 0 && nib != 4)
                || c == 9
                || c == 10
                || c == 11
                || t.track_id != 0
                || t.reserved8 != 0
                || t.var & 1 != 0
                || (bld && nib < 10);
            if unbuildable {
                t.flags6 |= 0x10;
            } else {
                t.flags6 &= 0xef;
            }
            if t.trees & 0x1f == 0 && matches!(nib, 1 | 2 | 4..=9) {
                t.flags2 &= 0xf0;
            }
            if t.flags6 & 0x10 == 0 && t.flags2 & 0xf != 0 {
                t.flags6 |= 4;
            } else {
                t.flags6 &= 0xfb;
            }
        }
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let bit4 = |s: &Save, px: i32, py: i32| tile_at(s, px, py).flags6 & 0x10 != 0;
            let f6 = tile_at(s, x, y).flags6;
            let border = f6 & 4 == 0
                || x < 1
                || bit4(s, x - 1, y)
                || y < 2
                || bit4(s, x, y - 1)
                || x >= w - 1
                || bit4(s, x + 1, y)
                || y >= h - 1
                || bit4(s, x, y + 1);
            let t = tile_at_mut(s, x, y);
            if border {
                t.flags6 |= 8;
            } else {
                t.flags6 &= 0xf7;
            }
        }
    }
}

/// `0x417880` (deferred redraw of the dirty rectangle): `tile+6 & 1` = the four corners of the
/// tile have the same elevation.
pub fn recompute_flat(s: &mut Save, x0: i32, y0: i32, x1: i32, y1: i32) {
    let (w, h) = (s.map.width as i32, s.map.height as i32);
    let stride = w + 1;
    let (x0, y0) = (x0.max(0), y0.max(0));
    let (x1, y1) = (x1.min(w - 1), y1.min(h - 1));
    for y in y0..=y1 {
        for x in x0..=x1 {
            let e = |ex: i32, ey: i32| s.map.elevation[(ey * stride + ex) as usize];
            let a = e(x, y);
            let flat = a == e(x + 1, y + 1) && a == e(x + 1, y) && a == e(x, y + 1);
            let t = tile_at_mut(s, x, y);
            t.flags6 = (t.flags6 & 0xfe) | u8::from(flat);
        }
    }
}

/// The redraw rectangle of the map (`0x417770`: the union of the rectangles passed since the
/// last redraw `0x417c40`, kept unclamped). Runtime state, not saved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirtyRect(pub Option<[i32; 4]>);

impl DirtyRect {
    /// `0x417770(x0, y0, x1, y1)`.
    pub fn add(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.0 = Some(match self.0 {
            None => [x0, y0, x1, y1],
            Some([a, b, c, d]) => [a.min(x0), b.min(y0), c.max(x1), d.max(y1)],
        });
    }
}

/// `0x416bf0`: the shade of a tile (runtime byte `tile+0xe`, 0..=31) from the slopes of its
/// corners ([`crate::build::tiles::tile_shade_from`]).
pub fn tile_light(s: &Save, x: i32, y: i32) -> u8 {
    let stride = s.map.width as i32 + 1;
    let e = |ex: i32, ey: i32| i32::from(s.map.elevation[(ey * stride + ex) as usize]);
    crate::build::tiles::tile_shade_from([e(x, y), e(x + 1, y), e(x + 1, y + 1), e(x, y + 1)]) as u8
}

/// The light pass of the deferred redraw (`0x417c40`) over an inclusive rectangle (clamped,
/// `0x425850`): the tiles' shades (`0x416bf0`), then per tile (`0x416d40`) the shade of its
/// four corners (mean of the up to four tiles around each corner) and `tile+6 & 0x20` = the
/// tile's shade equals all four. Shades outside the rectangle are taken from the current
/// elevation (the original keeps them from their last redraw).
pub fn recompute_light(s: &mut Save, x0: i32, y0: i32, x1: i32, y1: i32) {
    let (w, h) = (s.map.width as i32, s.map.height as i32);
    let (x0, y0) = (x0.max(0), y0.max(0));
    let (x1, y1) = (x1.min(w - 1), y1.min(h - 1));
    if x0 > x1 || y0 > y1 {
        return;
    }
    let (lx0, ly0, lx1, ly1) = (
        (x0 - 1).max(0),
        (y0 - 1).max(0),
        (x1 + 1).min(w - 1),
        (y1 + 1).min(h - 1),
    );
    let lw = lx1 - lx0 + 1;
    let mut light = vec![0u8; (lw * (ly1 - ly0 + 1)) as usize];
    for y in ly0..=ly1 {
        for x in lx0..=lx1 {
            light[((y - ly0) * lw + x - lx0) as usize] = tile_light(s, x, y);
        }
    }
    let at = |x: i32, y: i32| i32::from(light[((y - ly0) * lw + x - lx0) as usize]);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let own = at(x, y);
            let mut same = true;
            for (cx, cy) in [(x - 1, y - 1), (x, y - 1), (x, y), (x - 1, y)] {
                let (mut n, mut sum) = (0, 0);
                for ty in cy..=cy + 1 {
                    for tx in cx..=cx + 1 {
                        if tx >= 0 && tx < w && ty >= 0 && ty < h {
                            n += 1;
                            sum += at(tx, ty);
                        }
                    }
                }
                // The interior case of `0x416d40` is `>> 2` of four, the edge case `sum / n`.
                if (sum / n) as u8 != own as u8 {
                    same = false;
                }
            }
            let t = tile_at_mut(s, x, y);
            if same {
                t.flags6 |= 0x20;
            } else {
                t.flags6 &= 0xdf;
            }
        }
    }
}

/// Start of `building_destroy`'s tile pass: removes the building's kind-3 structure from every
/// tile of its bounding box (`+0x26..+0x2c`, `x` outer, `y` inner).
pub fn unmark_building(s: &mut Save, id: u16, bbox: [i16; 4]) {
    for x in i32::from(bbox[0])..=i32::from(bbox[2]) {
        for y in i32::from(bbox[1])..=i32::from(bbox[3]) {
            if in_map(s, x, y) {
                struct_remove(s, x, y, STRUCT_BUILDING, u32::from(id));
            }
        }
    }
}

/// `0x4091f0` (first part): gives back the tile data saved by `0x409120` for the cells of a
/// field. `head` is `building+0x50`; each record is `{i8 dx, i8 dy, u8 flags2/trees, u16 terrain,
/// u16 next}`. Returns the new head (0).
pub fn restore_saved_tiles(s: &mut Save, mut head: u16, cx: i32, cy: i32) -> u16 {
    while head != 0 {
        let Some(r) = pool::record(&s.pool_88, usize::from(head)) else {
            break;
        };
        let d = r.data.clone();
        let next = u16::from_le_bytes([d[5], d[6]]);
        let (x, y) = (cx + i32::from(d[0] as i8), cy + i32::from(d[1] as i8));
        if in_map(s, x, y) {
            let t = tile_at_mut(s, x, y);
            t.flags2 = ((d[2] ^ t.flags2) & 0xf) ^ t.flags2;
            t.trees = (t.trees & 0xe0) | (d[2] >> 4);
            let old = u16::from(t.var) | (u16::from(t.ground) << 8);
            let sav = u16::from_le_bytes([d[3], d[4]]);
            let v = ((sav & 0xfffc) << 4) | (u16::from(d[3] & 3) << 4) | (old & 0xf);
            t.var = v as u8;
            t.ground = (v >> 8) as u8;
        }
        pool::free(&mut s.pool_88, usize::from(head));
        head = next;
    }
    0
}

/// What `building_can_place` reads of a tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileFacts {
    pub var: u8,
    pub ground: u8,
    pub flags2: u8,
    pub trees: u8,
    /// A kind-3 structure with a live building record is on the tile (`0x42e8f0` + `0x4f18b0`).
    pub has_building: bool,
    /// The tile's overlay list holds a track layer (`0x415f80`: `word & 7` is 4 or 5).
    pub track_overlay: bool,
}

/// The map as `building_can_place` sees it.
pub trait PlaceMap {
    fn size(&self) -> (i32, i32);
    fn tile(&self, x: i32, y: i32) -> TileFacts;
    /// Terrain class of the terrain id (`0x581090`), 18 = water.
    fn class_of(&self, id: u16) -> i32;
}

/// `building_can_place` (`0x4087a0`) as `town_place_building` calls it (`company = 0`,
/// `param_5 = 0`, no allowed building, station flag 1): `early` is `param_10` (`tentativas > 300`).
/// Valid for the shapes of town buildings; the extra rules of a station (`shape type 0`: rail
/// edges beside the anchor, territory permission) are not modelled.
pub fn can_place(
    map: &impl PlaceMap,
    img: &DataImage,
    x: i32,
    y: i32,
    shape: usize,
    early: bool,
) -> bool {
    let (w, h) = map.size();
    let ox = (shape_byte(img, shape, 12) >> 1) + x;
    let oy = (shape_byte(img, shape, 13) >> 1) + y;
    let station = shape_type(img, shape) == 0;
    for (lin, col) in cells() {
        let c = shape_cell(img, shape, lin, col);
        if c & 0x3c3 == 0 {
            continue;
        }
        let (tx, ty) = (ox - col, oy - lin);
        if tx < 0 || tx >= w || ty < 0 || ty >= h {
            return false;
        }
        let t = map.tile(tx, ty);
        if c & 0x200 != 0 && t.trees & 0x20 == 0 && early {
            return false;
        }
        if t.has_building {
            return false;
        }
        if t.var & 8 != 0 || t.var & 1 != 0 {
            return false;
        }
        if t.trees & 0x20 != 0 && c & 1 != 0 {
            return false;
        }
        let id = u16::from(t.var) | (u16::from(t.ground) << 8);
        let ok = if map.class_of(id >> 6) == 0x12 {
            c & 0x102
        } else {
            c & 0x2c1
        };
        if ok == 0 {
            return false;
        }
        if station {
            if t.flags2 & 0x20 != 0 {
                return false;
            }
        } else if t.flags2 & 0x80 != 0 || t.trees & 0x40 != 0 || t.track_overlay {
            return false;
        }
    }
    true
}

/// The save with the drawn track of `0x4cb090` ([`crate::fields::TrackMask`]): the overlay
/// test of `building_can_place` (`0x415f80`) also sees the pieces a neighbour's track draws
/// into the tile.
pub struct PlaceView<'a> {
    pub save: &'a Save,
    pub track: &'a crate::fields::TrackMask,
}

impl PlaceMap for PlaceView<'_> {
    fn size(&self) -> (i32, i32) {
        self.save.size()
    }
    fn tile(&self, x: i32, y: i32) -> TileFacts {
        TileFacts {
            track_overlay: self.track.get(x, y),
            ..self.save.tile(x, y)
        }
    }
    fn class_of(&self, id: u16) -> i32 {
        self.save.class_of(id)
    }
}

impl PlaceMap for Save {
    fn size(&self) -> (i32, i32) {
        (self.map.width as i32, self.map.height as i32)
    }
    fn tile(&self, x: i32, y: i32) -> TileFacts {
        let t = tile_at(self, x, y);
        TileFacts {
            var: t.var,
            ground: t.ground,
            flags2: t.flags2,
            trees: t.trees,
            has_building: building_on(self, x, y),
            // Layer 4/5 overlays are the drawn track pieces of the tile.
            track_overlay: t.track_id != 0 || t.reserved8 != 0,
        }
    }
    fn class_of(&self, id: u16) -> i32 {
        i32::from(terrain::tables().class_of_id(id))
    }
}
