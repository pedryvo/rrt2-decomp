//! Station sites of the AI (ai.md 5.4/5.5, terrain-track.md 3.2): `ai_station_size`
//! (`0x434ef0`), `ai_find_station_site` (`0x430fd0`), the site test `0x408630`,
//! `station_placement_valid` (`0x4087a0`) and the footprint anchor `0x4085e0`.

use super::Ai;
use super::path::{DX, DY, Search};
use crate::trainstep::P;

/// `building_type_table` (`0x5457f8`, 0x154 bytes per type).
pub const BUILDING_TYPES: u32 = 0x0054_57f8;
const BT_SIZE: u32 = 0x154;
/// The footprint grid: cell `(row, col)` is the u16 at `0x5458c5 + t*0x154 - 20*row - 2*col`.
const FOOTPRINT: u32 = 0x0054_58c5;

/// A station site: anchor tile, orientation byte (the building type is `dir + size * 8`) and
/// size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub x: i32,
    pub y: i32,
    pub dir: u8,
    pub size: i32,
}

impl Site {
    pub fn building_type(&self) -> u32 {
        u32::from(self.dir) + self.size as u32 * 8
    }
}

impl Ai<'_, '_> {
    /// Footprint cell of building type `t`.
    pub fn footprint(&self, t: u32, row: i32, col: i32) -> u16 {
        self.ts.img.u16(
            (FOOTPRINT + t * BT_SIZE)
                .wrapping_sub((20 * row) as u32)
                .wrapping_sub((2 * col) as u32),
        )
    }

    /// Byte `o` of building type `t`.
    pub fn btype8(&self, t: u32, o: u32) -> u8 {
        self.ts.img.u8(BUILDING_TYPES + t * BT_SIZE + o)
    }

    /// `0x4085e0`: the tile of the anchor cell (flag `0x10`) of type `t` placed at `(x, y)`,
    /// scanning rows then columns.
    pub fn footprint_origin(&self, t: u32, x: i32, y: i32) -> (i32, i32) {
        for row in 0..10 {
            for col in 0..10 {
                if self.footprint(t, row, col) & 0x10 != 0 {
                    return (col + x, row + y);
                }
            }
        }
        (x, y)
    }

    /// `ai_station_size` (`0x434ef0`): `(size, size')` of the station for a town.
    pub fn station_size(&self, town: i32) -> (i32, i32) {
        let tx = i32::from(self.ts.i16(P::Place, town, 4));
        let ty = i32::from(self.ts.i16(P::Place, town, 6));
        let r = |k: u32| self.ts.img.i32(0x0054_fd9c + 4 * k);
        let (mut small, mut medium, mut large) = (0, 0, 0);
        for b in self.ids(P::Industry) {
            if self.ts.u8(P::Industry, b, 0x4f) != 0 {
                continue;
            }
            let bx = i32::from(self.ts.i16(P::Industry, b, 0x1a));
            let by = i32::from(self.ts.i16(P::Industry, b, 0x1c));
            if (bx - tx).abs() + (by - ty).abs() >= 0xf {
                continue;
            }
            let w = if self.ts.u8(P::Industry, b, 7) != 0x12 {
                3
            } else {
                2
            };
            let d = Ai::dist(bx, by, tx, ty);
            if d < r(2) {
                large += w;
                if d < r(1) {
                    medium += w;
                    if d < r(0) {
                        small += w;
                    }
                }
            }
        }
        let class = self.ts.u8(P::Place, town, 0x2c);
        let size = if class == 0 {
            0
        } else {
            i32::from(class > 3) + 1
        };
        let mut size2 = size;
        if size == 0 && small + 4 <= medium {
            size2 = 1;
        }
        if size2 == 1 && medium + 7 <= large {
            size2 = 2;
        }
        (size, size2)
    }

    /// `0x4629f0`: `track_edge_valid` with only the company set up (`0x566a50/54/58`, build
    /// rights `+0x422d`); the rest of the search state is the idle one (no box, no destination).
    #[allow(clippy::too_many_arguments)]
    pub fn edge_valid_for(
        &self,
        company: i32,
        x: i32,
        y: i32,
        d: u8,
        d_back: u8,
        bridge_table: bool,
        skip_mask2: bool,
    ) -> bool {
        let mut s = Search::idle();
        s.company = company;
        s.restricted = company != 0
            && self.ts.exists(P::Company, company)
            && self.ts.u8(P::Company, company, 0x422d) == 0;
        self.edge_valid_pub(&s, x, y, d, d_back, bridge_table, skip_mask2)
    }

    /// `0x408630`: may a station of `size` with orientation `dir_base` (type `dir_base +
    /// size*8`) sit at `(x, y)` beside the track direction `dir`?
    pub fn site_valid(
        &self,
        x: i32,
        y: i32,
        size: i32,
        company: i32,
        dir_base: u8,
        dir: u8,
    ) -> bool {
        let t = u32::from(dir_base) + size as u32 * 8;
        if !self.edge_valid_for(company, x, y, dir, dir, true, true) {
            return false;
        }
        let back = dir.wrapping_sub(4) & 7;
        if !self.edge_valid_for(company, x, y, back, back, true, true) {
            return false;
        }
        let (nx, ny) = (x + DX[usize::from(dir)], y + DY[usize::from(dir)]);
        if !self.edge_valid_for(company, nx, ny, dir, back, true, true)
            && !self.edge_valid_for(company, nx, ny, dir.wrapping_add(1) & 7, back, true, true)
            && !self.edge_valid_for(company, nx, ny, dir.wrapping_sub(1) & 7, back, true, true)
        {
            return false;
        }
        let (mut ax, mut ay) = (company, company);
        'found: for row in 0..10 {
            for col in 0..10 {
                if self.footprint(t, row, col) & 0x10 != 0 {
                    ax = x + col;
                    ay = row + y;
                    break 'found;
                }
            }
        }
        let w = i32::from(self.btype8(t, 3) >> 1);
        let h = i32::from(self.btype8(t, 4) >> 1);
        self.placement_valid(ax - w, ay - h, t, company, true, 0, true, false)
    }

    /// `station_placement_valid` (`0x4087a0`) for a station type with `param_6 == 0` (no
    /// building to ignore). `p5` skips the `tile & 8` test, `p7` checks the anchor and its two
    /// track neighbours, `p10` the `0x200` cells.
    #[allow(clippy::too_many_arguments)]
    pub fn placement_valid(
        &self,
        px: i32,
        py: i32,
        t: u32,
        company: i32,
        p5: bool,
        p6: i32,
        p7: bool,
        p10: bool,
    ) -> bool {
        let kind = self.btype8(t, 0);
        let x0 = i32::from(self.btype8(t, 3) >> 1) + px;
        let y0 = i32::from(self.btype8(t, 4) >> 1) + py;
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        for row in 0..10 {
            let yy = y0 - row;
            for col in 0..10 {
                let xx = x0 - col;
                let cell = self.footprint(t, row, col);
                if cell & 0x3c3 == 0 {
                    continue;
                }
                if xx < 0 || xx >= w || yy < 0 || yy >= h {
                    return false;
                }
                if cell & 0x200 != 0 && self.ts.t8(xx, yy, 3) & 0x20 == 0 && p10 {
                    return false;
                }
                let b = self.building_at(xx, yy);
                if b != 0 && (p6 == 0 || b != p6) {
                    return false;
                }
                if !p5 && self.ts.t8(xx, yy, 0) & 8 != 0 {
                    return false;
                }
                if self.ts.t8(xx, yy, 0) & 1 != 0 {
                    return false;
                }
                if self.ts.t8(xx, yy, 3) & 0x20 != 0 && cell & 1 != 0 {
                    return false;
                }
                let need = if self.is_water(xx, yy) {
                    cell & 0x102
                } else {
                    cell & 0x2c1
                };
                if need == 0 {
                    return false;
                }
                if kind == 0 {
                    if self.ts.t8(xx, yy, 2) & 0x20 != 0 {
                        return false;
                    }
                } else {
                    // 0x415f80 (other building kinds): never reached by the AI's stations.
                    return false;
                }
            }
        }
        if kind != 0 || !p7 {
            return true;
        }
        let dir = self.btype8(t, 2) as i8 as i32;
        let size = self.btype8(t, 1) as i8 as i32;
        let tt = (dir + size * 8) as u32;
        let (mut ax, mut ay) = (p6, p6);
        'found: for col in 0..10 {
            for row in 0..10 {
                if self.footprint(tt, row, col) & 0x10 != 0 {
                    ay = y0 - row;
                    ax = x0 - col;
                    break 'found;
                }
            }
        }
        let d1 = (dir + 2) as u8 & 7;
        let d2 = (dir - 2) as u8 & 7;
        let (bx, by) = (ax + DX[usize::from(d1)], ay + DY[usize::from(d1)]);
        let (cx, cy) = (DX[usize::from(d2)] + ax, DY[usize::from(d2)] + ay);
        let inside = |x: i32, y: i32| x >= 0 && x < w && y >= 0 && y < h;
        if !(inside(ax, ay) && inside(bx, by) && inside(cx, cy)) {
            return false;
        }
        if company != 0 {
            let terr = usize::from(self.ts.t8(ax, ay, 9));
            let need = if self
                .ts
                .g
                .rules
                .has(crate::ruleset::Fix::TerritoryRightsPerKind)
            {
                8
            } else {
                2
            };
            if self.ts.u8(P::Company, company, 0x479 + terr * 0x1ed) & need == 0 {
                return false;
            }
        }
        let m_a = self.piece_mask(ax, ay);
        let m_b = self.piece_mask(bx, by);
        let m_c = self.piece_mask(cx, cy);
        let b2 = 1u32 << d2;
        let b1 = 1u32 << d1;
        if !(b1 | b2) & m_a != 0 {
            return false;
        }
        let st = self.station_at(ax, ay);
        if st != 0 && (p6 == 0 || i32::from(self.ts.u16(P::Industry, p6, 8)) != st) {
            return false;
        }
        if m_a & b1 == 0 || m_a & b2 == 0 || m_b & b2 == 0 || m_c & b1 == 0 {
            if !self.edge_valid_for(company, ax, ay, d1, d1, true, true) {
                return false;
            }
            if !self.edge_valid_for(company, ax, ay, d2, d2, true, true) {
                return false;
            }
            if !self.edge_valid_for(company, bx, by, d2, d2, true, true) {
                return false;
            }
            if !self.may_build_at_pub(company, ax, ay)
                && !self.may_build_at_pub(company, bx, by)
                && !self.may_build_at_pub(company, cx, cy)
            {
                return false;
            }
        }
        true
    }

    /// `ai_find_station_site` (`0x430fd0`): the best tile within 4 (Manhattan) of the town
    /// center for a station facing `dir`, trying sizes from `size_max` down to `size_min`.
    pub fn find_station_site(
        &self,
        town: i32,
        company: i32,
        dir: u8,
        size_min: i32,
        size_max: i32,
    ) -> Option<Site> {
        let tx = i32::from(self.ts.i16(P::Place, town, 4));
        let ty = i32::from(self.ts.i16(P::Place, town, 6));
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let x0 = if tx - 3 < 1 { 0 } else { tx - 3 };
        let x1 = (tx + 3).min(w - 1);
        let y0 = if ty - 3 < 1 { 0 } else { ty - 3 };
        let y1 = (ty + 3).min(h - 1);
        let mut size = size_max;
        loop {
            if size < size_min {
                return None;
            }
            let mut best = 0i32;
            let mut pick = Site {
                x: 0,
                y: 0,
                dir: 0,
                size,
            };
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if (y - ty).abs() + (x - tx).abs() >= 5
                        || x <= 3
                        || x >= w - 4
                        || y <= 3
                        || y >= h - 4
                        || self.piece(x, y) != 0
                        || self.is_river(x, y)
                        || self.ts.t8(x, y, 3) & 0x20 != 0
                        || self.ts.t8(x, y, 2) & 0x10 != 0
                    {
                        continue;
                    }
                    let mut d2 = dir.wrapping_add(2) & 7;
                    if !self.site_valid(x, y, size, company, d2, dir) {
                        d2 = dir.wrapping_sub(2) & 7;
                        if !self.site_valid(x, y, size, company, d2, dir) {
                            continue;
                        }
                    }
                    let hsh = crate::rng::hash_range(self.now(), 0, 7);
                    let mut score = ((ty - y).abs() + (tx - x).abs()) * -3 + 0x32 + hsh;
                    let back = dir.wrapping_sub(4) & 7;
                    for k in 1..7 {
                        let (bx, by) =
                            (DX[usize::from(back)] * k + x, DY[usize::from(back)] * k + y);
                        if !self.edge_valid_for(company, bx, by, dir, back, false, false) {
                            score += k - 10;
                            break;
                        }
                    }
                    if best < score {
                        best = score;
                        pick = Site {
                            x,
                            y,
                            dir: d2,
                            size,
                        };
                    }
                }
            }
            if best != 0 {
                return Some(pick);
            }
            size -= 1;
        }
    }
}
