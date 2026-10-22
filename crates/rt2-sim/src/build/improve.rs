//! Station improvements (terrain-track.md 3.4): `improvement_available` (`0x468310`),
//! `station_buy_improvements` (`0x46aef0`, command `0x18`) and `ai_station_upgrades`
//! (`0x434850`).

use crate::ai::Ai;
use crate::fp::ftol;
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

    /// `station_buy_improvements(station, mask)` (`0x46aef0`) [V without size change]: price
    /// `ftol(m8 * cost + total)` accumulated per bit (effect 8 of the owner in the station's
    /// territory); account 1 gains the price minus the raw cost of the old size row, category 4
    /// loses that raw cost, the station building's value `+0x99` follows, `+0x49 |= mask`. No
    /// cash check (the UI makes it).
    pub fn station_buy_improvements(&mut self, st: i32, mask: u32) -> i32 {
        self.ts.log.ran("station_buy_improvements");
        let mut old = 0;
        if mask & 6 != 0 {
            old = self.improvement_cost(u32::from(self.ts.u8(P::Station, st, 8)));
            todo_hook!(
                self.ts.log,
                "station_size_change",
                "0x46aef0 size branch: re-mark the footprint (0x406f60, 0x42e320, 0x409320, 0x409770)",
                "terrain-track.md 3.4"
            );
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
