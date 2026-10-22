//! Terrain ids, terrain classes and the ground overlay pass of the original.
//!
//! A tile stores a 10-bit **terrain id** (`u16 >> 6` of its first two bytes) plus six low bits
//! (bit 0 river, bits 1-2 river direction, bit 3 unknown, bits 4-5 texture turns). The id picks a
//! class (`class_of_id`, the table at `0x581090`) and a `ground.imb` record (`record_of_id`, the
//! table at `0x5804ac`); both are built once at startup by `0x471b10` from the 19 class
//! descriptors at `0x540aa0`. [`Tables`] rebuilds them from the descriptors' counts (checked
//! against the tables dumped from the original).
//!
//! Ground transitions (shores, the blend between two soils) are not stored in files: the loader
//! derives them from the classes of the neighbours with `0x42a7d0`, rivers with `0x4294d0` and
//! roads with `0x428fe0` ([`TerrainGrid::build_overlays`]). Each tile ends up with a base id and a
//! list of overlay nodes drawn on top of it. See `docs/formats/map-gam.md` section 3.2 and
//! `docs/formats/sprite-catalog.md` section 4.1.

use std::sync::OnceLock;

pub const CLASS_COUNT: usize = 19;
/// Terrain class of water (ocean, lakes, bays).
pub const WATER_CLASS: u8 = 18;
/// Class slots after a class's own ids: one "filler" id per class (`S4 + j`).
const FILLERS: i32 = 19;
/// Number of terrain ids (`0..TERRAIN_IDS`).
pub const TERRAIN_IDS: usize = 613;
/// Records of `ground.imb`; the ocean records follow (`record - GROUND_RECORDS` = ocean variant).
pub const GROUND_RECORDS: u16 = 221;

/// Per class: sizes of the four own id groups (`0x540aa0 + 0x64`): `n0` plain variants, `n1` rare
/// variants (picked 1 time in 40), `n2` blend pieces (corner, side), `n3` shore pieces (class 0)
/// or crop stages (classes 9, 10, 11, 17).
const OWN_GROUPS: [[u8; 4]; CLASS_COUNT] = [
    [3, 0, 2, 8],
    [6, 0, 2, 0],
    [6, 0, 2, 0],
    [6, 3, 2, 0],
    [6, 3, 2, 0],
    [6, 3, 2, 0],
    [6, 3, 2, 0],
    [6, 4, 2, 0],
    [5, 0, 2, 0],
    [5, 0, 0, 12],
    [5, 0, 0, 12],
    [5, 0, 0, 12],
    [8, 0, 2, 0],
    [4, 0, 2, 0],
    [4, 0, 2, 0],
    [4, 0, 2, 0],
    [4, 0, 2, 0],
    [5, 0, 0, 12],
    [4, 0, 0, 0],
];

/// Pairs of classes with a three-piece soil transition (`0x540aa0 + 0x74`, `[a][b] = 3`).
const TRANSITION_PAIRS: [(u8, u8); 9] = [
    (3, 4),
    (3, 5),
    (3, 6),
    (3, 7),
    (4, 5),
    (4, 6),
    (5, 6),
    (5, 7),
    (6, 7),
];
const PIECES_PER_PAIR: i32 = 3;

/// Cost multiplier of building track on a class (`0x540aa0 + 0`, `terrain-track.md` 2.3).
pub const CLASS_COST_MULTIPLIER: [f32; CLASS_COUNT] = [
    1.25, 1.25, 0.8, 1.0, 1.0, 1.0, 1.0, 1.5, 2.0, 1.0, 1.0, 1.0, 1.5, 1.25, 1.2, 1.35, 1.3, 2.0,
    1.0,
];

/// Class of an id of the old file versions (`< 0x233D`, table `0x552b00`): run lengths of classes
/// `0..=13`; 13 is water (the loader maps it to [`WATER_CLASS`]). Ids past the table are class 0.
const OLD_CLASS_RUNS: [u16; 14] = [27, 22, 22, 37, 34, 37, 37, 35, 21, 31, 31, 31, 24, 18];

/// Class of an old-format terrain id (`ground << 2 | var >> 6` of a `0x233C` file).
pub fn old_class_of_id(id: u16) -> u8 {
    let mut left = id;
    for (class, &n) in OLD_CLASS_RUNS.iter().enumerate() {
        if left < n {
            return if class == 13 {
                WATER_CLASS
            } else {
                class as u8
            };
        }
        left -= n;
    }
    0
}

/// Ids and records of every class, derived from [`OWN_GROUPS`] and [`TRANSITION_PAIRS`].
#[derive(Debug)]
pub struct Tables {
    /// Per class `[S0, S1, S2, S3, S4]`: first id of each own group and the end of the own ids;
    /// `S4..S4 + 19` are the fillers, the transitions follow.
    pub start: [[i32; 5]; CLASS_COUNT],
    /// `[class][other]`: first transition id of `class` against `other` (0 = none).
    pub transition_start: [[i32; CLASS_COUNT]; CLASS_COUNT],
    pub class_of: [u8; TERRAIN_IDS],
    pub record_of: [u16; TERRAIN_IDS],
    /// `ground.imb` record of the first own id of each class.
    pub first_record: [u16; CLASS_COUNT],
}

fn pair_count(a: usize, b: usize) -> i32 {
    if a != b
        && TRANSITION_PAIRS
            .iter()
            .any(|&(x, y)| (x as usize, y as usize) == (a.min(b), a.max(b)))
    {
        PIECES_PER_PAIR
    } else {
        0
    }
}

impl Tables {
    fn build() -> Tables {
        let mut t = Tables {
            start: [[0; 5]; CLASS_COUNT],
            transition_start: [[0; CLASS_COUNT]; CLASS_COUNT],
            class_of: [0; TERRAIN_IDS],
            record_of: [0; TERRAIN_IDS],
            first_record: [0; CLASS_COUNT],
        };
        // Ids: classes one after the other, each own ids + fillers + transitions.
        let mut id = 0i32;
        let mut ends = [0i32; CLASS_COUNT];
        for c in 0..CLASS_COUNT {
            let n = OWN_GROUPS[c].map(i32::from);
            let s = &mut t.start[c];
            s[0] = id;
            s[1] = s[0] + n[0];
            s[2] = s[1] + n[1];
            s[3] = s[2] + n[2];
            s[4] = s[3] + n[3];
            let mut at = s[4] + FILLERS;
            for j in 0..CLASS_COUNT {
                let k = pair_count(c, j);
                if k != 0 {
                    t.transition_start[c][j] = at;
                    at += k;
                }
            }
            ends[c] = at;
            id = at;
        }
        assert_eq!(id as usize, TERRAIN_IDS);
        // Records: own pieces of a class, then the pieces of the pairs it opens (c < j).
        let mut rec = 0i32;
        let mut pair_record = [[0i32; CLASS_COUNT]; CLASS_COUNT];
        for c in 0..CLASS_COUNT {
            t.first_record[c] = rec as u16;
            rec += OWN_GROUPS[c].iter().map(|&n| i32::from(n)).sum::<i32>();
            for (j, slot) in pair_record[c].iter_mut().enumerate().skip(c + 1) {
                if pair_count(c, j) != 0 {
                    *slot = rec;
                    rec += PIECES_PER_PAIR;
                }
            }
        }
        for c in 0..CLASS_COUNT {
            let own = t.start[c][4] - t.start[c][0];
            for i in 0..own {
                let id = (t.start[c][0] + i) as usize;
                t.class_of[id] = c as u8;
                t.record_of[id] = (i32::from(t.first_record[c]) + i) as u16;
            }
            for j in 0..CLASS_COUNT {
                let id = (t.start[c][4] + j as i32) as usize;
                t.class_of[id] = c as u8;
                t.record_of[id] = t.first_record[j];
                let k = pair_count(c, j);
                for i in 0..k {
                    let id = (t.transition_start[c][j] + i) as usize;
                    t.class_of[id] = c as u8;
                    t.record_of[id] = (pair_record[c.min(j)][c.max(j)] + i) as u16;
                }
            }
        }
        t
    }

    /// Class of a terrain id; ids past the table are class 0 (the original's array is zero there).
    pub fn class_of_id(&self, id: u16) -> u8 {
        self.class_of.get(id as usize).copied().unwrap_or(0)
    }

    /// `ground.imb` record of a terrain id; records `>= GROUND_RECORDS` are ocean variants.
    pub fn record_of_id(&self, id: u16) -> u16 {
        self.record_of.get(id as usize).copied().unwrap_or(0)
    }
}

/// The shared tables.
pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(Tables::build)
}

/// The game's linear congruential generator (`rng_range` `0x4f0660`, state at `0x55d2c0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameRng(pub u32);

impl GameRng {
    fn next15(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        (self.0 >> 16) & 0x7fff
    }

    /// `lo + output % (hi - lo + 1)`; `hi == lo` does not advance the state.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let d = hi.wrapping_sub(lo);
        if d > 0x7fff {
            let a = self.range(0, 0x7fff);
            let b = self.range(0, 0x7fff);
            return ((b as u32 | (a as u32) << 15) % (d as u32 + 1)) as i32 + lo;
        }
        if hi == lo {
            return hi;
        }
        (i64::from(self.next15()) % (i64::from(d) + 1)) as i32 + lo
    }
}

/// `rng_hash_range` (`0x4f06f0`): a stateless hash of `seed` into `lo..=hi`.
pub fn hash_range(seed: i32, lo: i32, hi: i32) -> i32 {
    let d = hi.wrapping_sub(lo);
    if d > 0x7fff {
        let a = hash_range(seed, 0, 0x7fff);
        let b = hash_range(seed.wrapping_add(7), 0, 0x7fff);
        return ((b as u32 | (a as u32) << 15) % (d as u32 + 1)) as i32 + lo;
    }
    if hi == lo {
        return hi;
    }
    let h = (seed as u32).wrapping_mul(0xC1C6_4E6D).wrapping_add(0x3039);
    (i64::from((h >> 16) & 0x7fff) % (i64::from(d) + 1)) as i32 + lo
}

/// Overlay layer of a node (low 3 bits of its word).
pub const LAYER_GROUND: u8 = 0;
pub const LAYER_TERRITORY: u8 = 1;
pub const LAYER_RIVER: u8 = 2;
pub const LAYER_ROAD: u8 = 3;

/// Bank of a node (bits 3-5 of its word): index into the original's table at `0x580fb0`.
pub const BANK_GROUND: u8 = 0;
pub const BANK_TERRITORY: u8 = 1;
pub const BANK_RIVER: u8 = 2;
pub const BANK_ROAD: u8 = 3;

/// One overlay of a tile (8 bytes in memory at the tile's `+10` list): drawn over the tile's base
/// texture, transparent where the texture's palette index is 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    /// `record << 6 | bank << 3 | layer`.
    pub word: u16,
    /// Texture quarter turns (before the view rotation is added).
    pub turns: u8,
    /// Signed shade level added to the tile's.
    pub shade: i8,
}

impl Node {
    pub fn record(&self) -> u16 {
        self.word >> 6
    }

    pub fn bank(&self) -> u8 {
        ((self.word >> 3) & 7) as u8
    }

    pub fn layer(&self) -> u8 {
        (self.word & 7) as u8
    }
}

/// A tile as the terrain passes see it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cell {
    /// `terrain id << 6 | low bits` (bytes 0-1 of the tile).
    pub v: u16,
    /// Tile byte 2 (bit 6 is rewritten by the passes).
    pub flags2: u8,
    /// Tile byte 3 (bit 5 = road).
    pub flags3: u8,
    /// Tile byte 6 (bit 7 is rewritten by the passes).
    pub flags6: u8,
    /// Overlays, first = drawn first = on top (the base texture is drawn last, under all).
    pub nodes: Vec<Node>,
}

impl Cell {
    pub fn id(&self) -> u16 {
        self.v >> 6
    }

    /// Texture quarter turns of the base (`tile[0] >> 4 & 3`).
    pub fn turns(&self) -> u8 {
        ((self.v >> 4) & 3) as u8
    }

    fn insert(&mut self, node: Node) {
        // Before the first node whose layer is <= the new one's (the list is sorted by layer,
        // highest first).
        let at = self
            .nodes
            .iter()
            .position(|n| n.layer() <= node.layer())
            .unwrap_or(self.nodes.len());
        self.nodes.insert(at, node);
    }
}

/// The tiles of a map, with the terrain passes of the loader.
#[derive(Debug, Clone)]
pub struct TerrainGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
}

/// Neighbour offsets of the 8-neighbour scan (`0x537b48`, `0x537b68`): N, NE, E, SE, S, SW, W, NW.
const DX: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const DY: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

impl TerrainGrid {
    pub fn cell(&self, x: usize, y: usize) -> &Cell {
        &self.cells[y * self.width + x]
    }

    pub fn cell_mut(&mut self, x: usize, y: usize) -> &mut Cell {
        &mut self.cells[y * self.width + x]
    }

    /// A grid of version-`0x2348` tiles: `(byte 0, byte 1)` are final terrain ids.
    pub fn from_new_ids(
        width: usize,
        height: usize,
        tile: impl Fn(usize, usize) -> TileBytes,
    ) -> TerrainGrid {
        let mut cells = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let t = tile(x, y);
                cells.push(Cell {
                    v: u16::from(t.b1) << 8 | u16::from(t.b0),
                    flags2: t.b2,
                    flags3: t.b3,
                    flags6: t.b6,
                    nodes: Vec::new(),
                });
            }
        }
        TerrainGrid {
            width,
            height,
            cells,
        }
    }

    /// A grid of version-`0x233C` (or older) tiles: the loader maps the old id to a class
    /// (`0x552b00`), starts the tile at the class's first id, then `0x42a3e0` picks a random
    /// variant of the class (the rare group one time in 40) and clears the turn bits.
    pub fn from_old_ids(
        width: usize,
        height: usize,
        tile: impl Fn(usize, usize) -> TileBytes,
        rng: &mut GameRng,
    ) -> TerrainGrid {
        let tb = tables();
        let mut g = TerrainGrid::from_new_ids(width, height, &tile);
        for y in 0..height {
            for x in 0..width {
                let c = g.cell_mut(x, y);
                let old_id = c.v >> 6;
                let class = usize::from(old_class_of_id(old_id));
                let start = tb.start[class];
                c.v = (start[0] as u16) << 6 | (c.v & 0x3f);
            }
        }
        for y in 0..height {
            for x in 0..width {
                g.randomize_variant(x, y, rng);
            }
        }
        g
    }

    /// `0x42a3e0` for one tile with "force" set: new random variant of the tile's class.
    fn randomize_variant(&mut self, x: usize, y: usize, rng: &mut GameRng) {
        let tb = tables();
        let c = self.cell_mut(x, y);
        let class = usize::from(tb.class_of_id(c.id()));
        let n = OWN_GROUPS[class].map(i32::from);
        let s = tb.start[class];
        let pick = (y as i32)
            .wrapping_mul(0x41c6_4e6d)
            .wrapping_sub((x as i32).wrapping_mul(0x39b1_9300));
        let (lo, hi) = if n[1] != 0 && pick % 0x28 == 1 {
            (s[1], s[1] + n[1] - 1)
        } else {
            (s[0], s[0] + n[0] - 1)
        };
        let id = rng.range(lo, hi);
        c.v = (id as u16) << 6 | (c.v & 0x3f);
        if c.v & 0x30 != 0 {
            c.v &= 0xffcf;
        }
    }

    fn class_at(&self, x: i32, y: i32) -> u8 {
        tables().class_of_id(self.cells[y as usize * self.width + x as usize].id())
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    /// `0x4258e0`: refreshes the derived flags of a tile after its overlays or id changed.
    fn update_flags(&mut self, x: usize, y: usize) {
        let tb = tables();
        let c = self.cell_mut(x, y);
        c.flags6 &= 0x7f;
        if !c.nodes.is_empty() {
            c.flags6 |= 0x80;
        }
        let bit = (c.flags6 >> 1) & 0x40;
        c.flags2 = bit | (c.flags2 & 0xbf);
        if bit == 0 {
            let class = usize::from(tb.class_of_id(c.id()));
            let id = i32::from(c.id());
            if id < tb.start[class][0] || tb.start[class][4] < id {
                c.flags2 |= 0x40;
            }
        }
    }

    /// `0x4259e0`: removes the nodes of `layer` (all with `all`); a tile whose base id is outside
    /// its class's own ids gets a fresh plain variant. Returns whether nodes were removed.
    fn clear_layer(&mut self, x: usize, y: usize, layer: u8, all: bool, rng: &mut GameRng) -> bool {
        let tb = tables();
        let c = self.cell_mut(x, y);
        let before = c.nodes.len();
        c.nodes.retain(|n| !(n.layer() == layer || all));
        let removed = c.nodes.len() != before;
        let id = i32::from(c.id());
        let class = usize::from(tb.class_of_id(c.id()));
        let s = tb.start[class];
        if (layer == 0 || all) && (id < s[0] || s[4] <= id) {
            let r = rng.range(0, i32::from(OWN_GROUPS[class][0]) - 1);
            c.v = ((s[0] + r) as u16).wrapping_mul(0x40) ^ (c.v & 0x3f);
            return false;
        }
        if removed {
            self.update_flags(x, y);
        }
        removed
    }

    fn push_node(&mut self, x: usize, y: usize, node: Node) {
        self.cell_mut(x, y).insert(node);
        self.update_flags(x, y);
    }

    /// The loader's terrain passes in order: rivers, roads, ground transitions.
    pub fn build_overlays(&mut self, rng: &mut GameRng) {
        self.build_rivers(rng);
        self.build_roads(rng);
        self.build_ground(rng);
    }

    /// `0x42a7d0` as the loader calls it, `(0, 0, W - 1, H - 1)`: the arguments are a position and
    /// a size, so the last column and row are left out.
    pub fn build_ground(&mut self, rng: &mut GameRng) {
        let (w, h) = (self.width - 1, self.height - 1);
        self.build_ground_rect(0, 0, w, h, rng);
    }

    /// `0x42a7d0(x, y, w, h)`: soil blends and shores for a rectangle of tiles.
    pub fn build_ground_rect(
        &mut self,
        x0: usize,
        y0: usize,
        w: usize,
        h: usize,
        rng: &mut GameRng,
    ) {
        let tb = tables();
        let x1 = (x0 + w).saturating_sub(1).min(self.width - 1) as i32;
        let y1 = (y0 + h).saturating_sub(1).min(self.height - 1) as i32;
        for y in y0 as i32..=y1 {
            for x in x0 as i32..=x1 {
                let (ux, uy) = (x as usize, y as usize);
                self.clear_layer(ux, uy, 0, false, rng);
                let cls = usize::from(tb.class_of_id(self.cell(ux, uy).id()));
                // Filler ids cannot survive clear_layer; the original still checks.
                {
                    let id = i32::from(self.cell(ux, uy).id());
                    if tb.start[cls][4] <= id && id < tb.start[cls][4] + FILLERS {
                        let r = rng.range(0, i32::from(OWN_GROUPS[cls][0]) - 1);
                        let c = self.cell_mut(ux, uy);
                        c.v = ((tb.start[cls][0] + r) as u16).wrapping_mul(0x40) ^ (c.v & 0xf);
                    }
                }
                let mut st = TileState::default();
                for j in 0..CLASS_COUNT {
                    self.ground_class(x, y, cls, j, &mut st);
                }
            }
        }
    }

    /// The body of the class loop of `0x42a7d0` for neighbour class `j`.
    fn ground_class(&mut self, x: i32, y: i32, cls: usize, j: usize, st: &mut TileState) {
        let tb = tables();
        let (ux, uy) = (x as usize, y as usize);
        let is_field = |c: usize| matches!(c, 9..=11 | 17);
        if !(j != cls && j != 18 && (j != 0 || cls == 18) && !is_field(cls)) {
            return;
        }
        let mut mask: u8 = 0;
        if !is_field(j) {
            let owner = if cls == 18 { j } else { cls };
            if owner != 0 {
                // Three-neighbour scan: N, NW, W.
                for k in 0..3 {
                    let (nx, ny) = match k {
                        0 => (x, y - 1),
                        1 => (x - 1, y - 1),
                        _ => (x - 1, y),
                    };
                    if nx >= 0 && ny >= 0 && usize::from(self.class_at(nx, ny)) == j {
                        mask |= 1 << k;
                    }
                }
                if mask != 0 {
                    self.blend(x, y, j, mask, st);
                }
                return;
            }
        }
        // Eight-neighbour scan.
        for k in 0..8 {
            let (nx, ny) = (x + DX[k], y + DY[k]);
            if self.in_bounds(nx, ny) && usize::from(self.class_at(nx, ny)) == j {
                mask |= 1 << k;
            }
        }
        loop {
            if mask == 0 {
                break;
            }
            let (ty, rot) = run_piece(&mut mask);
            let Some(ty) = ty else { break };
            let mut emit = true;
            let word: u32;
            let r = i32::from(rot);
            if cls == 0 {
                if is_field(j) {
                    emit = ty < 3;
                    word = ((tb.start[j][3] + r * 3 + ty as i32) as u32 * 0x40) & 0xffff;
                } else if ty == 2 {
                    let c = self.cell_mut(ux, uy);
                    c.v = ((tb.start[0][4] + j as i32) as u16).wrapping_mul(0x40) ^ (c.v & 0x3f);
                    word = ((tb.start[0][2] as u32) << 6 & 0xffff)
                        | (((r + 1) as u32 & 0xff) << 16 & 0x3ffff);
                } else {
                    let id16 = ((tb.start[j][2] + ty as i32) * 0x40) as u16;
                    let mut wv = u32::from(id16) | ((r as u32 & 0xff) << 16);
                    if ty == 1 {
                        wv = (u32::from(id16) | (((r - 1) as u32 & 0xff) << 16)) & 0x3ffff;
                    }
                    word = wv;
                }
            } else if is_field(j) {
                emit = ty < 3;
                word = ((tb.start[j][3] + r * 3 + ty as i32) as u32 * 0x40) & 0xffff;
            } else {
                let mut t = ty as i32;
                if t < 3 && ((y * 0x6d + x * 0x17) & 0xff) < 0x80 {
                    t += 5;
                }
                word = u32::from(((tb.start[j][3] + t) * 0x40) as u16) | ((r as u32 & 0xff) << 16);
            }
            // id -> record.
            let id = ((word & 0xffff) >> 6) as u16;
            let rec = tb.record_of_id(id);
            let turns = ((word >> 16) & 0xff) as u8;
            if emit {
                self.push_node(
                    ux,
                    uy,
                    Node {
                        word: rec << 6,
                        turns,
                        shade: 0,
                    },
                );
            }
        }
    }

    /// `LAB_0042aa20` and `LAB_0042ab9c` of `0x42a7d0`: three-neighbour blends.
    fn blend(&mut self, x: i32, y: i32, j: usize, mut mask: u8, st: &mut TileState) {
        let tb = tables();
        let (ux, uy) = (x as usize, y as usize);
        loop {
            // LAB_0042aa20
            let (mut rot9, mut ty, mut rot4): (u32, u32, u32) = (0, 0, 0);
            match mask {
                1 | 5 => {}
                2 => {
                    rot9 = 1;
                    rot4 = 1;
                }
                3 => ty = 1,
                4 => {
                    rot9 = 2;
                    rot4 = 2;
                }
                6 => {
                    rot9 = 1;
                    ty = 1;
                    rot4 = 1;
                }
                _ => ty = 2,
            }
            let v = self.cell(ux, uy).v;
            let id = i32::from(v >> 6);
            let cur = usize::from(tb.class_of_id(v >> 6));
            let s = tb.start[cur];
            let outside = id < s[0] || s[4] < id;
            let has_nodes = !self.cell(ux, uy).nodes.is_empty();
            let mut make_node_rot = rot4;
            let mut goto_node = false;
            if tb.transition_start[cur][j] == 0 || has_nodes || st.second {
                goto_node = true;
            } else {
                st.second = true;
                let mut u7 = ty;
                let mut u10 = rot9;
                if ty == 2 {
                    u10 = (rot9 + 1) & 3;
                    u7 = 0;
                    rot4 = u10;
                }
                if j < cur {
                    u10 = u10.wrapping_sub(2) & 3;
                    rot4 = u10;
                    if u7 == 0 {
                        u10 = u10.wrapping_sub(2) & 3;
                        u7 = 2;
                        rot4 = u10;
                    }
                }
                if ty == 2 {
                    u7 = 2 - u7;
                    u10 = u10.wrapping_sub(2) & 3;
                    rot4 = u10;
                }
                u7 += tb.transition_start[cur][j] as u32;
                st.saved_ty = ty;
                st.saved_rot = rot9;
                st.saved_class = j;
                if !outside {
                    let c = self.cell_mut(ux, uy);
                    let old = c.v;
                    c.v = (rot4 as u16) << 4 | (old & 0xf) | ((u7 as u16) << 6);
                    self.update_flags(ux, uy);
                } else if u32::from(self.cell(ux, uy).v >> 6) != u7
                    && u32::from((self.cell(ux, uy).v & 0x30) >> 4) != u10
                {
                    goto_node = true;
                    make_node_rot = rot9;
                }
            }
            if goto_node {
                // LAB_0042ab9c
                let mut ty_now = ty;
                let mut rot_now = make_node_rot;
                let mut rot9_now = rot9;
                let mut jj = j;
                loop {
                    let (word_rec, turns): (u16, u8);
                    if ty_now == 2 {
                        let c = self.cell_mut(ux, uy);
                        c.v = ((s[4] + jj as i32) as u16).wrapping_mul(0x40) ^ (c.v & 0x3f);
                        let id = (s[2] as u16) << 6;
                        word_rec = tb.record_of_id(id >> 6);
                        turns = (rot_now.wrapping_sub(1) & 3) as u8;
                    } else {
                        let id = (tb.start[jj][2] as u32 + ty_now) as u16;
                        word_rec = tb.record_of_id(id);
                        turns = rot9_now as u8;
                    }
                    self.push_node(
                        ux,
                        uy,
                        Node {
                            word: word_rec << 6,
                            turns,
                            shade: 0,
                        },
                    );
                    if !st.second {
                        break;
                    }
                    ty_now = st.saved_ty;
                    rot_now = st.saved_rot;
                    rot9_now = st.saved_rot;
                    st.second = false;
                    jj = st.saved_class;
                }
            }
            // LAB_0042ad36
            if mask != 5 {
                break;
            }
            mask = 4;
        }
    }
}

/// State kept while the class loop of one tile runs (`local_30`, `local_18`, `local_14`, `local_c`).
#[derive(Debug, Default, Clone, Copy)]
struct TileState {
    second: bool,
    saved_ty: u32,
    saved_rot: u32,
    saved_class: usize,
}

/// The raw bytes of a tile that the passes read.
#[derive(Debug, Clone, Copy, Default)]
pub struct TileBytes {
    pub b0: u8,
    pub b1: u8,
    pub b2: u8,
    pub b3: u8,
    pub b6: u8,
}

/// `0x42a640`: takes the next run of set bits of an 8-neighbour mask. Returns `(type, rotation)`
/// (`type` 0 = one corner, 1 = a side, 2 = three sides, 3 = all but one, 4 = fallback; `None`
/// when the function leaves its output untouched) and clears the run from `mask`.
fn run_piece(mask: &mut u8) -> (Option<u32>, u8) {
    const TYPES: [u8; 18] = [
        0xff, 0xff, 0xff, 0, 0xff, 0xff, 0xff, 1, 0xff, 0xff, 0xff, 2, 0xff, 0xff, 0xff, 3, 4, 4,
    ];
    // Odd positions with a set orthogonal neighbour are set.
    let mut i = 1u32;
    while i < 8 {
        let m = *mask;
        if m & (1 << ((i - 1) & 7)) != 0 || m & (1 << ((i + 1) & 7)) != 0 {
            *mask |= 1 << i;
        }
        i += 2;
    }
    let m = u32::from(*mask);
    for start in 0..7u32 {
        if m & (1 << start) == 0 {
            let mut a = 1i32;
            let mut b = start as i32;
            while a < 8 {
                if m & (1 << ((b + 1) & 7)) != 0 {
                    let mut run = 1i32;
                    let mut p = b + 2;
                    while run < 8 {
                        if m & (1 << (p & 7)) == 0 {
                            let pos = (a + start as i32) as u32;
                            let ty = TYPES[(run * 2) as usize + (pos & 1) as usize];
                            let rot = (-(((pos >> 1) & 3) as i32) & 3) as u8;
                            if run >= 1 {
                                let mut bits = *mask;
                                let mut q = pos;
                                let mut r = run;
                                while r > 0 {
                                    bits &= !(1u8 << (q & 7));
                                    q += 1;
                                    r -= 1;
                                }
                                *mask = bits;
                            }
                            return (Some(u32::from(ty)), rot);
                        }
                        run += 1;
                        p += 1;
                    }
                }
                a += 1;
                b += 1;
            }
        }
    }
    *mask = 0;
    (Some(4), 0)
}

/// River pieces `(piece, turns)` by `(flow direction * 16 + neighbour mask) * 2` (`0x53a25c`, 255 =
/// none), followed in memory by [`RIVER_RECORDS`]; both are indexed as one byte array because the
/// original reads `piece * 6` with `piece = -1` into the end of the first table.
const RIVER_PIECES: [u8; 128] = [
    6, 2, 6, 2, 255, 0, 2, 0, 255, 0, 0, 2, 255, 0, 5, 0, 255, 0, 1, 1, 255, 0, 4, 1, 255, 0, 3, 2,
    255, 0, 4, 1, 6, 1, 255, 0, 6, 1, 1, 0, 255, 0, 255, 0, 2, 3, 4, 0, 255, 0, 255, 0, 0, 1, 3, 1,
    255, 0, 255, 0, 5, 3, 4, 0, 6, 0, 255, 0, 255, 0, 255, 0, 6, 0, 0, 0, 1, 3, 3, 0, 255, 0, 255,
    0, 255, 0, 255, 0, 2, 2, 5, 2, 4, 3, 4, 3, 6, 3, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
    0, 255, 0, 6, 3, 2, 1, 0, 3, 5, 1, 1, 2, 4, 2, 3, 3, 4, 2,
];
/// `river.imb` records, 6 hash variants per piece (`0x53a2dc`).
const RIVER_RECORDS: [u8; 72] = [
    0, 9, 12, 15, 16, 17, 1, 10, 13, 1, 10, 13, 2, 11, 14, 2, 11, 14, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4,
    4, 4, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 8, 8, 0, 0, 0, 8, 9,
    10, 11, 12, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1,
];
/// Road piece class by neighbour mask `(piece, turns)` (`0x53a13c`).
const ROAD_PIECE_BY_MASK: [u8; 32] = [
    4, 2, 4, 2, 4, 1, 1, 0, 4, 0, 0, 0, 1, 3, 2, 0, 4, 3, 1, 1, 0, 1, 2, 1, 1, 2, 2, 2, 2, 3, 3, 0,
];
/// Road piece for class 1 by `mask * 16 + links` (`0x53a15c`).
const ROAD_PIECE_BY_LINKS: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 6, 7, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 6, 1, 7, 1, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 7, 1, 1, 1, 1, 1, 1, 6, 5, 1, 1, 1, 1, 1, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 1, 1, 6, 1, 1, 1, 7, 1, 1, 1, 5, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// `road.imb` records, 9 hash variants per piece (`0x53a314`).
const ROAD_RECORDS: [u8; 72] = [
    0, 8, 9, 10, 11, 12, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 13, 14, 2, 13, 14, 2, 2, 2, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7,
];
/// Road records that continue into a neighbour (`0x53a35d`, indexed by record; `[0]` is the entry
/// for "no road node" = -1).
const ROAD_RECORD_LINKS: [u8; 16] = [0, 0, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 37, 115, 37, 115];

fn river_byte(i: i32) -> u8 {
    // The original reads past the front of the record table into the end of the piece table.
    if i < 0 {
        RIVER_PIECES[(128 + i) as usize]
    } else {
        RIVER_RECORDS.get(i as usize).copied().unwrap_or(0)
    }
}

impl TerrainGrid {
    fn has_river(&self, x: i32, y: i32) -> bool {
        self.cells[y as usize * self.width + x as usize].v & 1 != 0
    }

    /// `0x4294d0`: river overlays for the whole map (the loader passes `(0, 0, W, H)`, clamped).
    pub fn build_rivers(&mut self, rng: &mut GameRng) {
        let (w, h) = (self.width as i32, self.height as i32);
        for y in 0..h {
            for x in 0..w {
                let (ux, uy) = (x as usize, y as usize);
                self.clear_layer(ux, uy, LAYER_RIVER, false, rng);
                let v = self.cell(ux, uy).v;
                if v & 1 == 0 {
                    continue;
                }
                let mut mask = 0usize;
                if y == 0 || self.has_river(x, y - 1) {
                    mask |= 1;
                }
                if x == w - 1 || self.has_river(x + 1, y) {
                    mask |= 2;
                }
                if y == h - 1 || self.has_river(x, y + 1) {
                    mask |= 4;
                }
                if x == 0 || self.has_river(x - 1, y) {
                    mask |= 8;
                }
                let dir = ((v >> 1) & 3) as usize;
                let variant = hash_range(x * 7 + y * 0x13, 0, 5);
                let base: u16 = u16::from(BANK_RIVER) << 3 | u16::from(LAYER_RIVER);
                let tb = tables();
                let word: u16;
                let turns: u8;
                if tb.class_of_id(v >> 6) == WATER_CLASS {
                    // River mouth: look at the tile the river flows into.
                    let (mut nx, mut ny) = (x, y);
                    if dir == 0 {
                        ny = y - 1;
                    } else if dir == 1 {
                        nx = x + 1;
                    } else if dir == 2 {
                        ny = y + 1;
                    } else {
                        nx = x - 1;
                    }
                    let mut w16 = base;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h - 1 {
                        let into_water =
                            tb.class_of_id(self.cell(nx as usize, ny as usize).id()) == WATER_CLASS;
                        w16 = (base & 0x3f) | if into_water { 0x1c0 } else { 0x200 };
                    }
                    let d2 = (dir + 2) & 3;
                    // Record 11 of the ground layer becomes record 6.
                    for n in &mut self.cell_mut(ux, uy).nodes {
                        if n.layer() == LAYER_GROUND && n.record() == 11 {
                            n.word = (n.word & 0x3f) | (6 << 6);
                        }
                    }
                    word = w16;
                    turns = RIVER_PIECES[(d2 * 16) * 2 + 1];
                } else {
                    let mut d = dir;
                    if RIVER_PIECES[(d * 16 + mask) * 2] == 255 {
                        let d2 = (d + 2) & 3;
                        if RIVER_PIECES[(d2 * 16 + mask) * 2] != 255 {
                            d = d2;
                        }
                    }
                    let piece = RIVER_PIECES[(d * 16 + mask) * 2] as i8 as i32;
                    let rec = river_byte(piece * 6 + variant);
                    word = u16::from(rec) << 6 | (base & 0x3f);
                    turns = RIVER_PIECES[(d * 16 + mask) * 2 + 1];
                }
                self.push_node(
                    ux,
                    uy,
                    Node {
                        word,
                        turns,
                        shade: 0,
                    },
                );
            }
        }
    }

    fn road_node_links(&self, x: i32, y: i32) -> bool {
        let rec = self
            .cell(x as usize, y as usize)
            .nodes
            .iter()
            .find(|n| n.layer() == LAYER_ROAD)
            .map_or(-1, |n| i32::from(n.record()));
        ROAD_RECORD_LINKS
            .get((rec + 1) as usize)
            .is_some_and(|&b| b != 0)
    }

    /// `0x428fe0`: road overlays (tile byte 3 bit 5) for the whole map, two passes.
    pub fn build_roads(&mut self, rng: &mut GameRng) {
        let (w, h) = (self.width as i32, self.height as i32);
        for pass in 0..2 {
            for y in 0..h {
                for x in 0..w {
                    let (ux, uy) = (x as usize, y as usize);
                    if pass == 1 && !self.road_node_links(x, y) {
                        continue;
                    }
                    self.clear_layer(ux, uy, LAYER_ROAD, false, rng);
                    if self.cell(ux, uy).flags3 & 0x20 == 0 {
                        continue;
                    }
                    let road = |g: &TerrainGrid, x: i32, y: i32| {
                        g.cells[y as usize * g.width + x as usize].flags3 & 0x20 != 0
                    };
                    let mut mask = 0usize;
                    let mut links = 0usize;
                    if y == 0 || road(self, x, y - 1) {
                        mask |= 1;
                        if y == 0 || self.road_node_links(x, y - 1) {
                            links |= 1;
                        }
                    }
                    if x == w - 1 || road(self, x + 1, y) {
                        mask |= 2;
                        if x == w - 1 || self.road_node_links(x + 1, y) {
                            links |= 2;
                        }
                    }
                    if y == h - 1 || road(self, x, y + 1) {
                        mask |= 4;
                        if y == h - 1 || self.road_node_links(x, y + 1) {
                            links |= 4;
                        }
                    }
                    if x == 0 || road(self, x - 1, y) {
                        mask |= 8;
                        if x == 0 || self.road_node_links(x - 1, y) {
                            links |= 8;
                        }
                    }
                    let variant = hash_range(x * 7 + y * 0x13, 0, 8) as usize;
                    let mut piece = ROAD_PIECE_BY_MASK[mask * 2] as usize;
                    if piece == 1 {
                        piece = ROAD_PIECE_BY_LINKS[mask * 16 + links] as usize;
                    }
                    let rec = ROAD_RECORDS[variant + piece * 9];
                    self.push_node(
                        ux,
                        uy,
                        Node {
                            word: u16::from(rec) << 6
                                | u16::from(BANK_ROAD) << 3
                                | u16::from(LAYER_ROAD),
                            turns: ROAD_PIECE_BY_MASK[mask * 2 + 1],
                            shade: 0,
                        },
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_ranges_match_the_dump_of_the_original() {
        let t = tables();
        // First id of each class, from the table dumped out of the running original.
        let starts = [
            0, 32, 59, 86, 128, 167, 209, 251, 291, 317, 353, 389, 425, 454, 479, 504, 529, 554,
            590,
        ];
        for (c, &s) in starts.iter().enumerate() {
            assert_eq!(t.start[c][0], s, "class {c}");
            assert_eq!(t.class_of_id(s as u16), c as u8);
        }
        assert_eq!(t.class_of_id(612), 18);
        assert_eq!(t.class_of_id(613), 0);
        // Sizes: 613 ids and 225 records (221 of ground.imb plus 4 ocean variants).
        assert_eq!(*t.record_of.iter().max().unwrap(), 224);
        assert_eq!(t.first_record[18], GROUND_RECORDS);
    }

    #[test]
    fn records_follow_the_dumped_layout() {
        let t = tables();
        // Beach: ids 0..13 are records 0..13; its fillers point at the first record of each class.
        assert_eq!(t.record_of_id(5), 5);
        assert_eq!(t.record_of_id(13 + 6), 86); // beach on lush grass
        // Dirt (class 3) against dry grass (class 4): ids 116..119 and (from class 4's side) 158..161
        // share records 40..43.
        assert_eq!(t.transition_start[3][4], 116);
        assert_eq!(t.transition_start[4][3], 158);
        assert_eq!(t.record_of_id(116), 40);
        assert_eq!(t.record_of_id(160), 42);
        // Swamp against lush grass is the last pair: records 97..100.
        assert_eq!(t.record_of_id(t.transition_start[6][7] as u16), 97);
    }

    #[test]
    fn old_ids_map_to_classes_in_runs() {
        assert_eq!(old_class_of_id(0), 0);
        assert_eq!(old_class_of_id(26), 0);
        assert_eq!(old_class_of_id(27), 1);
        assert_eq!(old_class_of_id(71), 3);
        // Old water ids 389..=406 (ground 0x61 var 1.. up to 0x65) are class 13 -> water.
        assert_eq!(old_class_of_id(389), WATER_CLASS);
        assert_eq!(old_class_of_id(0x188), WATER_CLASS);
        // Old id 388 (ground 0x61, var 0) is still the last mountain id.
        assert_eq!(old_class_of_id(388), 12);
    }

    #[test]
    fn game_rng_matches_the_lcg() {
        let mut r = GameRng(1234);
        // state = 1234 * 0x41c64e6d + 0x3039; output (state >> 16) & 0x7fff
        let state = 1234u32.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        let want = ((state >> 16) & 0x7fff) % 6;
        assert_eq!(r.range(0, 5) as u32, want);
        assert_eq!(r.0, state);
        // hi == lo does not advance.
        assert_eq!(r.range(3, 3), 3);
        assert_eq!(r.0, state);
    }

    #[test]
    fn run_piece_takes_one_run_at_a_time() {
        // A single orthogonal neighbour (north) lights its two diagonal neighbours: a 3-run.
        let mut m = 1u8;
        let (ty, rot) = run_piece(&mut m);
        assert_eq!(ty, Some(1));
        assert_eq!(rot, 1); // run starts at position 7: -(7 >> 1 & 3) & 3
        assert_eq!(m & 0b1000_0011, 0);
        // A lone diagonal neighbour is a single corner.
        let mut m = 2u8;
        let (ty, _) = run_piece(&mut m);
        assert_eq!(ty, Some(0));
        assert_eq!(m, 0);
        // All eight neighbours: no gap, fallback piece 4.
        let mut m = 0xffu8;
        assert_eq!(run_piece(&mut m).0, Some(4));
        assert_eq!(m, 0);
    }

    #[test]
    fn a_grass_tile_beside_dirt_gets_a_blend() {
        // 2x2 map: dirt (class 3, id 86) at (0,0), grass (class 5, id 167) elsewhere. The tile
        // south-east of the dirt sees it in its NW neighbour and becomes a transition tile or
        // gets an overlay.
        let dirt = Cell {
            v: 86 << 6,
            ..Default::default()
        };
        let grass = Cell {
            v: 167 << 6,
            ..Default::default()
        };
        let mut g = TerrainGrid {
            width: 3,
            height: 3,
            cells: vec![grass.clone(); 9],
        };
        g.cells[0] = dirt;
        let mut rng = GameRng(5);
        g.build_ground_rect(0, 0, 3, 3, &mut rng);
        let touched = g.cell(1, 1);
        let tb = tables();
        assert!(
            i32::from(touched.id()) >= tb.start[5][4] || !touched.nodes.is_empty(),
            "{touched:?}"
        );
        // Water tiles never blend onto land: a water neighbour of land keeps the land's class.
        assert_eq!(tb.class_of_id(g.cell(2, 2).id()), 5);
    }
}
