//! Station supply and demand (`station_recompute_supply_demand` `0x4bebf0`, cargo.md 8): run
//! after every change in the catchment (a lot produced, a delivery taken by an industry), every
//! week for every station and every month after the decay of the delivery counters.

use super::mem::{P, Ts};
use crate::fp::ftol;

/// Cargo types of the cargo table.
const CARGOS: usize = 44;

impl Ts<'_> {
    /// `cargo_enabled[c]` (`0x5705d5`, state block): the scenario uses cargo `c`.
    fn cargo_enabled(&self, c: usize) -> bool {
        self.s.state[0xed + c] != 0
    }

    fn sandbox_flag(&self) -> bool {
        self.s.header.get(0x86ef).copied().unwrap_or(0) != 0
    }

    /// `industry_slot_active` (`0x4e2330`, slot of a type): the slot exists, is valid this year
    /// and its cargos are enabled (an input of a disabled cargo is tolerated with a bonus
    /// threshold above 0.01, or in a sandbox game).
    fn slot_active(&self, kind: usize, slot: usize) -> bool {
        let Some(t) = self.types.get(kind) else {
            return false;
        };
        let s = &t.slots[slot];
        let year = i32::from(self.year);
        let (out, in0, in1) = (s.output, &s.inputs[0], &s.inputs[1]);
        if out == 0x7f && in0.cargo == 0x7f {
            return false;
        }
        if year < i32::from(s.year_from) || i32::from(s.year_to) < year {
            return false;
        }
        if out != 0x7f && !self.cargo_enabled(usize::from(out)) {
            return false;
        }
        if in0.cargo != 0x7f
            && !self.cargo_enabled(usize::from(in0.cargo))
            && f64::from(in0.threshold) <= self.img.f64(0x0053_50b8)
            && !self.sandbox_flag()
        {
            return false;
        }
        if in1.cargo != 0x7f && !self.cargo_enabled(usize::from(in1.cargo)) {
            if self.img.f64(0x0053_50b8) < f64::from(in1.threshold) {
                return true;
            }
            if !self.sandbox_flag() {
                return false;
            }
        }
        true
    }

    /// `station_recompute_supply_demand` (`0x4bebf0`, cargo.md 8): waiting lots (`+0x61`),
    /// supply origin (`+0x169`), annual supply (`+0xb9`), demand (`+0x195`) and demand weight
    /// (`+0x1c1`) from the catchment links, town or city status (`+0x51`, `+0x52`) from the
    /// houses, and the price level (`+0x321`) of each cargo. Skipped when `+0x45 != 0`.
    pub fn station_recompute_supply_demand(&mut self, st: i32) {
        if self.i32(P::Station, st, 0x45) != 0 {
            return;
        }
        for c in 0..CARGOS {
            self.set16(P::Station, st, 0x61 + 2 * c, 0);
            self.set32(P::Station, st, 0xb9 + 4 * c, 0);
            self.set32(P::Station, st, 0x1c1 + 4 * c, 0);
            self.set8(P::Station, st, 0x169 + c, 0);
            self.set8(P::Station, st, 0x195 + c, 0);
        }
        let mut houses = 0;
        let mut w = 0f32;
        let nlinks = self.i32(P::Station, st, 0x41);
        for l in 0..nlinks.max(0) {
            let mut link = self.i32(P::Station, st, 0x3d);
            for _ in 0..l {
                link = self.i32(P::Link, link, 0xa);
            }
            let ind = i32::from(self.u16(P::Link, link, 0));
            let kind = self.u8(P::Industry, ind, 7);
            if kind == 0 {
                // A station building: the lots dropped at that station.
                let other = i32::from(self.u16(P::Industry, ind, 8));
                let mut lot = i32::from(self.u16(P::Station, other, 0x4be));
                while lot != 0 {
                    let c = i32::from(self.i8(P::Lot, lot, 0));
                    let o = (0x169 + c) as usize;
                    if self.u8(P::Station, st, o) == 0 {
                        self.set8(P::Station, st, o, 2);
                    }
                    let o = (0x61 + 2 * c) as usize;
                    let v = self.i16(P::Station, st, o).wrapping_add(1);
                    self.set16(P::Station, st, o, v as u16);
                    lot = i32::from(self.u16(P::Lot, lot, 0x13));
                }
                continue;
            }
            if self.flag_570229 != 0 && kind != 0x14 && kind != 0x18 {
                continue;
            }
            let k = usize::from(kind);
            for slot in 0..4usize {
                let Some(t) = self.types.get(k) else { break };
                let s = t.slots[slot];
                let out = usize::from(s.output);
                if s.output == 0x7f {
                    continue;
                }
                if self.slot_active(k, slot) {
                    self.set8(P::Station, st, 0x169 + out, 1);
                    if s.mode != 0 {
                        let o = 0xb9 + 4 * out;
                        let v = (f64::from(s.rate) + f64::from(self.f32(P::Station, st, o))) as f32;
                        self.setf32(P::Station, st, o, v);
                    }
                }
                let o = 0x61 + 2 * out;
                let v = self.i16(P::Station, st, o).wrapping_add(self.i16(
                    P::Industry,
                    ind,
                    0x8b + 2 * slot,
                ));
                self.set16(P::Station, st, o, v as u16);
            }
            if self.sandbox_flag() && self.u8(P::Industry, ind, 0x9e) & 0x80 == 0 {
                continue;
            }
            let links = f64::from(self.i8(P::Industry, ind, 0x33));
            if kind == 0x12 {
                houses += 1;
                w = (self.img.f64(0x0053_50c8) / links + f64::from(w)) as f32;
            }
            if kind == 0x27 {
                houses += 2;
                w = (self.img.f64(0x0053_5198) / links + f64::from(w)) as f32;
            }
            let Some(t) = self.types.get(k) else { continue };
            let (inputs, weights) = (t.inputs, t.weights);
            let nl = i32::from(self.i8(P::Industry, ind, 0x33));
            for j in 0..8usize {
                let c = inputs[j];
                if c == 0x7f || !self.cargo_enabled(usize::from(c)) {
                    continue;
                }
                let c = usize::from(c);
                self.set8(P::Station, st, 0x195 + c, 1);
                let o = 0x1c1 + 4 * c;
                let add = i32::from(weights[j]) / nl;
                let v = (f64::from(add) + f64::from(self.f32(P::Station, st, o))) as f32;
                self.setf32(P::Station, st, o, v);
            }
        }
        self.set8(P::Station, st, 0x52, 0);
        self.set8(P::Station, st, 0x51, 0);
        if houses >= 4 {
            // City (8 or more house points, cargo +0x16) or town (4..7, cargo +0x14).
            let limit_off = if houses >= 8 { 0x16 } else { 0x14 };
            for c in 0..CARGOS {
                let rec = 0x0054_04b1 + 0x21 * c as u32;
                let limit = i32::from(self.img.i16(rec + limit_off));
                if i32::from(self.year) <= limit && self.cargo_enabled(c) {
                    self.set8(P::Station, st, 0x195 + c, 1);
                    let o = 0x1c1 + 4 * c;
                    let v = (f64::from(w) * f64::from(self.img.f32(rec + 0x18))
                        + f64::from(self.f32(P::Station, st, o)))
                        as f32;
                    self.setf32(P::Station, st, o, v);
                }
            }
            self.set8(P::Station, st, 0x52, 1);
            if houses >= 8 {
                self.set8(P::Station, st, 0x51, 1);
            }
        }
        let building = self.i32(P::Station, st, 4);
        let age = i32::from(self.year) - i32::from(self.u16(P::Industry, building, 0x3c));
        let age = if age < 1 { 0 } else { age };
        let four = f64::from(self.img.f32(0x0053_75b4));
        for c in 0..CARGOS {
            let d = f64::from(self.f32(P::Station, st, 0x1c1 + 4 * c));
            let s = self.f32(P::Station, st, 0x271 + 4 * c);
            let mut n: u8 = if f64::from(s) <= f64::from(self.img.f32(0x0053_500c)) {
                9
            } else {
                ftol(d * four / f64::from(s)) as u8
            };
            if age < 5 {
                let v = (f64::from(5 - age) * d
                    + f64::from((age + 2) * i32::from(n))
                    + self.img.f64(0x0053_7768))
                    * self.img.f64(0x0053_7758);
                n = ftol(v) as u8;
            }
            let lim = (d + four) as f32;
            let x = f64::from(n);
            n = ftol(if x < f64::from(lim) {
                x
            } else {
                f64::from(lim)
            }) as u8;
            if n > 8 {
                n = 9;
            }
            self.set8(P::Station, st, 0x321 + c, n);
        }
    }

    /// The station part of `industry_spawn_lot` (`0x40a770`) for a lot of `cargo` just created by
    /// industry `ind`: the waiting counter of every linked station (`0x468af0`), then its
    /// supply and demand and its produced-lots counter (`+0x4b5`).
    pub fn industry_lot_spawned(&mut self, ind: i32, cargo: u8) {
        let n = i32::from(self.i8(P::Industry, ind, 0x33));
        let station_k = |ts: &Ts<'_>, k: i32| {
            let mut link = ts.i32(P::Industry, ind, 0x2f);
            for _ in 0..k {
                link = ts.i32(P::Link, link, 6);
            }
            i32::from(ts.u16(P::Link, link, 2))
        };
        for k in 0..n {
            let st = station_k(self, k);
            let o = 0x61 + 2 * usize::from(cargo);
            let v = self.i16(P::Station, st, o).wrapping_add(1);
            self.set16(P::Station, st, o, v as u16);
        }
        for k in 0..n {
            let st = station_k(self, k);
            self.station_recompute_supply_demand(st);
            let v = self.i16(P::Station, st, 0x4b5).wrapping_add(1);
            self.set16(P::Station, st, 0x4b5, v as u16);
        }
    }
}
