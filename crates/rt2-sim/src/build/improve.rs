//! Station improvements (terrain-track.md 3.4): `improvement_available` (`0x468310`),
//! `station_buy_improvements` (`0x46aef0`, command `0x18`) and `ai_station_upgrades`
//! (`0x434850`).

use crate::ai::Ai;
use crate::fp::ftol;
use crate::tilemark;
use crate::trainstep::P;

use super::account_move;

/// `improvement_table` (`0x5500c8`), 30 rows of `0x1b` bytes: cost `+7`, first year `+0xb`, last
/// year `+0xf`, enabled `+0x13`.
const TABLE: u32 = 0x0055_00c8;
const ROW: u32 = 0x1b;
pub const ROWS: u32 = 30;

impl Ai<'_, '_> {
    /// Base cost of improvement row `i`.
    pub fn improvement_cost(&self, i: u32) -> i32 {
        self.ts.img.i32(TABLE + ROW * i + 7)
    }

    /// `improvement_available(i)` (`0x468310`) [V]: in the industry expansion set only rows
    /// 0..4, 10, 11 and 21; from September of the first year to the last year; enabled rows.
    /// The scenario option that ignores the dates (`0x574bc8+0x60`) is not in the save and is
    /// taken as off.
    pub fn improvement_available(&self, i: u32) -> bool {
        if self.hdr8(0x0057_04ac) != 0 && !matches!(i, 0..=4 | 10 | 11 | 0x15) {
            return false;
        }
        let year = self.year();
        let first = self.ts.img.i32(TABLE + ROW * i + 0xb);
        let last = self.ts.img.i32(TABLE + ROW * i + 0xf);
        let started = year > first || (year == first && self.month() >= 9);
        started && year <= last && self.ts.img.i32(TABLE + ROW * i + 0x13) != 0
    }

    /// `station_buy_improvements(station, mask)` (`0x46aef0`) [V]: price `ftol(m8 * cost +
    /// total)` accumulated per bit (effect 8 of the owner in the station's territory); account 1
    /// gains the price minus the raw cost of the old size row, category 4 loses that raw cost,
    /// the station building's value `+0x99` follows, `+0x49 |= mask`. No cash check (the UI
    /// makes it). A mask with bit 1 or 2 first changes the size ([`Self::station_size_change`]).
    pub fn station_buy_improvements(&mut self, st: i32, mask: u32) -> i32 {
        self.ts.log.ran("station_buy_improvements");
        let mut old = 0;
        if mask & 6 != 0 {
            old = self.improvement_cost(u32::from(self.ts.u8(P::Station, st, 8)));
            self.station_size_change(st, mask);
        }
        let owner = self.ts.i32(P::Station, st, 0x39);
        let terr = self.ts.i32(P::Station, st, 0x4ad);
        let mut total = 0i32;
        for i in 0..ROWS {
            if mask & (1 << i) != 0 {
                let m = self.ts.effect(8, owner, 0, terr);
                total = ftol(f64::from(m) * f64::from(self.improvement_cost(i)) + f64::from(total));
            }
        }
        account_move(self.ts, owner, 1, f64::from(total - old));
        super::ledger_credit(self.ts, owner, 4, f64::from(-old));
        let b = self.ts.i32(P::Station, st, 4);
        let v = self.ts.i32(P::Industry, b, 0x99) + (total - old);
        self.ts.seti32(P::Industry, b, 0x99, v);
        let bits = self.ts.u32(P::Station, st, 0x49) | mask;
        self.ts.set32(P::Station, st, 0x49, bits);
        total
    }

    /// The shape a size change gives the station building: `+6 + 16` for bit 2 on a small
    /// station, `+6 + 8` otherwise (`0x46aef0`, and the same rule in the window `0x407540`).
    pub fn station_size_shape(&self, st: i32, mask: u32) -> i32 {
        let b = self.ts.i32(P::Station, st, 4);
        let shape = i32::from(self.ts.u8(P::Industry, b, 6));
        if mask & 4 != 0 && self.ts.u8(P::Station, st, 8) == 0 {
            shape + 0x10
        } else {
            shape + 8
        }
    }

    /// The space test of the Buy window before a size change (`0x407540`, button OK; LNG 346
    /// when it fails): `building_can_place(anchor +0x1a/+0x1c, new shape, company, any ground,
    /// allowed = the station's own building, station rules, early = 0)` (`0x4087a0`). The window
    /// tests the new shape at the old anchor, while `0x409770` re-anchors it on the old origin;
    /// with the station shapes of the executable both give the same cells.
    pub fn station_size_fits(&self, st: i32, mask: u32, company: i32) -> bool {
        if mask & 6 == 0 {
            return true;
        }
        let b = self.ts.i32(P::Station, st, 4);
        let x = i32::from(self.ts.i16(P::Industry, b, 0x1a));
        let y = i32::from(self.ts.i16(P::Industry, b, 0x1c));
        let shape = self.station_size_shape(st, mask);
        self.building_can_place(x, y, shape, company, true, b, true, false)
            .is_some()
    }

    /// The size branch of `0x46aef0` (terrain-track.md 3.4.1) [V]: the catchment links go
    /// (`0x406f60`); the building's kind-3 structures leave every tile of its old bounding box
    /// (`0x42e320`, `+0x26..+0x2c`); the shape `+6` grows by 8 (to Medium, or Medium to Large) or
    /// by 16 (Small to Large: bit 2 on a size-0 station); the station size `+8` becomes 1 with
    /// bit 1 in the mask, else 2 (so a mask with both bits on a small station makes a Large
    /// shape of size 1, as in the original); the links are rebuilt with the new radius
    /// (`0x4071b0`); the footprint geometry is recomputed around the unchanged origin
    /// `+0x1e/+0x20` and the new cells get their structures (`0x409770`); the terrain flags of the
    /// anchor +-4 (`0x424c10`) and the station's supply and demand (`0x4bebf0`) follow. The anchor
    /// cell, the station record's tile, the track, the terrain heights, `+0x22/+0x24` and the
    /// region `+0x9d` do not change. `0x409320`/`0x4093e0` (sprite reference counts) are
    /// interface only.
    pub fn station_size_change(&mut self, st: i32, mask: u32) {
        self.ts.log.ran("station_size_change");
        crate::convert::station_unlink(self.ts, st);
        let b = self.ts.i32(P::Station, st, 4);
        let id = u32::from(self.ts.u16(P::Industry, b, 0));
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let bb = [0x26, 0x28, 0x2a, 0x2c].map(|o| i32::from(self.ts.i16(P::Industry, b, o)));
        for x in bb[0]..=bb[2] {
            for y in bb[1]..=bb[3] {
                // The original indexes the tile array linearly (`w * y + x`), without a bounds
                // test; the bounding box of a placed building always lies in the map.
                let i = w * y + x;
                if (0..w * h).contains(&i) {
                    tilemark::struct_remove(self.ts.s, i % w, i / w, 3, id);
                }
            }
        }
        let shape = self.station_size_shape(st, mask) as u8;
        self.ts.set8(P::Industry, b, 6, shape);
        self.ts
            .set8(P::Station, st, 8, if mask & 2 == 0 { 2 } else { 1 });
        self.station_rebuild_links(st);
        // 0x409770: geometry around the origin, then the structures of the new footprint.
        let s = usize::from(shape);
        let (ox, oy) = (
            i32::from(self.ts.i16(P::Industry, b, 0x1e)),
            i32::from(self.ts.i16(P::Industry, b, 0x20)),
        );
        let e = tilemark::shape_extents(self.ts.img, s);
        let geo = tilemark::geometry(self.ts.img, s, ox - (e[0] >> 1), oy - (e[1] >> 1), w, h);
        for k in 0..4 {
            self.ts
                .set16(P::Industry, b, 0xa + 2 * k, geo.corner_x[k] as u16);
            self.ts
                .set16(P::Industry, b, 0x12 + 2 * k, geo.corner_y[k] as u16);
        }
        for (o, v) in [
            (0x1a, geo.anchor.0),
            (0x1c, geo.anchor.1),
            (0x26, geo.bbox[0]),
            (0x28, geo.bbox[1]),
            (0x2a, geo.bbox[2]),
            (0x2c, geo.bbox[3]),
        ] {
            self.ts.set16(P::Industry, b, o, v as u16);
        }
        tilemark::mark_building(self.ts.s, self.ts.img, s, id as u16, ox, oy);
        let (ax, ay) = (i32::from(geo.anchor.0), i32::from(geo.anchor.1));
        tilemark::recompute_flags(self.ts.s, ax - 4, ay - 4, ax + 4, ay + 4);
        self.ts.station_recompute_supply_demand(st);
    }

    /// `ai_station_upgrades(station)` (`0x434850`): what the AI adds to a new station.
    pub fn ai_station_upgrades(&mut self, st: i32) {
        self.ts.log.ran("ai_station_upgrades");
        let size = self.ts.u8(P::Station, st, 8);
        let has = |ai: &Ai<'_, '_>, m: u32| ai.ts.u32(P::Station, st, 0x49) & m != 0;
        if size != 0 {
            if self.year() < 0x78a && !has(self, 0x800) {
                self.station_buy_improvements(st, 0x800);
            }
            if !has(self, 0x100) {
                self.station_buy_improvements(st, 0x100);
            }
            if !has(self, 0x400) {
                self.station_buy_improvements(st, 0x400);
            }
        }
        if size > 1 && !has(self, 0x20_0000) {
            self.station_buy_improvements(st, 0x20_0000);
        }
        if size != 0 && !has(self, 0x80) && self.improvement_available(7) {
            self.station_buy_improvements(st, 0x80);
        }
        if size > 1 && !has(self, 0x40) && self.improvement_available(6) {
            self.station_buy_improvements(st, 0x40);
        }
        if size != 0 && !has(self, 0x60) && self.improvement_available(5) {
            self.station_buy_improvements(st, 0x20);
        }
    }
}
