//! Arrival at a station and cargo delivery (trains.md 10.5, cargo.md 4 and 6): `train_arrive`
//! (`0x4d29e0`), the unload rules (`0x4d2720`, `0x4d2590`), the delivery of a lot
//! (`0x414510` -> `cargo_delivery_revenue` `0x4bf760`) and the settlement (`0x4d2c10`).
//!
//! Lot record (`0x574c7c`, 0x15 bytes): `+0` cargo (i8), `+1` u16 origin industry, `+3` u16
//! origin station, `+5` u16 car, `+7` creation minute, `+0xb` loading minute, `+0xf` u16 next lot
//! of the industry pile, `+0x11` u16 station where it was dropped, `+0x13` u16 next of the
//! station list.

use super::mem::{P, Ts};
use crate::cargo::{Consumer, Delivery, delivery_revenue};
use crate::fp::ftol;

/// Station fee per delivered car, at the previous and at the current station (`0x4d2c10`).
const STATION_FEE: i32 = 2000;

impl Ts<'_> {
    /// The `i`-th car of the train whose vehicle carries cargo (`+0x30 != 0x7f`), or 0.
    fn cargo_car(&self, t: i32, i: i32) -> i32 {
        let mut n = 0;
        let mut c = self.i32(P::Train, t, 0);
        while c != 0 {
            if self.vt8(self.u8(P::Car, c, 8), 0x30) != 0x7f {
                n += 1;
                if n == i + 1 {
                    return c;
                }
            }
            c = i32::from(self.i16(P::Car, c, 2));
        }
        0
    }

    fn cargo_car_count(&self, t: i32) -> i32 {
        self.cars_with(t, |ts, c| ts.vt8(ts.u8(P::Car, c, 8), 0x30) != 0x7f)
            .len() as i32
    }

    fn car_cargo(&self, car: i32) -> i32 {
        i32::from(self.vt8(self.u8(P::Car, car, 8), 0x30) as i8)
    }

    fn stop_station_id(&self, t: i32) -> i32 {
        i32::from(self.stop16(t, self.cur_stop(t), 0))
    }

    // ---- train_arrive (0x4d29e0) ----------------------------------------------------------

    /// `train_arrive` (`0x4d29e0`): the head reached the middle of its stop's station tile. Every
    /// cargo car waits for its reservation (`+0x47 = 1`); the cars that unload here are
    /// estimated (commit 1) and a lot crossing a region boundary, or delivered to a port, adds
    /// 8 days of customs (4 with the customs house, station `+0x49 & 0x200`). With customs the
    /// train waits in state 9 (`0x4d2c10` runs when it ends); without, the settlement runs now.
    pub fn train_arrive(&mut self, t: i32) {
        // 0x49b610: camera distance for the volume; the sound variant is drawn anyway.
        self.crt.range(0, 3);
        let station = self.stop_station_id(t);
        self.set8(P::Station, station, 0x4bb, 1);
        let region = self.i32(P::Station, station, 0x4ad);
        let mut delay = 0;
        let mut i = 0;
        loop {
            if self.cargo_car_count(t) <= i {
                if delay != 0 {
                    self.seti32(P::Train, t, 0x69, 0);
                    self.reserve(t, 1, 0, 9);
                    let v = self.i32(P::Train, t, 0x6d).wrapping_add(delay);
                    self.seti32(P::Train, t, 0x71, v);
                    return;
                }
                self.settlement(t);
                return;
            }
            let car = self.cargo_car(t, i);
            self.set8(P::Car, car, 0x47, 1);
            if self.car_should_unload(t, i) && self.i32(P::Car, car, 0x2d) != 0 {
                let (_, _, industry) = self.car_deliver_lot(car, station, true);
                let lot = self.i32(P::Car, car, 0x2d);
                let src = i32::from(self.u16(P::Lot, lot, 1));
                if src != 0 && self.live(P::Industry, src) {
                    let src_region = i32::from(self.i8(P::Industry, src, 0x9d));
                    let to_port = industry != 0 && self.u8(P::Industry, industry as i32, 7) == 0x1c;
                    let same = self.i8(P::Territory, src_region, 0x2a)
                        == self.i8(P::Territory, region, 0x2a);
                    // 0x570239: header option that waives customs.
                    if to_port || !(same || self.s.header.get(0x8701).copied().unwrap_or(0) != 0) {
                        delay += if self.u32(P::Station, station, 0x49) & 0x200 != 0 {
                            0x1680
                        } else {
                            0x2d00
                        };
                    }
                }
            }
            i += 1;
        }
    }

    /// `car_should_unload` (`0x4d2720`, train, i): the `i`-th cargo car unloads at the current
    /// stop. Unload mode (stop `+3 & 0x30`): 1 keeps cargo meant for a transfer, 2 always
    /// unloads, 3 unloads only up to the cars of that cargo the stop asks for (`+2`, `+4..`),
    /// 0 unloads what this station demands or no later stop wants.
    pub fn car_should_unload(&self, t: i32, i: i32) -> bool {
        if self.flag_570236 != 0 {
            return false;
        }
        let k = self.cur_stop(t);
        let b = self.stop8(t, k, 3);
        if b & 8 != 0 {
            return false;
        }
        let mode = (b & 0x30) >> 4;
        if mode == 1 && self.car_keeps_for_transfer(t, i) {
            return false;
        }
        let n = i32::from(self.u8(P::Train, t, 0x3b));
        if n == 0 {
            return true;
        }
        let car = self.cargo_car(t, i);
        if self.i32(P::Car, car, 0x2d) == 0 {
            return true;
        }
        let c = self.car_cargo(car);
        let here = i32::from(self.stop16(t, k, 0));
        if self.u8(P::Station, here, (0x195 + c) as usize) != 0 {
            return true;
        }
        let lot = self.i32(P::Car, car, 0x2d);
        let origin = self.u16(P::Lot, lot, 3);
        let mut later = false;
        let mut j = (k + 1) % n;
        while j != k {
            let st = i32::from(self.stop16(t, j, 0));
            if self.stop8(t, j, 3) & 8 == 0
                && self.u8(P::Station, st, (0x195 + c) as usize) != 0
                && origin != self.stop16(t, j, 0)
            {
                later = true;
                break;
            }
            j = (j + 1) % n;
        }
        if self.s.header.get(0x86ef).copied().unwrap_or(0) != 0 {
            later = true;
        }
        if mode == 2 || (mode != 3 && !later) {
            return true;
        }
        let mut loaded = 0;
        for q in 0..i {
            let cq = self.cargo_car(t, q);
            if self.i32(P::Car, cq, 0x2d) != 0 && self.car_cargo(cq) == c {
                loaded += 1;
            }
        }
        let asked = (0..i32::from(self.stop8(t, k, 2)))
            .filter(|&q| i32::from(self.stop8(t, k, 4 + q as usize)) == c)
            .count() as i32;
        asked <= loaded
    }

    /// `0x4d2590`: unload mode 1 and the station does not demand the car's cargo: the lot is
    /// dropped here for a transfer.
    fn car_keeps_for_transfer(&self, t: i32, i: i32) -> bool {
        if self.flag_570236 != 0 {
            return false;
        }
        let k = self.cur_stop(t);
        let b = self.stop8(t, k, 3);
        if b & 8 != 0 {
            return false;
        }
        let car = self.cargo_car(t, i);
        let c = self.car_cargo(car);
        let here = i32::from(self.stop16(t, k, 0));
        self.i32(P::Car, car, 0x2d) != 0
            && b & 0x30 == 0x10
            && self.u8(P::Station, here, (0x195 + c) as usize) == 0
    }

    // ---- delivery (0x414510, 0x4bf760) ----------------------------------------------------

    /// `car_deliver_lot` (`0x414510`): revenue of the car's lot at `station`; with
    /// `estimate == false` the lot is delivered (side effects) and the car emptied.
    /// Returns `(revenue, on_time, consumer industry)`.
    pub fn car_deliver_lot(&mut self, car: i32, station: i32, estimate: bool) -> (i32, u32, u32) {
        let lot = self.i32(P::Car, car, 0x2d);
        if lot == 0 {
            return (0, 0, 0);
        }
        let r = self.cargo_delivery_revenue(station, lot, estimate);
        if !estimate {
            self.seti32(P::Car, car, 0x2d, 0);
        }
        r
    }

    /// `cargo_delivery_revenue` (`0x4bf760`, destination station, lot, commit): the revenue
    /// formula of cargo.md 4 ([`delivery_revenue`], verified) on the records, and the side
    /// effects of a real delivery (cargo.md 4.8).
    pub fn cargo_delivery_revenue(
        &mut self,
        dst: i32,
        lot: i32,
        estimate: bool,
    ) -> (i32, u32, u32) {
        let c = i32::from(self.i8(P::Lot, lot, 0));
        let car = i32::from(self.u16(P::Lot, lot, 5));
        let train = i32::from(self.i16(P::Car, car, 4));
        let company = self.i32(P::Train, train, 0x41);
        let human = self.company_is_human(company);
        let src = i32::from(self.u16(P::Lot, lot, 3));
        let dst_region = self.i32(P::Station, dst, 0x4ad);
        // Catchment of the destination: `+0x41` links from `+0x3d` (chained by `+0xa`).
        let mut consumers = Vec::new();
        let nlinks = self.i32(P::Station, dst, 0x41);
        let head = self.i32(P::Station, dst, 0x3d);
        let mut link = head;
        for _ in 0..nlinks.max(0) {
            let ind = i32::from(self.u16(P::Link, link, 0));
            let kind = self.u8(P::Industry, ind, 7);
            let ty = self.types.get(usize::from(kind));
            let mut stock = [0f32; 8];
            for (k, s) in stock.iter_mut().enumerate() {
                *s = self.f32(P::Industry, ind, 0x6b + 4 * k);
            }
            consumers.push(Consumer {
                id: ind as u16,
                kind,
                inputs: ty.map_or([0x7f; 8], |t| t.inputs),
                stock,
            });
            link = self.i32(P::Link, link, 0xa);
        }
        let effect_cargo = if c < 32 { 0x3e + c } else { 0xf1 + c } as usize;
        let mut d = Delivery {
            year: self.year,
            now: self.now,
            econ_state: self.state_i32(0x22f).clamp(0, 4) as usize,
            model: self.s.state[0x3890],
            slider_lo: self.s.trailer[0x34],
            slider_hi: self.s.trailer[0x35],
            cargo_id: c as usize,
            cargo: self.cargo[c as usize],
            lot_id: lot as u32,
            lot_industry: self.u16(P::Lot, lot, 1),
            lot_origin_station: src as u16,
            t_created: self.u32(P::Lot, lot, 7),
            t_loaded: self.u32(P::Lot, lot, 0xb),
            src_station_id: self.u32(P::Station, src, 0),
            src_xy: (
                self.i32(P::Station, src, 0xa),
                self.i32(P::Station, src, 0xe),
            ),
            src_upgrades: self.u32(P::Station, src, 0x49),
            dst_station_id: self.u32(P::Station, dst, 0),
            dst_xy: (
                self.i32(P::Station, dst, 0xa),
                self.i32(P::Station, dst, 0xe),
            ),
            dst_demands_cargo: self.u8(P::Station, dst, (0x195 + c) as usize) != 0,
            dst_price_level: self.u8(P::Station, dst, (0x321 + c) as usize),
            consumers: &consumers,
            train_flags: self.u8(P::Train, train, 0xbb),
            human,
            computer_factor: self.f32(P::Company, company, 0x4276),
            company_multiplier: self.f32(P::Company, company, 0x4248),
            effect_all: self.effect(0x3d, company, 0, dst_region),
            effect_cargo: self.effect(effect_cargo, company, 0, dst_region),
        };
        let mut rev = delivery_revenue(&d, self.revenue);
        if !estimate && rev.industry != 0 {
            // The consuming industry takes the lot in the middle of the formula (`0x4bfa78`):
            // its conversion recomputes the linked stations, and the price level `+0x321+c` is
            // read after the call (Example, day 3647: a milk delivery converted by a cheese
            // factory lowers the level of the next lot of the same settlement).
            self.industry_receive_cargo(rev.industry as i32, c as u32);
            d.dst_price_level = self.u8(P::Station, dst, (0x321 + c) as usize);
            rev = delivery_revenue(&d, self.revenue);
        }
        if self.flag_570229 != 0 && c == 0x11 && !estimate {
            self.set8(P::Station, dst, 0x4c0, 1);
        }
        let src_ind = i32::from(self.u16(P::Lot, lot, 1));
        if rev.base != 0 && src_ind != 0 && self.exists(P::Industry, src_ind) {
            let kind = usize::from(self.u8(P::Industry, src_ind, 7));
            let slot = self.output_slot(kind, c);
            if !estimate && rev.accepted {
                let o = (0x63 + slot) as usize;
                let v = self.u8(P::Industry, src_ind, o).wrapping_add(1);
                self.set8(P::Industry, src_ind, o, v);
            }
        }
        if !estimate {
            self.deliver_side_effects(dst, src, lot, c, rev.amount, rev.industry);
        }
        (rev.amount, rev.flag, rev.industry)
    }

    /// `industry_type_output_slot` (`0x468470`): slot of type `kind` whose output is cargo `c`
    /// (-1 when none).
    fn output_slot(&self, kind: usize, c: i32) -> i32 {
        self.types.get(kind).map_or(-1, |t| {
            t.slots
                .iter()
                .position(|s| i32::from(s.output) == c)
                .map_or(-1, |k| k as i32)
        })
    }

    /// `industry_receive_cargo(industry, c)` (`0x40b250`, cargo.md 7.4): the input stock of `c`
    /// (`+0x6b`, f32, `+= 1.0` from the f64 `0x5350c8`) and the delivery counter `+0x34`; then,
    /// for every converter slot (mode 0) valid this year, while the stock covers both inputs:
    /// takes them and makes `ceil(rate)` output lots (`industry_spawn_lot`); at the end, if a
    /// conversion happened, the active industry's linked stations recompute their supply and
    /// demand (`0x4bebf0`). Seen in the Example at day 3313 (a coal-and-iron delivery).
    pub fn industry_receive_cargo(&mut self, ind: i32, c: u32) {
        self.log.ran("industry_receive_cargo");
        let tid = usize::from(self.u8(P::Industry, ind, 7));
        let Some(ty) = self.types.get(tid).cloned() else {
            return;
        };
        if let Some(i) = ty.inputs.iter().position(|&x| u32::from(x) == c) {
            let o = 0x6b + 4 * i;
            let v = (f64::from(self.f32(P::Industry, ind, o)) + 1.0) as f32;
            self.setf32(P::Industry, ind, o, v);
        }
        let n = self.i16(P::Industry, ind, 0x34).wrapping_add(1);
        self.set16(P::Industry, ind, 0x34, n as u16);
        let year = i32::from(self.year);
        let mut converted = false;
        for slot in &ty.slots {
            if slot.output == 0x7f
                || i32::from(slot.year_from) > year
                || year > i32::from(slot.year_to)
                || slot.mode != 0
            {
                continue;
            }
            let [a, b] = slot.inputs;
            if a.need == 0 && b.need == 0 {
                // The original loops forever here; no converter of the data has no input.
                continue;
            }
            loop {
                let sa = 0x6b + 4 * usize::from(a.index);
                let sb = 0x6b + 4 * usize::from(b.index);
                if (a.need != 0 && self.f32(P::Industry, ind, sa) < f32::from(a.need))
                    || (b.need != 0 && self.f32(P::Industry, ind, sb) < f32::from(b.need))
                {
                    break;
                }
                if a.need != 0 {
                    let v = self.f32(P::Industry, ind, sa) - f32::from(a.need);
                    self.setf32(P::Industry, ind, sa, v);
                }
                if b.need != 0 {
                    let v = self.f32(P::Industry, ind, sb) - f32::from(b.need);
                    self.setf32(P::Industry, ind, sb, v);
                }
                if slot.rate > 0.0 {
                    let mut k = 0i32;
                    loop {
                        self.industry_spawn_lot(ind, slot.output);
                        k += 1;
                        if (k as f32) >= slot.rate {
                            break;
                        }
                    }
                }
                converted = true;
            }
        }
        let n = i32::from(self.i8(P::Industry, ind, 0x33));
        if converted && self.u8(P::Industry, ind, 0x4f) == 0 && n > 0 {
            for k in 0..n {
                let mut link = self.i32(P::Industry, ind, 0x2f);
                for _ in 0..k {
                    link = self.i32(P::Link, link, 6);
                }
                let st = i32::from(self.u16(P::Link, link, 2));
                self.station_recompute_supply_demand(st);
            }
        }
    }

    /// `industry_spawn_lot(industry, cargo)` (`0x40a770`) on the train-step view: the same rule
    /// as [`crate::industry`]'s weekly production (50 waiting lots per output slot at most,
    /// except in the sandbox), then the linked stations ([`Ts::industry_lot_spawned`]).
    pub fn industry_spawn_lot(&mut self, ind: i32, cargo: u8) {
        use crate::industry::{
            LOT_CARGO, LOT_CREATED, LOT_INDUSTRY, LOT_NEXT, OFF_INACTIVE, OFF_PILE, OFF_PILE_COUNT,
            OFF_PRODUCED, OFF_STATIONS, OFF_TYPE,
        };
        if self.u8(P::Industry, ind, OFF_INACTIVE) != 0 {
            return;
        }
        let tid = usize::from(self.u8(P::Industry, ind, OFF_TYPE));
        let Some(i) = self
            .types
            .get(tid)
            .and_then(|t| t.slots.iter().position(|s| s.output == cargo))
        else {
            return;
        };
        let sandbox = self
            .s
            .header
            .get((0x0057_0227 - crate::ai::HDR_BASE) as usize)
            .is_some_and(|&b| b != 0);
        let waiting = self.i16(P::Industry, ind, OFF_PILE_COUNT + 2 * i);
        if waiting >= 0x32 && !sandbox {
            return;
        }
        let p = self.u8(P::Industry, ind, OFF_PRODUCED + i).wrapping_add(1);
        self.set8(P::Industry, ind, OFF_PRODUCED + i, p);
        self.set16(
            P::Industry,
            ind,
            OFF_PILE_COUNT + 2 * i,
            waiting.wrapping_add(1) as u16,
        );
        let mut lot = vec![0u8; self.record_size(P::Lot)];
        lot[LOT_CARGO] = cargo;
        lot[LOT_INDUSTRY..LOT_INDUSTRY + 2].copy_from_slice(&(ind as u16).to_le_bytes());
        lot[LOT_CREATED..LOT_CREATED + 4].copy_from_slice(&self.now.to_le_bytes());
        let head = self.u16(P::Industry, ind, OFF_PILE);
        lot[LOT_NEXT..LOT_NEXT + 2].copy_from_slice(&head.to_le_bytes());
        let slot = self.alloc(P::Lot, lot);
        self.set16(P::Industry, ind, OFF_PILE, slot as u16);
        if self.i8(P::Industry, ind, OFF_STATIONS) > 0 {
            self.industry_lot_spawned(ind, cargo);
        }
    }

    /// The `commit == 0` branch of `0x4bf760` (cargo.md 4.8).
    fn deliver_side_effects(&mut self, dst: i32, src: i32, lot: i32, c: i32, r: i32, ind: u32) {
        // `industry_receive_cargo` (`0x40b250`) already ran inside the formula when an industry
        // took the lot (`ind != 0`, see `cargo_delivery_revenue`).
        let _ = ind;
        let v = self.i16(P::Station, src, 0x4b7).wrapping_add(1);
        self.set16(P::Station, src, 0x4b7, v as u16);
        let v = self.i16(P::Station, dst, 0x4b9).wrapping_add(1);
        self.set16(P::Station, dst, 0x4b9, v as u16);
        let town = self.i32(P::Station, src, 0x53);
        if town != 0 {
            let v = self.i16(P::Place, town, 0x40).wrapping_add(1);
            self.set16(P::Place, town, 0x40, v as u16);
        }
        let town = self.i32(P::Station, dst, 0x53);
        if town != 0 {
            let v = self.i16(P::Place, town, 0x3e).wrapping_add(1);
            self.set16(P::Place, town, 0x3e, v as u16);
        }
        let v = self.state_i32(0x227).wrapping_add(1);
        self.set_state_i32(0x227, v);
        let origin = u32::from(self.u16(P::Lot, lot, 3));
        if self.u32(P::Station, dst, 0) == origin {
            // Delivered back at its own station: the same rule as `0x4bf450` (a fresh lot of an
            // existing industry returns to its pile, else it is freed). Example, day 5215.
            self.lot_back_to_industry(lot);
        } else {
            self.lot_free(lot);
        }
        let mut bonus = 0;
        if c == 0 {
            let up = self.u32(P::Station, dst, 0x49);
            let rf = f64::from(r);
            if up & 0x40 != 0 {
                bonus = ftol(rf * self.img.f64(0x0053_56d0));
            } else if up & 0x20 != 0 {
                bonus = ftol(rf * self.img.f64(0x0053_5178));
            }
            if up & 0x8000 != 0 {
                bonus = ftol(rf * self.img.f64(0x0053_50d8) + f64::from(bonus));
            } else if up & 0x10000 != 0 {
                bonus = ftol(rf * self.img.f64(0x0053_56d0) + f64::from(bonus));
            }
            if up & 0x80 != 0 {
                bonus = ftol(rf * self.img.f64(0x0053_5470) + f64::from(bonus));
            }
        }
        let owner = self.i32(P::Station, dst, 0x39);
        let m = self.effect(0x3c, owner, 0, self.i32(P::Station, dst, 0x4ad));
        // Original bug: `fiadd`, the multiplier is added (cargo.md 4.8).
        let srev = if self.g.rules.has(crate::ruleset::Fix::StationRevenueEffect) {
            ftol(f64::from(m) * f64::from(bonus))
        } else {
            ftol(f64::from(m) + f64::from(bonus))
        };
        let v = self
            .i32(P::Station, dst, 0x5d)
            .wrapping_add(r.wrapping_add(srev));
        self.seti32(P::Station, dst, 0x5d, v);
        let o = (0x271 + 4 * c) as usize;
        let f =
            (f64::from(self.f32(P::Station, dst, o)) + f64::from(self.img.f32(0x0053_5078))) as f32;
        self.setf32(P::Station, dst, o, f);
        let v = self.i32(P::Station, src, 0x59).wrapping_add(r);
        self.seti32(P::Station, src, 0x59, v);
        if srev != 0 && owner != 0 {
            self.ledger_credit(owner, 2, f64::from(srev));
        }
    }

    /// `0x4bf450(lot)` (and the same branch of `0x4bf760`): a lot unloaded back at its own
    /// station while still fresh (age below half its cargo's life, `0x5404b9 + 0x21*cargo`) returns to its industry: the
    /// industry's waiting count for that output (`+0x8b + 2*k`, `k` from `0x40b1b0`) grows,
    /// the lot forgets its station and car (`+3..+6`, `+0xb..+0xe`) and every linked station
    /// counts it again (`0x468af0`); otherwise it is freed (`0x466a60`).
    pub fn lot_back_to_industry(&mut self, lot: i32) {
        let cargo = self.i8(P::Lot, lot, 0);
        let age = self.now.wrapping_sub(self.u32(P::Lot, lot, 7));
        // `cargo.+8` of the run-time table (`0x5404b9 + 0x21*cargo`), which the loader
        // (`0x471b10`) turns into minutes: `wait_scale` (the static image still has days).
        let life = (self
            .cargo
            .get(cargo as u8 as usize)
            .map_or(0, |k| k.wait_scale)
            >> 1) as u32;
        let ind = i32::from(self.u16(P::Lot, lot, 1));
        if age < life && ind != 0 && self.exists(P::Industry, ind) {
            let set = u32::from(
                self.s
                    .header
                    .get((0x0057_04ac - crate::ai::HDR_BASE) as usize)
                    .copied()
                    .unwrap_or(0),
            );
            let ty = u32::from(self.u8(P::Industry, ind, 7));
            let base = 0x0054_1ae5 + (set * 0x35 + ty) * 0x90;
            let k = (0..4u32)
                .find(|&k| self.img.u8(base + 0x20 * k) == cargo as u8)
                .map_or(-1, |k| k as i32);
            let o = (0x8b + 2 * k) as usize;
            let v = self.i16(P::Industry, ind, o).wrapping_add(1);
            self.set16(P::Industry, ind, o, v as u16);
            for off in [3usize, 4, 5, 6, 0xb, 0xc, 0xd, 0xe] {
                self.set8(P::Lot, lot, off, 0);
            }
            let n = i32::from(self.i8(P::Industry, ind, 0x33));
            let mut link = self.i32(P::Industry, ind, 0x2f);
            for _ in 0..n {
                let st = i32::from(self.u16(P::Link, link, 2));
                let o = (0x61 + 2 * i32::from(cargo)) as usize;
                let v = self.i16(P::Station, st, o).wrapping_add(1);
                self.set16(P::Station, st, o, v as u16);
                link = self.i32(P::Link, link, 6);
            }
            self.log.ran("lot_back_to_industry");
        } else {
            self.lot_free(lot);
        }
    }

    /// `lot_free` (`0x466a60`): unlinks the lot from its industry pile, its station list and
    /// its car, and frees it.
    pub fn lot_free(&mut self, lot: i32) {
        let ind = i32::from(self.u16(P::Lot, lot, 1));
        if ind != 0 {
            // 0x40ad50: unlink from the pile `+0x4d` (chained by lot `+0xf`).
            let mut link: Option<i32> = None;
            let mut cur = i32::from(self.u16(P::Industry, ind, 0x4d));
            loop {
                if cur == 0 {
                    self.set16(P::Lot, lot, 0x11, 0);
                    break;
                }
                let nx = self.u16(P::Lot, cur, 0xf);
                if cur == lot {
                    match link {
                        None => self.set16(P::Industry, ind, 0x4d, nx),
                        Some(l) => self.set16(P::Lot, l, 0xf, nx),
                    }
                    self.set16(P::Lot, cur, 0xf, 0);
                    break;
                }
                link = Some(cur);
                cur = i32::from(nx);
            }
        }
        if self.u16(P::Lot, lot, 0x11) != 0 {
            todo_hook!(
                self.log,
                "station_unlink_lot",
                "0x4beb20 (unlink a dropped lot from its station)",
                "cargo.md 6"
            );
        }
        if self.u16(P::Lot, lot, 3) == 0 {
            self.lot_unload_origin(lot);
        }
        let car = i32::from(self.u16(P::Lot, lot, 5));
        if car != 0 && self.i32(P::Car, car, 0x2d) == lot {
            self.seti32(P::Car, car, 0x2d, 0);
        }
        self.free(P::Lot, lot);
    }

    // ---- ledgers (0x439970, 0x4d4200, 0x43bde0) -------------------------------------------

    /// `company_ledger_credit` (`0x439970`, finance.md 1) on the raw company record.
    pub fn ledger_credit(&mut self, company: i32, cat: usize, x: f64) {
        let f64at = |ts: &Ts<'_>, o: usize| -> f64 {
            let lo = ts.u32(P::Company, company, o);
            let hi = ts.u32(P::Company, company, o + 4);
            f64::from_bits(u64::from(lo) | (u64::from(hi) << 32))
        };
        let put = |ts: &mut Ts<'_>, o: usize, v: f64| {
            let b = v.to_bits();
            ts.set32(P::Company, company, o, b as u32);
            ts.set32(P::Company, company, o + 4, (b >> 32) as u32);
        };
        self.set32(P::Company, company, 0x4250, 0);
        let o = 0x4286 + 8 * cat;
        let v = x + f64at(self, o);
        put(self, o, v);
        let o = 0x46fe + 8 * cat;
        let v = x + f64at(self, o);
        put(self, o, v);
        let v = x + f64at(self, 0x4766);
        put(self, 0x4766, v);
        let v = f64at(self, 0x479e) - x;
        put(self, 0x479e, v);
    }

    /// `train_ledger_add` (`0x4d4200`, train, category, x): the train's monthly and yearly
    /// accumulators (`+0xfd`, `+0x13d`; categories 0x65..0x67 count as 0) and the company
    /// ledger (1 -> 6, 2 -> 7, 3 -> 8 with the train total `+0xb7`; 0x65 -> 0; 0x66, 0x67 -> 1).
    pub fn train_ledger_add(&mut self, t: i32, cat: i32, x: i32) {
        let k = if cat > 100 && cat < 0x68 { 0 } else { cat };
        let o = (0xfd + 4 * k) as usize;
        let v = self.i32(P::Train, t, o).wrapping_add(x);
        self.seti32(P::Train, t, o, v);
        let o = (0x13d + 4 * k) as usize;
        let v = self.i32(P::Train, t, o).wrapping_add(x);
        self.seti32(P::Train, t, o, v);
        let company = self.i32(P::Train, t, 0x41);
        let ledger = match cat {
            1 => Some(6),
            2 => Some(7),
            3 => Some(8),
            0x65 => Some(0),
            0x66 | 0x67 => Some(1),
            _ => None,
        };
        if matches!(cat, 1..=3) {
            let v = self.i32(P::Train, t, 0xb7).wrapping_add(x);
            self.seti32(P::Train, t, 0xb7, v);
        }
        if let Some(l) = ledger {
            self.ledger_credit(company, l, f64::from(x));
        }
    }

    /// `company_delivery_stats` (`0x43bde0`, company, cars, units, speed, from region, to
    /// region, cargo list): delivery counters of the company (totals, per region pair and per
    /// cargo), of the destination territory and the world totals (`0x573d05..0x573d19`).
    #[allow(clippy::too_many_arguments)]
    fn company_delivery_stats(
        &mut self,
        company: i32,
        n: i32,
        units: i32,
        speed: i32,
        from: i32,
        to: i32,
        cargos: &[u8],
    ) {
        let add32 = |ts: &mut Ts<'_>, o: usize, d: i32| {
            let v = ts.i32(P::Company, company, o).wrapping_add(d);
            ts.seti32(P::Company, company, o, v);
        };
        let add16 = |ts: &mut Ts<'_>, o: i32, d: i32| {
            let o = o as usize;
            let v = ts.i16(P::Company, company, o).wrapping_add(d as i16);
            ts.set16(P::Company, company, o, v as u16);
        };
        add32(self, 0x19e, n);
        add32(self, 0x192, n);
        let to_o = to * 0x1ed;
        let from_o = from * 0x1ed;
        add16(self, to_o + 0x480, n);
        add16(self, to_o + 0x47e, n);
        add16(self, to_o + 0x482 + from * 2, n);
        add16(self, from_o + 0x574, n);
        add16(self, from_o + 0x572, n);
        add16(self, from_o + 0x576 + to * 2, n);
        for &c in cargos.iter().take(n.max(0) as usize) {
            let c = i32::from(c);
            add16(self, to_o + 0x4c2 + c * 2, 1);
            add16(self, to_o + 0x51a + c * 2, 1);
            add16(self, from_o + 0x5b6 + c * 2, 1);
            add16(self, from_o + 0x60e + c * 2, 1);
        }
        let v = self.i16(P::Territory, to, 0x52).wrapping_add(n as i16);
        self.set16(P::Territory, to, 0x52, v as u16);
        let v = self.i32(P::Territory, to, 0x54).wrapping_add(n);
        self.seti32(P::Territory, to, 0x54, v);
        let us = units.wrapping_mul(speed);
        add32(self, 0x196, units);
        add32(self, 0x1a2, units);
        add32(self, 0x19a, us);
        add32(self, 0x1a6, us);
        for (o, d) in [
            (0x381d, n),
            (0x3829, n),
            (0x3821, units),
            (0x382d, units),
            (0x3825, us),
            (0x3831, us),
        ] {
            let v = self.state_i32(o).wrapping_add(d);
            self.set_state_i32(o, v);
        }
    }

    // ---- settlement (0x4d2c10) -------------------------------------------------------------

    /// `train_arrival_settlement` (`0x4d2c10`, cargo.md 6): unload and pay. Lots for a transfer
    /// are dropped at the station; the others are delivered (each car pays $2000 to the
    /// previous and to the current station); the net revenue is shared by track mileage
    /// (`+0x7d` total, `+0x7f` own, `+0x81..+0x8a` per owner) and booked as passengers, mail
    /// and freight; the dwell is `(cars + 1) * 2880` minutes (station services, effect 0x29).
    pub fn settlement(&mut self, t: i32) {
        self.reserve(t, 1, 0, 2);
        let k = self.cur_stop(t);
        if self.stop8(t, k, 3) & 8 != 0 {
            let v = self.i32(P::Train, t, 0x6d);
            self.seti32(P::Train, t, 0x71, v);
            return;
        }
        let elapsed = self.now.wrapping_sub(self.u32(P::Train, t, 0xe5)) as i32;
        let per_year = f64::from(self.img.f32(0x0053_780c));
        let leg = if k != 0 && f64::from(self.f32(P::Train, t, 0xe9)) == self.img.f64(0x0053_5198) {
            (f64::from(elapsed) * per_year / f64::from(self.u8(P::Train, t, 0x3b))) as f32
        } else {
            (f64::from(elapsed) * per_year) as f32
        };
        self.setf32(P::Train, t, 0xe9, leg);
        if f64::from(leg) < self.img.f64(0x0053_5128) {
            self.set32(P::Train, t, 0xe9, 0x3f00_0000);
        }
        let prev = i32::from(self.u16(P::Train, t, 0x9e));
        let from_region = if prev != 0 && self.live(P::Station, prev) {
            self.i32(P::Station, prev, 0x4ad)
        } else {
            0
        };
        self.seti32(P::Train, t, 0x69, 0);
        let station = self.stop_station_id(t);
        let (mut pass, mut mail, mut freight) = (0i32, 0i32, 0i32);
        let (mut fee_prev, mut fee_cur) = (0i32, 0i32);
        let mut cargos: Vec<u8> = Vec::new();
        let mut i = 0;
        loop {
            if self.cargo_car_count(t) <= i {
                break;
            }
            if self.car_keeps_for_transfer(t, i) {
                let car = self.cargo_car(t, i);
                let st = self.stop16(t, k, 0);
                self.set16(P::Train, t, 0xf5, st);
                let v = self.i32(P::Train, t, 0x6d);
                self.seti32(P::Train, t, 0xf7, v);
                if self.i32(P::Car, car, 0x2d) != 0 {
                    todo_hook!(
                        self.log,
                        "station_drop_lot",
                        "0x4144e0 -> 0x4beba0 (drop a lot at the station for a transfer)",
                        "cargo.md 6"
                    );
                }
                i += 1;
                continue;
            }
            if self.car_should_unload(t, i) {
                let car = self.cargo_car(t, i);
                if self.i32(P::Car, car, 0x2d) != 0 {
                    let cargo = self.vt8(self.u8(P::Car, car, 8), 0x30);
                    let (r, _flag, _ind) = self.car_deliver_lot(car, station, false);
                    cargos.push(cargo);
                    match cargo {
                        0 => pass = pass.wrapping_add(r),
                        1 => mail = mail.wrapping_add(r),
                        _ => freight = freight.wrapping_add(r),
                    }
                    fee_prev += STATION_FEE;
                    fee_cur += STATION_FEE;
                }
            }
            i += 1;
        }
        let n = cargos.len() as i32;
        let mut d = (n + 1) * 2880;
        let services = self.u32(P::Station, station, 0x49);
        if services & 8 != 0 {
            d = ftol(f64::from(d) * self.img.f64(0x0053_5128));
        } else if services & 0x10 != 0 {
            d = ftol(f64::from(d) * self.img.f64(0x0053_5228));
        }
        let company = self.i32(P::Train, t, 0x41);
        let terr = i32::from(self.u8(P::Train, t, 0xf3));
        let mut m = f64::from(self.effect(0x29, company, 0, terr));
        if m < self.img.f64(0x0053_5178) {
            m = f64::from(self.img.f32(0x0053_77dc));
        }
        let v = ftol(f64::from(d) / m).wrapping_add(self.i32(P::Train, t, 0x6d));
        self.seti32(P::Train, t, 0x71, v);
        let gross = freight.wrapping_add(pass).wrapping_add(mail);
        let (mut own_prev, mut own_cur) = (false, false);
        // Original bugs: the owner is credited even when it is the train's company (which pays
        // nothing), and an unowned station takes the fee without crediting anyone.
        let fix_own = self.g.rules.has(crate::ruleset::Fix::StationFeeOwnStation);
        let fix_unowned = self.g.rules.has(crate::ruleset::Fix::StationFeeUnowned);
        if fee_prev != 0 {
            let owner = self.i32(P::Station, prev, 0x39);
            if owner != 0 && !(fix_own && owner == company) {
                self.ledger_credit(owner, 2, f64::from(fee_prev));
            }
            own_prev = owner == company || (fix_unowned && owner == 0);
        }
        if fee_cur != 0 {
            let owner = self.i32(P::Station, station, 0x39);
            if owner != 0 && !(fix_own && owner == company) {
                self.ledger_credit(owner, 2, f64::from(fee_cur));
            }
            own_cur = owner == company || (fix_unowned && owner == 0);
        }
        let deducted =
            (if own_cur { 0 } else { fee_cur }).wrapping_add(if own_prev { 0 } else { fee_prev });
        let mut net = gross.wrapping_sub(deducted);
        if net < 0 {
            net = 0;
        }
        let total = i32::from(self.i16(P::Train, t, 0x7d));
        for j in 0..usize::from(self.u8(P::Train, t, 0x81)) {
            let owner = i32::from(self.u16(P::Train, t, 0x82 + 2 * j));
            let share = i32::from(self.i16(P::Train, t, 0x8a + 2 * j));
            let ratio = (f64::from(share) / f64::from(total)) as f32;
            if self.live(P::Company, owner) && self.i32(P::Company, owner, 0x16e) != 0 {
                self.ledger_credit(owner, 4, f64::from(net) * f64::from(ratio));
            }
        }
        let own = (f64::from(self.i16(P::Train, t, 0x7f)) / f64::from(total)) as f32;
        let g = gross as f32;
        let nown = (f64::from(net) * f64::from(own)) as f32;
        let part = |x: i32| ftol(f64::from(x) / f64::from(g) * f64::from(nown));
        self.train_ledger_add(t, 0x65, part(pass));
        self.train_ledger_add(t, 0x66, part(mail));
        self.train_ledger_add(t, 0x67, part(freight));
        self.seti32(P::Train, t, 0xb7, 0);
        let dt = self.now.wrapping_sub(self.u32(P::Train, t, 0xc7)) as i32;
        let speed = if dt > 0 {
            ftol(f64::from(total.wrapping_mul(140_000)) / f64::from(dt) + self.img.f64(0x0053_5128))
        } else {
            0x1e
        };
        let to_region = self.i32(P::Station, station, 0x4ad);
        self.company_delivery_stats(
            company,
            n,
            total.wrapping_mul(n).wrapping_mul(10),
            speed,
            from_region,
            to_region,
            &cargos,
        );
        if self.is_ui_company(company) {
            // `0x46abf0`: the cash register sound (`0x47d100`, `chin161m.raw`) plays
            // `clamp(v / 25000, 1, 10)` times when the delivery earned `v = ftol(gross * own
            // share - station fees) > 0`, and draws its variant with `crt_rand_range(0, 2)`
            // (seen in the Example at day 529). Assumes the sound option `[0x574bc8]+0x224 != 2`
            // and that the message box accepted the message (otherwise `v` comes from the last
            // accepted one, `0x574430`).
            let v = ftol(f64::from(gross) * f64::from(own) - f64::from(deducted));
            if v > 0 {
                self.crt.range(0, 2);
            }
            todo_hook!(
                self.log,
                "delivery_message",
                "0x46abf0 (message box, speed record prompt of the viewed company)",
                "cargo.md 6"
            );
        } else {
            let best = self.i32(P::Company, company, 0x15c).max(speed);
            self.seti32(P::Company, company, 0x15c, best);
        }
        self.set16(P::Train, t, 0x9e, self.stop16(t, k, 0));
        self.seti32(P::Train, t, 0xaf, 0);
        self.seti32(P::Train, t, 0xb3, 0);
        let mut c = self.i32(P::Train, t, 0);
        while c != 0 {
            let w = self.vehicle_weight(c);
            let v = self.i32(P::Train, t, 0xaf) + w;
            self.seti32(P::Train, t, 0xaf, v);
            if self.vt8(self.u8(P::Car, c, 8), 0x2e) == 2 {
                let v = self.i32(P::Train, t, 0xb3) + w;
                self.seti32(P::Train, t, 0xb3, v);
            }
            c = i32::from(self.i16(P::Car, c, 2));
        }
        if self.flag_570229 != 0 {
            todo_hook!(
                self.log,
                "settlement_570229",
                "0x4d2c10 cargo 0x11 flag and 0x4d14b0 (flag 0x570229)",
                "cargo.md 6"
            );
            return;
        }
        let f = self.u8(P::Train, t, 0xbb);
        self.set8(P::Train, t, 0xbb, f & 0xdf);
    }

    /// `0x574414` (runtime): the company the interface shows; the clone takes the company of
    /// the human player (person `+0x122`, chairman of `+0x263`).
    pub(crate) fn is_ui_company_pub(&self, company: i32) -> bool {
        self.is_ui_company(company)
    }

    fn is_ui_company(&self, company: i32) -> bool {
        let mut p = self.first_id(P::Person);
        while p != 0 {
            if self.i32(P::Person, p, 0x122) != 0 {
                return self.i32(P::Person, p, 0x263) == company;
            }
            p = self.next_id(P::Person, p);
        }
        false
    }
}

impl Ts<'_> {
    /// `vehicle_type_for_cargo` (`0x4682b0`): the first vehicle type carrying cargo `c` that is
    /// available this year (`+0x1c..=+0x1e`), 0 when none.
    pub fn vehicle_type_for_cargo(&self, c: i32) -> u8 {
        for t in 0..175u8 {
            if i32::from(self.vt8(t, 0x30) as i8) == c
                && self.vt16(t, 0x1c) <= self.year
                && self.year <= self.vt16(t, 0x1e)
            {
                return t;
            }
        }
        0
    }

    /// `train_consist_at_stop` (`0x4d1bb0`, trains.md 10.9 and 12): an AI train first picks the
    /// cargo of its cars (`0x4d8db0`); then the cars carrying cargo are matched one by one with
    /// the cargo list of the current stop (`+2` cars of vehicle type
    /// `vehicle_type_for_cargo(stop.+4+j)`). From the first mismatch on, the cars (from the
    /// last backwards) are removed (`0x4d0fa0`, their lots kept aside) and one car per cargo of
    /// the stop is appended (`0x414220`, off the track; the lots go back to a new car of the
    /// same cargo); cars beyond the list are removed. Without changes the train starts the
    /// station work (`0x4d18b0`); with changes the consist flags and weights are recomputed.
    pub fn train_consist_at_stop(&mut self, t: i32) {
        let company = self.i32(P::Train, t, 0x41);
        if !self.company_is_human(company) {
            self.ai_choose_consist(t);
        }
        let k = self.cur_stop(t);
        let want = i32::from(self.stop8(t, k, 2));
        let mut changes = 0;
        let mut saved: Vec<i32> = Vec::new();
        let mut j = 0;
        while j < want {
            let car = self.cargo_car(t, j);
            let cargo = i32::from(self.stop8(t, k, 4 + j as usize));
            let vt = self.vehicle_type_for_cargo(cargo);
            if car != 0 && self.u8(P::Car, car, 8) == vt {
                j += 1;
                continue;
            }
            // Mismatch at cargo car `j`: remove cargo cars `n-1 ..= j` (last first).
            let n = self.cargo_car_count(t);
            for q in (j..n).rev() {
                let c = self.cargo_car(t, q);
                changes += 1;
                let lot = self.i32(P::Car, c, 0x2d);
                if lot != 0 {
                    saved.push(lot);
                    self.seti32(P::Car, c, 0x2d, 0);
                }
                self.car_remove(t, c);
            }
            // Append a car for each cargo `j..want`, handing the saved lots back.
            while j < want {
                changes += 1;
                let cargo = i32::from(self.stop8(t, k, 4 + j as usize));
                let vt = self.vehicle_type_for_cargo(cargo);
                self.vehicle_init(t, vt);
                let car = self.i32(P::Train, t, 4);
                for lot in saved.iter_mut() {
                    if *lot != 0
                        && i32::from(self.u8(P::Lot, *lot, 0)) == cargo
                        && self.i32(P::Car, car, 0x2d) == 0
                    {
                        self.seti32(P::Car, car, 0x2d, *lot);
                        let id = self.u16(P::Car, car, 6);
                        self.set16(P::Lot, *lot, 5, id);
                        *lot = 0;
                    }
                }
                j += 1;
            }
            // Lots without a car of their cargo are freed.
            for lot in saved.drain(..) {
                if lot != 0 {
                    self.set16(P::Lot, lot, 5, 0);
                    self.lot_free(lot);
                }
            }
            break;
        }
        // Cars past the list (`0x4d1f72`).
        let n = self.cargo_car_count(t);
        if n > want {
            for q in (want..n).rev() {
                let c = self.cargo_car(t, q);
                changes += 1;
                self.car_remove(t, c);
            }
        }
        self.set8(P::Train, t, 0x9a, 6);
        if changes == 0 {
            self.station_work(t);
            self.trim_reservations_pub(t);
            return;
        }
        // Flags of the cargo mix (`+0xbb` bits 0 and 1: cargo `0x2c` / `0x2d`).
        let mut f = self.u8(P::Train, t, 0xbb) & 0xfc;
        for c in self.cars_with(t, |_, _| true) {
            match self.vt8(self.u8(P::Car, c, 8), 0x30) {
                0x2c => f |= 1,
                0x2d => f |= 2,
                _ => {}
            }
        }
        self.set8(P::Train, t, 0xbb, f);
        if self.i32(P::Train, t, 0x71) == 0 {
            let v = self.i32(P::Train, t, 0x6d);
            self.seti32(P::Train, t, 0x71, v);
        }
        let (mut tons, mut steam) = (0i32, 0i32);
        for c in self.cars_with(t, |_, _| true) {
            let w = self.vehicle_weight(c);
            tons += w;
            if self.vt8(self.u8(P::Car, c, 8), 0x2e) == 2 {
                steam += w;
            }
        }
        self.seti32(P::Train, t, 0xaf, tons);
        self.seti32(P::Train, t, 0xb3, steam);
        if self.flag_570229 != 0 {
            todo_hook!(
                self.log,
                "train_consist_570229",
                "0x4d1bb0 with the option 0x570229 (cargo 0x11 flag, 0x4d14b0)",
                "trains.md 6.1"
            );
        } else {
            let b = self.u8(P::Train, t, 0xbb) & 0xdf;
            self.set8(P::Train, t, 0xbb, b);
        }
        self.seti32(P::Train, t, 0x2f, 0);
        self.trim_reservations_pub(t);
    }

    /// `ai_choose_consist` (`0x4d8db0`): for an AI train between two stations, each car whose
    /// cargo has no lot waiting at this station (`+0x61[c]` beyond the cars already counted)
    /// takes instead the waiting cargo with the best price level at the next station
    /// (`+0x195`, `+0x321`).
    fn ai_choose_consist(&mut self, t: i32) {
        let k = self.cur_stop(t);
        let n = i32::from(self.u8(P::Train, t, 0x3b));
        let next = (k + 1) % n;
        if self.stop8(t, k, 3) & 8 != 0 || self.stop8(t, next, 3) & 8 != 0 {
            return;
        }
        let here = i32::from(self.stop16(t, k, 0));
        let there = i32::from(self.stop16(t, next, 0));
        let mut used = [0u8; 44];
        let mut best_level = 0i32;
        for j in 0..i32::from(self.stop8(t, k, 2)) {
            let mut c = u32::from(self.stop8(t, k, 4 + j as usize));
            let waiting = |ts: &Ts<'_>, c: u32, used: &[u8; 44]| {
                i32::from(ts.i16(P::Station, here, 0x61 + 2 * c as usize))
                    - i32::from(used[c as usize])
            };
            if c > 0x2b || waiting(self, c, &used) <= 0 {
                let mut pick: Option<u32> = None;
                for q in 0..0x2cu32 {
                    if waiting(self, q, &used) > 0 {
                        let level = if self.u8(P::Station, there, 0x195 + q as usize) == 0 {
                            -1
                        } else {
                            i32::from(self.u8(P::Station, there, 0x321 + q as usize))
                        };
                        if pick.is_none() || best_level < level {
                            pick = Some(q);
                            best_level = level;
                        }
                    }
                }
                if let Some(q) = pick {
                    self.set_stop8(t, k, 4 + j as usize, q as u8);
                    c = q;
                }
            }
            if c < 0x2c {
                used[c as usize] += 1;
            }
        }
    }

    /// `station_load_lot` (`0x4bf260`, station, cargo, car id): the oldest lot of `cargo` not yet
    /// loaded in the piles of the station's catchment industries, or dropped at the station,
    /// goes into the car (origin station, car, loading minute, pile counters).
    pub fn station_load_lot(&mut self, station: i32, c: i32, car_id: u16) -> i32 {
        let mut best_t = 1_999_999_999u32;
        if self.now.wrapping_add(1) > 0x7735_93fe {
            best_t = self.now.wrapping_add(1);
        }
        let mut best = 0i32;
        let mut best_ind = 0i32;
        let mut last_ind = 0i32;
        let nlinks = self.i32(P::Station, station, 0x41);
        for l in 0..nlinks.max(0) {
            let mut link = self.i32(P::Station, station, 0x3d);
            for _ in 0..l {
                link = self.i32(P::Link, link, 0xa);
            }
            let ind = i32::from(self.u16(P::Link, link, 0));
            last_ind = ind;
            let kind = usize::from(self.u8(P::Industry, ind, 7));
            for slot in 0..4usize {
                let out = self.types.get(kind).map_or(0x7f, |t| t.slots[slot].output);
                if i32::from(out) != c {
                    continue;
                }
                if self.i16(P::Industry, ind, 0x8b + 2 * slot) == 0 {
                    break;
                }
                let mut lot = i32::from(self.u16(P::Industry, ind, 0x4d));
                while lot != 0 {
                    if i32::from(self.i8(P::Lot, lot, 0)) == c
                        && self.u16(P::Lot, lot, 3) == 0
                        && self.u32(P::Lot, lot, 7) < best_t
                    {
                        best_t = self.u32(P::Lot, lot, 7);
                        best = lot;
                        best_ind = ind;
                    }
                    lot = i32::from(self.u16(P::Lot, lot, 0xf));
                }
            }
        }
        let mut lot = i32::from(self.u16(P::Station, station, 0x4be));
        while lot != 0 {
            if i32::from(self.i8(P::Lot, lot, 0)) == c && self.u32(P::Lot, lot, 7) < best_t {
                best = lot;
                best_t = self.u32(P::Lot, lot, 7);
                best_ind = last_ind;
            }
            lot = i32::from(self.u16(P::Lot, lot, 0x13));
        }
        if self.u16(P::Lot, best, 3) == 0 {
            let id = self.u16(P::Station, station, 0);
            self.set16(P::Lot, best, 3, id);
        }
        self.set16(P::Lot, best, 5, car_id);
        self.set32(P::Lot, best, 0xb, self.now);
        if self.u16(P::Lot, best, 0x11) == 0 {
            let kind = usize::from(self.u8(P::Industry, best_ind, 7));
            let slot = self.output_slot(kind, c);
            if self.u16(P::Lot, best, 0x13) == 0 {
                let o = (0x8b + 2 * slot) as usize;
                let v = self.i16(P::Industry, best_ind, o).wrapping_sub(1);
                self.set16(P::Industry, best_ind, o, v as u16);
            }
        }
        self.lot_unload_origin(best);
        if best != 0 && c == 0x11 && self.u16(P::Lot, best, 0x11) == 0 {
            self.set8(P::Station, station, 0x4bd, 1);
        }
        best
    }

    /// `0x468b40`: a lot leaves its pile: the waiting counter `+0x61[c]` of the station where it
    /// was dropped, or of every station linked to its industry, drops by one.
    fn lot_unload_origin(&mut self, lot: i32) {
        let c = i32::from(self.i8(P::Lot, lot, 0));
        let at = i32::from(self.u16(P::Lot, lot, 0x11));
        if at != 0 {
            let o = (0x61 + 2 * c) as usize;
            let v = self.i16(P::Station, at, o).wrapping_sub(1);
            self.set16(P::Station, at, o, v as u16);
            if v == 0 && self.u8(P::Station, at, (0x169 + c) as usize) == 2 {
                self.set8(P::Station, at, (0x169 + c) as usize, 0);
            }
            todo_hook!(
                self.log,
                "station_unlink_lot",
                "0x4beb20 (unlink a dropped lot from its station)",
                "cargo.md 6"
            );
            return;
        }
        let ind = i32::from(self.u16(P::Lot, lot, 1));
        if ind == 0 {
            return;
        }
        let n = i32::from(self.i8(P::Industry, ind, 0x33));
        for k in 0..n {
            // 0x40a5c0: the k-th station link of the industry (`+0x2f`, chained by `+6`).
            let mut link = self.i32(P::Industry, ind, 0x2f);
            for _ in 0..k {
                link = self.i32(P::Link, link, 6);
            }
            let st = i32::from(self.u16(P::Link, link, 2));
            let o = (0x61 + 2 * c) as usize;
            let v = self.i16(P::Station, st, o).wrapping_sub(1);
            self.set16(P::Station, st, o, v as u16);
        }
    }

    /// `train_left_station` (`0x4d5560`): the first tile after a station resets the leg
    /// mileage (`+0x7d`, `+0x7f`, `+0x81`) and counts the current tile again.
    pub fn train_left_station(&mut self, t: i32) {
        self.set16(P::Train, t, 0x7d, 0);
        self.set16(P::Train, t, 0x7f, 0);
        self.set8(P::Train, t, 0x81, 0);
        self.set16(P::Train, t, 0xcb, 0xffff);
        let head = self.i32(P::Train, t, 0);
        if self.u8(P::Car, head, 0x2a) != 0 {
            let x = i32::from(self.i16(P::Car, head, 9));
            let y = i32::from(self.i16(P::Car, head, 0xb));
            self.enter_tile(t, x, y);
        }
    }
}
