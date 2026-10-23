//! Train movement along the route ring buffer (trains.md 10.3-10.5): `train_step` (`0x4d0ce0`),
//! the speed update with the station dwell (`0x4d0400`), the head look-ahead (`0x415040`), the
//! car advance (`0x4147e0`), the route steps (`0x414dd0`, `0x412ff0`) and the track reservations
//! (`0x4d3c80`, `0x4d3950`, `0x416050`).
//!
//! Car record (`0x574c70`, 0x4f bytes) fields used here: `+0` i16 car ahead, `+2` i16 car behind,
//! `+4` i16 train, `+6` u16 vehicle id, `+8` vehicle type, `+9`/`+0xb` i16 tile, `+0xd`/`+0xf`
//! i16 previous tile, `+0x11` i16 route index, `+0x13` segment length (16.16), `+0x17` position
//! in the segment (16.16), `+0x1b`/`+0x1f`/`+0x26` sub-tile x/y and facing (`0x4ca550`),
//! `+0x23`/`+0x24` entry/exit direction, `+0x25` curve kind, `+0x2a` on the track, `+0x2d` cargo
//! lot (0 = empty), `+0x31` stopped at the station of the current half tile, `+0x32`/`+0x34`
//! wheel frame and distance, `+0x38`/`+0x39` rod frame and distance, `+0x3d` minute of the last
//! puff, `+0x41` distance to the next puff, `+0x45` look-ahead nesting (no tile chain updates),
//! `+0x47` waiting for a reservation, `+0x48`/`+0x4a` i16 reserved tile.

use super::anim::STRUCT_CAR;
use super::mem::{P, Ts, ring};
use crate::fp::ftol;
use crate::trains::{SpeedMods, engine_max_speed};

/// `(vehicle +0x06 & ~1) << 15`: half the coupling length of vehicle type `t`, in 16.16 units.
fn half_len(ts: &Ts<'_>, t: u8) -> i32 {
    (i32::from(ts.vt16(t, 6)) & !1) << 15
}

impl Ts<'_> {
    fn car_train(&self, car: i32) -> i32 {
        i32::from(self.i16(P::Car, car, 4))
    }
    fn first_car(&self, train: i32) -> i32 {
        self.i32(P::Train, train, 0)
    }
    fn car_next(&self, car: i32) -> i32 {
        i32::from(self.i16(P::Car, car, 2))
    }
    fn car_prev(&self, car: i32) -> i32 {
        i32::from(self.i16(P::Car, car, 0))
    }
    fn train_id(&self, train: i32) -> u32 {
        self.u32(P::Train, train, 0x3d)
    }

    /// Track piece record of tile `(x, y)` (`track_piece_ptr(tile+7)`, `0x466800`).
    fn piece_at(&self, x: i32, y: i32) -> i32 {
        self.tile_track(x, y)
    }

    /// Curve kind of a route step on tile `(x, y)` entered from `din`: the double-track side
    /// `0x536c4c[din]` when the piece is double track, else 0.
    fn side_for(&self, x: i32, y: i32, din: u8) -> u8 {
        let p = self.piece_at(x, y);
        if p != 0 && self.u8(P::Track, p, 0x23) & 1 != 0 {
            self.img.i32(0x0053_6c4c + 4 * u32::from(din)) as u8
        } else {
            0
        }
    }

    /// Station record under the stop `k` of a train, `None` for a waypoint (`+3 & 8`).
    fn stop_station(&self, train: i32, k: i32) -> Option<i32> {
        if self.stop8(train, k, 3) & 8 != 0 {
            None
        } else {
            Some(i32::from(self.stop16(train, k, 0)))
        }
    }

    /// The car stands on the tile of the station of the current stop.
    fn at_stop_station(&self, train: i32, car: i32) -> bool {
        let k = self.cur_stop(train);
        match self.stop_station(train, k) {
            Some(st) => {
                self.i32(P::Station, st, 0xa) == i32::from(self.i16(P::Car, car, 9))
                    && self.i32(P::Station, st, 0xe) == i32::from(self.i16(P::Car, car, 0xb))
            }
            None => false,
        }
    }

    // ---- train_step (0x4d0ce0) ------------------------------------------------------------

    /// `train_step(train, dt, flag)` (`0x4d0ce0`, trains.md 10.3).
    pub fn train_step(&mut self, t: i32, dt: i32, update_speed: bool) {
        if self.u8(P::Train, t, 0xf2) != 0 && self.u32(P::Train, t, 0xee) <= self.now {
            todo_hook!(
                self.log,
                "train_delete",
                "train_delete 0x4cdd50 (crashed train expires)",
                "trains.md 5.3"
            );
            return;
        }
        let mut on = self.u8(P::Train, t, 0x9c) != 0;
        if !on && self.u8(P::Train, t, 0x3b) != 0 {
            // A train that is not on the track: its consist is matched with the stop
            // (`0x4d1bb0`), the locomotive is put on the track at the stop's station (`0x413550`)
            // and the electric-without-wire bit (`+0xbb & 8`) is recomputed (trains.md 12).
            self.train_consist_at_stop(t);
            self.place_head_car(t);
            self.update_wire_bit(t);
            on = self.u8(P::Train, t, 0x9c) != 0;
        }
        if on {
            if update_speed {
                self.update_speed(t, self.now);
            }
            let mut do_move = false;
            if self.u8(P::Train, t, 0x99) == 0 {
                if self.u8(P::Train, t, 0xa7) != 0 {
                    do_move = true;
                } else {
                    self.reserve(t, 1, 0, 5);
                }
            } else if self.i32(P::Train, t, 0x71) == 0 && self.depart_check(t) {
                do_move = true;
            }
            if do_move {
                self.move_train(t, dt);
            }
            let mut c = self.first_car(t);
            while c != 0 {
                self.car_smoke_step(c);
                c = self.car_next(c);
            }
        }
        self.seti32(P::Train, t, 0xc3, 0);
        if self.g.refresh_574b84 != 0 {
            todo_hook!(
                self.log,
                "train_consist_refresh",
                "0x4d14b0 for every train (consist changed)",
                "trains.md 10.3"
            );
            self.g.refresh_574b84 = 0;
        }
    }

    /// The moving part of `train_step` (`0x4d0e27..0x4d0f12`).
    fn move_train(&mut self, t: i32, dt: i32) {
        let vel = self.i32(P::Train, t, 0x69);
        let tmp = (f64::from(vel) * self.img.f64(0x0053_7800) * f64::from(dt)) as f32;
        self.seti32(P::Train, t, 0xbf, ftol(f64::from(tmp) / f64::from(dt)));
        let head = self.first_car(t);
        let real = self.vehicle_advance_head(head, ftol(f64::from(tmp)));
        let mut d = real as f32;
        self.set32(P::Train, t, 0xc3, self.now.wrapping_sub(dt as u32));
        let mut c = head;
        while c != 0 {
            let r = self.vehicle_advance(c, ftol(f64::from(d)));
            if r as f32 != d && c == self.first_car(t) {
                d = r as f32;
            }
            c = self.car_next(c);
        }
        self.trim_reservations(t);
        if self.i32(P::Train, t, 0x69) > 0 && self.u8(P::Train, t, 0x99) == 0 {
            self.breakdown_check(t);
        }
    }

    // ---- train_update_speed (0x4d0400) ----------------------------------------------------

    /// `train_update_speed(train, now)` (`0x4d0400`, trains.md 4.4 and 10.4).
    pub fn update_speed(&mut self, t: i32, now: u32) {
        if (self.u8(P::Train, t, 0xed) != 0 || self.u8(P::Train, t, 0xbc) != 0)
            && self.u32(P::Train, t, 0xee) <= self.now
        {
            self.set8(P::Train, t, 0xed, 0);
            self.set8(P::Train, t, 0xbc, 0);
            let a7 = self.u8(P::Train, t, 0xa7);
            self.train_set_throttle_state(t, a7);
        }
        let dep = self.u32(P::Train, t, 0x71);
        if dep != 0 && self.now < dep {
            self.set32(P::Train, t, 0x6d, now);
            return;
        }
        let head = self.first_car(t);
        let eng = self.vt_engine(self.u8(P::Car, head, 8));
        let (target, restricted) = self.target_speed(t);
        let restricted = restricted.unwrap_or(self.g.stale_restricted);
        self.g.stale_restricted = restricted;
        let mut tau = self.en32(eng, 0x1c).wrapping_mul(1440);
        if self.i32(P::Train, t, 0x69) > target {
            tau >>= if restricted { 4 } else { 1 };
        }
        if self.u8(P::Train, t, 0xed) != 0 || self.u8(P::Train, t, 0xf2) != 0 {
            tau >>= 1;
        }
        tau -= tau >> 2;
        let company = self.i32(P::Train, t, 0x41);
        let terr = i32::from(self.u8(P::Train, t, 0xf3));
        let kind = usize::from(self.en8(eng, 0x28));
        let m_kind = self.effect(0x2f + kind, company, 0, terr);
        let m_all = self.effect(0x2e, company, 0, terr);
        let mut m = f64::from(m_all) * f64::from(m_kind);
        if m < self.img.f64(0x0053_5178) {
            m = f64::from(self.img.f32(0x0053_77dc));
        }
        let tau2 = ftol(f64::from(tau) / m);
        let elapsed;
        if self.u8(P::Train, t, 0x99) == 0 {
            let last = self.u32(P::Train, t, 0x6d);
            elapsed = if last != 0 {
                now.wrapping_sub(last) as i32
            } else {
                0
            };
        } else {
            self.seti32(P::Train, t, 0x69, 0);
            match self.stopped_branch(t, now) {
                Some(e) => elapsed = e,
                None => {
                    self.set32(P::Train, t, 0x6d, now);
                    return;
                }
            }
        }
        let e = elapsed.clamp(i32::MIN, tau2).max(0);
        let div = if target < 0 { tau2 / 2 } else { tau2 };
        let vel = self.i32(P::Train, t, 0x69);
        let delta = ftol(f64::from(e) / f64::from(div) * f64::from(target.wrapping_sub(vel)));
        self.seti32(P::Train, t, 0x69, delta.wrapping_add(vel));
        self.set32(P::Train, t, 0x6d, now);
    }

    /// The stopped-train loop of `0x4d0400` (`0x4d054b..0x4d07ff`): station work while the
    /// departure minute has passed. `Some(elapsed)` when the train departs (the speed step then
    /// runs with `elapsed = now - departure`), `None` otherwise (`+0x6d = now`).
    fn stopped_branch(&mut self, t: i32, now: u32) -> Option<i32> {
        loop {
            let dep = self.i32(P::Train, t, 0x71);
            if dep == 0 || dep >= self.i32(P::Train, t, 0x6d) {
                return None;
            }
            let st = self.u8(P::Train, t, 0x9a);
            if st == 3 || st == 6 || st == 4 {
                self.station_work(t);
            }
            let dep = self.i32(P::Train, t, 0x71);
            if dep == 0 || dep >= self.i32(P::Train, t, 0x6d) || self.cur_stop(t) < 0 {
                return None;
            }
            let st = self.u8(P::Train, t, 0x9a);
            if st == 9 {
                self.seti32(P::Train, t, 0x71, 0);
                self.settlement(t);
            } else if st == 2 {
                self.train_consist_at_stop(t);
            } else {
                if !self.wait_for_load(t) && self.wait_load_pending(t) {
                    self.set8(P::Train, t, 0x9a, 4);
                    return None;
                }
                let dep = self.u32(P::Train, t, 0x71);
                if !self.depart_check(t) {
                    return None;
                }
                let elapsed = now.wrapping_sub(dep) as i32;
                self.seti32(P::Train, t, 0x71, 0);
                if self.cur_stop(t) == 1 {
                    self.set32(P::Train, t, 0xe5, self.now);
                }
                // 0x49b610: distance to the camera, only for the whistle volume (no state).
                self.crt.range(0, 3);
                self.set32(P::Train, t, 0xc7, self.now);
                self.set8(P::Train, t, 0xa6, 1);
                return Some(elapsed);
            }
            if self.i32(P::Train, t, 0x71) == 0 {
                return None;
            }
        }
    }

    /// The "skip the wait" test of the stopped loop: `true` when the stop does not ask the
    /// train to wait for a load (waypoint, another station, or no wait mode).
    fn wait_for_load(&self, t: i32) -> bool {
        let k = self.cur_stop(t);
        if self.stop8(t, k, 3) & 8 != 0 {
            return true;
        }
        let head = self.first_car(t);
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
        if here != u32::from(self.stop16(t, k, 0)) {
            return true;
        }
        let mode = self.stop8(t, k, 3) & 0xc0;
        mode != 0x40 && mode != 0x80
    }

    /// Wait modes `0x40` (half the cargo cars loaded) and `0x80` (all loaded): `true` while
    /// fewer cars than required carry a load.
    fn wait_load_pending(&self, t: i32) -> bool {
        let k = self.cur_stop(t);
        let mode = self.stop8(t, k, 3) & 0xc0;
        let mut n = 0;
        let mut c = self.first_car(t);
        while c != 0 {
            let cargo = self.vt8(self.u8(P::Car, c, 8), 0x30);
            if cargo != 0x7f && cargo != 0x2c && cargo != 0x2d {
                n += 1;
            }
            c = self.car_next(c);
        }
        if mode == 0x40 {
            n = (n + 1) >> 1;
        }
        if n == 0 {
            return false;
        }
        let mut loaded = 0;
        let mut c = self.first_car(t);
        while c != 0 {
            if self.i32(P::Car, c, 0x2d) != 0 {
                loaded += 1;
            }
            c = self.car_next(c);
        }
        loaded < n
    }

    /// `train_target_speed` (`0x4d00c0`, trains.md 4.3): `(target, restricted)`; `restricted` is
    /// `None` when the original leaves its output untouched (stopped by a breakdown or crash).
    pub fn target_speed(&self, t: i32) -> (i32, Option<bool>) {
        if self.u8(P::Train, t, 0xf2) != 0
            || self.u8(P::Train, t, 0xed) != 0
            || self.u8(P::Train, t, 0xbc) != 0
        {
            return (0, None);
        }
        let head = self.first_car(t);
        let eng_idx = self.vt_engine(self.u8(P::Car, head, 8));
        let prio = |c: u32| self.img.i32(0x0055_c558 + 4 * c);
        let mut curve = false;
        let mut class = 0u32;
        let mut c = head;
        while c != 0 {
            if self.u8(P::Car, c, 0x2a) != 0 {
                let (din, dout) = (self.u8(P::Car, c, 0x23), self.u8(P::Car, c, 0x24));
                if din.wrapping_sub(dout) & 3 != 0 {
                    curve = true;
                    break;
                }
                let (x, y) = (
                    i32::from(self.i16(P::Car, c, 9)),
                    i32::from(self.i16(P::Car, c, 0xb)),
                );
                if self.t8(x, y, 2) & 0x80 != 0 {
                    let b = u32::from(self.u8(P::Track, self.piece_at(x, y), 0x11));
                    if prio(class) < prio(b) {
                        class = b;
                    }
                }
                let p = self.piece_at(x, y);
                if self.t8(x, y, 3) & 0x40 != 0
                    && p != 0
                    && self.u8(P::Track, p, 5) & 8 != 0
                    && prio(class) < self.img.i32(0x0055_c564)
                {
                    class = 3;
                }
            }
            c = self.car_next(c);
        }
        let mut restricted = curve;
        let g = self.train_grade(t);
        let mut g = g;
        if f64::from(self.f32(P::Train, t, 0xcf)) <= self.img.f64(0x0053_50c8) {
            g += g;
        }
        let company = self.i32(P::Train, t, 0x41);
        let terr = i32::from(self.u8(P::Train, t, 0xf3));
        let kind = usize::from(self.en8(eng_idx, 0x28));
        let mods = SpeedMods {
            power_all: f64::from(self.effect(0x2a, company, 0, terr)),
            power_kind: f64::from(self.effect(0x2b + kind, company, 0, terr)),
            speed_all: f64::from(self.effect(0x23, company, 0, terr)),
            speed_kind: f64::from(self.effect(0x24 + kind, company, 0, terr)),
        };
        let eng = self.engine_struct(eng_idx);
        let age = i32::from(self.year) - i32::from(self.u16(P::Train, t, 0xbd));
        let s = engine_max_speed(&eng, self.i32(P::Train, t, 0xaf), g, curve, age, &mods);
        let mut s = ftol(
            f64::from(self.f32(P::Train, t, 0xdb))
                * f64::from(self.img.f32(0x0053_77d4))
                * f64::from(s),
        );
        if class == 3 {
            s = ftol(f64::from(s) * self.img.f64(0x0053_50f8));
            restricted = true;
        } else if class == 1 {
            s = ftol(f64::from(s) * self.img.f64(0x0053_5108));
            restricted = true;
        }
        if self.en8(eng_idx, 0x28) == 0
            && f64::from(self.f32(P::Train, t, 0xd7)) <= self.img.f64(0x0053_50c8)
        {
            s = ftol(f64::from(s) * self.img.f64(0x0053_5128));
        }
        // 0x574c08 (runtime flag, 0 in a loaded game) would double the target.
        (s, Some(restricted))
    }

    pub(crate) fn engine_struct_pub(&self, e: i32) -> crate::trains::Engine {
        self.engine_struct(e)
    }

    fn engine_struct(&self, e: i32) -> crate::trains::Engine {
        let base = 0x0053_cec0u32.wrapping_add((e * 0x3a) as u32);
        let raw: Vec<u8> = (0..0x3a).map(|i| self.img.u8(base + i)).collect();
        let mut eng = crate::trains::Engine::from_raw(&raw);
        eng.base_tons = self.img.i32(base + 0x2e);
        eng
    }

    /// `vehicle_weight` (`0x414460`): loaded or empty tons of a car.
    pub fn vehicle_weight(&self, car: i32) -> i32 {
        let t = self.u8(P::Car, car, 8);
        if self.i32(P::Car, car, 0x2d) == 0 && self.flag_570236 == 0 {
            i32::from(self.vt16(t, 0x18))
        } else {
            i32::from(self.vt16(t, 0x1a))
        }
    }

    /// `vehicle_grade` (`0x4144a0`): grade of the car's segment (`0x4284e0`).
    pub fn vehicle_grade(&self, car: i32) -> f64 {
        let x = i32::from(self.i16(P::Car, car, 9));
        let y = i32::from(self.i16(P::Car, car, 0xb));
        let w = self.map_w() + 1;
        let elev = &self.s.map.elevation;
        let base = y * w + x;
        let height = |off: i32| i32::from(elev[(base + off) as usize]);
        crate::trains::track_segment_grade(
            &height,
            [0, 1, w + 1, w],
            i32::from(self.u8(P::Car, car, 0x23)),
            i32::from(self.u8(P::Car, car, 0x24)),
        )
    }

    /// `train_grade` (`0x4cfd40`) as computed inline by `0x4d00c0`: weighted grade of the first
    /// two cars, `f32`.
    pub fn train_grade(&self, t: i32) -> f32 {
        if self.u8(P::Train, t, 0x9c) == 0 {
            return 0.0;
        }
        let (mut sum, mut acc) = (0i32, 0f32);
        let mut c = self.first_car(t);
        let mut n = 0;
        while c != 0 {
            if self.u8(P::Car, c, 0x2a) != 0 {
                let w = self.vehicle_weight(c);
                let g = self.vehicle_grade(c);
                sum += w;
                acc = (g * f64::from(w) + f64::from(acc)) as f32;
            }
            c = self.car_next(c);
            n += 1;
            if n == 2 {
                break;
            }
        }
        if sum == 0 {
            0.0
        } else {
            (f64::from(acc) / f64::from(sum)) as f32
        }
    }

    // ---- departure and reservations ------------------------------------------------------

    /// `train_depart_check` (`0x4d0a70`): `true` when the stopped train may start; reserves the
    /// route ahead and moves the head one unit (`0x415040(head, 0x10000)`).
    pub fn depart_check(&mut self, t: i32) -> bool {
        if self.u8(P::Train, t, 0x9a) == 7 && self.u8(P::Train, t, 0x92) != 0 {
            return false;
        }
        if self.u8(P::Train, t, 0xa7) == 0 || self.u8(P::Train, t, 0x3b) == 0 {
            return false;
        }
        if self.route_blocked(t) {
            self.set8(P::Train, t, 0x9a, 1);
            return false;
        }
        if self.u8(P::Train, t, 0x9b) != 0 {
            let r95 = i32::from(self.i16(P::Train, t, 0x95));
            if r95 != self.i32(P::Train, t, 0x5d) {
                let i = ring(r95 + 1);
                let (x, y) = (self.rx(t, i), self.ry(t, i));
                if self.tile_blocked(x, y, self.train_id(t), self.rdin(t, i), self.rdout(t, i)) {
                    self.set8(P::Train, t, 0x9a, 1);
                    return false;
                }
            }
        }
        self.reserve(t, 0, 0, 0);
        if self.u8(P::Train, t, 0x9a) != 0 {
            return false;
        }
        let head = self.first_car(t);
        self.vehicle_advance_head(head, 0x10000);
        true
    }

    /// `route_blocked` (`0x4d3e30`): some step of the reserved range `+0x97..=+0x95` is taken by
    /// another train (`0x416050`); also `true` when nothing is reserved.
    fn route_blocked(&mut self, t: i32) -> bool {
        if self.u8(P::Train, t, 0x9c) == 0 {
            return false;
        }
        let (r95, r97) = (
            i32::from(self.i16(P::Train, t, 0x95)),
            i32::from(self.i16(P::Train, t, 0x97)),
        );
        if r95 == -1 && r97 == -1 {
            return true;
        }
        let mut i = r97;
        loop {
            let (x, y) = (self.rx(t, i), self.ry(t, i));
            let kind = (self.rflags(t, i) >> 1) & 3;
            if self.piece_conflict(
                x,
                y,
                u32::from(kind),
                self.train_id(t),
                self.rdin(t, i),
                self.rdout(t, i),
            ) {
                return true;
            }
            if i == r95 {
                return false;
            }
            i = ring(i + 1);
        }
    }

    /// `tile_blocked` (`0x415ff0`): `0x416050` with the double-track side of the piece.
    fn tile_blocked(&mut self, x: i32, y: i32, train_id: u32, din: u8, dout: u8) -> bool {
        let side = self.side_for(x, y, din);
        self.piece_conflict(x, y, u32::from(side), train_id, din, dout)
    }

    /// `piece_conflict` (`0x416050`): another train holds the piece of tile `(x, y)` in a way
    /// that conflicts with the move `din -> dout` (on double track only when the paths cross).
    /// A train with a lower priority (`+0xa8`) that holds it is stopped (`0x4d3c80(.., 1, 1,
    /// 1)`) instead.
    fn piece_conflict(
        &mut self,
        x: i32,
        y: i32,
        _side: u32,
        train_id: u32,
        din: u8,
        dout: u8,
    ) -> bool {
        let p = self.piece_at(x, y);
        let me = self.train_by_id(train_id);
        let prio = i32::from(self.u8(P::Train, me, 0xa8)) - 1;
        let mask = self.u8(P::Track, p, 0);
        let double = self.u8(P::Track, p, 0x23) & 1 != 0;
        for k in 0..2usize {
            if k == 1 && !double {
                continue;
            }
            let holder = u32::from(self.u16(P::Track, p, 7 + 2 * k));
            if holder == 0 || holder == train_id {
                continue;
            }
            if double {
                let hd = self.u8(P::Track, p, 0xd + k);
                let rel = u32::from(hd).wrapping_sub(u32::from(dout)) & 7;
                if rel > 1 && rel < 7 {
                    if (((1u32 << din) | (1u32 << dout)) & u32::from(mask)) == u32::from(mask) {
                        continue;
                    }
                    let a = i32::from(din) * 2;
                    let b = i32::from(dout) * 2 + 1;
                    let hi = i32::from(self.u8(P::Track, p, 0xd + k)) * 2 + 1;
                    let lo = i32::from(self.u8(P::Track, p, 0xb + k)) * 2;
                    let (ia, ib) = if lo < hi {
                        (lo < a && a < hi, lo < b && b < hi)
                    } else {
                        (hi < a && a < lo, !(b <= hi || lo <= b))
                    };
                    if ia != ib {
                        if prio < 1 {
                            return true;
                        }
                        let other = self.train_by_id(holder);
                        if prio < i32::from(self.u8(P::Train, other, 0xa8)) {
                            return true;
                        }
                        self.reserve(other, 1, 1, 1);
                    }
                    continue;
                }
            }
            if prio < 1 {
                return true;
            }
            let other = self.train_by_id(holder);
            if prio < i32::from(self.u8(P::Train, other, 0xa8)) {
                return true;
            }
            self.reserve(other, 1, 1, 1);
        }
        false
    }

    /// `train_ptr(id)` (`0x466850`): the train record of a train id (the slot).
    fn train_by_id(&self, id: u32) -> i32 {
        id as i32
    }

    /// `train_set_stop_state` (`0x4d3c80`, `this, stopped, flag9b, state9a`): sets the stop
    /// state; when `+0x99` changes, writes (start) or clears (stop) the train's hold on the
    /// pieces of the reserved range `+0x97..=+0x95`.
    pub fn reserve(&mut self, t: i32, stopped: u8, f9b: u8, state: i32) {
        let mut stopped = stopped;
        let mut state = state;
        let fb = self.i8(P::Train, t, 0xfb);
        if state == 0 && fb != 0 && fb != 1 {
            state = i32::from(fb);
            stopped = 1;
            let v = self.i32(P::Train, t, 0x6d).wrapping_add(1000);
            self.seti32(P::Train, t, 0x71, v);
        }
        self.set8(P::Train, t, 0x9a, state as u8);
        self.set8(P::Train, t, 0xfb, 0);
        self.set8(P::Train, t, 0x9b, f9b);
        if stopped == self.u8(P::Train, t, 0x99) {
            return;
        }
        self.set8(P::Train, t, 0x99, stopped);
        let (r95, r97) = (
            i32::from(self.i16(P::Train, t, 0x95)),
            i32::from(self.i16(P::Train, t, 0x97)),
        );
        if r95 == -1 && r97 == -1 {
            return;
        }
        let id = self.train_id(t);
        let mut i = r97;
        loop {
            let p = self.piece_at(self.rx(t, i), self.ry(t, i));
            if stopped == 0 {
                let dout = self.rdout(t, i);
                let din = self.rdin(t, i);
                let h7 = u32::from(self.u16(P::Track, p, 7));
                if (h7 == id && self.u8(P::Track, p, 0xd) == dout) || h7 == 0 {
                    self.set16(P::Track, p, 7, id as u16);
                    self.set8(P::Track, p, 0xb, din);
                    self.set8(P::Track, p, 0xd, dout);
                } else if self.u8(P::Track, p, 0x23) & 1 != 0 {
                    let h9 = u32::from(self.u16(P::Track, p, 9));
                    if (h9 == id && self.u8(P::Track, p, 0xe) == dout) || h9 == 0 {
                        self.set16(P::Track, p, 9, id as u16);
                        self.set8(P::Track, p, 0xc, din);
                        self.set8(P::Track, p, 0xe, dout);
                    }
                }
            } else {
                if u32::from(self.u16(P::Track, p, 7)) == id {
                    self.set16(P::Track, p, 7, 0);
                }
                if u32::from(self.u16(P::Track, p, 9)) == id {
                    self.set16(P::Track, p, 9, 0);
                }
            }
            if i == r95 {
                break;
            }
            i = ring(i + 1);
        }
    }

    // ---- reservation range (0x4d0bc0, 0x4d3950, 0x4d36a0, 0x4d3770) -------------------------

    pub(crate) fn trim_reservations_pub(&mut self, t: i32) {
        self.trim_reservations(t);
    }

    /// `train_trim_reservations` (`0x4d0bc0`): moves the reserved range to `[behind the last
    /// car, the head's step (+1 when the head is past half its length)]`.
    fn trim_reservations(&mut self, t: i32) {
        if self.u8(P::Train, t, 0x9c) == 0 {
            return;
        }
        let head = self.first_car(t);
        let mut front = i32::from(self.i16(P::Car, head, 0x11));
        let ht = self.u8(P::Car, head, 8);
        if self.i32(P::Car, head, 0x13) <= half_len(self, ht) + self.i32(P::Car, head, 0x17)
            && front != self.i32(P::Train, t, 0x5d)
        {
            front = ring(front + 1);
        }
        let mut c = self.i32(P::Train, t, 4);
        loop {
            if self.u8(P::Car, c, 0x2a) != 0
                && !(self.u8(P::Car, c, 0x47) != 0 && self.car_prev(c) != 0)
            {
                break;
            }
            c = self.car_prev(c);
        }
        let mut back = i32::from(self.i16(P::Car, c, 0x11));
        let ct = self.u8(P::Car, c, 8);
        let mut pos = self.i32(P::Car, c, 0x17) - half_len(self, ct);
        while pos < 0 {
            let tail = ring(self.i32(P::Train, t, 0x5d) - self.i32(P::Train, t, 0x61) + 0x21);
            if back == tail {
                break;
            }
            back = ring(back + 0x1f);
            let l = self.seg_len(
                u32::from(self.rdin(t, back)),
                u32::from(self.rdout(t, back)),
                u32::from((self.rflags(t, back) >> 1) & 3),
            );
            pos += l << 16;
        }
        self.set_reserved_range(t, front, back);
    }

    /// `train_set_reserved_range` (`0x4d3950`, `this, front, back`).
    pub(crate) fn set_reserved_range(&mut self, t: i32, front: i32, back: i32) {
        if self.u8(P::Train, t, 0x9c) == 0 {
            return;
        }
        let head = self.i32(P::Train, t, 0x5d);
        let count = self.i32(P::Train, t, 0x61);
        if front == -1 {
            if back == -1 {
                let mut i = i32::from(self.i16(P::Train, t, 0x97));
                self.unreserve_step(t, i);
                while i != i32::from(self.i16(P::Train, t, 0x95)) {
                    i = ring(i + 1);
                    self.set16(P::Train, t, 0x97, i as u16);
                    self.unreserve_step(t, i);
                }
                self.set16(P::Train, t, 0x95, 0xffff);
                self.set16(P::Train, t, 0x97, 0xffff);
                return;
            }
        } else {
            if self.i16(P::Train, t, 0x95) == -1 {
                if self.i16(P::Train, t, 0x97) == -1 {
                    self.set16(P::Train, t, 0x95, back as u16);
                }
                let i = i32::from(self.i16(P::Train, t, 0x95));
                self.reserve_step(t, i);
            }
            let r95 = i32::from(self.i16(P::Train, t, 0x95));
            if !ring_ahead(r95, front, head, count) {
                let mut i = r95;
                while i != front {
                    i = ring(i + 1);
                    self.set16(P::Train, t, 0x95, i as u16);
                    self.reserve_step(t, i);
                }
            } else {
                let mut i = r95;
                while i != front {
                    self.unreserve_step(t, i);
                    i = ring(i + 0x1f);
                    self.set16(P::Train, t, 0x95, i as u16);
                }
            }
            if back == -1 {
                return;
            }
        }
        let mut back = back;
        if self.i16(P::Train, t, 0x97) == -1 {
            self.set16(P::Train, t, 0x97, back as u16);
        }
        let r97 = i32::from(self.i16(P::Train, t, 0x97));
        if !ring_ahead(back, r97, head, count) {
            let mut i = r97;
            while i != back {
                let n = ring(i + 0x1f);
                self.set16(P::Train, t, 0x97, n as u16);
                if !self.reserve_step(t, n) {
                    self.set16(P::Train, t, 0x97, i as u16);
                    back = i;
                }
                i = i32::from(self.i16(P::Train, t, 0x97));
            }
        } else {
            let mut i = r97;
            while i != back {
                self.unreserve_step(t, i);
                i = ring(i + 1);
                self.set16(P::Train, t, 0x97, i as u16);
            }
        }
    }

    /// `track_reserve_step` (`0x4d36a0`): pushes `(dout, train id)` on the piece's reservation
    /// list (`piece+3`, records of `0x574c80`) and, for a moving train, takes a hold slot.
    fn reserve_step(&mut self, t: i32, i: i32) -> bool {
        let p = self.piece_at(self.rx(t, i), self.ry(t, i));
        let id = self.u16(P::Train, t, 0x3d);
        let din = self.rdin(t, i);
        let dout = self.rdout(t, i);
        let head = self.u16(P::Track, p, 3);
        if self.u8(P::Train, t, 0x99) == 0 {
            let id32 = u32::from(id);
            let h9 = self.u16(P::Track, p, 9);
            let k = if self.u8(P::Track, p, 0x23) & 1 == 0
                || (h9 != 0 && (u32::from(h9) != id32 || self.u8(P::Track, p, 0xe) != dout))
                || (u32::from(self.u16(P::Track, p, 7)) == id32
                    && self.u8(P::Track, p, 0xd) == dout)
            {
                0
            } else {
                1
            };
            self.set16(P::Track, p, 7 + 2 * k, id);
            self.set8(P::Track, p, 0xb + k, din);
            self.set8(P::Track, p, 0xd + k, dout);
        }
        let mut rec = vec![dout];
        rec.extend_from_slice(&id.to_le_bytes());
        rec.extend_from_slice(&head.to_le_bytes());
        let n = self.alloc(P::P80, rec);
        self.set16(P::Track, p, 3, n as u16);
        true
    }

    /// `track_unreserve_step` (`0x4d3770`): removes the train's `(dout)` entry from the piece's
    /// reservation list; a moving train also drops its hold when no other entry of it is left.
    fn unreserve_step(&mut self, t: i32, i: i32) {
        let p = self.piece_at(self.rx(t, i), self.ry(t, i));
        let id = self.u32(P::Train, t, 0x3d);
        let dout = self.rdout(t, i);
        // `link` = where the pointer to `cur` lives: None = piece+3, Some(r) = record r +3.
        let mut link: Option<i32> = None;
        let mut cur = i32::from(self.u16(P::Track, p, 3));
        if cur == 0 {
            return;
        }
        loop {
            if u32::from(self.u16(P::P80, cur, 1)) == id && self.u8(P::P80, cur, 0) == dout {
                break;
            }
            let nx = i32::from(self.u16(P::P80, cur, 3));
            if nx == 0 {
                return;
            }
            link = Some(cur);
            cur = nx;
        }
        let nx = self.u16(P::P80, cur, 3);
        match link {
            None => self.set16(P::Track, p, 3, nx),
            Some(r) => self.set16(P::P80, r, 3, nx),
        }
        self.free(P::P80, cur);
        if self.u8(P::Train, t, 0x99) != 0 {
            return;
        }
        let double = self.u8(P::Track, p, 0x23) & 1 != 0;
        let (mut any, mut same) = (false, false);
        let mut r = i32::from(self.u16(P::Track, p, 3));
        while r != 0 {
            if u32::from(self.u16(P::P80, r, 1)) == id {
                any = true;
                if self.u8(P::P80, r, 0) == dout || !double {
                    same = true;
                }
            }
            r = i32::from(self.u16(P::P80, r, 3));
        }
        if !double {
            if !any {
                if u32::from(self.u16(P::Track, p, 7)) == id {
                    self.set16(P::Track, p, 7, 0);
                } else if u32::from(self.u16(P::Track, p, 9)) == id {
                    self.set16(P::Track, p, 9, 0);
                }
            }
        } else if !same {
            if u32::from(self.u16(P::Track, p, 7)) == id && self.u8(P::Track, p, 0xd) == dout {
                self.set16(P::Track, p, 7, 0);
            } else if u32::from(self.u16(P::Track, p, 9)) == id && self.u8(P::Track, p, 0xe) == dout
            {
                self.set16(P::Track, p, 9, 0);
            }
        }
    }

    // ---- head look-ahead (0x415040) --------------------------------------------------------

    /// `vehicle_advance_head` (`0x415040`): how far the head car can move by `dist` (16.16)
    /// without running into a blocked step, simulated on a copy of the car (the car record is
    /// restored; the route, reservations and train counters touched on the way stay).
    pub fn vehicle_advance_head(&mut self, car: i32, dist: i32) -> i32 {
        let t = self.car_train(car);
        let saved = self.record(P::Car, car);
        let v45 = self.u16(P::Car, car, 0x45).wrapping_add(1);
        self.set16(P::Car, car, 0x45, v45);
        let vt = self.u8(P::Car, car, 8);
        let hl = half_len(self, vt);
        let mut rem = dist + hl;
        let mut acc = 0i32;
        let mut second = false;
        let half_tile = |ts: &Ts<'_>| {
            let i = i32::from(ts.i16(P::Car, car, 0x11));
            ts.rflags(t, i) & 1 != 0
                && (ts.i16(P::Car, car, 0xd) != ts.i16(P::Car, car, 9)
                    || ts.i16(P::Car, car, 0xf) != ts.i16(P::Car, car, 0xb))
        };
        let seg = |ts: &Ts<'_>| {
            ts.seg_len(
                u32::from(ts.u8(P::Car, car, 0x23)),
                u32::from(ts.u8(P::Car, car, 0x24)),
                u32::from(ts.u8(P::Car, car, 0x25)),
            ) << 16
        };
        let mut l = seg(self);
        self.seti32(P::Car, car, 0x13, l);
        if half_tile(self) {
            self.seti32(P::Car, car, 0x13, l >> 1);
        }
        while self.i32(P::Car, car, 0x13) <= self.i32(P::Car, car, 0x17) + rem {
            let (s13, s17) = (self.i32(P::Car, car, 0x13), self.i32(P::Car, car, 0x17));
            if !half_tile(self) {
                if !self.route_next_step(car) {
                    self.reserve(t, 1, 1, 7);
                    rem = self.i32(P::Car, car, 0x13) - self.i32(P::Car, car, 0x17) - 1;
                    break;
                }
                acc += s13 - s17;
                rem += s17 - s13;
                self.seti32(P::Car, car, 0x17, 0);
                l = seg(self);
                self.seti32(P::Car, car, 0x13, l);
                let i = i32::from(self.i16(P::Car, car, 0x11));
                if self.rflags(t, i) & 1 != 0 {
                    self.seti32(P::Car, car, 0x13, l >> 1);
                }
            } else if second {
                second = false;
                rem += hl;
                let at_station = self.at_stop_station(t, car);
                if self.u8(P::Car, car, 0x31) == 0 && at_station {
                    acc += s13 - s17 + hl;
                    rem = 0;
                } else {
                    acc += s13 - s17;
                    self.set8(P::Car, car, 0x31, 0);
                    rem += s17 - s13;
                    let ok = self.route_step(car);
                    if ok {
                        self.after_step_reservation(car, t);
                    }
                    let s13 = self.i32(P::Car, car, 0x13);
                    self.seti32(P::Car, car, 0x17, s13);
                    if !ok {
                        rem = 0;
                        break;
                    }
                    let l = seg(self);
                    self.seti32(P::Car, car, 0x13, l);
                }
            } else {
                second = true;
                rem -= hl;
            }
        }
        acc += rem;
        let v = self.i32(P::Car, car, 0x17) + rem;
        self.seti32(P::Car, car, 0x17, v);
        if !second {
            acc -= hl;
        }
        if acc < 0 {
            acc = 0;
        }
        let r95 = i32::from(self.i16(P::Train, t, 0x95));
        if ring_ahead(
            r95,
            i32::from(self.i16(P::Car, car, 0x11)),
            self.i32(P::Train, t, 0x5d),
            self.i32(P::Train, t, 0x61),
        ) {
            self.set16(P::Car, car, 0x11, r95 as u16);
        }
        let v45 = self.u16(P::Car, car, 0x45).wrapping_sub(1);
        self.set16(P::Car, car, 0x45, v45);
        // The whole record is restored (the decrement above lands in the copy, as in the
        // original, whose copy was taken before the increment).
        self.set_record(P::Car, car, &saved);
        acc
    }

    /// The common tail of a successful route step of `0x415040`/`0x4147e0` (`0x4151c4..`):
    /// first-tile bookkeeping of the head and the `+0x47` reservation retry.
    fn after_step_reservation(&mut self, car: i32, t: i32) {
        if car == self.first_car(t)
            && self.i16(P::Train, t, 0x7d) == 1
            && self.u8(P::Train, t, 0xa6) != 0
        {
            self.set8(P::Train, t, 0xa6, 0);
            self.train_left_station(t);
        }
        let (x, y) = (self.i16(P::Car, car, 9), self.i16(P::Car, car, 0xb));
        self.set16(P::Car, car, 0xd, x as u16);
        self.set16(P::Car, car, 0x48, x as u16);
        self.set16(P::Car, car, 0xf, y as u16);
        self.set16(P::Car, car, 0x4a, y as u16);
        self.retry_wait_reservation(car, t);
    }

    /// `+0x47` handling shared by `0x412ff0`, `0x413550`, `0x415040`, `0x4147e0`.
    fn retry_wait_reservation(&mut self, car: i32, t: i32) {
        if self.u8(P::Car, car, 0x47) == 0 {
            return;
        }
        let prev = self.car_prev(car);
        let go = if prev == 0 || !self.exists(P::Car, prev) {
            true
        } else {
            self.u8(P::Car, prev, 0x47) == 0
                && (self.i16(P::Car, car, 0x48) == -1
                    || (self.i16(P::Car, car, 0x48) == self.i16(P::Car, prev, 0x48)
                        && self.i16(P::Car, car, 0x4a) == self.i16(P::Car, prev, 0x4a)))
        };
        if go {
            let (x, y) = (
                i32::from(self.i16(P::Car, car, 9)),
                i32::from(self.i16(P::Car, car, 0xb)),
            );
            let blocked = self.tile_blocked(
                x,
                y,
                self.train_id(t),
                self.u8(P::Car, car, 0x23),
                self.u8(P::Car, car, 0x24),
            );
            if !blocked && !self.rear_blocked(car) {
                self.set8(P::Car, car, 0x47, 0);
            }
        }
    }

    /// `0x412770`: the step behind the car (one car length back) is blocked.
    fn rear_blocked(&mut self, car: i32) -> bool {
        let t = self.car_train(car);
        if self.i32(P::Train, t, 0x65) == 0 {
            return false;
        }
        let mut i = i32::from(self.i16(P::Car, car, 0x11));
        let vt = self.u8(P::Car, car, 8);
        let mut pos = self.i32(P::Car, car, 0x17) - half_len(self, vt);
        while pos < 0 {
            let tail = ring(self.i32(P::Train, t, 0x5d) - self.i32(P::Train, t, 0x61) + 0x21);
            if i == tail {
                break;
            }
            i = ring(i + 0x1f);
            let l = self.seg_len(
                u32::from(self.rdin(t, i)),
                u32::from(self.rdout(t, i)),
                u32::from((self.rflags(t, i) >> 1) & 3),
            );
            pos += l << 16;
        }
        let (x, y) = (self.rx(t, i), self.ry(t, i));
        self.tile_blocked(x, y, self.train_id(t), self.rdin(t, i), self.rdout(t, i))
    }

    // ---- car advance (0x4147e0) ------------------------------------------------------------

    /// `vehicle_advance` (`0x4147e0`): moves a car `dist` (16.16) along the route; returns the
    /// distance moved. Emits the distance-based puffs of steam engines (every `+0x41`).
    pub fn vehicle_advance(&mut self, car: i32, dist: i32) -> i32 {
        let t = self.car_train(car);
        let mut moved = 0i32;
        if self.u8(P::Car, car, 0x2a) == 0 {
            self.place_trailing_car(car);
            return dist;
        }
        let vt = self.u8(P::Car, car, 8);
        let mut rem = dist;
        if self.vt8(vt, 0x2b) != 0 && self.i32(P::Train, t, 0xbf) != 0 {
            let mut e = self.i32(P::Car, car, 0x41);
            while rem > e {
                let mut r = self.vehicle_advance(car, e);
                if r == 0 {
                    r = rem;
                }
                rem -= r;
                moved += r;
                let c3 = self
                    .i32(P::Train, t, 0xc3)
                    .wrapping_add(r / self.i32(P::Train, t, 0xbf));
                self.seti32(P::Train, t, 0xc3, c3);
                e = self.i32(P::Car, car, 0x41);
            }
            if rem == self.i32(P::Car, car, 0x41) {
                let at = self.u32(P::Train, t, 0xc3);
                self.car_smoke_puff(car, at);
            } else {
                let v = self.i32(P::Car, car, 0x41) - rem;
                self.seti32(P::Car, car, 0x41, v);
            }
        }
        let seg = |ts: &Ts<'_>| {
            ts.seg_len(
                u32::from(ts.u8(P::Car, car, 0x23)),
                u32::from(ts.u8(P::Car, car, 0x24)),
                u32::from(ts.u8(P::Car, car, 0x25)),
            ) << 16
        };
        let half_tile = |ts: &Ts<'_>| {
            let i = i32::from(ts.i16(P::Car, car, 0x11));
            ts.rflags(t, i) & 1 != 0
                && (ts.i16(P::Car, car, 0xd) != ts.i16(P::Car, car, 9)
                    || ts.i16(P::Car, car, 0xf) != ts.i16(P::Car, car, 0xb))
        };
        let l = seg(self);
        self.seti32(P::Car, car, 0x13, l);
        if half_tile(self) {
            self.seti32(P::Car, car, 0x13, l >> 1);
        }
        while self.i32(P::Car, car, 0x13) <= rem + self.i32(P::Car, car, 0x17) {
            let (s13, s17) = (self.i32(P::Car, car, 0x13), self.i32(P::Car, car, 0x17));
            if !half_tile(self) {
                if !self.route_step(car) {
                    rem = self.i32(P::Car, car, 0x13) - self.i32(P::Car, car, 0x17) - 1;
                    break;
                }
                // net_min_peer_version() > 0x232c in a local game
                self.set8(P::Car, car, 0x31, 0);
                moved += s13 - s17;
                rem += s17 - s13;
                let l = seg(self);
                self.seti32(P::Car, car, 0x13, l);
                let i = i32::from(self.i16(P::Car, car, 0x11));
                let f = self.rflags(t, i);
                if f & 1 != 0 {
                    self.seti32(P::Car, car, 0x13, l >> 1);
                }
                if f & 8 == 0 || f & 1 != 0 {
                    self.seti32(P::Car, car, 0x17, 0);
                }
            } else {
                let at_station = self.at_stop_station(t, car);
                if car == self.first_car(t) && self.u8(P::Car, car, 0x31) == 0 && at_station {
                    self.train_arrive(t);
                    rem = 0;
                    self.set8(P::Car, car, 0x31, 1);
                } else {
                    rem += s17 - s13;
                    moved += s13 - s17;
                    self.set8(P::Car, car, 0x31, 0);
                    let ok = self.route_step(car);
                    if ok {
                        self.after_step_reservation(car, t);
                    }
                    let s13 = self.i32(P::Car, car, 0x13);
                    self.seti32(P::Car, car, 0x17, s13);
                    if !ok {
                        rem = 0;
                        break;
                    }
                    let l = seg(self);
                    self.seti32(P::Car, car, 0x13, l);
                }
            }
        }
        let pos = self.i32(P::Car, car, 0x17) + rem;
        moved += rem;
        self.seti32(P::Car, car, 0x17, pos);
        let wheel_len = self.vt32(vt, 0x31);
        let mut w = self.i32(P::Car, car, 0x34) + moved;
        self.seti32(P::Car, car, 0x34, w);
        while w > wheel_len {
            let f = self.u16(P::Car, car, 0x32).wrapping_add(1);
            self.set16(P::Car, car, 0x32, f);
            w = self.i32(P::Car, car, 0x34) - wheel_len;
            self.seti32(P::Car, car, 0x34, w);
        }
        let rod_len = self.vt32(vt, 0x35);
        let mut r = self.i32(P::Car, car, 0x39) + moved;
        self.seti32(P::Car, car, 0x39, r);
        if self.vt8(vt, 0x2b) != 0 && r > rod_len {
            while r > rod_len {
                let f = self.u8(P::Car, car, 0x38).wrapping_add(1);
                self.set8(P::Car, car, 0x38, f);
                r = self.i32(P::Car, car, 0x39) - rod_len;
                self.seti32(P::Car, car, 0x39, r);
            }
        }
        let p = i32::from(self.vt8(vt, 0x26) as i8) + (pos >> 16);
        let (x, y, f) = self.track_point(
            u32::from(self.u8(P::Car, car, 0x23)),
            u32::from(self.u8(P::Car, car, 0x24)),
            p,
            u32::from(self.u8(P::Car, car, 0x25)),
        );
        self.seti32(P::Car, car, 0x1b, x);
        self.seti32(P::Car, car, 0x1f, y);
        self.set32(P::Car, car, 0x26, f);
        moved
    }

    // ---- route steps (0x414dd0, 0x412ff0) --------------------------------------------------

    /// The route is exhausted at the car (`+0x65 == 0`, `+0x61 == 0` or the car is on the newest
    /// step): ask the pathfinder for more (`0x4cf8c0`).
    fn route_needs_more(&self, car: i32, t: i32) -> bool {
        self.i32(P::Train, t, 0x65) == 0
            || self.i32(P::Train, t, 0x61) == 0
            || i32::from(self.i16(P::Car, car, 0x11)) == self.i32(P::Train, t, 0x5d)
    }

    /// `route_next_step` (`0x414dd0`): the head's look-ahead enters the next route step (on the
    /// copy made by `0x415040`); `false` when the route ends or the step is blocked.
    fn route_next_step(&mut self, car: i32) -> bool {
        let t = self.car_train(car);
        if self.route_needs_more(car, t) && !self.extend_route(t) {
            return false;
        }
        let i = ring(i32::from(self.i16(P::Car, car, 0x11)) + 1);
        let (x, y, din, dout) = (
            self.rx(t, i),
            self.ry(t, i),
            self.rdin(t, i),
            self.rdout(t, i),
        );
        if self.tile_blocked(x, y, self.train_id(t), din, dout) {
            self.reserve(t, 1, 1, 1);
            return false;
        }
        let side = self.side_for(x, y, din);
        let f = self.rflags(t, i);
        self.set_rflags(t, i, ((side & 3) << 1) | (f & 0xf9));
        self.set16(P::Car, car, 0x11, i as u16);
        self.set16(P::Car, car, 9, x as u16);
        self.set16(P::Car, car, 0xb, y as u16);
        self.set8(P::Car, car, 0x23, din);
        self.set8(P::Car, car, 0x24, dout);
        self.set8(P::Car, car, 0x25, (self.rflags(t, i) >> 1) & 3);
        true
    }

    /// `route_step` (`0x412ff0`): the car moves to the next route step. The head checks and
    /// reserves the step, consumes oil, water and sand, and refills at a station with water,
    /// oil or sand services; every car updates its tile chain (unless in a look-ahead).
    fn route_step(&mut self, car: i32) -> bool {
        let t = self.car_train(car);
        if self.route_needs_more(car, t) && !self.extend_route(t) {
            return false;
        }
        let (cx, cy) = (
            i32::from(self.i16(P::Car, car, 9)),
            i32::from(self.i16(P::Car, car, 0xb)),
        );
        if car != self.first_car(t) {
            self.retry_wait_reservation(car, t);
        } else {
            let n = ring(i32::from(self.i16(P::Car, car, 0x11)) + 1);
            let (nx, ny, ndin, ndout) = (
                self.rx(t, n),
                self.ry(t, n),
                self.rdin(t, n),
                self.rdout(t, n),
            );
            if self.tile_blocked(nx, ny, self.train_id(t), ndin, ndout) {
                self.reserve(t, 1, 1, 1);
                return false;
            }
            self.reserve(t, 0, 0, 0);
            let region = self.t8(nx, ny, 9);
            self.set8(P::Train, t, 0xf3, region);
            let cur = i32::from(self.i16(P::Car, car, 0x11));
            if self.u8(P::Train, t, 0x94) == 0 {
                let a2 = self
                    .i32(P::Train, t, 0xa2)
                    .wrapping_add(self.i32(P::Train, t, 0xaf));
                self.seti32(P::Train, t, 0xa2, a2);
                let oil = f64::from(self.f32(P::Train, t, 0xd3)) - self.img.f64(0x0053_5128);
                self.setf32(P::Train, t, 0xd3, oil as f32);
                let water = f64::from(self.f32(P::Train, t, 0xd7)) - self.img.f64(0x0053_51f8);
                self.setf32(P::Train, t, 0xd7, water as f32);
                let sand = self.f32(P::Train, t, 0xcf);
                let g = self.train_grade_full(t);
                let s = f64::from(sand) - g.abs() * self.img.f64(0x0053_51e8);
                self.setf32(P::Train, t, 0xcf, s as f32);
            } else {
                let (rx, ry) = (self.rx(t, cur), self.ry(t, cur));
                let side = self.side_for(rx, ry, self.rdin(t, cur));
                let f = self.rflags(t, cur);
                self.set_rflags(t, cur, ((side & 3) << 1) | (f & 0xf9));
                let k = (self.rflags(t, cur) >> 1) & 3;
                self.set8(P::Car, car, 0x25, k);
            }
            if self.flag_570229 != 0 && self.t8(cx, cy, 6) & 2 != 0 {
                todo_hook!(
                    self.log,
                    "route_step_570229",
                    "0x412ff0 cargo 0x11 station marking (flag 0x570229)",
                    "trains.md 10.4"
                );
            }
            if self.t8(nx, ny, 6) & 2 != 0 {
                let st = self.station_at(nx, ny);
                let services = self.u32(P::Station, st, 0x49);
                if services & 0x400 != 0 {
                    self.set32(P::Train, t, 0xcf, 0x42c8_0000);
                }
                if services & 0x800 != 0 {
                    self.set32(P::Train, t, 0xd7, 0x42c8_0000);
                }
                if services & 0x0020_0000 != 0 {
                    self.set32(P::Train, t, 0xd3, 0x42c8_0000);
                }
            }
            let side = self.side_for(nx, ny, ndin);
            let f = self.rflags(t, n);
            self.set_rflags(t, n, ((side & 3) << 1) | (f & 0xf9));
        }
        let car_id = self.i16(P::Car, car, 6) as i32 as u32;
        if self.u16(P::Car, car, 0x45) == 0 {
            self.tile_struct_remove(cx, cy, STRUCT_CAR, car_id);
        }
        let i = ring(i32::from(self.i16(P::Car, car, 0x11)) + 1);
        self.set16(P::Car, car, 0x11, i as u16);
        if car == self.i32(P::Train, t, 4) {
            let head = self.i32(P::Train, t, 0x5d);
            let ahead = if head < i {
                head - i + 0x21
            } else {
                head - i + 1
            };
            if self.i32(P::Train, t, 0x61) - ahead > 8 {
                self.seti32(P::Train, t, 0x61, ahead + 8);
            }
        }
        let (x, y) = (self.rx(t, i), self.ry(t, i));
        self.set16(P::Car, car, 9, x as u16);
        self.set16(P::Car, car, 0xb, y as u16);
        self.set8(P::Car, car, 0x23, self.rdin(t, i));
        self.set8(P::Car, car, 0x24, self.rdout(t, i));
        self.set8(P::Car, car, 0x25, (self.rflags(t, i) >> 1) & 3);
        if car == self.first_car(t) {
            self.enter_tile(t, x, y);
        }
        if self.u16(P::Car, car, 0x45) == 0 {
            self.tile_struct_add(x, y, STRUCT_CAR, car_id);
        }
        true
    }

    /// `train_grade` (`0x4cfd40`) as a full x87 value (used by the sand consumption).
    fn train_grade_full(&self, t: i32) -> f64 {
        if self.u8(P::Train, t, 0x9c) == 0 {
            return f64::from(self.img.f32(0x0053_500c));
        }
        let (mut sum, mut acc) = (0i32, 0f32);
        let mut c = self.first_car(t);
        let mut n = 0;
        while c != 0 {
            if self.u8(P::Car, c, 0x2a) != 0 {
                let w = self.vehicle_weight(c);
                let g = self.vehicle_grade(c);
                sum += w;
                acc = (g * f64::from(w) + f64::from(acc)) as f32;
            }
            c = self.car_next(c);
            n += 1;
            if n == 2 {
                break;
            }
        }
        if sum == 0 {
            f64::from(self.img.f32(0x0053_500c))
        } else {
            f64::from(acc) / f64::from(sum)
        }
    }

    /// `train_enter_tile` (`0x4d5370`, trains.md 4.5): counters, track wear and the
    /// foreign-track statistics when the head enters a new tile.
    pub(crate) fn enter_tile(&mut self, t: i32, x: i32, y: i32) {
        if x == i32::from(self.i16(P::Train, t, 0xcb))
            && y == i32::from(self.i16(P::Train, t, 0xcd))
        {
            return;
        }
        self.set16(P::Train, t, 0xcb, x as u16);
        self.set16(P::Train, t, 0xcd, y as u16);
        let v = self.u16(P::Train, t, 0x7d).wrapping_add(1);
        self.set16(P::Train, t, 0x7d, v);
        let v = self.u16(P::Train, t, 0xa0).wrapping_add(1);
        self.set16(P::Train, t, 0xa0, v);
        let p = self.piece_at(x, y);
        let w = self.i32(P::Train, t, 0xaf);
        let v = self.i32(P::Track, p, 0x17).wrapping_add(w);
        self.seti32(P::Track, p, 0x17, v);
        let v = self.i32(P::Track, p, 0x1b).wrapping_add(w);
        self.seti32(P::Track, p, 0x1b, v);
        let f = self.u8(P::Track, p, 0x23);
        if f & 0x80 == 0 {
            self.set8(P::Track, p, 0x23, f | 0x80);
            if self.flag_570228() != 0 {
                todo_hook!(
                    self.log,
                    "tile_redraw_570228",
                    "0x4cb090 around the tile (option 0x570228)",
                    "trains.md 10.4"
                );
            }
        }
        if f64::from(self.f32(P::Train, t, 0xdb)) > f64::from(self.img.f32(0x0053_7814)) {
            let a7 = self.u8(P::Train, t, 0xa7);
            self.set8(P::Train, t, 0xa8, a7);
            let (cx, cy) = (
                i32::from(self.i16(P::Train, t, 0xcb)),
                i32::from(self.i16(P::Train, t, 0xcd)),
            );
            if cx >= 0 && self.u8(P::Train, t, 0x9c) != 0 && a7 != 0 {
                let pp = self.piece_at(cx, cy);
                let owner = if pp == 0 {
                    0
                } else {
                    u32::from(self.u8(P::Track, pp, 2))
                };
                if owner == self.u32(P::Train, t, 0x41) {
                    self.set8(P::Train, t, 0xa8, a7.wrapping_add(4));
                }
            }
        } else {
            self.set8(P::Train, t, 0xa8, 0);
        }
        let owner = self.u8(P::Track, p, 2);
        let mut counted = false;
        if u32::from(owner) != self.u32(P::Train, t, 0x41) {
            let n = usize::from(self.u8(P::Train, t, 0x81));
            for k in 0..n {
                if u16::from(owner) == self.u16(P::Train, t, 0x82 + 2 * k) {
                    let v = self.u16(P::Train, t, 0x8a + 2 * k).wrapping_add(1);
                    self.set16(P::Train, t, 0x8a + 2 * k, v);
                    return;
                }
            }
            if n < 4 {
                self.set16(P::Train, t, 0x8a + 2 * n, 1);
                self.set16(P::Train, t, 0x82 + 2 * n, u16::from(owner));
                self.set8(P::Train, t, 0x81, (n + 1) as u8);
                counted = true;
            }
        }
        if !counted {
            let v = self.u16(P::Train, t, 0x7f).wrapping_add(1);
            self.set16(P::Train, t, 0x7f, v);
        }
        if self.u8(P::Train, t, 0xf2) != 0
            || self.u8(P::Train, t, 0xed) != 0
            || self.u8(P::Train, t, 0xbc) != 0
        {
            self.set8(P::Train, t, 0xa8, 0x14);
        }
        let v = self.i32(P::Train, t, 0xe1).wrapping_add(1);
        self.seti32(P::Train, t, 0xe1, v);
    }

    fn flag_570228(&self) -> u8 {
        self.s.header.get(0x86f0).copied().unwrap_or(0)
    }

    /// The pathfinder asks for more route (`0x4cf8c0`).
    fn extend_route(&mut self, t: i32) -> bool {
        self.route_extend(t)
    }

    // ---- the head car (0x413550, head branch) ---------------------------------------------

    /// `+0xbb & 8` after a placement (`0x4d0ce0`): an electric locomotive on a tile without
    /// catenary (`piece.+0x23 & 0x10`).
    fn update_wire_bit(&mut self, t: i32) {
        let head = self.first_car(t);
        let he = self.vt_engine(self.u8(P::Car, head, 8));
        let b = self.u8(P::Train, t, 0xbb);
        let no_wire = self.en8(he, 0x28) == 2
            && self.u8(P::Train, t, 0x9c) != 0
            && self.u8(P::Car, head, 0x2a) != 0
            && {
                let (x, y) = (
                    i32::from(self.i16(P::Car, head, 9)),
                    i32::from(self.i16(P::Car, head, 0xb)),
                );
                let p = self.tile_track(x, y);
                p == 0 || self.u8(P::Track, p, 0x23) & 0x10 == 0
            };
        self.set8(P::Train, t, 0xbb, if no_wire { b | 8 } else { b & 0xf7 });
    }

    /// `vehicle_place(head, -1, 0xffff, 0)` (`0x413550`, head branch; trains.md 12): puts the
    /// locomotive of a train that is not on the track on the tile of the current stop (the
    /// station's, or the waypoint's), asks the pathfinder for a route from there (`0x4cf8c0`),
    /// sets the first step of the route ring (entry 0), the car's segment and sub-tile point,
    /// the tile structure, and reserves the step. Needs `train.+0x9d != 0` (set by command
    /// `0x2c`, or by the AI when it buys a train).
    pub(crate) fn place_head_car(&mut self, t: i32) {
        let car = self.first_car(t);
        if self.u8(P::Train, t, 0x9d) == 0 {
            return;
        }
        let k = self.cur_stop(t);
        if self.stop8(t, k, 3) & 8 == 0 {
            let st = i32::from(self.stop16(t, k, 0));
            let id = self.u16(P::Station, st, 0);
            self.set16(P::Train, t, 0x9e, id);
            let (x, y) = (self.u16(P::Station, st, 10), self.u16(P::Station, st, 14));
            self.set16(P::Car, car, 9, x);
            self.set16(P::Car, car, 0xb, y);
        } else {
            // Both coordinates come from the waypoint's `+10` in the original.
            let x = self.stop16(t, k, 10);
            self.set16(P::Car, car, 9, x);
            self.set16(P::Car, car, 0xb, x);
        }
        self.set8(P::Train, t, 0x9c, 1);
        let idx = 0;
        self.set16(P::Car, car, 0x11, idx as u16);
        let stop_before = self.u8(P::Train, t, 0x3c);
        let has_route = if self.i32(P::Train, t, 0x65) == 0
            || self.i32(P::Train, t, 0x61) == 0
            || idx == self.i32(P::Train, t, 0x5d)
        {
            self.route_extend(t)
        } else {
            true
        };
        let mut reset_route = false;
        if self.u8(P::Train, t, 0x3c) != stop_before {
            self.set8(P::Train, t, 0x3c, stop_before);
            reset_route = has_route;
        }
        let (x, y) = (
            i32::from(self.i16(P::Car, car, 9)),
            i32::from(self.i16(P::Car, car, 0xb)),
        );
        if has_route {
            self.set8(P::Car, car, 0x23, self.rdin(t, idx));
            self.set8(P::Car, car, 0x24, self.rdout(t, idx));
        } else {
            self.set8(P::Car, car, 0x23, 0x7f);
            let mask = u32::from(self.piece_mask(x, y));
            for d in 0..8u32 {
                if mask & (1 << d) != 0 {
                    self.set8(P::Car, car, 0x23, (d as u8).wrapping_sub(4) & 7);
                    self.set8(P::Car, car, 0x24, d as u8);
                    break;
                }
            }
        }
        if reset_route {
            self.route_free(t);
        }
        let din = self.u8(P::Car, car, 0x23);
        let p = self.piece_at(x, y);
        let side = if p == 0 || self.u8(P::Track, p, 0x23) & 1 == 0 {
            0
        } else {
            self.img.i32(0x0053_6c4c + 4 * u32::from(din)) as u8
        };
        self.set8(P::Car, car, 0x25, side);
        let dout = self.u8(P::Car, car, 0x24);
        if self.i32(P::Train, t, 0x65) == 0 {
            self.route_init(t, 0x20);
            self.set_route_step(t, idx, x as u16, y as u16, din, dout);
            let f = self.rflags(t, idx) & 0xfe;
            self.set_rflags(t, idx, f);
            self.seti32(P::Train, t, 0x5d, idx);
            self.seti32(P::Train, t, 0x61, 1);
            self.seti32(P::Train, t, 0x65, 1);
        } else {
            let f = self.rflags(t, idx) & 0xfe;
            self.set_rflags(t, idx, f);
        }
        let f = self.rflags(t, idx);
        self.set_rflags(t, idx, ((side & 3) << 1) | (f & 0xf9));
        let len = self.seg_len(u32::from(din), u32::from(dout), u32::from(side)) << 16;
        self.seti32(P::Car, car, 0x13, len);
        self.seti32(P::Car, car, 0x17, len >> 1);
        self.set32(P::Car, car, 0x26, 0);
        self.set8(P::Car, car, 0x2a, 1);
        if self.u16(P::Car, car, 0x45) == 0 {
            let id = self.i16(P::Car, car, 6) as i32 as u32;
            self.tile_struct_add(x, y, STRUCT_CAR, id);
        }
        let (px, py, pf) = self.track_point(
            u32::from(din),
            u32::from(dout),
            self.i32(P::Car, car, 0x17) >> 16,
            u32::from(side),
        );
        self.seti32(P::Car, car, 0x1b, px);
        self.seti32(P::Car, car, 0x1f, py);
        self.set32(P::Car, car, 0x26, pf);
        self.set_reserved_range(t, idx, idx);
        let (cx, cy) = (self.u16(P::Car, car, 9), self.u16(P::Car, car, 0xb));
        self.set16(P::Car, car, 0xd, cx);
        self.set16(P::Car, car, 0xf, cy);
        self.set16(P::Car, car, 0x48, cx);
        self.set16(P::Car, car, 0x4a, cy);
    }

    // ---- trailing cars (0x413550, non-head branch) ----------------------------------------

    /// `vehicle_place` (`0x413550`) for a car that is not the head: puts an off-track car on the
    /// route one coupling length behind the car ahead, when the route there is free.
    fn place_trailing_car(&mut self, car: i32) {
        let t = self.car_train(car);
        if car == self.first_car(t) {
            self.place_head_car(t);
            return;
        }
        let prev = self.car_prev(car);
        if self.u8(P::Train, t, 0x9c) == 0
            || prev == 0
            || !self.exists(P::Car, prev)
            || self.u8(P::Car, prev, 0x2a) == 0
            || self.i32(P::Train, t, 0x65) == 0
            || self.i32(P::Train, t, 0x61) == 0
        {
            return;
        }
        let mut idx = i32::from(self.i16(P::Car, prev, 0x11));
        let vt = self.u8(P::Car, car, 8);
        let mut pos = self.i32(P::Car, prev, 0x17)
            - (half_len(self, self.u8(P::Car, prev, 8)) + half_len(self, vt));
        let (mut s_pos, mut s_idx) = (0, 0);
        let mut pass = 0;
        while pass < 2 {
            while pos < 0 {
                let tail = ring(self.i32(P::Train, t, 0x5d) - self.i32(P::Train, t, 0x61) + 0x21);
                if idx == tail {
                    return;
                }
                idx = ring(idx + 0x1f);
                let f = self.rflags(t, idx);
                let l = self.seg_len(
                    u32::from(self.rdin(t, idx)),
                    u32::from(self.rdout(t, idx)),
                    u32::from((f >> 1) & 3),
                ) << 16;
                if f & 8 == 0 || f & 1 == 0 {
                    pos += l;
                }
                let (x, y) = (self.rx(t, idx), self.ry(t, idx));
                if self.piece_conflict(
                    x,
                    y,
                    u32::from((f >> 1) & 3),
                    self.train_id(t),
                    self.rdin(t, idx),
                    self.rdout(t, idx),
                ) {
                    return;
                }
            }
            if pass == 0 {
                s_pos = pos;
                s_idx = idx;
                pos -= half_len(self, vt);
            } else {
                pos = s_pos;
                idx = s_idx;
            }
            pass += 1;
        }
        let f = self.rflags(t, idx);
        if f & 1 != 0 {
            let l = self.seg_len(
                u32::from(self.rdin(t, idx)),
                u32::from(self.rdout(t, idx)),
                u32::from((f >> 1) & 3),
            );
            if pos < (l << 16) >> 1 {
                return;
            }
        }
        let (x, y) = (self.rx(t, idx), self.ry(t, idx));
        self.set16(P::Car, car, 9, x as u16);
        self.set16(P::Car, car, 0xb, y as u16);
        self.set16(P::Car, car, 0xf, y as u16);
        self.set16(P::Car, car, 0x4a, y as u16);
        self.set16(P::Car, car, 0xd, x as u16);
        self.set16(P::Car, car, 0x48, x as u16);
        if self.exists(P::Car, prev) {
            self.set16(P::Car, car, 0xd, self.u16(P::Car, prev, 0xd));
            self.set16(P::Car, car, 0xf, self.u16(P::Car, prev, 0xf));
            self.set16(P::Car, car, 0x48, self.u16(P::Car, prev, 0x48));
            self.set16(P::Car, car, 0x4a, self.u16(P::Car, prev, 0x4a));
        }
        self.set16(P::Car, car, 0x11, idx as u16);
        self.seti32(P::Car, car, 0x17, pos);
        self.set8(P::Car, car, 0x24, self.rdout(t, idx));
        self.set8(P::Car, car, 0x23, self.rdin(t, idx));
        self.set8(P::Car, car, 0x25, (self.rflags(t, idx) >> 1) & 3);
        self.retry_wait_reservation(car, t);
        self.set8(P::Car, car, 0x2a, 1);
        if self.u16(P::Car, car, 0x45) == 0 {
            let id = self.i16(P::Car, car, 6) as i32 as u32;
            self.tile_struct_add(x, y, STRUCT_CAR, id);
        }
        let (px, py, pf) = self.track_point(
            u32::from(self.u8(P::Car, car, 0x23)),
            u32::from(self.u8(P::Car, car, 0x24)),
            self.i32(P::Car, car, 0x17) >> 16,
            u32::from(self.u8(P::Car, car, 0x25)),
        );
        self.seti32(P::Car, car, 0x1b, px);
        self.seti32(P::Car, car, 0x1f, py);
        self.set32(P::Car, car, 0x26, pf);
    }

    // ---- breakdowns (0x4cf390) -------------------------------------------------------------

    /// `train_breakdown_check` (`0x4cf390`, trains.md 5.1) after a move: the dice of
    /// [`crate::trains::breakdown_check`] with the train's inputs; `+0xa0` restarts at 0.
    fn breakdown_check(&mut self, t: i32) {
        // 0x5748fc (runtime, 0 in a loaded game) would switch the check off.
        let cells = i32::from(self.i16(P::Train, t, 0xa0));
        if cells < 10
            || self.u8(P::Train, t, 0xed) != 0
            || self.u8(P::Train, t, 0xf2) != 0
            || self.u8(P::Train, t, 0xbc) != 0
        {
            return;
        }
        let company = self.i32(P::Train, t, 0x41);
        let human = self.company_is_human(company);
        let head = self.first_car(t);
        let eng_idx = self.vt_engine(self.u8(P::Car, head, 8));
        let eng = self.engine_struct(eng_idx);
        let terr = i32::from(self.u8(P::Train, t, 0xf3));
        let chance = crate::trains::breakdown_chance(
            &eng,
            &crate::trains::BreakdownInput {
                tiles: cells,
                check_age: true,
                year: self.year,
                bought: self.u16(P::Train, t, 0xbd),
                reliability_rate: self
                    .img
                    .f32(0x0055_c52c + 4 * self.en32(eng_idx, 0x08) as u32),
                throttle: self.f32(P::Train, t, 0xdb),
                reliability_mod: f64::from(self.effect(0x27, company, 0, terr)),
                grade: self.train_grade_full(t),
                tons: self.i32(P::Train, t, 0xaf),
                caboose: self.u8(P::Train, t, 0xbb) & 2 != 0,
                oil: self.f32(P::Train, t, 0xd3),
                // `/nowreck` (0x574b74): a command-line option, off in a normal game.
                nowreck: false,
            },
        ) as f32;
        self.set16(P::Train, t, 0xa0, 0);
        let st = &self.s.state;
        let pct = i16::from_le_bytes([st[0x391a], st[0x391b]]);
        if pct != 0 {
            todo_hook!(
                self.log,
                "theft_town_search",
                "0x42fa30 nearest town for the theft roll (0x573e02 != 0)",
                "trains.md 6.4"
            );
        }
        let theft = crate::trains::TheftInput {
            pct,
            nearby_excess: None,
            theft_mod: self.effect(0x21, company, 0, terr),
            caboose: self.u8(P::Train, t, 0xbb) & 2 != 0,
            carries_people: !self
                .cars_with(t, |ts, c| {
                    ts.i32(P::Car, c, 0x2d) != 0 && ts.vt8(ts.u8(P::Car, c, 8), 0x30) <= 1
                })
                .is_empty(),
            eligible_thieves: 0,
        };
        let out = crate::trains::breakdown_check(
            self.rng,
            &crate::trains::CheckInput {
                tiles: cells,
                chance,
                human,
                no_accidents: self.flag_570229 != 0,
                year: self.year,
                theft,
            },
        );
        match out {
            crate::trains::CheckOutcome::Idle => {}
            crate::trains::CheckOutcome::Breakdown => self.train_breakdown(t),
            crate::trains::CheckOutcome::Crash => todo_hook!(
                self.log,
                "train_crash",
                "train_crash 0x4ce730 (rng_range(30,45) days, penalty)",
                "trains.md 5.3"
            ),
            _ => todo_hook!(
                self.log,
                "train_theft",
                "train_theft 0x4ceaa0 / arrest (thief names 0x573db7)",
                "trains.md 6.4"
            ),
        }
    }

    /// `train_breakdown` (`0x4ce4c0`, train, 0): broken down for `rng_range(60, 120)` days;
    /// priority 20, the engine's puff clock restarts, and the company pays the reputation
    /// penalty `2000 / (trains^0.66 + 4)` (`+0x4229`, floor 0; and per territory of the tile,
    /// `+0x47a + terr*0x1ed`, floor -10000).
    pub fn train_breakdown(&mut self, t: i32) {
        if self.u8(P::Train, t, 0xed) != 0
            || self.u8(P::Train, t, 0xf2) != 0
            || self.u8(P::Train, t, 0xbc) != 0
        {
            return;
        }
        self.set8(P::Train, t, 0xbc, 0);
        self.set8(P::Train, t, 0xf2, 0);
        self.set8(P::Train, t, 0xed, 1);
        let days = self.rng.range(0x3c, 0x78);
        self.set32(
            P::Train,
            t,
            0xee,
            (days as u32).wrapping_mul(0x5a0).wrapping_add(self.now),
        );
        self.set8(P::Train, t, 0xa8, 0x14);
        let head = self.first_car(t);
        self.set32(P::Car, head, 0x3d, self.now.wrapping_sub(500));
        let company = self.i32(P::Train, t, 0x41);
        let terr = if self.u8(P::Train, t, 0x9c) == 0 || self.u8(P::Car, head, 0x2a) == 0 {
            1
        } else {
            i32::from(self.t8(
                i32::from(self.i16(P::Car, head, 9)),
                i32::from(self.i16(P::Car, head, 0xb)),
                9,
            ))
        };
        let n = f64::from(self.i32(P::Company, company, 0x27));
        let p = self.img.f64(0x0053_5690)
            / (crate::fp::pow(n, self.img.f64(0x0053_5b70)) + self.img.f64(0x0053_5138));
        let v = ftol(f64::from(self.i32(P::Company, company, 0x4229)) - p);
        self.seti32(P::Company, company, 0x4229, if v < 1 { 0 } else { v });
        let o = (0x47a + terr * 0x1ed) as usize;
        let v = ftol(f64::from(self.i16(P::Company, company, o)) - p) as i16;
        self.set16(P::Company, company, o, v.max(-10000) as u16);
        if self.is_ui_company_pub(company) {
            todo_hook!(
                self.log,
                "breakdown_message",
                "0x4ce4c0 message, sound and 0x4d0870/0x45cd60 for the viewed company",
                "trains.md 5.3"
            );
        }
    }

    /// `train_set_throttle_state` (`0x4d59b0`, train, a7): end of a breakdown: priority
    /// `+0xa8` as in `0x4d5370` (throttle above 65, +4 on own track).
    pub fn train_set_throttle_state(&mut self, t: i32, a7: u8) {
        if self.u8(P::Train, t, 0xf2) != 0
            || self.u8(P::Train, t, 0xed) != 0
            || self.u8(P::Train, t, 0xbc) != 0
        {
            return;
        }
        self.set8(P::Train, t, 0xa7, a7);
        if f64::from(self.f32(P::Train, t, 0xdb)) > f64::from(self.img.f32(0x0053_7814)) {
            self.set8(P::Train, t, 0xa8, a7);
            let (cx, cy) = (
                i32::from(self.i16(P::Train, t, 0xcb)),
                i32::from(self.i16(P::Train, t, 0xcd)),
            );
            if cx >= 0 && self.u8(P::Train, t, 0x9c) != 0 && a7 != 0 {
                let pp = self.piece_at(cx, cy);
                let owner = if pp == 0 {
                    0
                } else {
                    u32::from(self.u8(P::Track, pp, 2))
                };
                if owner == self.u32(P::Train, t, 0x41) {
                    let v = self.u8(P::Train, t, 0xa7).wrapping_add(4);
                    self.set8(P::Train, t, 0xa8, v);
                }
            }
        } else {
            self.set8(P::Train, t, 0xa8, 0);
        }
    }

    /// `company_is_human` (`0x43c030`): the company's person (`+0x23`) has `+0x122 != 0`.
    pub fn company_is_human(&self, company: i32) -> bool {
        let person = self.i32(P::Company, company, 0x23);
        person > 0 && self.i32(P::Person, person, 0x122) != 0
    }
}

/// `ring_in_range` (`0x4f0830`, `a, b, head, count, 32`): `b` is strictly ahead of `a` in the
/// ring counted back from `head`.
pub fn ring_ahead(a: i32, b: i32, head: i32, _count: i32) -> bool {
    let da = if head < a { head - a + 0x20 } else { head - a };
    if b <= head {
        da < head - b
    } else {
        da < head - b + 0x20
    }
}
