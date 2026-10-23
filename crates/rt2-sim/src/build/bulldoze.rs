//! The bulldozer (terrain-track.md 2.4, command `0x0a`): `bulldoze_area` (`0x428930`),
//! `track_bulldoze_tile` (`0x4286a0`), the track removal of a rectangle with its validation
//! and restore (`0x4caaa0`), the removal of edges of one tile (`0x4ca920`) and
//! `tile_track_remove` (`0x4162b0`).

use crate::ai::Ai;
use crate::fp::ftol;
use crate::trainstep::P;

use super::{DX, DY, account_move, ledger_credit};

/// Why the bulldozer refused (`0x553868[reason]`, LNG 1121..1128).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulldozeError {
    /// 1: track that is the main exit of a station.
    StationTrack,
    /// 2: track of another company.
    ForeignTrack,
    /// 3: a train is on the track.
    TrainOnTrack,
    /// 5: illegal track layout left behind (never set by the original, see 10.6).
    IllegalLayout,
    /// 6: not enough cash.
    Cash,
    /// 7: building of another company.
    ForeignBuilding,
    /// 8: building in the catchment of another company's station.
    ForeignStation,
}

fn reason(r: i32) -> BulldozeError {
    match r {
        1 => BulldozeError::StationTrack,
        2 => BulldozeError::ForeignTrack,
        3 => BulldozeError::TrainOnTrack,
        5 => BulldozeError::IllegalLayout,
        6 => BulldozeError::Cash,
        7 => BulldozeError::ForeignBuilding,
        _ => BulldozeError::ForeignStation,
    }
}

const ST_CELLS: usize = 0x3835;
const ST_SINGLE: usize = 0x3839;
const ST_DOUBLE: usize = 0x383d;
const ST_ELECTRIC: usize = 0x3841;

impl Ai<'_, '_> {
    fn st_dec(&mut self, o: usize) {
        let v = self.ts.state_i32(o) - 1;
        self.ts.set_state_i32(o, v);
    }

    /// `tile_track_remove(tile, mask, owner, count)` (`0x4162b0`) with `mask == 0` removes the
    /// track of the tile (counters with `count`), otherwise sets the edges. The book value of
    /// a removed piece (`+0x15 * 8`) is added to `removed` (`0x567684`).
    pub(crate) fn tile_track_remove(
        &mut self,
        x: i32,
        y: i32,
        mask: u8,
        owner: i32,
        count: bool,
        removed: &mut i32,
    ) {
        let p = self.piece(x, y);
        if mask != 0 {
            if p != 0 {
                self.ts.set8(P::Track, p, 0, mask);
                let b = self.ts.t8(x, y, 2);
                self.ts.set_t8(x, y, 2, b | 0x20);
            } else {
                self.tile_track_add(x, y, mask, 0, 0, owner);
            }
            return;
        }
        if p == 0 {
            return;
        }
        let o = i32::from(self.ts.u8(P::Track, p, 2));
        if count && o != 0 {
            let terr = i32::from(self.ts.t8(x, y, 9));
            if self.ts.exists(P::Territory, terr) {
                let v = self.ts.i16(P::Territory, terr, 0x50) - 1;
                self.ts.set16(P::Territory, terr, 0x50, v as u16);
            }
            let off = terr as usize * 0x1ed + 0x47c;
            let v = self.ts.i16(P::Company, o, off) - 1;
            self.ts.set16(P::Company, o, off, v as u16);
            let v = self.ts.i32(P::Company, o, 0x182) - 1;
            self.ts.seti32(P::Company, o, 0x182, v);
            self.st_dec(ST_CELLS);
            let b23 = self.ts.u8(P::Track, p, 0x23);
            if b23 & 1 == 0 {
                let v = self.ts.i32(P::Company, o, 0x186) - 1;
                self.ts.seti32(P::Company, o, 0x186, v);
                self.st_dec(ST_SINGLE);
            } else {
                let v = self.ts.i32(P::Company, o, 0x18a) - 1;
                self.ts.seti32(P::Company, o, 0x18a, v);
                self.st_dec(ST_DOUBLE);
            }
            if b23 & 0x10 != 0 {
                let v = self.ts.i32(P::Company, o, 0x18e) - 1;
                self.ts.seti32(P::Company, o, 0x18e, v);
                self.st_dec(ST_ELECTRIC);
            }
        }
        let book = i32::from(self.ts.u16(P::Track, p, 0x15)) * 8;
        if self.ts.u8(P::Track, p, 1) == 0 {
            self.ts.free(P::Track, p);
            self.ts.set_t16(x, y, 7, 0);
            let b = self.ts.t8(x, y, 2);
            self.ts.set_t8(x, y, 2, b & 0x7f);
            let b = self.ts.t8(x, y, 3);
            self.ts.set_t8(x, y, 3, b & 0xbf);
        } else {
            self.ts.set8(P::Track, p, 0, 0);
            if self.ts.t8(x, y, 3) & 0x40 != 0 && self.ts.u8(P::Track, p, 5) & 0xc == 0 {
                let b = self.ts.t8(x, y, 3);
                self.ts.set_t8(x, y, 3, b & 0xbf);
            }
            if self.ts.u8(P::Track, p, 0x11) != 0 && self.ts.u8(P::Track, p, 0x23) & 0x20 == 0 {
                self.ts.set8(P::Track, p, 0x11, 0);
                let b = self.ts.t8(x, y, 2);
                self.ts.set_t8(x, y, 2, b & 0x7f);
            }
        }
        *removed = removed.wrapping_add(book);
        let b = self.ts.t8(x, y, 2);
        self.ts.set_t8(x, y, 2, b & 0xdf);
    }

    /// `0x4ca920(x, y, bits, company, &reason)`: removes the edges `bits` of the tile; in the
    /// validation mode (`check`) refuses a train on the piece, a station's main tile and
    /// another company's track (and still removes, without counters).
    fn tile_remove_edges(
        &mut self,
        x: i32,
        y: i32,
        bits: u8,
        company: i32,
        check: Option<&mut i32>,
        removed: &mut i32,
    ) -> bool {
        let p = self.piece(x, y);
        let m = if p == 0 {
            0
        } else {
            self.ts.u8(P::Track, p, 0)
        };
        if self.ts.t8(x, y, 2) & 0x20 == 0 || bits & m == 0 {
            return false;
        }
        let owner = i32::from(self.ts.u8(P::Track, p, 2));
        match check {
            None => {
                self.tile_track_remove(x, y, !bits & m, owner, true, removed);
                true
            }
            Some(r) => {
                if self.ts.u16(P::Track, p, 3) != 0 {
                    *r = 3;
                } else {
                    for st in self.ids(P::Station) {
                        if self.ts.i32(P::Station, st, 0xa) == x
                            && self.ts.i32(P::Station, st, 0xe) == y
                            && self.ts.i32(P::Station, st, 0x45) == 0
                        {
                            *r = 1;
                        }
                    }
                }
                if *r == 0 && owner != company {
                    *r = 2;
                }
                if *r != 0 {
                    return false;
                }
                self.tile_track_remove(x, y, !bits & m, owner, false, removed);
                true
            }
        }
    }

    /// `0x4caaa0(x0, y0, x1, y1, company, &reason)`: removes the track of a rectangle and the
    /// edges of the ring that point into it; validation mode restores the pieces afterwards.
    fn track_remove_rect(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        company: i32,
        mut check: Option<&mut i32>,
        removed: &mut i32,
    ) {
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(w - 1), y1.min(h - 1));
        if x0 > x1 || y0 > y1 {
            return;
        }
        let full = x0 == 0 && x1 == w - 1 && y0 == 0 && y1 == h - 1;
        let (bx0, by0, bx1, by1) = (
            (x0 - 1).max(0),
            (y0 - 1).max(0),
            (x1 + 1).min(w - 1),
            (y1 + 1).min(h - 1),
        );
        let mut backup: Vec<(i32, i32, Vec<u8>)> = Vec::new();
        if !full {
            for y in by0..=by1 {
                for x in bx0..=bx1 {
                    let p = self.piece(x, y);
                    if p != 0 {
                        backup.push((x, y, self.ts.record(P::Track, p)));
                    }
                }
            }
        }
        self.snapshot_overlays(x0 - 2, y0 - 2, x1 + 2, y1 + 2);
        let (mut rx0, mut ry0, mut rx1, mut ry1) = (w, h, -1, -1);
        let mut aborted = false;
        'rect: for y in y0..=y1 {
            for x in x0..=x1 {
                if self.tile_remove_edges(x, y, 0xff, company, check.as_deref_mut(), removed) {
                    rx0 = rx0.min(x);
                    ry0 = ry0.min(y);
                    rx1 = rx1.max(x);
                    ry1 = ry1.max(y);
                }
                if check.as_deref().is_some_and(|r| *r != 0) {
                    aborted = true;
                    break 'rect;
                }
            }
        }
        if !aborted && rx1 != -1 {
            'ring: {
                for ix in rx0 - 1..=rx1 + 1 {
                    if ix < 0 || ix >= w {
                        continue;
                    }
                    let (mut up, mut down) = (0u8, 0u8);
                    if ix < rx1 {
                        up = 8;
                        down = 2;
                    }
                    if rx0 < ix {
                        up |= 0x20;
                        down |= 0x80;
                    }
                    if rx0 <= ix && ix <= rx1 {
                        up |= 0x10;
                        down |= 1;
                    }
                    if ry0 > 0 {
                        self.tile_remove_edges(
                            ix,
                            ry0 - 1,
                            up,
                            company,
                            check.as_deref_mut(),
                            removed,
                        );
                        if check.as_deref().is_some_and(|r| *r != 0) {
                            aborted = true;
                            break 'ring;
                        }
                    }
                    if ry1 < h - 1 {
                        self.tile_remove_edges(
                            ix,
                            ry1 + 1,
                            down,
                            company,
                            check.as_deref_mut(),
                            removed,
                        );
                        if check.as_deref().is_some_and(|r| *r != 0) {
                            aborted = true;
                            break 'ring;
                        }
                    }
                }
                for iy in ry0..=ry1 {
                    let (mut l, mut r) = (0u8, 0u8);
                    if iy < ry1 {
                        l = 8;
                        r = 0x20;
                    }
                    if ry0 < iy {
                        l |= 2;
                        r |= 0x80;
                    }
                    if ry0 <= iy && iy <= ry1 {
                        l |= 4;
                        r |= 0x40;
                    }
                    if rx0 > 0 {
                        self.tile_remove_edges(
                            rx0 - 1,
                            iy,
                            l,
                            company,
                            check.as_deref_mut(),
                            removed,
                        );
                        if check.as_deref().is_some_and(|r| *r != 0) {
                            aborted = true;
                            break 'ring;
                        }
                    }
                    if rx1 < w - 1 {
                        self.tile_remove_edges(
                            rx1 + 1,
                            iy,
                            r,
                            company,
                            check.as_deref_mut(),
                            removed,
                        );
                        if check.as_deref().is_some_and(|r| *r != 0) {
                            aborted = true;
                            break 'ring;
                        }
                    }
                }
                self.tile_rebuild(rx0 - 1, ry0 - 1, rx1 + 1, ry1 + 1);
                if check.is_none() {
                    if self.ts.s.stations.count < 0x14 {
                        self.station_networks_rebuild();
                    } else {
                        self.networks_dirty = true;
                    }
                    self.trains_refresh_all();
                    self.overlay_snapshot = None;
                    return;
                }
            }
        }
        if check.is_some() && rx1 != -1 {
            for (x, y, data) in backup {
                let p = self.piece(x, y);
                if p == 0 {
                    let id = self.ts.alloc(P::Track, data.clone());
                    self.ts.set_t16(x, y, 7, id as u16);
                    let b = self.ts.t8(x, y, 2);
                    self.ts.set_t8(x, y, 2, b | 0x20);
                } else {
                    self.ts.set_record(P::Track, p, &data);
                }
                let b = self.ts.t8(x, y, 2);
                self.ts
                    .set_t8(x, y, 2, (u8::from(data[0x11] != 0) << 7) | (b & 0x7f));
            }
            self.tile_rebuild(rx0 - 1, ry0 - 1, rx1 + 1, ry1 + 1);
        }
        let _ = aborted;
        self.overlay_snapshot = None;
    }

    /// `track_bulldoze_tile(x, y, company, &reason)` (`0x4286a0`): the tile's track, and on a
    /// bridge or water span the whole crossing.
    fn track_bulldoze_tile(
        &mut self,
        x: i32,
        y: i32,
        company: i32,
        mut check: Option<&mut i32>,
        removed: &mut i32,
    ) {
        let p = self.piece(x, y);
        let m = if p == 0 {
            0
        } else {
            u32::from(self.ts.u8(P::Track, p, 0))
        };
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let inm = |x: i32, y: i32| x >= 0 && x < w && y >= 0 && y < h;
        if self.ts.t8(x, y, 3) & 0x40 == 0 {
            if self.ts.t8(x, y, 2) & 0x80 != 0 {
                let mut k = 0usize;
                for d in 0..8 {
                    if m & (1 << d) != 0 && self.ts.t8(x + DX[d], y + DY[d], 2) & 0x80 != 0 {
                        k = d;
                        break;
                    }
                    if d == 7 {
                        k = 0;
                    }
                }
                let (ax, ay) = (x + DX[k], y + DY[k]);
                let (mut bx, mut by) = (x + 2 * DX[k], y + 2 * DY[k]);
                if !inm(bx, by) || self.ts.t8(bx, by, 2) & 0x80 == 0 {
                    let o = (k + 4) & 7;
                    bx = x + DX[o];
                    by = y + DY[o];
                    if !inm(bx, by) || self.ts.t8(bx, by, 2) & 0x80 == 0 {
                        bx = -1;
                    }
                }
                self.track_remove_rect(ax, ay, ax, ay, company, check.as_deref_mut(), removed);
                if check.as_deref().is_some_and(|r| *r != 0) {
                    return;
                }
                if bx != -1 {
                    self.track_remove_rect(bx, by, bx, by, company, check.as_deref_mut(), removed);
                }
                if check.as_deref().is_some_and(|r| *r != 0) {
                    return;
                }
            }
        } else {
            for d in 0..8 {
                if m & (1 << d) == 0 {
                    continue;
                }
                let (mut sx, mut sy) = (x + DX[d], y + DY[d]);
                while inm(sx, sy) && self.ts.t8(sx, sy, 3) & 0x40 != 0 {
                    self.track_remove_rect(sx, sy, sx, sy, company, check.as_deref_mut(), removed);
                    if check.as_deref().is_some_and(|r| *r != 0) {
                        return;
                    }
                    sx += DX[d];
                    sy += DY[d];
                }
            }
        }
        self.track_remove_rect(x, y, x, y, company, check, removed);
    }

    /// `bulldoze_area(company, x, y, radius, cost_only, &cost, &reason, ...)` (`0x428930`):
    /// pass 0 validates and prices (1000 per track tile and per tree; buildings
    /// `m(0x130) * price * 0.1` own, `* 0.75` neutral, also capitalized to account 2), refuses
    /// above the cash; pays (category 5, reputation `+0x4229 -= sqrt(cost) + 200`); pass 1
    /// removes (the book value of the track is written off: category 5 and account 3).
    /// Returns the cost rounded down to thousands.
    pub fn bulldoze_area(
        &mut self,
        company: i32,
        cx: i32,
        cy: i32,
        radius: i32,
        cost_only: bool,
    ) -> Result<i32, BulldozeError> {
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let tiles: Vec<(i32, i32)> = (cx - radius..=cx + radius)
            .flat_map(|x| (cy - radius..=cy + radius).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                x >= 0
                    && x < w
                    && y >= 0
                    && y < h
                    && crate::track::tile_distance(x, y, cx, cy) <= radius
            })
            .collect();
        let (mut cost, mut capital) = (0i32, 0i32);
        let mut r = 0i32;
        let mut scratch = 0i32;
        for &(x, y) in &tiles {
            if self.ts.t8(x, y, 2) & 0x20 != 0 {
                self.track_bulldoze_tile(x, y, company, Some(&mut r), &mut scratch);
                if r != 0 {
                    return Err(reason(r));
                }
                cost += 1000;
            }
            let b = self.building_at(x, y);
            if b != 0 {
                let owner = i32::from(self.ts.i16(P::Industry, b, 0x97));
                let kind = self.ts.u8(P::Industry, b, 7);
                if owner == company || (kind != 0 && owner == 0) {
                    let links = self.ts.i8(P::Industry, b, 0x33);
                    let mut l = self.ts.i32(P::Industry, b, 0x2f);
                    for _ in 0..links.max(0) {
                        let st = i32::from(self.ts.u16(P::Link, l, 2));
                        if self.ts.i32(P::Station, st, 0x39) != company {
                            return Err(BulldozeError::ForeignStation);
                        }
                        l = self.ts.i32(P::Link, l, 6);
                    }
                } else {
                    return Err(BulldozeError::ForeignBuilding);
                }
                let terr = i32::from(self.ts.i8(P::Industry, b, 0x9d));
                let m = f64::from(self.ts.effect(0x130, company, 0, terr));
                let shape = u32::from(self.ts.u8(P::Industry, b, 6));
                let price = f64::from(self.ts.img.i32(0x0054_57ef + 0x154 * shape));
                let k = if owner == company { 0.1 } else { 0.75 };
                let x = m * price * k;
                cost = ftol(f64::from(cost) + x);
                capital = ftol(f64::from(capital) + x);
            }
            if self.ts.t8(x, y, 2) & 0x20 == 0 {
                let ti = crate::track::TileInfo {
                    terrain_class: 0,
                    flags2: self.ts.t8(x, y, 2),
                    flags3: self.ts.t8(x, y, 3),
                };
                cost += ti.tree_count() * 1000;
            }
        }
        let cash = if company != 0 {
            crate::pool::f64_at(&self.ts.record(P::Company, company), 0x4766)
        } else {
            0.0
        };
        if company != 0 && cash < f64::from(cost) {
            return Err(BulldozeError::Cash);
        }
        let rounded = (cost / 1000) * 1000;
        if cost_only {
            return Ok(rounded);
        }
        if cost != 0 && company != 0 {
            account_move(self.ts, company, 2, f64::from(capital));
            let net = cost - capital;
            ledger_credit(self.ts, company, 5, f64::from(-net));
            let s = if net > 0 { f64::from(net).sqrt() } else { 0.0 };
            let rep = self.ts.i32(P::Company, company, 0x4229);
            self.ts.seti32(
                P::Company,
                company,
                0x4229,
                ftol(f64::from(rep) - (s + 200.0)),
            );
        }
        for &(x, y) in &tiles {
            let mut v = 0i32;
            self.track_bulldoze_tile(x, y, company, None, &mut v);
            if company != 0 {
                ledger_credit(self.ts, company, 5, f64::from(-v));
                account_move(self.ts, company, 0, f64::from(v));
                account_move(self.ts, company, 3, f64::from(-v));
                account_move(self.ts, company, 0, f64::from(-v));
            }
            self.clear_trees(x, y);
            if self.building_at(x, y) != 0 {
                todo_hook!(
                    self.ts.log,
                    "bulldoze_building",
                    "0x408e70 -> building_destroy 0x40adb0 (the SimState port of tick/town.rs is not on the train-step view yet)",
                    "terrain-track.md 2.4"
                );
            }
        }
        self.flush_redraw();
        Ok(rounded)
    }

    /// `0x429fc0(x, y, 0, -100, 1)` as the bulldozer calls it: radius 0, no draw (`|-100| >
    /// 0x13`), brush type `0x53a0a4 = 1`: a bare tile takes vegetation 1 and loses its trees;
    /// then the trees fall to 0 (`tile+2 &= 0xf0`).
    fn clear_trees(&mut self, x: i32, y: i32) {
        self.brush_tile(x, y);
        crate::tilemark::recompute_flags(self.ts.s, x - 1, y - 1, x + 1, y + 1);
    }

    fn brush_tile(&mut self, x: i32, y: i32) {
        let c = self.terrain_class(x, y);
        if c == 0x12 || c == 0 {
            return;
        }
        let mut b2 = self.ts.t8(x, y, 2);
        if b2 & 0xf == 0 {
            b2 = (b2 & 0xf0) | 1;
            self.ts.set_t8(x, y, 2, b2);
            let t = self.ts.t8(x, y, 3) & 0xe0;
            self.ts.set_t8(x, y, 3, t);
        }
        let b2 = self.ts.t8(x, y, 2);
        if b2 & 0xf == 3 || b2 & 0xf == 0xd {
            self.ts.set_t8(x, y, 2, b2 & 0xf0);
        } else {
            let t = self.ts.t8(x, y, 3);
            let v = i32::from(t & 0x1f) - 100;
            if v < 1 {
                self.ts.set_t8(x, y, 2, b2 & 0xf0);
            }
        }
    }
}
