//! `building_can_place(x, y, shape, company, any_ground, allowed, station_rules, &simple,
//! &needs_track, early)` (`0x4087a0`, world-growth.md 4.6, terrain-track.md 3.2) with the rules
//! of a station, as the build command (`0x17`) and the AI call it. The town-building subset is
//! [`crate::tilemark::can_place`] (differential test `rt2-oracle/tests/placement.rs`).

use crate::ai::Ai;
use crate::ai::path::Search;
use crate::tilemark;
use crate::trainstep::P;

use super::{DX, DY};

/// Result of a successful placement test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Placement {
    /// `*param_8`: the anchor and both neighbours are free of track (a simple station).
    pub simple: bool,
    /// `*param_9`: the track beside the anchor must still be built.
    pub needs_track: bool,
}

impl Ai<'_, '_> {
    /// `0x4087a0`. `allowed` is a building that may already be on the footprint (0 = none),
    /// `any_ground` skips the bit-3 tile test, `station_rules` enables the anchor tests of a
    /// station, `early` the road test of cells flagged `0x200`.
    #[allow(clippy::too_many_arguments)]
    pub fn building_can_place(
        &self,
        x: i32,
        y: i32,
        shape: i32,
        company: i32,
        any_ground: bool,
        allowed: i32,
        station_rules: bool,
        early: bool,
    ) -> Option<Placement> {
        let img = self.ts.img;
        let s = shape as usize;
        let kind = tilemark::shape_type(img, s);
        let e = tilemark::shape_extents(img, s);
        let ox = (e[0] >> 1) + x;
        let oy = (e[1] >> 1) + y;
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        for (lin, col) in tilemark::cells() {
            let c = tilemark::shape_cell(img, s, lin, col);
            if c & 0x3c3 == 0 {
                continue;
            }
            let (tx, ty) = (ox - col, oy - lin);
            if tx < 0 || tx >= w || ty < 0 || ty >= h {
                return None;
            }
            if c & 0x200 != 0 && self.ts.t8(tx, ty, 3) & 0x20 == 0 && early {
                return None;
            }
            let b = self.building_at(tx, ty);
            if b != 0 && (allowed == 0 || b != allowed) {
                return None;
            }
            if !any_ground && self.ts.t8(tx, ty, 0) & 8 != 0 {
                return None;
            }
            if self.ts.t8(tx, ty, 0) & 1 != 0 {
                return None;
            }
            if self.ts.t8(tx, ty, 3) & 0x20 != 0 && c & 1 != 0 {
                return None;
            }
            let ok = if self.is_water(tx, ty) {
                c & 0x102
            } else {
                c & 0x2c1
            };
            if ok == 0 {
                return None;
            }
            if kind == 0 {
                if self.ts.t8(tx, ty, 2) & 0x20 != 0 {
                    return None;
                }
            } else if self.ts.t8(tx, ty, 2) & 0x80 != 0
                || self.ts.t8(tx, ty, 3) & 0x40 != 0
                || self.track_overlay(tx, ty)
            {
                return None;
            }
        }
        let mut out = Placement::default();
        if kind != 0 || !station_rules {
            return Some(out);
        }
        let dir = img.u8(0x0054_57fa + 0x154 * shape as u32);
        let (mut ax, mut ay) = (allowed, 0);
        let mut found = false;
        'f: for col in 0..10 {
            for lin in 0..10 {
                if tilemark::shape_cell(img, s, lin, col) & 0x10 != 0 {
                    ax = ox - col;
                    ay = oy - lin;
                    found = true;
                    break 'f;
                }
            }
        }
        let _ = found;
        let fwd = dir.wrapping_add(2) & 7;
        let back = dir.wrapping_sub(2) & 7;
        let (fx, fy) = (ax + DX[usize::from(fwd)], ay + DY[usize::from(fwd)]);
        let (bx, by) = (ax + DX[usize::from(back)], ay + DY[usize::from(back)]);
        if !self.in_map(ax, ay) || !self.in_map(fx, fy) || !self.in_map(bx, by) {
            return None;
        }
        if company != 0 {
            let terr = usize::from(self.ts.t8(ax, ay, 9));
            if self.ts.u8(P::Company, company, 0x479 + terr * 0x1ed) & 2 == 0 {
                return None;
            }
        }
        let m_a = self.mask_at(ax, ay);
        let m_f = self.mask_at(fx, fy);
        let m_b = self.mask_at(bx, by);
        let (bf, bb) = (1u32 << fwd, 1u32 << back);
        if !(bf | bb) & m_a != 0 {
            return None;
        }
        let st = self.station_at(ax, ay);
        if st != 0 && (allowed == 0 || i32::from(self.ts.u16(P::Industry, allowed, 8)) != st) {
            return None;
        }
        if m_a & bf == 0 || m_a & bb == 0 || m_f & bb == 0 || m_b & bf == 0 {
            let mut sr = Search::idle();
            sr.company = company;
            sr.restricted = company != 0 && self.ts.u8(P::Company, company, 0x422d) == 0;
            if !self.edge_valid_pub(&sr, ax, ay, fwd, fwd, true, true)
                || !self.edge_valid_pub(&sr, ax, ay, back, back, true, true)
                || !self.edge_valid_pub(&sr, fx, fy, back, back, true, true)
            {
                return None;
            }
            if !self.may_build_at_pub(company, ax, ay)
                && !self.may_build_at_pub(company, fx, fy)
                && !self.may_build_at_pub(company, bx, by)
            {
                return None;
            }
            let near = |d: u8| {
                (1u32 << (d.wrapping_sub(4) & 7))
                    | (1 << (d.wrapping_sub(3) & 7))
                    | (1 << (d.wrapping_add(3) & 7))
            };
            if (bf | bb) & m_a == 0 && m_f & (near(back) | bb) == 0 && m_b & (near(fwd) | bf) == 0 {
                out.simple = true;
            }
            out.needs_track = true;
        }
        Some(out)
    }
}
