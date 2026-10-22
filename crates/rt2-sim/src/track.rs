//! Track construction cost and station catchment (docs/spec/terrain-track.md sections 1.1, 1.4,
//! 2.2, 3.3). Both rules are proven bit-exact against the original by
//! `crates/rt2-oracle/tests/track.rs`.
//!
//! The terrain multipliers, bridge prices, economy factors, station radii, valid edge shapes and
//! building footprints are game data: [`TrackTables::load`] reads them from the executable at run
//! time (they are never stored in the repository).

use crate::exe::{Exe, ExeError};
use crate::fp::ftol;

// ---- representation (spec 1.1, 1.4) -----------------------------------------------------------

/// One of the eight directions, clockwise from north (spec 1.1). Even = orthogonal, odd =
/// diagonal; the opposite direction is `(d + 4) & 7`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dir(u8);

impl Dir {
    pub const ALL: [Dir; 8] = [
        Dir(0),
        Dir(1),
        Dir(2),
        Dir(3),
        Dir(4),
        Dir(5),
        Dir(6),
        Dir(7),
    ];
    /// `dir_dx` (`0x537b48`) and `dir_dy` (`0x537b68`); [`TrackTables::load`] checks them
    /// against the executable.
    pub const DX: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
    pub const DY: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

    pub fn new(d: u8) -> Option<Dir> {
        (d < 8).then_some(Dir(d))
    }

    pub fn index(self) -> u8 {
        self.0
    }

    pub fn dx(self) -> i32 {
        Self::DX[usize::from(self.0)]
    }

    pub fn dy(self) -> i32 {
        Self::DY[usize::from(self.0)]
    }

    pub fn is_diagonal(self) -> bool {
        self.0 & 1 == 1
    }

    pub fn opposite(self) -> Dir {
        Dir((self.0 + 4) & 7)
    }
}

/// One bit per edge of a tile (bit `d` = edge towards direction `d`), the byte `+0` of a track
/// piece.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EdgeMask(pub u8);

impl EdgeMask {
    pub fn contains(self, d: Dir) -> bool {
        self.0 & (1 << d.0) != 0
    }

    pub fn with(self, d: Dir) -> EdgeMask {
        EdgeMask(self.0 | (1 << d.0))
    }

    pub fn without(self, d: Dir) -> EdgeMask {
        EdgeMask(self.0 & !(1 << d.0))
    }

    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub fn dirs(self) -> impl Iterator<Item = Dir> {
        Dir::ALL.into_iter().filter(move |d| self.contains(*d))
    }
}

/// A placed track piece (`track_piece_pool` record, spec 1.3), the fields the cost needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrackPiece {
    pub edges: EdgeMask,
    /// `+2`: owning company.
    pub owner: u8,
    /// `+0x11`: bridge type, 0 = none.
    pub bridge_type: u8,
    /// `+0x23` bit 0: double track.
    pub double: bool,
    /// `+0x23` bit 4: electrified.
    pub electrified: bool,
}

/// The tile fields the cost reads (spec 1.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileInfo {
    /// `terrain_class_of_id[tile id]`, 0..=18 (18 = water).
    pub terrain_class: usize,
    /// Tile byte `+2`: low nibble vegetation class, `0x20` already cleared, `0x80` has a bridge.
    pub flags2: u8,
    /// Tile byte `+3`: `& 0x1f` trees, `0x40` water span.
    pub flags3: u8,
}

pub const TILE2_CLEARED: u8 = 0x20;
pub const TILE2_BRIDGE: u8 = 0x80;
pub const TILE3_SPAN: u8 = 0x40;

impl TileInfo {
    /// `tile_tree_count` (`0x416860`): vegetation class 0 -> 0; 1, 2, 4, 5, 7, 8, 9 -> the tree
    /// count; other classes -> 1.
    pub fn tree_count(&self) -> i32 {
        match self.flags2 & 0xf {
            0 => 0,
            1 | 2 | 4 | 5 | 7 | 8 | 9 => i32::from(self.flags3 & 0x1f),
            _ => 1,
        }
    }
}

// ---- game data --------------------------------------------------------------------------------

pub const ADDR_EDGE_SHAPES: u32 = 0x0053_65e8;
pub const ADDR_EDGE_SHAPES_BRIDGE: u32 = 0x0053_67e8;
pub const ADDR_DIR_DX: u32 = 0x0053_7b48;
pub const ADDR_DIR_DY: u32 = 0x0053_7b68;
pub const ADDR_BRIDGE_COST: u32 = 0x0053_ce60;
pub const ADDR_TERRAIN_CLASS: u32 = 0x0054_0aa0;
pub const TERRAIN_CLASS_STRIDE: u32 = 0xc0;
pub const TERRAIN_CLASSES: usize = 19;
pub const ADDR_ECON_TRACK: u32 = 0x0055_04a8;
pub const ADDR_STATION_RADIUS: u32 = 0x0054_fd9c;
pub const ADDR_BUILDING_TYPES: u32 = 0x0054_57f8;
pub const BUILDING_RECORD_SIZE: u32 = 0x154;
/// Offset of the footprint grid inside a building record (cell `(row, col)` at
/// `+0xcd - 20 * row - 2 * col`).
pub const FOOTPRINT_OFFSET: u32 = 0xcd;
/// Building types loaded (the oracle test exercises types 24..63).
pub const BUILDING_TYPES: usize = 64;

/// Footprint of one building type: byte of cell `(row, col)` at `[row * 10 + col]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Footprint(pub [u8; 100]);

impl Footprint {
    /// Cells whose flag byte has `0x43` set take part in the catchment test (spec 3.3).
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..10i32).flat_map(move |row| {
            (0..10i32).filter_map(move |col| {
                (self.0[(row * 10 + col) as usize] & 0x43 != 0).then_some((row, col))
            })
        })
    }
}

/// Track and station tables read from the executable.
#[derive(Clone, Debug)]
pub struct TrackTables {
    /// `bridge_cost_table` (`0x53ce60`) `i32[type * 2 + single]`, 6 types.
    pub bridge_cost: [i32; 12],
    /// `terrain_class_table[class].f32_0` (`0x540aa0 + class * 0xc0`).
    pub terrain_mult: [f32; TERRAIN_CLASSES],
    /// `econ_track_cost` (`0x5504a8`) by economy state.
    pub econ_track: [f32; 5],
    /// `station_radius` (`0x54fd9c`) by station size.
    pub station_radius: [i32; 3],
    /// Valid edge sets on open ground (`edge_shape_table`, 50 entries).
    pub shape_valid: [bool; 256],
    /// Valid edge sets on bridges (42 entries).
    pub shape_valid_bridge: [bool; 256],
    pub footprints: Vec<Footprint>,
}

impl TrackTables {
    pub fn load(exe: &Exe) -> Result<TrackTables, ExeError> {
        let i32s = |va: u32, n: usize| -> Result<Vec<i32>, ExeError> {
            Ok(exe
                .read(va, n * 4)?
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| i32::from_le_bytes(*c))
                .collect())
        };
        let shapes = |va: u32| -> Result<[bool; 256], ExeError> {
            let raw = exe.read(va, 512)?;
            let mut v = [false; 256];
            for (i, ok) in v.iter_mut().enumerate() {
                *ok = raw[i * 2] != b'c';
            }
            Ok(v)
        };
        // The direction tables are structural; make sure the constants agree with the data.
        debug_assert_eq!(i32s(ADDR_DIR_DX, 8)?, Dir::DX);
        debug_assert_eq!(i32s(ADDR_DIR_DY, 8)?, Dir::DY);
        let mut terrain_mult = [0f32; TERRAIN_CLASSES];
        for (c, m) in terrain_mult.iter_mut().enumerate() {
            *m = exe.f32s(ADDR_TERRAIN_CLASS + c as u32 * TERRAIN_CLASS_STRIDE, 1)?[0];
        }
        let mut footprints = Vec::with_capacity(BUILDING_TYPES);
        for t in 0..BUILDING_TYPES as u32 {
            let base = ADDR_BUILDING_TYPES + t * BUILDING_RECORD_SIZE + FOOTPRINT_OFFSET;
            let mut g = [0u8; 100];
            for row in 0..10u32 {
                for col in 0..10u32 {
                    g[(row * 10 + col) as usize] = exe.read(base - row * 20 - col * 2, 1)?[0];
                }
            }
            footprints.push(Footprint(g));
        }
        Ok(TrackTables {
            bridge_cost: i32s(ADDR_BRIDGE_COST, 12)?.try_into().expect("12"),
            terrain_mult,
            econ_track: exe.f32s(ADDR_ECON_TRACK, 5)?.try_into().expect("5"),
            station_radius: i32s(ADDR_STATION_RADIUS, 3)?.try_into().expect("3"),
            shape_valid: shapes(ADDR_EDGE_SHAPES)?,
            shape_valid_bridge: shapes(ADDR_EDGE_SHAPES_BRIDGE)?,
            footprints,
        })
    }

    /// Whether `edges` is an allowed set of edges for one piece (spec 1.4).
    pub fn is_valid_shape(&self, edges: EdgeMask, bridge: bool) -> bool {
        let t = if bridge {
            &self.shape_valid_bridge
        } else {
            &self.shape_valid
        };
        t[usize::from(edges.0)]
    }
}

// ---- track tile cost (spec 2.2, 0x463520) -----------------------------------------------------

/// Global inputs of the cost (spec 2.1), written by the caller before each call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CostGlobals {
    /// `track_single` (`0x5534b0`): `true` = single track, `false` = double.
    pub single: bool,
    /// `track_bridge_type` (`0x53ce18`).
    pub bridge_type: u32,
    /// `track_electric` (`0x566fcc`).
    pub electric: bool,
    /// `track_company` (`0x566a50`).
    pub company: u32,
    /// `track_edge_base` (`0x566a64`), see [`edge_base`].
    pub edge_base: i32,
    /// `track_elec_base` (`0x566a60`), see [`elec_base`].
    pub elec_base: i32,
    /// Economy state index (`0x570717`) into [`TrackTables::econ_track`].
    pub econ_state: usize,
    /// `effect_multiplier(0xe, ...)` for the tile's territory (Bridge Building).
    pub bridge_mult: f32,
}

/// `track_edge_base = ftol(effect_multiplier(9, ..) * 2000)` (spec 2.1).
pub fn edge_base(track_building_mult: f32) -> i32 {
    ftol(f64::from(track_building_mult) * 2000.0)
}

/// `track_elec_base = ftol(effect_multiplier(0xb, ..) * 6000)` (spec 2.1).
pub fn elec_base(electric_building_mult: f32) -> i32 {
    ftol(f64::from(electric_building_mult) * 6000.0)
}

/// One node of a path: the edges to lay on the tile and what the tile is used for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CostStep {
    pub d_in: Option<Dir>,
    pub d_out: Option<Dir>,
    /// Node mode: 0 ground; 2, 4, 7 river tile (bridge); 1, 3, 5, 6 approach.
    pub mode: u8,
    /// Water span, 0 = none, 1..=3 = water ahead/on the tile.
    pub span: u8,
}

/// Result of the cost function: `cost` is charged; `cost2` is the informational second sum and
/// stays `None` when the original does not write it (spans).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileCost {
    pub cost: i32,
    pub cost2: Option<i32>,
}

/// Cost of one edge `d` on a tile (spec 2.2 `aresta`): returns `(v, f)`.
fn edge_cost(
    base: i32,
    edges: u32,
    d: Dir,
    piece: Option<&TrackPiece>,
    single: bool,
) -> (i32, f32) {
    if edges & (1 << d.0) != 0 {
        if piece.is_some_and(|p| p.double) || single {
            (0, 0.0)
        } else {
            (base, 0.5)
        }
    } else if single {
        (base, 0.0)
    } else {
        (ftol(f64::from(base) * 1.5), 0.0)
    }
}

/// `track_tile_cost` (`0x463520`): cost of building `step` on `tile` holding `piece`
/// (`None` = no track yet), `[V]`.
pub fn track_tile_cost(
    t: &TrackTables,
    tile: &TileInfo,
    piece: Option<&TrackPiece>,
    g: &CostGlobals,
    step: CostStep,
) -> TileCost {
    let bridge = f64::from(g.bridge_mult);
    if step.span != 0 {
        let c = if tile.flags3 & TILE3_SPAN != 0 {
            0
        } else {
            ftol(bridge * 120_000.0)
        };
        return TileCost {
            cost: c,
            cost2: None,
        };
    }
    let mut single = g.single;
    let mut base = g.edge_base;
    if step.mode == 0 {
        if tile.flags2 & TILE2_BRIDGE != 0 {
            // as in the original: `single` takes the existing piece's double-track bit
            single = piece.is_some_and(|p| p.double);
        }
    } else {
        if !g.single && g.bridge_type == 1 {
            single = true;
        }
        if matches!(step.mode, 2 | 4 | 7) {
            let mut c2 = 0;
            if tile.flags2 & TILE2_BRIDGE != 0 {
                let p = piece.copied().unwrap_or_default();
                if u32::from(p.owner) != g.company {
                    return TileCost {
                        cost: 0,
                        cost2: Some(0),
                    };
                }
                let skip = if !p.double && !single {
                    false
                } else {
                    u32::from(p.bridge_type) == g.bridge_type
                };
                if skip {
                    return TileCost {
                        cost: 0,
                        cost2: Some(0),
                    };
                }
                let idx = usize::from(p.bridge_type) * 2 + usize::from(p.double);
                c2 = ftol(bridge * f64::from(t.bridge_cost[idx]));
            }
            let idx = usize::from(single) + g.bridge_type as usize * 2;
            return TileCost {
                cost: ftol(bridge * f64::from(t.bridge_cost[idx])),
                cost2: Some(c2),
            };
        }
        base = 0;
    }
    let edges = piece.map_or(0, |p| u32::from(p.edges.0));
    let mut acc1: i32;
    let mut acc2: i32 = 0;
    if let Some(d) = step.d_in {
        let (v, f) = edge_cost(base, edges, d, piece, single);
        if d.is_diagonal() {
            acc1 = ftol(f64::from(v) * 1.4);
            acc2 = ftol(f64::from(v) * f64::from(f) * 1.4);
        } else {
            acc1 = v;
            acc2 = ftol(f64::from(v) * f64::from(f));
        }
        if g.electric && !piece.is_some_and(|p| p.electrified) {
            let e = if single && !piece.is_some_and(|p| p.double) {
                g.elec_base
            } else {
                ftol(f64::from(g.elec_base) * 1.5)
            };
            if edges & (1 << d.0) != 0 {
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
    if let Some(d) = step.d_out {
        let (v, f) = edge_cost(base, edges, d, piece, single);
        if d.is_diagonal() {
            acc1 = ftol(f64::from(v) * 1.4 + f64::from(acc1));
            acc2 = ftol(f64::from(v) * f64::from(f) * 1.4 + f64::from(acc2));
        } else {
            acc1 += v;
            acc2 = ftol(f64::from(v) * f64::from(f) + f64::from(acc2));
        }
    }
    if tile.flags2 & TILE2_CLEARED == 0 {
        acc1 += 1000 * tile.tree_count();
    }
    let by_terrain = ftol(f64::from(acc1) * f64::from(t.terrain_mult[tile.terrain_class]));
    TileCost {
        cost: ftol(f64::from(by_terrain) * f64::from(t.econ_track[g.econ_state])),
        cost2: Some(acc2),
    }
}

// ---- station catchment (spec 3.3, 0x407030) ---------------------------------------------------

/// `dist(a, b)` (`0x4f0ca0`): `ftol(sqrt(dx^2 + dy^2))` with the coordinates stored as `f32`.
pub fn tile_distance(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    let dx = f64::from(bx as f32) - f64::from(ax as f32);
    let dy = f64::from(by as f32) - f64::from(ay as f32);
    ftol((dx * dx + dy * dy).sqrt())
}

/// A building as the catchment test sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatchmentBuilding {
    /// Index into [`TrackTables::footprints`] (building type).
    pub building_type: usize,
    /// `+0x1a`, `+0x1c`: centre.
    pub centre: (i32, i32),
    /// `+0x1e`, `+0x20`: footprint origin; cell `(row, col)` is at `(x - col, y - row)`.
    pub origin: (i32, i32),
}

/// `station_catchment_test` (`0x407030`), `[V]` for the distance rule: returns the minimum
/// footprint distance when the building is linked to a station of `size` (0..=2) at `station`,
/// `None` otherwise. (The checks that a station never links another station, and the own-station
/// building, depend on object kinds and are not part of the verified distance rule.)
pub fn station_catchment(
    t: &TrackTables,
    station: (i32, i32),
    size: usize,
    b: &CatchmentBuilding,
) -> Option<i32> {
    let radius = t.station_radius[size];
    if tile_distance(station.0, station.1, b.centre.0, b.centre.1) > radius + 11 {
        return None;
    }
    let min = t.footprints[b.building_type]
        .cells()
        .map(|(row, col)| tile_distance(station.0, station.1, b.origin.0 - col, b.origin.1 - row))
        .fold(9999, i32::min);
    (min <= radius).then_some(min)
}

/// Whether the tile at offset `(dx, dy)` from a station of `size` is inside its catchment
/// (distance truncated, spec 3.3: small 5x5, medium diameter 9, large diameter 17).
pub fn in_catchment(t: &TrackTables, size: usize, dx: i32, dy: i32) -> bool {
    tile_distance(0, 0, dx, dy) <= t.station_radius[size]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirs_and_masks() {
        assert_eq!(Dir::ALL[3].opposite(), Dir::ALL[7]);
        assert!(Dir::ALL[1].is_diagonal() && !Dir::ALL[2].is_diagonal());
        let m = EdgeMask::default().with(Dir::ALL[0]).with(Dir::ALL[4]);
        assert_eq!(m.0, 0x11);
        assert_eq!(m.count(), 2);
        assert_eq!(m.dirs().collect::<Vec<_>>(), vec![Dir::ALL[0], Dir::ALL[4]]);
        assert_eq!(m.without(Dir::ALL[0]).0, 0x10);
        assert_eq!((Dir::ALL[1].dx(), Dir::ALL[1].dy()), (1, -1));
    }

    fn tables() -> Option<TrackTables> {
        let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
        });
        let bytes = std::fs::read(path).ok()?;
        TrackTables::load(&Exe::from_bytes(bytes).ok()?).ok()
    }

    /// Spec 1.4: 50 valid shapes on ground, 42 on bridges; spec 2.2 neutral costs; spec 3.3
    /// catchment shapes.
    #[test]
    fn spec_values_from_the_executable() {
        let Some(t) = tables() else {
            eprintln!("skipping: executable not found");
            return;
        };
        assert_eq!(t.shape_valid.iter().filter(|v| **v).count(), 50);
        assert_eq!(t.shape_valid_bridge.iter().filter(|v| **v).count(), 42);
        assert!(t.is_valid_shape(EdgeMask(85), false) && !t.is_valid_shape(EdgeMask(3), false));
        assert_eq!(t.station_radius, [2, 4, 8]);
        assert_eq!(t.econ_track, [0.8, 0.9, 1.0, 1.1, 1.2]);
        assert_eq!(
            &t.bridge_cost[2..8],
            &[90_000, 60_000, 300_000, 200_000, 225_000, 150_000] // [type * 2 + single]
        );
        let g = CostGlobals {
            single: true,
            bridge_type: 1,
            electric: false,
            company: 0,
            edge_base: edge_base(1.0),
            elec_base: elec_base(1.0),
            econ_state: 2,
            bridge_mult: 1.0,
        };
        let tile = TileInfo {
            terrain_class: 5, // grass
            flags2: TILE2_CLEARED,
            flags3: 0,
        };
        let straight = CostStep {
            d_in: Some(Dir::ALL[0]),
            d_out: Some(Dir::ALL[4]),
            mode: 0,
            span: 0,
        };
        // spec 2.2: single orthogonal 4000, double 6000, electrified 10000 / 15000
        assert_eq!(track_tile_cost(&t, &tile, None, &g, straight).cost, 4000);
        let dbl = CostGlobals { single: false, ..g };
        assert_eq!(track_tile_cost(&t, &tile, None, &dbl, straight).cost, 6000);
        let el = CostGlobals {
            electric: true,
            ..g
        };
        assert_eq!(track_tile_cost(&t, &tile, None, &el, straight).cost, 10_000);
        let eld = CostGlobals {
            electric: true,
            single: false,
            ..g
        };
        assert_eq!(
            track_tile_cost(&t, &tile, None, &eld, straight).cost,
            15_000
        );
        // catchment: small covers the 5x5 square, medium (4,3) is outside, large diameter 17
        assert!((-2..=2).all(|x| (-2..=2).all(|y| in_catchment(&t, 0, x, y))));
        assert!(!in_catchment(&t, 0, 3, 0));
        assert!(in_catchment(&t, 1, 4, 0) && !in_catchment(&t, 1, 4, 3));
        assert!(in_catchment(&t, 2, 8, 0) && !in_catchment(&t, 2, 9, 0));
    }
}
