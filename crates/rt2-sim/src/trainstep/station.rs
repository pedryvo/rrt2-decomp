//! Station work of a stopped train (trains.md 10.5): loading at the current stop (`0x4d18b0`,
//! `0x414580`).

use super::mem::{P, Ts};
use crate::fp::ftol;

impl Ts<'_> {
    /// `train_station_work` (`0x4d18b0`): at the stop's station, each cargo car tries to load
    /// (`0x414580`); every loaded car adds 2880 minutes (x0.5 or x0.75 with the station's
    /// loading services, divided by `effect_multiplier(0x29)`) to the departure minute. Then
    /// the train weights are recomputed.
    pub fn station_work(&mut self, t: i32) {
        let k = self.cur_stop(t);
        if self.stop8(t, k, 3) & 8 != 0 {
            return;
        }
        let head = self.i32(P::Train, t, 0);
        let here = if self.u8(P::Train, t, 0x9c) != 0 && self.u8(P::Car, head, 0x2a) != 0 {
            let s = self.station_at(
                i32::from(self.i16(P::Car, head, 9)),
                i32::from(self.i16(P::Car, head, 0xb)),
            );
            if s != 0 {
                self.u32(P::Station, s, 0)
            } else {
                0
            }
        } else {
            0
        };
        let stop_station = self.stop16(t, k, 0);
        if here != u32::from(stop_station) {
            return;
        }
        let mut loaded = 0;
        let mut i = 0;
        loop {
            let cargo_cars = self.cars_with(t, |ts, c| ts.vt8(ts.u8(P::Car, c, 8), 0x30) != 0x7f);
            if cargo_cars.len() <= i {
                break;
            }
            let car = cargo_cars[i];
            if self.car_load(car, i32::from(stop_station)) {
                loaded += 1;
            }
            i += 1;
        }
        if loaded != 0 {
            if self.i32(P::Train, t, 0x71) == 0 {
                let v = self.i32(P::Train, t, 0x6d);
                self.seti32(P::Train, t, 0x71, v);
            }
            let mut d = loaded * 2880;
            if self.stop8(t, k, 3) & 8 == 0 {
                let services = self.u32(P::Station, i32::from(stop_station), 0x49);
                if services & 8 != 0 {
                    d = ftol(f64::from(d) * self.img.f64(0x0053_5128));
                } else if services & 0x10 != 0 {
                    d = ftol(f64::from(d) * self.img.f64(0x0053_5228));
                }
            }
            let company = self.i32(P::Train, t, 0x41);
            let terr = i32::from(self.u8(P::Train, t, 0xf3));
            let mut m = f64::from(self.effect(0x29, company, 0, terr));
            if m < self.img.f64(0x0053_5178) {
                m = f64::from(self.img.f32(0x0053_77dc));
            }
            let v = self
                .i32(P::Train, t, 0x71)
                .wrapping_add(ftol(f64::from(d) / m));
            self.seti32(P::Train, t, 0x71, v);
            self.set8(P::Train, t, 0x9a, 3);
        }
        self.seti32(P::Train, t, 0x2f, 0);
        self.seti32(P::Train, t, 0xaf, 0);
        self.seti32(P::Train, t, 0xb3, 0);
        let mut c = head;
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
        if self.flag_570229 == 0 {
            let f = self.u8(P::Train, t, 0xbb);
            self.set8(P::Train, t, 0xbb, f & 0xdf);
            return;
        }
        todo_hook!(
            self.log,
            "station_work_570229",
            "0x4d18b0 cargo 0x11 flag and 0x4d14b0 (flag 0x570229)",
            "trains.md 10.5"
        );
    }

    /// The cars of a train (head first) that satisfy `f`.
    pub fn cars_with(&self, t: i32, f: impl Fn(&Ts<'_>, i32) -> bool) -> Vec<i32> {
        let mut out = Vec::new();
        let mut c = self.i32(P::Train, t, 0);
        while c != 0 {
            if f(self, c) {
                out.push(c);
            }
            c = i32::from(self.i16(P::Car, c, 2));
        }
        out
    }

    /// `car_load` (`0x414580`): an empty cargo car takes a lot of its cargo from the station.
    /// Generic freight cars (`cargo_table[c].+0 != 0`, `0x5404b0`) first pick the cargo with a
    /// game RNG draw.
    pub fn car_load(&mut self, car: i32, station: i32) -> bool {
        let vt = self.u8(P::Car, car, 8);
        let cargo = self.vt8(vt, 0x30);
        if self.flag_570236 != 0
            || self.i32(P::Car, car, 0x2d) != 0
            || cargo == 0x2c
            || cargo == 0x2d
        {
            return false;
        }
        let cargo = i32::from(cargo as i8);
        if self.img.u8((0x0054_04b0 + cargo * 0x21) as u32) != 0 {
            todo_hook!(
                self.log,
                "car_load_generic",
                "0x414580 generic car: rng_range(0,5) cargo pick and car type swap",
                "trains.md 10.5"
            );
            return false;
        }
        if self.i16(P::Station, station, (0x61 + cargo * 2) as usize) != 0 {
            let id = self.u16(P::Car, car, 6);
            let lot = self.station_load_lot(station, cargo, id);
            self.seti32(P::Car, car, 0x2d, lot);
            return true;
        }
        false
    }
}
