//! The track tile rebuild `0x4cb090(x0, y0, x1, y1, preview)` and the redraw bookkeeping that
//! the save keeps (terrain-track.md "Reconstrução dos tiles").
//!
//! `0x4cb090` rebuilds, for a rectangle and the track layers 4 (single) and 5 (double), the
//! overlay sprites of each tile (its own track and the corner pieces of the neighbours'
//! diagonals) and the derived fields of the pieces: span direction (`piece+5`), bridge sprite
//! (`piece+0x10`), straight axis (`piece+0x23 & 8`, `piece+0xf`), the span bit of the tile
//! (`tile+3 & 0x40`). The overlays themselves are a run-time list (`tile+10`, never saved, and
//! only this function adds to it); the save keeps "the tile has overlays" (`tile+6 & 0x80`, and
//! `tile+2 & 0x40`, recomputed by `0x4258e0`). Here only the presence is modelled.
//!
//! The redraw rectangle of `0x417770` is accumulated and flushed like `0x417c40` -> `0x417880`
//! (`tile+6 & 1`, flat tile), which the original runs when the screen is painted (at the latest
//! when the game is saved).

use crate::ai::Ai;
use crate::ai::path::Search;
use crate::fp::ftol;
use crate::trainstep::P;

use super::{DX, DY};

/// `0x5365e8`: (shape, aux) per edge mask; `0x5369e8`: (shape, aux) per corner mask.
const SHAPES: u32 = 0x0053_65e8;
const CORNER_SHAPES: u32 = 0x0053_69e8;
/// `0x536594 + shape*4 + aux`: sprite of a shape (99 = none).
const SPRITES: u32 = 0x0053_6594;

impl Ai<'_, '_> {
    fn sprite_ok(&self, table: u32, m: u32) -> bool {
        let s = self.ts.img.u8(table + 2 * m);
        if s == 99 {
            return false;
        }
        let a = self.ts.img.u8(table + 2 * m + 1);
        self.ts.img.u8(SPRITES + u32::from(s) * 4 + u32::from(a)) != 99
    }

    /// `0x417770`: adds a rectangle to the pending redraw.
    pub fn dirty(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.dirty_rect = Some(match self.dirty_rect {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    }

    /// `0x417c40`: flushes the pending redraw rectangle: the flat-tile bit (`0x417880`,
    /// `tile+6 & 1`) and the uniform-shade bit (`0x416bf0` + `0x416d40`, `tile+6 & 0x20`).
    pub fn flush_redraw(&mut self) {
        let Some((x0, y0, x1, y1)) = self.dirty_rect.take() else {
            return;
        };
        crate::tilemark::recompute_flat(self.ts.s, x0, y0, x1, y1);
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(w - 1), y1.min(h - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.shade_flag(x, y);
            }
        }
    }

    /// `0x416bf0`: the light level of a tile (`tile+0xe`, run time) from the slopes of its
    /// corners: `31 - ftol((0.25 v2 + 0.75 v1) * 31.99)` with `v = (atan(s * 0.046875) +
    /// 0.6108...)` folded at pi/2 and scaled by 0.8187, clamped to `[0.01, 1]`.
    pub fn tile_shade(&self, x: i32, y: i32) -> i32 {
        let h = |c| self.corner_h(x, y, c);
        tile_shade_from([h(0), h(1), h(2), h(3)])
    }

    /// `0x416d40`: the corner light levels (mean of the tiles around each vertex) and
    /// `tile+6 & 0x20` when they all equal the tile's own level.
    fn shade_flag(&mut self, x: i32, y: i32) {
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let s = |xx: i32, yy: i32| self.tile_shade(xx, yy);
        let me = s(x, y);
        let corners: [i32; 4] = if x < 1 || y < 1 || w - 1 <= x || h - 1 <= y {
            std::array::from_fn(|k| {
                let (bx, by) = match k {
                    0 => (x - 1, y - 1),
                    1 => (x, y - 1),
                    2 => (x, y),
                    _ => (x - 1, y),
                };
                let (mut n, mut sum) = (0, 0);
                for yy in by..=by + 1 {
                    for xx in bx..=bx + 1 {
                        if self.in_map(xx, yy) {
                            n += 1;
                            sum += s(xx, yy);
                        }
                    }
                }
                sum / n
            })
        } else {
            [
                (s(x, y - 1) + s(x - 1, y - 1) + s(x - 1, y) + me) >> 2,
                (s(x + 1, y) + s(x + 1, y - 1) + s(x, y - 1) + me) >> 2,
                (s(x + 1, y) + s(x + 1, y + 1) + s(x, y + 1) + me) >> 2,
                (s(x - 1, y) + s(x, y + 1) + s(x - 1, y + 1) + me) >> 2,
            ]
        };
        let b = |v: i32| v as u8;
        let f6 = self.ts.t8(x, y, 6);
        let uniform = corners.iter().all(|&c| b(c) == b(me));
        self.ts
            .set_t8(x, y, 6, if uniform { f6 | 0x20 } else { f6 & 0xdf });
    }

    /// Terrain id outside the base range of its class (`0x4258e0`): the run-time table
    /// `0x540aa4 + class*0xc0` (first own id) and `+0x10` (end of the own ids), built by
    /// `0x471b10` ([`rt2_formats::terrain::Tables::start`]).
    fn terrain_off_base(&self, x: i32, y: i32) -> bool {
        let id = i32::from(self.ts.tile_terrain_id(x, y));
        let c = usize::from(self.terrain_class(x, y));
        let st = rt2_formats::terrain::tables().start[c];
        id < st[0] || st[4] < id
    }

    /// `0x4cb090(x0, y0, x1, y1, 0)` (real layers 4 and 5).
    pub fn tile_rebuild(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let (x0, y0) = (x0.max(0), y0.max(0));
        let (x1, y1) = (x1.min(w - 1), y1.min(h - 1));
        if x0 > x1 || y0 > y1 {
            return;
        }
        let bw = (x1 - x0 + 1) as usize;
        let n = bw * (y1 - y0 + 1) as usize;
        let at = |x: i32, y: i32| (y - y0) as usize * bw + (x - x0) as usize;
        let mut had = vec![false; n];
        let mut added = vec![false; n];
        for y in y0..=y1 {
            for x in x0..=x1 {
                had[at(x, y)] = self.ts.t8(x, y, 6) & 0x80 != 0;
            }
        }
        let search = Search::idle();
        let mut stale50: u32 = 0; // `local_50` survives from one tile to the next
        for layer in 4..=5u8 {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let b3 = self.ts.t8(x, y, 3);
                    self.ts.set_t8(x, y, 3, b3 & 0xbf);
                    let mut c1 = false;
                    let mut cmask = 0u32;
                    for k in 0..8 {
                        let (dx, dy, bit) = match k {
                            0 => (0, -1, 8),
                            1 => (1, 0, 0x80),
                            2 => (1, 0, 0x20),
                            3 => (0, 1, 2),
                            4 => (0, 1, 0x80),
                            5 => (-1, 0, 8),
                            6 => (-1, 0, 2),
                            _ => (0, -1, 0x20),
                        };
                        let (nx, ny) = (x + dx, y + dy);
                        if !self.in_map(nx, ny) {
                            continue;
                        }
                        let np = self.piece(nx, ny);
                        stale50 = if np == 0 {
                            0
                        } else {
                            u32::from(self.ts.u8(P::Track, np, 0))
                        };
                        if stale50 & bit == 0 {
                            continue;
                        }
                        let double = np != 0 && self.ts.u8(P::Track, np, 0x23) & 1 != 0;
                        let take = if layer == 4 { !double } else { double };
                        if !take {
                            continue;
                        }
                        if !c1
                            && self.ts.t8(nx, ny, 2) & 0x80 == 0
                            && self.ts.t8(nx, ny, 3) & 0x40 == 0
                        {
                            c1 = true;
                        }
                        cmask |= 1 << (k >> 1);
                    }
                    let mut to_870 = false;
                    if cmask != 0 && c1 {
                        if self.sprite_ok(CORNER_SHAPES, cmask) {
                            added[at(x, y)] = true;
                        } else {
                            to_870 = true;
                        }
                    }
                    let p = self.piece(x, y);
                    if !to_870 {
                        // LAB_004cb5df
                        if p == 0 {
                            stale50 = 0;
                        } else {
                            stale50 = u32::from(self.ts.u8(P::Track, p, 0));
                        }
                        let mut done = false;
                        if p != 0 && self.ts.u8(P::Track, p, 5) & 0xc != 0 {
                            let b3 = self.ts.t8(x, y, 3);
                            self.ts.set_t8(x, y, 3, b3 | 0x40);
                            let m = u32::from(self.ts.u8(P::Track, p, 0))
                                | u32::from(self.ts.u8(P::Track, p, 1));
                            let (mut cnt, mut last) = (0, 0u8);
                            for k in 0..8u8 {
                                if m & (1 << k) == 0 {
                                    continue;
                                }
                                let (nx, ny) = (x + DX[usize::from(k)], y + DY[usize::from(k)]);
                                if !self.in_map(nx, ny) {
                                    continue;
                                }
                                let q = self.piece(nx, ny);
                                if q != 0 && self.ts.u8(P::Track, q, 5) & 0xc != 0 {
                                    cnt += 1;
                                    last = k;
                                }
                            }
                            let p5 = self.ts.u8(P::Track, p, 5);
                            let v = if cnt == 1 {
                                (!(p5 | 0xf0) & 0x1f) ^ (last << 5).wrapping_sub(0x61)
                            } else {
                                ((last & 3) << 5) | (p5 & 0xf)
                            };
                            self.ts.set8(P::Track, p, 5, v);
                            done = true;
                        }
                        if !done && self.ts.t8(x, y, 2) & 0x80 != 0 && p != 0 {
                            let b = self.ts.u8(P::Track, p, 0x23);
                            if b & 0x20 == 0 {
                                let lm = stale50;
                                let cv: u8 = if lm & 0x11 != 0 {
                                    0
                                } else if lm & 0x22 != 0 {
                                    let (ux, uy) = self.bridge_probe(x, y, b, 1, 5);
                                    if self.river_mask(&search, ux, uy) & 0x50 != 0 {
                                        5
                                    } else {
                                        1
                                    }
                                } else if lm & 0x44 != 0 {
                                    2
                                } else {
                                    let (ux, uy) = self.bridge_probe(x, y, b, 3, 7);
                                    if self.river_mask(&search, ux, uy) & 0x14 != 0 {
                                        7
                                    } else {
                                        3
                                    }
                                };
                                let c3 = (self.ts.u8(P::Track, p, 0x11).wrapping_mul(16))
                                    .wrapping_add(cv)
                                    .wrapping_mul(2);
                                let v = if b & 3 != 0 {
                                    c3.wrapping_sub(0x10)
                                } else {
                                    c3.wrapping_sub(0x20)
                                };
                                self.ts.set8(P::Track, p, 0x10, v);
                            }
                        }
                    }
                    // LAB_004cb870: straight axis.
                    let skip = p == 0
                        || self.ts.t8(x, y, 2) & 0x80 != 0
                        || self.ts.t8(x, y, 3) & 0x40 != 0
                        || (self.ts.u8(P::Track, p, 0x23) & 0x10 == 0
                            && (!self.track_electric || self.ts.t8(x, y, 6) & 0x40 == 0));
                    if skip {
                        if p != 0 {
                            let b = self.ts.u8(P::Track, p, 0x23);
                            self.ts.set8(P::Track, p, 0x23, b & 0xf7);
                        }
                    } else {
                        let m = u32::from(self.ts.u8(P::Track, p, 0))
                            | u32::from(self.ts.u8(P::Track, p, 1));
                        let (mut axis, mut last) = (-1i32, -1i32);
                        for k in 0..8u32 {
                            if m & (1 << k) != 0 {
                                last = k as i32;
                                if m & (1 << ((k + 4) & 7)) != 0
                                    && m & (1 << ((k + 2) & 7)) == 0
                                    && m & (1 << ((k + 6) & 7)) == 0
                                {
                                    axis = k as i32;
                                }
                            }
                        }
                        if axis == -1
                            && m & (1 << ((last + 2) & 7)) == 0
                            && m & (1 << ((last - 2) & 7)) == 0
                        {
                            axis = last;
                        }
                        let b = self.ts.u8(P::Track, p, 0x23);
                        if axis == -1 {
                            self.ts.set8(P::Track, p, 0x23, b & 0xf7);
                        } else {
                            self.ts.set8(P::Track, p, 0x23, b | 8);
                            self.ts.set8(P::Track, p, 0xf, (axis & 3) as u8);
                        }
                    }
                    // Own track overlay.
                    let lm = stale50;
                    if self.ts.t8(x, y, 3) & 0x40 == 0
                        && lm != 0
                        && self.ts.u8(P::Track, p, 0x11) == 0
                    {
                        let double = p != 0 && self.ts.u8(P::Track, p, 0x23) & 1 != 0;
                        let want = if layer == 4 { !double } else { double };
                        if want && self.sprite_ok(SHAPES, lm) {
                            added[at(x, y)] = true;
                        }
                    }
                    if cmask != 0 && lm != 0 && layer == 5 {
                        added[at(x, y)] = true;
                    }
                }
            }
        }
        // 0x4258e0 as called by the removals and additions of the passes.
        for y in y0..=y1 {
            for x in x0..=x1 {
                let i = at(x, y);
                let f2 = self.ts.t8(x, y, 2);
                let f6 = self.ts.t8(x, y, 6);
                // Overlays of the other layers: rivers (layer 2, `0x4294d0`, tile bit 0) and
                // roads (layer 3, `0x428fe0`, `tile+3 & 0x20`).
                // Terrain transitions (layers 0/1, `0x42a7d0`) are not modelled: a tile that
                // had overlays without track overlays before the change keeps them.
                let other = self.ts.t8(x, y, 0) & 1 != 0
                    || self.ts.t8(x, y, 3) & 0x20 != 0
                    || !self.overlay_pre(x, y);
                if added[i] || (had[i] && other) {
                    self.ts.set_t8(x, y, 6, f6 | 0x80);
                    self.ts.set_t8(x, y, 2, f2 | 0x40);
                } else if had[i] {
                    self.ts.set_t8(x, y, 6, f6 & 0x7f);
                    let off = self.terrain_off_base(x, y);
                    self.ts
                        .set_t8(x, y, 2, (f2 & 0xbf) | if off { 0x40 } else { 0 });
                }
            }
        }
        crate::tilemark::recompute_flags(self.ts.s, x0 - 1, y0 - 1, x1 + 1, y1 + 1);
    }

    /// Records which tiles of a rectangle have track overlays (layers 4/5) before a change,
    /// for [`Ai::tile_rebuild`].
    pub fn snapshot_overlays(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let mut v = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                v.push(self.in_map(x, y) && self.track_overlay(x, y));
            }
        }
        self.overlay_snapshot = Some((x0, y0, x1, y1, v));
    }

    fn overlay_pre(&self, x: i32, y: i32) -> bool {
        if let Some((x0, y0, x1, y1, v)) = &self.overlay_snapshot
            && (*x0..=*x1).contains(&x)
            && (*y0..=*y1).contains(&y)
        {
            return v[((y - y0) * (x1 - x0 + 1) + (x - x0)) as usize];
        }
        self.track_overlay(x, y)
    }

    /// Whether `0x4cb090` gives the tile an overlay in layer 4 or 5 (presence only, without
    /// the side effects).
    pub fn track_overlay(&self, x: i32, y: i32) -> bool {
        let own = self.piece(x, y);
        let own_mask = if own == 0 {
            0
        } else {
            u32::from(self.ts.u8(P::Track, own, 0))
        };
        let own_double = own != 0 && self.ts.u8(P::Track, own, 0x23) & 1 != 0;
        for layer in 4..=5u8 {
            let mut c1 = false;
            let mut cmask = 0u32;
            for k in 0..8 {
                let (dx, dy, bit) = match k {
                    0 => (0, -1, 8),
                    1 => (1, 0, 0x80),
                    2 => (1, 0, 0x20),
                    3 => (0, 1, 2),
                    4 => (0, 1, 0x80),
                    5 => (-1, 0, 8),
                    6 => (-1, 0, 2),
                    _ => (0, -1, 0x20),
                };
                let (nx, ny) = (x + dx, y + dy);
                if !self.in_map(nx, ny) {
                    continue;
                }
                let np = self.piece(nx, ny);
                if np == 0 || u32::from(self.ts.u8(P::Track, np, 0)) & bit == 0 {
                    continue;
                }
                let double = self.ts.u8(P::Track, np, 0x23) & 1 != 0;
                if (layer == 4) == double {
                    continue;
                }
                if self.ts.t8(nx, ny, 2) & 0x80 == 0 && self.ts.t8(nx, ny, 3) & 0x40 == 0 {
                    c1 = true;
                }
                cmask |= 1 << (k >> 1);
            }
            if cmask != 0 && c1 && self.sprite_ok(CORNER_SHAPES, cmask) {
                return true;
            }
            if own_mask != 0
                && self.ts.t8(x, y, 3) & 0x40 == 0
                && self.ts.u8(P::Track, own, 5) & 0xc == 0
                && self.ts.u8(P::Track, own, 0x11) == 0
                && (layer == 4) != own_double
                && self.sprite_ok(SHAPES, own_mask)
            {
                return true;
            }
            if layer == 5 && cmask != 0 && own_mask != 0 {
                return true;
            }
        }
        false
    }

    /// Tile probed for the river of a bridge sprite: the tile itself on a river bridge
    /// (`piece+0x23 & 0x40`), else the neighbour `d1` if it is a bridge tile, else `d2`.
    fn bridge_probe(&self, x: i32, y: i32, b23: u8, d1: usize, d2: usize) -> (i32, i32) {
        if b23 & 0x40 != 0 {
            return (x, y);
        }
        let (ax, ay) = (x + DX[d1], y + DY[d1]);
        if self.ts.t8(ax, ay, 2) & 0x80 != 0 {
            (ax, ay)
        } else {
            (x + DX[d2], y + DY[d2])
        }
    }
}

/// `0x416bf0` on the corner heights `[NW, NE, SE, SW]` of a tile: its light level `0..=31`.
/// (`3.14159265` is the executable's constant `0x535248`, not `PI`.)
#[allow(clippy::approx_constant)]
pub fn tile_shade_from(hs: [i32; 4]) -> i32 {
    let (e0, e1, se, sw) = (hs[0], hs[1], hs[2], hs[3]);
    let fold = |s: i32| {
        let mut t = (f64::from(s) * 0.046875).atan() + 0.610_820_244_328_097_8;
        if t > 1.570_796_325 {
            t = 3.141_592_65 - t;
        }
        t
    };
    let t1 = fold((e1 - e0 - sw + se) >> 1);
    let t2 = fold((sw - e0 - e1 + se) >> 1);
    let k = 0.818_693_028_200_202_8;
    let mut v1 = (t1 * k) as f32;
    let mut v2 = t2 * k;
    if f64::from(v1) < 0.01 {
        v1 = f32::from_bits(0x3c23_d70a);
    }
    if v2 < 0.01 {
        v2 = f64::from(0.01f32);
    }
    if v1 > 1.0 {
        v1 = 1.0;
    }
    if v2 > 1.0 {
        v2 = 1.0;
    }
    let r = ftol((v2 * 0.25 + f64::from(v1) * 0.75) * 31.99);
    i32::from((31u8).wrapping_sub(r as u8))
}
