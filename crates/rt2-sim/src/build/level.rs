//! Earthwork under new track (terrain-track.md 2.3): `track_level_edge` (`0x427ff0`) and its
//! helpers `0x4274b0` (two corners of one tile), `0x427050` (corners of two tiles), `0x4278f0`
//! (diagonal edge), `0x427ba0` (bridge approach) and `0x4260c0` (set one corner), with the
//! vertex log of `0x426250`/`0x426280` that `track_lay_route` charges.
//!
//! A corner is "free" when no track of the four tiles around it uses it (tables `0x53a0ac`
//! (dx, dy) per corner and `0x53a12c` edge masks); only free corners move. Heights are clamped
//! to `-255..=510`.

use crate::ai::Ai;

/// `0x53a0ac + c*0x20 + k*8`: the four tiles around corner `c` of a tile.
const AROUND: [[(i32, i32); 4]; 4] = [
    [(0, 0), (-1, 0), (-1, -1), (0, -1)],
    [(1, 0), (0, 0), (0, -1), (1, -1)],
    [(1, 1), (0, 1), (0, 0), (1, 0)],
    [(0, 1), (-1, 1), (-1, 0), (0, 0)],
];
/// `0x53a12c`: edges of the k-th tile around a corner that depend on that corner.
const AROUND_MASK: [u32; 4] = [238, 119, 238, 119];

fn clamp(v: i32) -> i32 {
    if v < 0x1ff {
        if v < -0xff { -0xff } else { v }
    } else {
        0x1fe
    }
}

impl Ai<'_, '_> {
    /// `0x426250`: starts recording the vertices the earthwork moves.
    pub fn elev_log_begin(&mut self) {
        self.elev_log = Some(Vec::new());
    }

    /// `0x426280` / `0x4263b0`: stops recording; returns the log.
    pub fn elev_log_end(&mut self) -> Vec<(i32, i32)> {
        self.elev_log.take().unwrap_or_default()
    }

    /// `0x4263c0` (and the inline copies): log vertex `i` with its old height once.
    fn elev_log_add(&mut self, i: i32) {
        let old = self.vertex(i);
        if let Some(l) = self.elev_log.as_mut()
            && !l.iter().any(|&(k, _)| k == i)
        {
            l.push((i, old));
        }
    }

    /// Logged write of a (clamped) height.
    fn put_vertex(&mut self, i: i32, v: i32) {
        self.elev_log_add(i);
        if self.vertex(i) != v {
            self.set_vertex(i, v);
        }
    }

    /// No track of the four tiles around corner `c` of `(x, y)` uses the corner.
    pub fn corner_free(&self, x: i32, y: i32, c: usize) -> bool {
        for k in 0..4 {
            let (dx, dy) = AROUND[c & 3][k];
            if AROUND_MASK[k] & self.mask_at(x + dx, y + dy) != 0 {
                return false;
            }
        }
        true
    }

    /// `0x4260c0(x, y, c, v, redraw)`: sets corner `c` to `clamp(v)` (logged).
    pub fn set_corner(&mut self, x: i32, y: i32, c: usize, v: i32) {
        let i = self.corner_index(x, y, c);
        self.put_vertex(i, clamp(v));
    }

    /// `0x4274b0(x, y, ca, cb, test_only)`: levels two corners of one tile.
    pub fn level_pair(&mut self, x: i32, y: i32, ca: usize, cb: usize, test: bool) -> bool {
        let (ia, ib) = (self.corner_index(x, y, ca), self.corner_index(x, y, cb));
        if self.vertex(ia) == self.vertex(ib) {
            return true;
        }
        let fa = self.corner_free(x, y, ca);
        let fb = self.corner_free(x, y, cb);
        if !test {
            if fa {
                if fb {
                    let v = (self.vertex(ib) + self.vertex(ia)) >> 1;
                    self.put_vertex(ia, clamp(v));
                    self.put_vertex(ib, clamp(v));
                } else {
                    let v = clamp(self.vertex(ib));
                    self.put_vertex(ia, v);
                }
            } else {
                if !fb {
                    return false;
                }
                let v = self.vertex(ia);
                self.set_corner(x, y, cb, v);
            }
        } else if !fa && !fb {
            return false;
        }
        true
    }

    /// `0x427050(x1, y1, x2, y2, c1, c2, test_only)`: levels corner `c1` of tile 1 with corner
    /// `c2` of tile 2. Quirk of the original kept: when only the first corner is free, the vertex
    /// written is at `(x1, y2)` (`y2*stride + x1 + offset(c1)`).
    #[allow(clippy::too_many_arguments)]
    pub fn level_cross(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        c1: usize,
        c2: usize,
        test: bool,
    ) -> bool {
        let (i1, i2) = (self.corner_index(x1, y1, c1), self.corner_index(x2, y2, c2));
        if self.vertex(i1) == self.vertex(i2) {
            return true;
        }
        let f1 = self.corner_free(x1, y1, c1);
        let f2 = self.corner_free(x2, y2, c2);
        if !test {
            if f1 {
                if f2 {
                    let v = (self.vertex(i1) + self.vertex(i2)) >> 1;
                    self.put_vertex(i1, clamp(v));
                    self.put_vertex(i2, clamp(v));
                } else {
                    let v = clamp(self.vertex(i2));
                    let i = self.corner_index(x1, y2, c1);
                    self.put_vertex(i, v);
                }
            } else {
                if !f2 {
                    return false;
                }
                let v = self.vertex(i1);
                self.set_corner(x2, y2, c2, v);
            }
        } else if !f1 && !f2 {
            return false;
        }
        true
    }

    /// `0x4278f0`: earthwork of a diagonal edge from `(x, y)` to `(x2, y2)`.
    #[allow(clippy::too_many_arguments)]
    fn level_diag(
        &mut self,
        x: i32,
        y: i32,
        x2: i32,
        y2: i32,
        c5: usize,
        c6: usize,
        c7: usize,
        test: bool,
        pre: bool,
    ) -> bool {
        let s2 = self.corner_h(x, y, c6);
        let s3 = self.corner_h(x2, y2, c6);
        let s4 = self.corner_h(x, y, c5);
        if s2 == self.corner_h(x, y, c7) && s3 == self.corner_h(x2, y2, c7) && s2 == s4 && s3 == s4
        {
            return true;
        }
        if !self.corner_free(x, y, c5) {
            if !pre {
                return true;
            }
            return self.level_pair(x, y, c6, c7, test) && self.level_pair(x2, y2, c6, c7, test);
        }
        if pre {
            if !self.level_pair(x, y, c6, c7, test) {
                return false;
            }
            if !self.level_pair(x2, y2, c6, c7, test) {
                return false;
            }
        }
        if !test {
            let v =
                (self.corner_h(x, y, c5) + self.corner_h(x2, y2, c6) + 1 + self.corner_h(x, y, c6))
                    / 3;
            let i = self.corner_index(x, y, c5);
            self.put_vertex(i, clamp(v));
        }
        true
    }

    /// `track_level_edge(x, y, d, test_only, pre)` (`0x427ff0`): earthwork of the edge from
    /// `(x, y)` towards `d`; `pre` (first pass) also levels the two tiles' cross corners first.
    pub fn level_edge(&mut self, x: i32, y: i32, d: u8, test: bool, pre: bool) -> bool {
        let du = usize::from(d & 7);
        let (nx, ny) = (x + super::DX[du], y + super::DY[du]);
        if self.ts.t8(x, y, 0) & 1 != 0 || self.ts.t8(nx, ny, 0) & 1 != 0 {
            return false;
        }
        if d & 1 == 0 {
            let h = usize::from(d >> 1);
            let c1 = (h + 1) & 3;
            if pre {
                if !self.level_pair(x, y, h, c1, test) {
                    return false;
                }
                if !self.level_pair(x, y, (h + 2) & 3, (c1 + 2) & 3, test) {
                    return false;
                }
                if !self.level_pair(nx, ny, h, c1, test) {
                    return false;
                }
            }
            if !test {
                if !self.corner_free(x, y, h) || !self.corner_free(x, y, c1) {
                    return true;
                }
                let i = self.corner_index(x, y, h);
                let v = (self.corner_h(x, y, (h + 2) & 3)
                    + self.corner_h(nx, ny, h)
                    + 1
                    + self.vertex(i))
                    / 3;
                self.put_vertex(i, clamp(v));
                let i2 = self.corner_index(x, y, c1);
                self.put_vertex(i2, clamp(v));
            }
        } else {
            let u = ((usize::from(d) + 3) >> 1) & 3;
            return self.level_diag(x, y, nx, ny, (u + 3) & 3, (u + 2) & 3, u, test, pre);
        }
        true
    }

    /// `0x427ba0(x, y, d, x2, y2)`: levels the approach of a bridge or water span from
    /// `(x, y)` (direction `d`) to its far end `(x2, y2)`.
    pub fn level_span(&mut self, x: i32, y: i32, d: u8, x2: i32, y2: i32) -> bool {
        if d & 1 == 0 {
            let h = usize::from(d >> 1);
            let c11 = (h + 3) & 3;
            let c2 = (h + 2) & 3;
            let c12 = (h + 1) & 3;
            if !self.level_pair(x, y, c2, c11, false) {
                return false;
            }
            if !self.level_pair(x2, y2, h, c12, false) {
                return false;
            }
            if !self.corner_free(x, y, c2)
                || !self.corner_free(x, y, c11)
                || !self.corner_free(x2, y2, h)
                || !self.corner_free(x2, y2, c12)
            {
                return true;
            }
            let i = self.corner_index(x, y, c2);
            let v = (self.corner_h(x2, y2, c12)
                + self.corner_h(x2, y2, h)
                + self.corner_h(x, y, c11)
                + 2
                + self.vertex(i))
                >> 2;
            self.put_vertex(i, clamp(v));
            let i2 = self.corner_index(x, y, c11);
            self.put_vertex(i2, clamp(v));
            self.set_corner(x2, y2, h, v);
            self.set_corner(x2, y2, c12, v);
        } else {
            let u = ((i32::from(d) - 3) >> 1) as usize & 3;
            if !self.level_cross(x, y, x2, y2, u, (u + 2) & 3, false) {
                return false;
            }
        }
        true
    }
}
