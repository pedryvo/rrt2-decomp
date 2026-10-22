//! `track_pathfind` (`0x460850`) in both modes, with the helpers of the build mode:
//! `track_edge_valid` (`0x462b80`), the river/bridge node mode (`0x460640`), the water span kind
//! (`0x460520`), the river neighbour mask (`0x462a70`), `track_company_may_build_at`
//! (`0x4603a0`) and the step cost (`0x463b30`). Docs: terrain-track.md 1.4 and 2; ai.md 5.
//!
//! Mode 0 (train/route mode, `param_7 == 0`) searches along existing track; mode 1 (build mode,
//! used by the AI to plan and lay track) searches over the terrain with the A* step cost of
//! `0x463b30`, which adds a bearing penalty per direction (`0x566a08[8]`), ownership penalties
//! and, during the AI turn (`0x5746ec != 0`), a slope penalty. The open list is the bucket queue
//! of `0x4fa140` (width 4, a new entry before the first one of equal or higher cost), the nodes
//! are per-tile records of `0x566a74` (cost and parent per entry direction, mask of directions
//! seen). In build mode stale queue entries are skipped; in route mode they are expanded again.

use std::collections::HashMap;

use super::Ai;
use crate::fp::ftol;
use crate::trainstep::P;

pub const DX: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
pub const DY: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

/// `0x53cd90`: node modes that are a bridge tile (2, 4, 7).
const MODE_BRIDGE: [bool; 8] = [false, false, true, false, true, false, false, true];
/// `0x53cdb0`: node modes after which the next node recomputes its mode.
const MODE_TERMINAL: [bool; 8] = [true, false, false, true, false, true, false, true];
/// `0x53cdd0`: node modes allowed on the start tile.
const MODE_START_OK: [bool; 8] = [false, true, false, false, true, false, true, false];
/// `0x53cdf0`: node modes allowed on the destination tile.
const MODE_END_OK: [bool; 8] = [true, true, false, true, false, true, true, true];
/// `0x53ce20` / `0x53ce40`: river neighbour masks of the diagonal bridge test.
const RIVER_DIAG_A: [u32; 8] = [0, 5, 0, 20, 0, 80, 0, 65];
const RIVER_DIAG_B: [u32; 8] = [0, 80, 0, 65, 0, 5, 0, 20];

const SHAPES: u32 = 0x0053_65e8;
const SHAPES_BRIDGE: u32 = 0x0053_67e8;

/// One node of a found path (arrays `X, Y, A, B` of `0x460850` and, in build mode, the bridge
/// mode and water span of the node). Index 0 is the destination.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Node {
    pub x: i32,
    pub y: i32,
    pub a: u8,
    pub b: u8,
    pub mode: u8,
    pub span: u8,
}

/// Arguments of `track_pathfind` that the AI uses (the others are null/0).
#[derive(Clone, Copy, Debug)]
pub struct Req {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    /// `param_7`: build mode.
    pub build: bool,
    /// `param_8`: the path does not include the source tile.
    pub p8: bool,
    /// `param_10` / `param_11`: required start / end direction (-1 = any).
    pub start_dir: i32,
    pub end_dir: i32,
    /// `param_12`: company (0 = none).
    pub company: i32,
    /// `param_15`.
    pub p15: bool,
    /// `param_17`: electric track only (route mode).
    pub electric: bool,
    /// The path is wanted (`param_13` or `param_5` non-null, or `0x566a5c`).
    pub want_path: bool,
    /// `0x566a5c`: count the path tiles (`0x566a44`) and those not owned by this company
    /// (`0x566a40`).
    pub count_owner: i32,
}

impl Req {
    pub fn route(x1: i32, y1: i32, x2: i32, y2: i32) -> Req {
        Req {
            x1,
            y1,
            x2,
            y2,
            build: false,
            p8: false,
            start_dir: -1,
            end_dir: -1,
            company: 0,
            p15: false,
            electric: false,
            want_path: false,
            count_owner: 0,
        }
    }
}

/// Result of a search.
#[derive(Clone, Debug, Default)]
pub struct Found {
    pub ok: bool,
    /// `*param_13`: sum of `track_tile_cost` over the path (build mode).
    pub cost: i32,
    /// `*param_14`: sum of the second cost.
    pub cost2: i32,
    pub nodes: Vec<Node>,
}

#[derive(Clone, Copy, Default)]
struct NodeRec {
    cost: [u32; 8],
    back: [u8; 8],
    mask: u8,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    cost: u32,
    x: i32,
    y: i32,
    dir: u8,
    mode: u8,
    span: u8,
}

/// The bucket queue of `0x4fa140` (width 4).
#[derive(Default)]
struct Buckets {
    lists: Vec<Vec<Entry>>,
}

impl Buckets {
    fn push(&mut self, e: Entry) {
        let b = (e.cost / 4) as usize;
        if self.lists.len() <= b {
            self.lists.resize_with(b + 1, Vec::new);
        }
        let l = &mut self.lists[b];
        let at = l.iter().position(|o| e.cost <= o.cost).unwrap_or(l.len());
        l.insert(at, e);
    }

    fn pop(&mut self, from: u32) -> Entry {
        let mut b = (from as i32 / 4) as usize;
        while self.lists.get(b).is_none_or(|l| l.is_empty()) {
            b += 1;
        }
        self.lists[b].remove(0)
    }
}

/// Search state: the globals `0x566a2c..0x566a74` of the original.
pub struct Search {
    nodes: HashMap<(i32, i32), NodeRec>,
    queue: Buckets,
    count: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    limit: i32,
    /// `0x566a4c`: the search box keeps away from the map edge (bounds checks relaxed).
    pub inner: bool,
    /// `0x566a34`: build mode.
    pub build: bool,
    /// `0x566a50` / `0x566a54`: company.
    pub company: i32,
    /// `0x566a58`: the company is restricted to its territories.
    pub restricted: bool,
    /// `0x566a08[8]`: bearing penalty per direction.
    pub bearing: [i32; 8],
    /// `0x53ce10/14`: destination tile.
    pub dst: (i32, i32),
}

impl Search {
    /// The search globals between two searches (reset at the end of `0x460850`).
    pub fn idle() -> Search {
        Search::new()
    }

    fn new() -> Search {
        Search {
            nodes: HashMap::new(),
            queue: Buckets::default(),
            count: 0,
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 0,
            limit: 999_999_999,
            inner: false,
            build: false,
            company: 0,
            restricted: false,
            bearing: [0; 8],
            dst: (-1, -1),
        }
    }

    /// `0x462690`: relax `(x, y)` entered with `dir`.
    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, x: i32, y: i32, dir: u8, back: u8, cost: u32, mode: u8, span: u8) {
        if x < self.x0 || x > self.x1 || y < self.y0 || y > self.y1 || cost as i32 > self.limit {
            return;
        }
        let n = self.nodes.entry((x, y)).or_default();
        let bit = 1u8 << dir;
        if n.mask & bit != 0 && n.cost[usize::from(dir)] as i32 <= cost as i32 {
            return;
        }
        n.mask |= bit;
        n.cost[usize::from(dir)] = cost;
        n.back[usize::from(dir)] = back;
        self.queue.push(Entry {
            cost,
            x,
            y,
            dir,
            mode,
            span,
        });
        self.count += 1;
    }
}

impl Ai<'_, '_> {
    // ---- tile helpers ------------------------------------------------------------------------

    pub fn in_map(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.ts.map_w() && y < self.ts.map_h()
    }

    /// `terrain_class_of_id[tile >> 6]`.
    pub fn terrain_class(&self, x: i32, y: i32) -> u8 {
        rt2_formats::terrain::tables().class_of_id(self.ts.tile_terrain_id(x, y))
    }

    pub fn is_water(&self, x: i32, y: i32) -> bool {
        self.terrain_class(x, y) == 0x12
    }

    /// Tile byte 0 bit 0: river.
    pub fn is_river(&self, x: i32, y: i32) -> bool {
        self.ts.t8(x, y, 0) & 1 != 0
    }

    /// The track piece of the tile (0 = none).
    pub fn piece(&self, x: i32, y: i32) -> i32 {
        self.ts.tile_track(x, y)
    }

    /// `piece+0` (0 without piece).
    pub fn piece_mask(&self, x: i32, y: i32) -> u32 {
        let p = self.piece(x, y);
        if p == 0 {
            0
        } else {
            u32::from(self.ts.u8(P::Track, p, 0))
        }
    }

    /// `piece+1` (0 without piece).
    fn piece_mask2(&self, x: i32, y: i32) -> u32 {
        let p = self.piece(x, y);
        if p == 0 {
            0
        } else {
            u32::from(self.ts.u8(P::Track, p, 1))
        }
    }

    /// `piece+2` (0 without piece).
    pub fn piece_owner(&self, x: i32, y: i32) -> i32 {
        let p = self.piece(x, y);
        if p == 0 {
            0
        } else {
            i32::from(self.ts.u8(P::Track, p, 2))
        }
    }

    fn shape_ok(&self, mask: u32, bridge: bool) -> bool {
        let base = if bridge { SHAPES_BRIDGE } else { SHAPES };
        self.ts.img.u8(base + 2 * (mask & 0xff)) != b'c'
    }

    /// `company_is_human` (`0x43c030`).
    pub fn company_is_human(&self, company: i32) -> bool {
        let p = self.ts.i32(P::Company, company, super::CO_CHAIRMAN);
        p > 0 && self.ts.i32(P::Person, p, 0x122) != 0
    }

    /// `0x462a70`: river neighbours of a river tile (bit 0 north, 2 east, 4 south, 6 west).
    fn river_mask(&self, s: &Search, x: i32, y: i32) -> u32 {
        if !self.is_river(x, y) {
            return 0;
        }
        let (w1, h1) = (self.ts.map_w() - 1, self.ts.map_h() - 1);
        let r = |xx: i32, yy: i32| self.is_river(xx, yy);
        let mut v = 0u32;
        if !s.inner {
            if y > 0 && r(x, y - 1) {
                v = 1;
            }
            if x < w1 && r(x + 1, y) {
                v |= 4;
            }
            if y < h1 && r(x, y + 1) {
                v |= 0x10;
            }
            if x > 0 && r(x - 1, y) {
                v |= 0x40;
            }
        } else {
            if r(x, y - 1) {
                v = 1;
            }
            if r(x + 1, y) {
                v |= 4;
            }
            if r(x, y + 1) {
                v |= 0x10;
            }
            if r(x - 1, y) {
                v |= 0x40;
            }
        }
        v
    }

    /// `0x460640`: node mode of the step from `(x, y)` towards `d` (0 ground, 1/2 orthogonal
    /// river approach/bridge, 4/7 diagonal bridge, 6 diagonal approach).
    fn bridge_mode(&self, s: &Search, x: i32, y: i32, d: u8) -> u8 {
        let (nx, ny) = (x + DX[usize::from(d)], y + DY[usize::from(d)]);
        let here_ok = s.inner || self.in_map(x, y);
        if d & 1 == 0 {
            if here_ok {
                if self.is_river(x, y) {
                    return 2;
                }
                if !s.inner && !self.in_map(nx, ny) {
                    return 0;
                }
            } else if !self.in_map(nx, ny) {
                return 0;
            }
            if !self.is_river(nx, ny) && !self.is_river(x, y) {
                0
            } else {
                1
            }
        } else {
            if here_ok {
                if self.is_river(x, y) {
                    let m = self.river_mask(s, x, y);
                    return if m & RIVER_DIAG_A[usize::from(d)] != 0 {
                        4
                    } else {
                        7
                    };
                }
                if !s.inner && !self.in_map(nx, ny) {
                    return 0;
                }
            } else if !self.in_map(nx, ny) {
                return 0;
            }
            if self.is_river(nx, ny)
                && RIVER_DIAG_B[usize::from(d)] & self.river_mask(s, nx, ny) != 0
            {
                6
            } else {
                0
            }
        }
    }

    /// `0x460520`: water around the step: 2 = the tile is water, 1 = the next tile, 3 = the
    /// tile behind, 0 = none.
    fn span_kind(&self, x: i32, y: i32, d: u8) -> u8 {
        let (dx, dy) = (DX[usize::from(d)], DY[usize::from(d)]);
        let (ax, ay) = (x + dx, y + dy);
        let ahead = self.in_map(ax, ay) && self.is_water(ax, ay);
        let (bx, by) = (ax - 2 * dx, ay - 2 * dy);
        let behind = self.in_map(bx, by) && self.is_water(bx, by);
        if self.is_water(x, y) {
            2
        } else if ahead {
            1
        } else if behind {
            3
        } else {
            0
        }
    }

    /// `track_edge_valid` (`0x462b80`, terrain-track.md 1.4): may the edge from `(x, y)` towards
    /// `d` be built (`d_back` = the opposite edge used on the neighbour)?
    #[allow(clippy::too_many_arguments)]
    fn edge_valid(
        &self,
        s: &Search,
        x: i32,
        y: i32,
        d: u8,
        d_back: u8,
        bridge_table: bool,
        skip_mask2: bool,
        mode: u8,
        span: u8,
    ) -> bool {
        let du = usize::from(d);
        let (nx, ny) = (x + DX[du], y + DY[du]);
        let mut p5 = bridge_table;
        if (nx, ny) == s.dst && !p5 {
            p5 = true;
        }
        if !s.inner && (!self.in_map(x, y) || !self.in_map(nx, ny)) {
            return false;
        }
        let opp = |d: u8| d.wrapping_sub(4) & 7;
        let bit = |d: u8| 1u32 << (d & 7);
        let here_piece = self.piece(x, y);
        let local8 = bit(d);
        let mut m: u32;
        if here_piece == 0 {
            m = local8;
        } else {
            m = self.piece_mask(x, y);
            if mode != 0 && span != 0 && ((bit(opp(d)) | bit(d)) & m) != m {
                return false;
            }
            if !skip_mask2 {
                m |= self.piece_mask2(x, y);
            }
            m |= local8;
            let both = self.piece_mask(x, y)
                | if skip_mask2 {
                    0
                } else {
                    self.piece_mask2(x, y)
                };
            if self.ts.t8(x, y, 2) & 0x80 != 0 && both & (bit(opp(d)) | local8) == 0 {
                return false;
            }
            if self.ts.t8(x, y, 3) & 0x40 != 0 && both & (bit(opp(d)) | local8) == 0 {
                return false;
            }
        }
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        if span != 0 && d & 1 != 0 && x > 0 && x < w - 1 && y > 0 && y < h - 1 {
            for k in [d.wrapping_sub(1) & 7, d.wrapping_add(1) & 7] {
                let ku = usize::from(k);
                if self.piece(x + DX[ku], y + DY[ku]) != 0 {
                    return false;
                }
            }
        }
        if !self.shape_ok(m, p5) {
            return false;
        }
        if self.ts.t8(x, y, 3) & 0x20 != 0 && self.ts.t8(nx, ny, 3) & 0x20 != 0 {
            return false;
        }
        if self.is_water(nx, ny) && span == 0 {
            return false;
        }
        if self.ts.t8(nx, ny, 2) & 0x10 != 0 {
            return false;
        }
        if s.restricted && span == 0 && s.company != 0 {
            let terr = usize::from(self.ts.t8(nx, ny, 9));
            let rights = self.ts.u8(P::Company, s.company, 0x479 + terr * 0x1ed);
            if rights & 2 == 0 {
                return false;
            }
            if rights & 1 == 0 && !self.company_is_human(s.company) {
                return false;
            }
        }
        if !MODE_BRIDGE[usize::from(mode & 7)] {
            if self.is_river(x, y) {
                return false;
            }
        } else {
            let rm = self.river_mask(s, x, y);
            if m & rm != 0 {
                return false;
            }
            if rm & 0x11 == 0 || rm & 0x44 == 0 {
                if d & 1 != 0 {
                    return false;
                }
            } else {
                if rm & 0x11 == 0x11 || rm & 0x44 == 0x44 {
                    return false;
                }
                if d & 1 == 0 {
                    return false;
                }
                let (mut first, mut last) = (-1i32, -1i32);
                for k in 0..8 {
                    if rm & (1 << k) != 0 {
                        if first == -1 {
                            first = k;
                        } else {
                            last = k;
                        }
                    }
                }
                if last != -1 {
                    let a = i32::from(d) * 2;
                    let b = i32::from(opp(d)) * 2 + 1;
                    let lo = first * 2;
                    let hi = last * 2 + 1;
                    let (ia, ib) = if lo < hi {
                        (lo < a && a < hi, lo < b && b < hi)
                    } else {
                        (hi < a && a < lo, !(b <= hi || lo <= b))
                    };
                    if ia == ib {
                        return false;
                    }
                }
            }
        }
        // Diagonal next to a building tile (`tile+2 & 0x10`) that is not a station: the edge
        // must already exist.
        if DX[du] != 0 && DY[du] != 0 {
            let side_blocked = |xx: i32, yy: i32| {
                self.ts.t8(xx, yy, 2) & 0x10 != 0 && {
                    let st = self.station_of_building_at(xx, yy);
                    st == 0 || self.building_at_flags6(xx, yy) & 1 == 0
                }
            };
            if side_blocked(x + DX[du], y) || side_blocked(x, y + DY[du]) {
                let want = (1u32 << (d_back & 0x1f)) + local8;
                if self.piece_mask(x, y) & want != want {
                    return false;
                }
            }
        }
        let back_bit = bit(opp(d));
        let m2 = if self.piece(nx, ny) == 0 {
            back_bit
        } else {
            let both = self.piece_mask(nx, ny)
                | if skip_mask2 {
                    0
                } else {
                    self.piece_mask2(nx, ny)
                };
            let bd = opp(d);
            let req = bit(opp(bd)) | bit(bd);
            if self.ts.t8(nx, ny, 2) & 0x80 != 0 && both & req == 0 {
                return false;
            }
            if self.ts.t8(nx, ny, 3) & 0x40 != 0 && both & req == 0 {
                return false;
            }
            both | back_bit
        };
        if !self.shape_ok(m2, p5) {
            return false;
        }
        if self.ts.t8(x, y, 6) & 2 != 0 {
            let pm = self.piece_mask(x, y);
            if local8 & pm == 0 && pm & back_bit == 0 {
                return false;
            }
        }
        if self.ts.t8(nx, ny, 6) & 2 != 0 {
            let pm = self.piece_mask(nx, ny);
            if local8 & pm == 0 && pm & back_bit == 0 {
                return false;
            }
        }
        true
    }

    /// [`Self::edge_valid`] for the other AI modules (mode and span 0).
    #[allow(clippy::too_many_arguments)]
    pub fn edge_valid_pub(
        &self,
        s: &Search,
        x: i32,
        y: i32,
        d: u8,
        d_back: u8,
        bridge_table: bool,
        skip_mask2: bool,
    ) -> bool {
        self.edge_valid(s, x, y, d, d_back, bridge_table, skip_mask2, 0, 0)
    }

    pub fn may_build_at_pub(&self, company: i32, x: i32, y: i32) -> bool {
        self.may_build_at(company, x, y)
    }

    /// `0x4bf1b0`: the station of the building on the tile (0 if none or not a station).
    fn station_of_building_at(&self, x: i32, y: i32) -> i32 {
        let b = self.building_at(x, y);
        if b != 0 && self.ts.u8(P::Industry, b, 7) == 0 {
            i32::from(self.ts.u16(P::Industry, b, 8))
        } else {
            0
        }
    }

    /// `0x408fd0` then `+6`: flags of the building on the tile.
    fn building_at_flags6(&self, x: i32, y: i32) -> u8 {
        let b = self.building_at(x, y);
        self.ts.u8(P::Industry, b, 6)
    }

    /// `0x408fd0`: the building (structure type 3) on the tile, 0 if none.
    pub fn building_at(&self, x: i32, y: i32) -> i32 {
        let st = self.ts.tile_struct_find(x, y, 3);
        if st == 0 {
            return 0;
        }
        let id = self.ts.u32(P::Struct, st, 1) as i32;
        if self.ts.exists(P::Industry, id) {
            id
        } else {
            0
        }
    }

    /// `0x4bf1d0`: the station (structure type 4) on the tile, 0 if none.
    pub fn station_at(&self, x: i32, y: i32) -> i32 {
        self.ts.station_at(x, y)
    }

    /// `track_company_may_build_at` (`0x4603a0`): with the scenario options `0x5706c1/c2`, a
    /// human company must build from a cleared tile and own track within 2 tiles (AI: always).
    fn may_build_at(&self, company: i32, x: i32, y: i32) -> bool {
        if company == 0 {
            return true;
        }
        if !self.company_is_human(company) {
            return true;
        }
        let c1 = self.perm(0x0057_06c1);
        let c2 = self.perm(0x0057_06c2);
        if !((!c1 || !c2) && self.ts.i32(P::Company, company, 0x182) > 0) || !self.in_map(x, y) {
            return true;
        }
        if self.ts.t8(x, y, 2) & 0x20 == 0 {
            return false;
        }
        if c2 {
            return true;
        }
        if self.piece_owner(x, y) == company {
            return true;
        }
        for xx in x - 2..=x + 2 {
            for yy in y - 2..=y + 2 {
                if self.in_map(xx, yy) && self.piece_owner(xx, yy) == company {
                    return true;
                }
            }
        }
        false
    }

    /// `0x463b30`: cost of the step from `(x, y)` towards `d`, entered from `d_in`.
    fn step_cost(&self, s: &Search, x: i32, y: i32, d: u8, d_in: u8) -> u32 {
        if !s.build {
            let own = s.company == 0 || self.piece_owner(x, y) == s.company;
            let c = if own {
                u32::from(d & 1) | 2
            } else if d & 1 != 0 {
                9
            } else {
                6
            };
            return if d != d_in.wrapping_sub(4) & 7 {
                c
            } else {
                c * 3
            };
        }
        let du = usize::from(d);
        let (nx, ny) = (x + DX[du], y + DY[du]);
        let base = s.bearing[du] as u32;
        let p = self.piece(x, y);
        let mut c = if p == 0 {
            base + 10
        } else if i32::from(self.ts.u8(P::Track, p, 2)) == s.company {
            if u32::from(self.ts.u8(P::Track, p, 0)) & (1 << (d.wrapping_sub(4) & 7)) == 0 {
                base + 10
            } else {
                base
            }
        } else {
            base + 0x1e
        };
        let q = self.piece(nx, ny);
        if q != 0 {
            if i32::from(self.ts.u8(P::Track, q, 2)) != s.company {
                c += 0x1e;
            } else if u32::from(self.ts.u8(P::Track, q, 0)) & (1 << d) == 0 {
                c += 10;
            }
        } else {
            c += 10;
        }
        if d != d_in {
            c += 0xf;
        }
        // 0x5746ec: the AI turn is running (always here).
        let e = |xx: i32, yy: i32| i32::from(self.corner(xx, yy));
        let dh = e(x, y) - e(nx, ny);
        if !(-8..=8).contains(&dh) {
            c = ((dh.wrapping_mul(dh).wrapping_mul(c as i32)) >> 6) as u32;
        }
        c
    }

    /// Corner elevation (`0x574c28`, stride `w + 1`).
    pub fn corner(&self, x: i32, y: i32) -> i16 {
        let w = self.ts.s.map.width as i32 + 1;
        self.ts
            .s
            .map
            .elevation
            .get((y * w + x) as usize)
            .copied()
            .unwrap_or(0)
    }

    /// `track_pathfind` (`0x460850`).
    pub fn pathfind(&mut self, r: &Req) -> Found {
        let mut out = Found::default();
        let (x1, y1, x2, y2) = (r.x1, r.y1, r.x2, r.y2);
        if r.count_owner != 0 {
            self.route_foreign = 0;
            self.route_total = 0;
        }
        let mut s = Search::new();
        s.dst = (x2, y2);
        if !(self.in_map(x1, y1) && self.in_map(x2, y2)) {
            return out;
        }
        if r.electric && !r.build {
            let elec = |a: &Ai<'_, '_>, x: i32, y: i32| {
                let p = a.piece(x, y);
                p != 0 && a.ts.u8(P::Track, p, 0x23) & 0x10 != 0
            };
            if !elec(self, x1, y1) || !elec(self, x2, y2) {
                return out;
            }
        }
        if r.build {
            if self.is_water(x1, y1)
                || self.ts.t8(x1, y1, 2) & 0x10 != 0
                || self.is_water(x2, y2)
                || self.ts.t8(x2, y2, 2) & 0x10 != 0
            {
                return out;
            }
            let (dx, dy) = (f64::from(x1 - x2), f64::from((y1 - y2) * (y1 - y2)));
            s.limit = ftol(((dx * dx + dy).sqrt() + 5.0) * 240.0);
        }
        s.company = r.company;
        if r.company != 0 {
            let flag = self
                .ts
                .u8(P::Company, r.company, if r.build { 0x422d } else { 0x422e });
            s.restricted = flag == 0;
            if s.restricted {
                let bit = if r.build { 2 } else { 1 };
                let ok = |a: &Ai<'_, '_>, x: i32, y: i32| {
                    let terr = usize::from(a.ts.t8(x, y, 9));
                    a.ts.u8(P::Company, r.company, 0x479 + terr * 0x1ed) & bit != 0
                        || a.is_water(x, y)
                };
                if !ok(self, x1, y1) || !ok(self, x2, y2) {
                    return out;
                }
            }
        }
        if r.build && !self.may_build_at(s.company, x1, y1) && !self.may_build_at(s.company, x2, y2)
        {
            return out;
        }
        s.build = r.build;
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let d12 = Ai::dist(x1, y1, x2, y2);
        if !r.build {
            let st = |va: u32| self.st_i32(va);
            if st(0x0057_3d7e) > 2
                && st(0x0057_3d82) > 2
                && st(0x0057_3d86) <= w - 4
                && st(0x0057_3d8a) <= h - 4
            {
                s.inner = true;
            }
            s.x0 = 0;
            s.y0 = 0;
            s.x1 = w - 1;
            s.y1 = h - 1;
        } else {
            if x1 > 6
                && y1 > 6
                && x1 <= w - 8
                && y1 <= h - 8
                && x2 > 6
                && y2 > 6
                && x2 <= w - 8
                && y2 <= h - 8
            {
                s.inner = true;
            }
            let m = ((d12 >> 1) + 3).min(8);
            s.x0 = x1.min(x2) - m;
            s.x1 = x1.max(x2) + m;
            s.y0 = y1.min(y2) - m;
            s.y1 = y1.max(y2) + m;
        }
        let edge = if s.inner { 3 } else { 0 };
        if s.x0 <= edge {
            s.x0 = edge;
        }
        if w - edge - 1 <= s.x1 {
            s.x1 = w - edge - 1;
        }
        if s.y0 <= edge {
            s.y0 = edge;
        }
        if h - edge - 1 <= s.y1 {
            s.y1 = h - edge - 1;
        }
        if r.build {
            // 0x566a60/64: electric and track base of the source tile's territory.
            let terr = i32::from(self.ts.t8(x1, y1, 9));
            let m_el = self.ts.effect(0xb, s.company, 0, terr);
            self.elec_base = ftol(f64::from(m_el) * f64::from(self.ts.img.i32(0x0055_34c0)));
            let m_tr = self.ts.effect(9, s.company, 0, terr);
            self.edge_base = ftol(f64::from(m_tr) * 2000.0);
            let a = f64::from(y1 - y2) / (f64::from(x2 - x1) + 0.01);
            let ang = a.atan() * 57.3;
            let v = ftol(if x2 >= x1 { 90.0 - ang } else { 270.0 - ang });
            for (k, b) in s.bearing.iter_mut().enumerate() {
                let mut t = (360 + 45 * k as i32 - v) % 360;
                if t > 180 {
                    t = 360 - t;
                }
                if t > 150 {
                    t *= 2;
                } else if t > 100 {
                    t = ftol(f64::from(t) * 1.5);
                }
                t = (t >> 1) + 20;
                if k & 1 != 0 {
                    t = ftol(f64::from(t) * 1.41);
                }
                *b = t;
            }
        }
        let mut start_dir = r.start_dir;
        let mut end_dir = r.end_dir;
        if r.build {
            if start_dir != -1 {
                let sd = start_dir as u8;
                let sd4 = sd.wrapping_sub(4) & 7;
                if !self.edge_valid(&s, x1, y1, sd, sd, true, false, 0, 0)
                    && !self.edge_valid(&s, x1, y1, sd4, sd4, true, false, 0, 0)
                {
                    start_dir = -1;
                }
            }
            if end_dir != -1 {
                let ed = end_dir as u8;
                let ed4 = ed.wrapping_sub(4) & 7;
                if !self.edge_valid(&s, x2, y2, ed, ed, true, false, 0, 0)
                    && !self.edge_valid(&s, x2, y2, ed4, ed4, true, false, 0, 0)
                {
                    end_dir = -1;
                }
            }
        }
        // Initial nodes.
        if start_dir == -1 {
            for d in 0..8u8 {
                let du = usize::from(d);
                let (nx, ny) = (x1 + DX[du], y1 + DY[du]);
                if !r.build {
                    if self.piece_mask(x1, y1) & (1 << d) != 0 {
                        let c = self.step_cost(&s, x1, y1, d, d);
                        s.push(nx, ny, d.wrapping_sub(4) & 7, 0, c + 1, 0, 0);
                    }
                } else {
                    let m = self.bridge_mode(&s, x1, y1, d);
                    let sp = self.span_kind(x1, y1, d);
                    let cleared = self.ts.t8(x1, y1, 2) & 0x20 != 0;
                    if (m == 0 || MODE_START_OK[usize::from(m & 7)] || cleared)
                        && (sp != 2 || cleared)
                    {
                        let back = d.wrapping_sub(4) & 7;
                        if self.edge_valid(&s, x1, y1, d, back, true, false, m, sp) {
                            let c = self.step_cost(&s, x1, y1, d, d);
                            let m2 = self.bridge_mode(&s, nx, ny, d);
                            let s2 = self.span_kind(nx, ny, d);
                            s.push(nx, ny, back, 0, c + 1, m2, s2);
                        }
                    }
                }
            }
        } else if !r.build {
            s.push(x1, y1, start_dir as u8, 0, 1, 0, 0);
        } else {
            let sd = start_dir as u8;
            let pv = sd.wrapping_sub(4) & 7;
            if self.edge_valid(&s, x1, y1, pv, pv, true, false, 0, 0) {
                let (nx, ny) = (x1 + DX[usize::from(pv)], y1 + DY[usize::from(pv)]);
                let m = self.bridge_mode(&s, nx, ny, pv);
                let sp = self.span_kind(nx, ny, pv);
                s.push(nx, ny, sd, 0, 1, m, sp);
            }
            if self.edge_valid(&s, x1, y1, sd, sd, true, false, 0, 0) {
                let (nx, ny) = (x1 + DX[usize::from(sd)], y1 + DY[usize::from(sd)]);
                let m = self.bridge_mode(&s, nx, ny, pv.wrapping_sub(4) & 7);
                let sp = self.span_kind(nx, ny, pv.wrapping_sub(4) & 7);
                s.push(nx, ny, pv, 0, 1, m, sp);
            }
        }
        // Main loop.
        let mut last = 0u32;
        let mut span_var = 0u8; // [esp+0x14]: stale span of the straight branch
        let mut mode_var = 0u8; // [esp+0x1c]: local_88
        let mut found: Option<Entry> = None;
        'search: while s.count >= 1 {
            s.count -= 1;
            let e = s.queue.pop(last);
            last = e.cost;
            if r.build {
                let n = s.nodes[&(e.x, e.y)];
                if e.cost > n.cost[usize::from(e.dir)] {
                    continue;
                }
            }
            let (x, y, d) = (e.x, e.y, e.dir);
            if (x, y) == (x2, y2)
                && (end_dir == -1 || i32::from(d) == end_dir || (end_dir - 4) & 7 == i32::from(d))
            {
                if !r.build
                    || self.edge_valid(
                        &s,
                        x,
                        y,
                        d,
                        d.wrapping_sub(4) & 7,
                        false,
                        false,
                        e.mode,
                        e.span,
                    )
                {
                    found = Some(e);
                    break 'search;
                }
                continue;
            }
            let mut mask = 0u32;
            if !r.build {
                if r.electric {
                    let p = self.piece(x, y);
                    if p == 0 || self.ts.u8(P::Track, p, 0x23) & 0x10 == 0 {
                        continue;
                    }
                }
                if s.restricted {
                    let terr = usize::from(self.ts.t8(x, y, 9));
                    if self.ts.u8(P::Company, s.company, 0x479 + terr * 0x1ed) & 1 == 0
                        && !self.is_water(x, y)
                    {
                        continue;
                    }
                }
                mask = self.piece_mask(x, y);
            }
            let cost = e.cost;
            let opp = |d: u8| d.wrapping_sub(4) & 7;
            // Turn right: (d + 3) & 7.
            let t = d.wrapping_add(3) & 7;
            let turn = |a: &Ai<'_, '_>, s: &mut Search, t: u8, mode_var: &mut u8, span_var: u8| {
                let tu = usize::from(t);
                let (nx, ny) = (x + DX[tu], y + DY[tu]);
                if !r.build {
                    if mask & (1 << t) == 0 {
                        return;
                    }
                } else {
                    if e.mode != 0 || e.span != 0 {
                        return;
                    }
                    if !a.edge_valid(s, x, y, t, d, false, false, 0, 0) {
                        return;
                    }
                    *mode_var = a.bridge_mode(s, nx, ny, t);
                    if *mode_var != 0 {
                        return;
                    }
                }
                let c = a.step_cost(s, x, y, t, opp(d));
                s.push(nx, ny, opp(t), d, c + cost, *mode_var, span_var);
            };
            turn(self, &mut s, t, &mut mode_var, span_var);
            // Straight ahead.
            let a = opp(d);
            let au = usize::from(a);
            let (nx, ny) = (x + DX[au], y + DY[au]);
            let go = if !r.build {
                mask & (1 << a) != 0
            } else if self.edge_valid(&s, x, y, a, d, false, false, e.mode, e.span) {
                mode_var = if e.mode != 0 {
                    if MODE_TERMINAL[usize::from(e.mode & 7)] {
                        0
                    } else {
                        e.mode + 1
                    }
                } else {
                    self.bridge_mode(&s, nx, ny, a)
                };
                span_var = self.span_kind(nx, ny, a);
                true
            } else {
                false
            };
            if go {
                let c = self.step_cost(&s, x, y, a, opp(d));
                s.push(nx, ny, opp(a), d, c + cost, mode_var, span_var);
            }
            // Turn left: (d - 3) & 7.
            let t = d.wrapping_sub(3) & 7;
            turn(self, &mut s, t, &mut mode_var, span_var);
            // Reverse (route mode only).
            if !r.build && mask & (1 << d) != 0 {
                let du = usize::from(d);
                let c = self.step_cost(&s, x, y, d, opp(d));
                s.push(x + DX[du], y + DY[du], opp(d), d, c + cost, 0, 0);
            }
        }
        let Some(e) = found else {
            return out;
        };
        out.ok = true;
        if !r.want_path && r.count_owner == 0 {
            return out;
        }
        // Path reconstruction (0x461a63..), destination first.
        let mut nodes: Vec<Node> = Vec::new();
        let (mut x, mut y) = (e.x, e.y);
        let mut d = e.dir;
        let mut prev = d;
        let mut back = s.nodes[&(x, y)].back[usize::from(d)];
        loop {
            let b_k = back;
            nodes.push(Node {
                x,
                y,
                a: d,
                b: prev.wrapping_sub(4) & 7,
                mode: 0,
                span: 0,
            });
            prev = d;
            if r.count_owner != 0 {
                self.route_total += 1;
                if self.piece_owner(x, y) != r.count_owner {
                    self.route_foreign += 1;
                }
            }
            x += DX[usize::from(d)];
            y += DY[usize::from(d)];
            let sd = start_dir;
            let ok_dir = r.p15
                || sd == -1
                || [0i32, 1, -1, 3, -4, -3]
                    .iter()
                    .any(|k| sd == (i32::from(d) + k) & 7);
            if (x, y) == (x1, y1) && ok_dir {
                break;
            }
            back = s.nodes[&(x, y)].back[usize::from(b_k)];
            d = b_k;
        }
        nodes.push(Node {
            x,
            y,
            a: back,
            b: back.wrapping_sub(4) & 7,
            mode: 0,
            span: 0,
        });
        let n = if r.p8 { nodes.len() - 1 } else { nodes.len() };
        nodes.truncate(n);
        if !r.build {
            out.nodes = nodes;
            return out;
        }
        // Build mode: node modes and spans, checked against the shapes (0x461c4b..).
        let mut marks: HashMap<(i32, i32), u32> = HashMap::new();
        let mut modes_at: HashMap<(i32, i32), u8> = HashMap::new();
        let mut spans_at: HashMap<(i32, i32), u8> = HashMap::new();
        let mut prev_mode = 0u8;
        let last_i = n - 1;
        for i in (0..n).rev() {
            let nd = nodes[i];
            let mode = if MODE_TERMINAL[usize::from(prev_mode & 7)] || prev_mode == 0 {
                self.bridge_mode(&s, nd.x, nd.y, nd.b)
            } else {
                prev_mode + 1
            };
            nodes[i].mode = mode;
            let mut sp = self.span_kind(nd.x, nd.y, nd.b);
            if sp == 3 && (i == last_i || nd.b != nodes[i + 1].b) {
                sp = 0;
            }
            nodes[i].span = sp;
            let key = (nd.x, nd.y);
            let mk = marks.entry(key).or_insert(0);
            *mk |= (1 << (nd.a & 7)) | (1 << (nd.b & 7));
            if !self.shape_ok(*mk, false) {
                return Found::default();
            }
            *mk |= self.piece_mask(nd.x, nd.y);
            let mk = *mk;
            if !self.shape_ok(mk, false) || modes_at.get(&key).copied().unwrap_or(0) != 0 {
                return Found::default();
            }
            modes_at.insert(key, mode);
            if spans_at.get(&key).copied().unwrap_or(0) != 0 {
                return Found::default();
            }
            spans_at.insert(key, sp);
            prev_mode = mode;
        }
        let first = nodes[0];
        let cleared = self.ts.t8(first.x, first.y, 2) & 0x20 != 0;
        if !(MODE_END_OK[usize::from(first.mode & 7)] || cleared) {
            return Found::default();
        }
        if first.mode == 1 || first.mode == 6 {
            nodes[0].mode = 0;
        }
        if nodes[0].span == 2 && !cleared {
            return Found::default();
        }
        if nodes[0].span == 1 {
            nodes[0].span = 0;
        }
        // Cost of the path (0x462260..), walking from the source (index n-1) to the destination.
        let (mut cost, mut cost2) = (0i32, 0i32);
        for k in 0..n {
            let nd = nodes[n - 1 - k];
            let (d_in, d_out) = if k == 0 {
                (i32::from(nd.b), -1)
            } else if k + 1 == n {
                (-1, i32::from(nd.a))
            } else {
                (i32::from(nd.b), i32::from(nd.a))
            };
            let (c, c2) = self.tile_cost(nd.x, nd.y, d_in, d_out, nd.mode, nd.span, s.company);
            cost = cost.wrapping_add(c);
            cost2 = cost2.wrapping_add(c2);
        }
        out.cost = cost;
        out.cost2 = cost2;
        out.nodes = nodes;
        out
    }

    /// `track_tile_cost` (`0x463520`) with the track globals of the turn ([`Ai::track_single`],
    /// [`Ai::track_bridge`], [`Ai::track_electric`]) and the bases of the company/territory.
    #[allow(clippy::too_many_arguments)]
    pub fn tile_cost(
        &self,
        x: i32,
        y: i32,
        d_in: i32,
        d_out: i32,
        mode: u8,
        span: u8,
        company: i32,
    ) -> (i32, i32) {
        use crate::track::{CostGlobals, CostStep, Dir, TileInfo, TrackPiece, track_tile_cost};
        let terr = i32::from(self.ts.t8(x, y, 9));
        let m = |e: usize| self.ts.effect(e, company, 0, terr);
        let g = CostGlobals {
            single: self.track_single,
            bridge_type: self.track_bridge as u32,
            electric: self.track_electric,
            company: company as u32,
            edge_base: self.edge_base,
            elec_base: self.elec_base,
            econ_state: self.st_i32(0x0057_0717).clamp(0, 4) as usize,
            bridge_mult: m(0xe),
        };
        let tile = TileInfo {
            terrain_class: usize::from(self.terrain_class(x, y)),
            flags2: self.ts.t8(x, y, 2),
            flags3: self.ts.t8(x, y, 3),
        };
        let p = self.piece(x, y);
        let piece = (p != 0).then(|| TrackPiece {
            edges: crate::track::EdgeMask(self.ts.u8(P::Track, p, 0)),
            owner: self.ts.u8(P::Track, p, 2),
            bridge_type: self.ts.u8(P::Track, p, 0x11),
            double: self.ts.u8(P::Track, p, 0x23) & 1 != 0,
            electrified: self.ts.u8(P::Track, p, 0x23) & 0x10 != 0,
        });
        let step = CostStep {
            d_in: (d_in >= 0).then(|| Dir::new(d_in as u8)).flatten(),
            d_out: (d_out >= 0).then(|| Dir::new(d_out as u8)).flatten(),
            mode,
            span,
        };
        let c = track_tile_cost(&self.tables.track, &tile, piece.as_ref(), &g, step);
        (c.cost, c.cost2.unwrap_or(0))
    }
}
