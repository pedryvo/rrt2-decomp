//! `track_lay_route` (`0x4cc060`, terrain-track.md "Construção de trilho"): finds the build path
//! between two tiles (`track_pathfind` in build mode), levels the terrain under it
//! (`track_level_edge` x3, `0x427ba0` at bridges and spans), adds the track pieces
//! (`tile_track_add` `0x4164f0`, `tile_track_set_double` `0x416210`), sets each piece's grade
//! and book value, charges the path and the earthwork to account 3, rebuilds the tiles
//! (`0x4cb090`) and the station networks (`0x46a370`) and flags every train for a route check.

use crate::ai::Ai;
use crate::ai::path::Req;
use crate::fp::ftol;
use crate::trainstep::P;

use super::account_move;

/// State-block offsets (VA - `0x5704e8`) of the global track counters.
const ST_CELLS: usize = 0x3835; // 0x573d1d
const ST_SINGLE: usize = 0x3839; // 0x573d21
const ST_DOUBLE: usize = 0x383d; // 0x573d25
const ST_ELECTRIC: usize = 0x3841; // 0x573d29
/// `0x573d7e..0x573d8a`: bounding box of the built area (x0, y0, x1, y1).
const ST_BOX: usize = 0x3896;

/// Arguments of `track_lay_route` (`fastcall(company, x1, y1, x2, y2, d1, d2, preview, single,
/// electric, bridge)`); `preview` (the UI ghost) is not modelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayRoute {
    pub company: i32,
    pub x1: i32,
    pub y1: i32,
    /// `-1` (or the source tile): one step from the source towards `d1`.
    pub x2: i32,
    pub y2: i32,
    /// Direction the track must leave the source tile with (`-1` = any).
    pub d1: i32,
    /// Direction it must reach the destination with (`-1` = any).
    pub d2: i32,
    /// `track_single` (`0x5534b0`): 1 simple, 0 double track.
    pub single: bool,
    /// `track_electric` (`0x566fcc`).
    pub electric: bool,
    /// `track_bridge_type` (`0x53ce18`).
    pub bridge: i32,
}

/// What a successful [`Ai::track_lay_route`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Laid {
    /// Total charged to account 3 (`0x5676a8`): path cost plus earthwork.
    pub cost: i32,
    pub tiles: i32,
}

impl Ai<'_, '_> {
    fn st_add(&mut self, o: usize, d: i32) {
        let v = self.ts.state_i32(o).wrapping_add(d);
        self.ts.set_state_i32(o, v);
    }

    fn co_add(&mut self, c: i32, o: usize, d: i32) {
        let v = self.ts.i32(P::Company, c, o).wrapping_add(d);
        self.ts.seti32(P::Company, c, o, v);
    }

    /// `tile_track_add(tile, mask, mode, span, company, count)` (`0x4164f0`).
    pub fn tile_track_add(&mut self, x: i32, y: i32, mask: u8, mode: u8, span: u8, company: i32) {
        let tid = self.piece(x, y);
        let bridge_river = matches!(mode, 2 | 4 | 7);
        if tid == 0 {
            if company != 0 {
                let terr = i32::from(self.ts.t8(x, y, 9));
                self.co_add(company, 0x182, 1);
                if self.ts.exists(P::Territory, terr) {
                    let v = self.ts.i16(P::Territory, terr, 0x50).wrapping_add(1);
                    self.ts.set16(P::Territory, terr, 0x50, v as u16);
                }
                let o = terr as usize * 0x1ed + 0x47c;
                let v = self.ts.i16(P::Company, company, o).wrapping_add(1);
                self.ts.set16(P::Company, company, o, v as u16);
                self.co_add(company, 0x186, 1);
                self.st_add(ST_CELLS, 1);
                self.st_add(ST_SINGLE, 1);
                if self.track_electric {
                    self.co_add(company, 0x18e, 1);
                    self.st_add(ST_ELECTRIC, 1);
                }
            }
            let mut r = vec![0u8; self.ts.record_size(P::Track)];
            r[0] = mask;
            let mut b23 = u8::from(self.track_electric) << 4;
            r[5] = u8::from(span != 0) << 3;
            r[2] = company as u8;
            if mode != 0 {
                r[0x11] = self.track_bridge as u8;
                if self.track_bridge == 1 {
                    b23 &= 0xfe;
                }
                let b2 = self.ts.t8(x, y, 2);
                self.ts.set_t8(x, y, 2, b2 | 0x80);
                b23 = (u8::from(bridge_river) << 6) | (b23 & 0x9f);
            }
            r[0x23] = b23;
            let id = self.ts.alloc(P::Track, r);
            self.ts.set_t16(x, y, 7, id as u16);
            let b2 = self.ts.t8(x, y, 2);
            self.ts.set_t8(x, y, 2, b2 | 0x20);
            return;
        }
        let m = self.ts.u8(P::Track, tid, 0) | mask;
        self.ts.set8(P::Track, tid, 0, m);
        let b23 = self.ts.u8(P::Track, tid, 0x23);
        if self.track_electric && b23 & 0x10 == 0 {
            self.ts.set8(P::Track, tid, 0x23, b23 | 0x10);
            let owner = i32::from(self.ts.u8(P::Track, tid, 2));
            if owner != 0 {
                self.co_add(owner, 0x18e, 1);
            }
            self.st_add(ST_ELECTRIC, 1);
        }
        if mode != 0 {
            self.ts.set8(P::Track, tid, 0x11, self.track_bridge as u8);
            if self.track_bridge == 1 {
                let b = self.ts.u8(P::Track, tid, 0x23);
                self.ts.set8(P::Track, tid, 0x23, b & 0xfe);
            }
            let b = self.ts.u8(P::Track, tid, 0x23);
            self.ts.set8(
                P::Track,
                tid,
                0x23,
                (u8::from(bridge_river) << 6) | (b & 0x9f),
            );
            let b2 = self.ts.t8(x, y, 2);
            self.ts.set_t8(x, y, 2, b2 | 0x80);
        }
        let b5 = self.ts.u8(P::Track, tid, 5);
        self.ts
            .set8(P::Track, tid, 5, (u8::from(span != 0) << 3) | (b5 & 0xf7));
        let b2 = self.ts.t8(x, y, 2);
        self.ts.set_t8(x, y, 2, b2 | 0x20);
    }

    /// `tile_track_set_double(tile, double)` (`0x416210`): refused on a span tile or a wooden
    /// bridge when doubling.
    pub fn tile_track_set_double(&mut self, x: i32, y: i32, double: bool) {
        let p = self.piece(x, y);
        let b23 = self.ts.u8(P::Track, p, 0x23);
        if (b23 & 1 != 0) == double {
            return;
        }
        if double
            && (self.ts.t8(x, y, 3) & 0x40 != 0
                || (self.ts.t8(x, y, 2) & 0x80 != 0 && self.ts.u8(P::Track, p, 0x11) == 1))
        {
            return;
        }
        let owner = i32::from(self.ts.u8(P::Track, p, 2));
        let d = if double { 1 } else { -1 };
        if owner != 0 {
            self.co_add(owner, 0x18a, d);
            self.co_add(owner, 0x186, -d);
        }
        self.st_add(ST_DOUBLE, d);
        self.st_add(ST_SINGLE, -d);
        self.ts
            .set8(P::Track, p, 0x23, (b23 & 0xfe) | u8::from(double));
    }

    /// `track_lay_route` (`0x4cc060`) in real mode. Returns `None` when no path was found
    /// (nothing changes then).
    pub fn track_lay_route(&mut self, a: LayRoute) -> Option<Laid> {
        self.track_single = a.single;
        self.track_bridge = a.bridge;
        self.track_electric = a.electric;
        let company = a.company;
        let (mut x2, mut y2) = (a.x2, a.y2);
        if x2 == -1 || (a.x1 == x2 && a.y1 == y2) {
            x2 = a.x1 + super::DX[(a.d1 & 7) as usize];
            y2 = a.y1 + super::DY[(a.d1 & 7) as usize];
        }
        let mut r = Req::route(a.x1, a.y1, x2, y2);
        r.build = true;
        r.start_dir = if a.d1 == -1 { -1 } else { (a.d1 - 4) & 7 };
        r.end_dir = a.d2;
        r.company = company;
        r.want_path = true;
        r.count_new = true;
        let found = self.pathfind(&r);
        if !found.ok {
            return None;
        }
        self.ts.log.ran("track_lay_route");
        // The path ring of 0x460850 is source first.
        let mut steps = found.nodes.clone();
        steps.reverse();
        let n = steps.len();
        let costs = &found.node_costs;
        let (xs, ys) = (|i: usize| steps[i].x, |i: usize| steps[i].y);
        {
            let (mut a0, mut b0, mut a1, mut b1) = (9999, 9999, -1, -1);
            for st in &steps {
                a0 = a0.min(st.x);
                b0 = b0.min(st.y);
                a1 = a1.max(st.x);
                b1 = b1.max(st.y);
            }
            self.snapshot_overlays(a0 - 1, b0 - 1, a1 + 1, b1 + 1);
        }
        self.elev_log_begin();
        for pass in 0..3 {
            for i in 0..n.saturating_sub(1) {
                self.level_edge(xs(i), ys(i), steps[i].b, false, pass == 0);
            }
        }
        if n > 2 {
            let mut j = 1;
            while j <= n - 2 {
                let mode = steps[j].mode;
                let span = steps[j].span;
                if matches!(mode, 1 | 4 | 6) || span == 1 {
                    let k;
                    if span == 1 {
                        let mut kk = j + 1;
                        while kk < n - 1 && steps[kk].span != 3 {
                            kk += 1;
                        }
                        if steps[kk].span != 3 {
                            j += 1;
                            continue;
                        }
                        k = kk;
                    } else {
                        let kk = if steps[j].b & 1 == 0 { j + 2 } else { j + 1 };
                        if kk >= n || steps[kk].mode == 0 {
                            break;
                        }
                        k = kk;
                    }
                    self.level_span(xs(j), ys(j), steps[j].b, xs(k), ys(k));
                }
                j += 1;
            }
        }
        let (mut bx0, mut by0, mut bx1, mut by1) = (9999, 9999, -1, -1);
        for i in 0..n {
            let (x, y) = (xs(i), ys(i));
            bx0 = bx0.min(x);
            by0 = by0.min(y);
            bx1 = bx1.max(x);
            by1 = by1.max(y);
            let st = steps[i];
            let mask: u8 = if i == n - 1 {
                1 << st.a
            } else if i == 0 {
                1 << st.b
            } else {
                (1 << st.a) | (1 << st.b)
            };
            let p = self.piece(x, y);
            if st.mode != 0 && p != 0 && self.ts.u8(P::Track, p, 2) != 0 {
                let owner = i32::from(self.ts.u8(P::Track, p, 2));
                if owner != company {
                    let b = self.ts.u8(P::Track, p, 0x23);
                    self.ts.set8(P::Track, p, 0x23, b & 0xfd);
                    continue;
                }
            }
            self.tile_track_add(x, y, mask, st.mode, st.span, company);
            let p = self.piece(x, y);
            if !a.single
                && st.span == 0
                && (st.mode != 0 || self.ts.t8(x, y, 2) & 0x80 == 0)
                && self.ts.u8(P::Track, p, 0x23) & 1 == 0
            {
                let res = self.ts.u16(P::Track, p, 3);
                if res != 0 && self.ts.u16(P::P80, i32::from(res), 1) != 0 {
                    todo_hook!(
                        self.ts.log,
                        "track_double_under_train",
                        "0x4cc060: trains on a tile being doubled (0x4d3c80, 0x4d4e40, 0x4d52a0)",
                        "terrain-track.md 2.6"
                    );
                }
                self.tile_track_set_double(x, y, true);
            }
        }
        for i in 0..n {
            let (x, y) = (xs(i), ys(i));
            let p = self.piece(x, y);
            let w1 = self.ts.map_w() + 1;
            let base = y * w1 + x;
            let elev = &self.ts.s.map.elevation;
            let height =
                |off: i32| i32::from(elev.get((base + off) as usize).copied().unwrap_or(0));
            let g = crate::trains::track_segment_grade(
                &height,
                [0, 1, w1 + 1, w1],
                i32::from(steps[i].a),
                i32::from(steps[i].b),
            );
            let g = ftol(g * 1000.0).unsigned_abs().min(0x78) as u8;
            self.ts.set8(P::Track, p, 0x12, g);
            let c = costs.get(i).copied().unwrap_or(0) >> 3;
            let v = self.ts.i16(P::Track, p, 0x15).wrapping_add(c as i16);
            self.ts.set16(P::Track, p, 0x15, v as u16);
        }
        // Earthwork: 600 * m(0xa) per unit of height moved, also spread over the book value of
        // the track pieces around each moved vertex.
        let per = ftol(f64::from(self.ts.effect(10, company, 0, 0)) * f64::from(600.0f32));
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let edge = bx0 < 1 || by0 < 1 || w - 1 <= bx1 || h - 1 <= by1;
        let log = self.elev_log_end();
        let mut moved = 0i32;
        for &(idx, old) in &log {
            let d = (self.vertex(idx) - old).abs();
            if d == 0 {
                continue;
            }
            moved += d;
            let c = d.wrapping_mul(per);
            let (cy, cx) = (idx / (w + 1), idx % (w + 1));
            if edge && !(0 < cx && cx < w - 1 && 0 < cy && cy < h - 1) {
                continue;
            }
            let around = [(0, 0), (-1, 0), (-1, -1), (0, -1)];
            let cnt = around
                .iter()
                .filter(|&&(dx, dy)| self.ts.t8(cx + dx, cy + dy, 2) & 0x20 != 0)
                .count() as i32;
            let share = if cnt < 1 { 0 } else { (c / cnt) >> 3 };
            for (dx, dy) in around {
                if self.ts.t8(cx + dx, cy + dy, 2) & 0x20 != 0 {
                    let p = self.piece(cx + dx, cy + dy);
                    let v = self.ts.i16(P::Track, p, 0x15).wrapping_add(share as i16);
                    self.ts.set16(P::Track, p, 0x15, v as u16);
                }
            }
        }
        let cost = found.cost.wrapping_add(per.wrapping_mul(moved));
        self.tile_rebuild(bx0 - 1, by0 - 1, bx1 + 1, by1 + 1);
        self.overlay_snapshot = None;
        self.dirty(bx0 - 1, by0 - 1, bx1 + 1, by1 + 1);
        if cost != 0 || found.cost2 != 0 {
            account_move(self.ts, company, 3, f64::from(cost));
            self.co_add(company, 0x4260, -found.new_tiles);
            let (sx, sy) = (a.x1, a.y1);
            let (xa, xb) = ((sx - 20).max(0), (sx + 20).min(w - 1));
            let (ya, yb) = ((sy - 20).max(0), (sy + 20).min(h - 1));
            for y in ya..=yb {
                for x in xa..=xb {
                    if self.piece_owner(x, y) == company {
                        if x < self.ts.i32(P::Company, company, 0x4233) {
                            self.ts.seti32(P::Company, company, 0x4233, x);
                        }
                        if self.ts.i32(P::Company, company, 0x423b) < x {
                            self.ts.seti32(P::Company, company, 0x423b, x);
                        }
                        if y < self.ts.i32(P::Company, company, 0x4237) {
                            self.ts.seti32(P::Company, company, 0x4237, y);
                        }
                        if self.ts.i32(P::Company, company, 0x423f) < y {
                            self.ts.seti32(P::Company, company, 0x423f, y);
                        }
                    }
                    let b: [i32; 4] = std::array::from_fn(|k| self.ts.state_i32(ST_BOX + 4 * k));
                    if x < b[0] {
                        self.ts.set_state_i32(ST_BOX, x);
                    }
                    if b[2] < x {
                        self.ts.set_state_i32(ST_BOX + 8, x);
                    }
                    if y < b[1] {
                        self.ts.set_state_i32(ST_BOX + 4, y);
                    }
                    if b[3] < y {
                        self.ts.set_state_i32(ST_BOX + 12, y);
                    }
                }
            }
        }
        // net_min_peer_version() is the game's own version (>= 0x232f) in a local game.
        if self.ts.s.stations.count < 0x14 {
            self.station_networks_rebuild();
        } else {
            self.networks_dirty = true;
        }
        self.trains_refresh_all();
        self.trains_flag_route_check();
        Some(Laid {
            cost,
            tiles: n as i32,
        })
    }
}
