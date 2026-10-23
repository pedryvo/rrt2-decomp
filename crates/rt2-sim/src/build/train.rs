//! Train purchase and route editing (trains.md 6.1 and "Compra e paradas"): `train_buy`
//! (`0x4cdf00`) -> `train_create` (`0x4cdf80`) with the cars (`vehicle_init` `0x414220`), the
//! default or copied stops (`train_default_stops` `0x4d3f40`), stop insertion and deletion
//! (`0x4d11f0`, `0x4d5710`), the leg length (`0x4d1130`) and the train class (`0x4d59b0`).
//!
//! A new train is not on the track: the train step (`0x4d0ce0`) matches its consist with the first
//! stop (`0x4d1bb0`) and puts the locomotive on the track (`0x413550`, head branch) once command
//! `0x2c` has marked it (trains.md 12); the other cars follow as it moves. The stop-editing
//! commands `0x1a`, `0x1b`, `0x1c`, `0x21` and `0x2f` are here too (commands.md 2.1).

use crate::ai::Ai;
use crate::trainstep::{P, Ts};

use super::account_move;

/// A stop record (`train+0x37`, 14 bytes): u16 station, u8 cargo count, u8 flags (`8` =
/// waypoint, `0x40` AI "return"), 6 cargo bytes, i16 x, i16 y (waypoint position).
pub type StopRec = [u8; 14];

impl Ai<'_, '_> {
    fn stops(&self, t: i32) -> Vec<StopRec> {
        let n = usize::from(self.ts.u8(P::Train, t, 0x3b));
        (0..n)
            .map(|k| {
                let mut r = [0u8; 14];
                for (o, b) in r.iter_mut().enumerate() {
                    *b = self.ts.stop8(t, k as i32, o);
                }
                r
            })
            .collect()
    }

    fn set_stops(&mut self, t: i32, v: &[StopRec]) {
        let e = self
            .ts
            .s
            .train_extras
            .iter_mut()
            .find(|e| e.slot == t as usize)
            .expect("train extras");
        e.cars_raw = v.iter().flatten().copied().collect();
        self.ts.set8(P::Train, t, 0x3b, v.len() as u8);
    }

    /// Position of a stop: the station's tile, or the waypoint's.
    fn stop_pos(&self, s: &StopRec) -> (i32, i32) {
        if s[3] & 8 != 0 {
            (
                i32::from(i16::from_le_bytes([s[10], s[11]])),
                i32::from(i16::from_le_bytes([s[12], s[13]])),
            )
        } else {
            let st = i32::from(u16::from_le_bytes([s[0], s[1]]));
            (
                self.ts.i32(P::Station, st, 0xa),
                self.ts.i32(P::Station, st, 0xe),
            )
        }
    }

    /// `0x4d1130`: the length of the round trip, `Σ int_dist(stop k-1, stop k) + 8` (`+0x33`).
    fn train_leg_length(&mut self, t: i32) {
        self.ts.seti32(P::Train, t, 0x33, 0);
        let v = self.stops(t);
        if v.len() < 2 {
            return;
        }
        let mut sum = 0i32;
        let mut prev = self.stop_pos(&v[0]);
        for k in 1..=v.len() {
            let cur = self.stop_pos(&v[k % v.len()]);
            sum = sum.wrapping_add(Ai::dist(prev.0, prev.1, cur.0, cur.1) + 8);
            prev = cur;
        }
        self.ts.seti32(P::Train, t, 0x33, sum);
    }

    /// `train_buy(engine, station, company)` (`0x4cdf00`): fails (`-1`, message LNG 352) with
    /// fewer than two stations in the world.
    pub fn train_buy(&mut self, engine: i32, company: i32) -> Option<i32> {
        if self.ts.s.stations.count < 2 {
            return None;
        }
        let size = self.ts.record_size(P::Train);
        let t = self.ts.alloc(P::Train, vec![0u8; size]);
        self.ts.s.train_extras.push(rt2_formats::map::TrainExtra {
            slot: t as usize,
            cars_raw: Vec::new(),
            route_raw: Vec::new(),
        });
        self.ts.s.train_extras.sort_by_key(|e| e.slot);
        self.train_create(t, engine, company);
        Some(t)
    }

    /// `train_create(train, id, engine, station, company)` (`0x4cdf80`).
    fn train_create(&mut self, t: i32, e: i32, company: i32) {
        self.ts.log.ran("train_create");
        let now = self.ts.now;
        let year = self.ts.year;
        let s8 = |ai: &mut Ai<'_, '_>, o: usize, v: u8| ai.ts.set8(P::Train, t, o, v);
        let s16 = |ai: &mut Ai<'_, '_>, o: usize, v: u16| ai.ts.set16(P::Train, t, o, v);
        let s32 = |ai: &mut Ai<'_, '_>, o: usize, v: u32| ai.ts.set32(P::Train, t, o, v);
        s8(self, 0x94, 1);
        s8(self, 0xf4, 0);
        s32(self, 0x3d, t as u32);
        s16(self, 0xa9, year);
        s16(self, 0xbd, year);
        s16(self, 0xf5, 0);
        s32(self, 0xf7, 0);
        s8(self, 0xfb, 0);
        for k in 0..0x1e {
            s8(self, 0xc + k, 0);
        }
        s8(self, 0xf3, 1);
        for o in [0xcf, 0xd3, 0xd7] {
            self.ts.setf32(P::Train, t, o, 100.0);
        }
        self.ts.setf32(P::Train, t, 0xdb, 85.0);
        s16(self, 0xdf, 0);
        let b = self.ts.u8(P::Train, t, 0xbb) & 0xcf;
        s8(self, 0xbb, b);
        for o in [0xbc, 0xf2, 0xed, 0xfc] {
            s8(self, o, 0);
        }
        s32(self, 0xee, 0);
        s16(self, 0xa0, 0);
        for o in [0xfd, 0x101, 0x105, 0x109, 0x13d, 0x141, 0x145, 0x149] {
            s32(self, o, 0);
        }
        for k in 0..0xc {
            s32(self, 0x10d + 4 * k, 0);
        }
        for o in [0xbf, 0xc3, 0x71, 0x2f] {
            s32(self, o, 0);
        }
        s32(self, 0x6d, now);
        self.ts.setf32(P::Train, t, 0xe9, 2.0);
        let b = self.ts.u8(P::Train, t, 0xbb) & 0xf8;
        s8(self, 0xbb, b);
        for o in [0xb7, 0x69, 0x65] {
            s32(self, o, 0);
        }
        s8(self, 0x9c, 0);
        s8(self, 0xa7, 2);
        s8(self, 0xa8, 2);
        s32(self, 0xab, 0);
        s32(self, 0, 0);
        s32(self, 4, 0);
        s8(self, 0x99, 1);
        s8(self, 0x9a, 3);
        s8(self, 0x9b, 0);
        s16(self, 0x95, 0xffff);
        s16(self, 0x97, 0xffff);
        s32(self, 0x37, 0);
        s8(self, 0x3b, 0);
        s8(self, 0x3c, 0xff);
        s8(self, 0x92, 0);
        s16(self, 0x9e, 0);
        s8(self, 0x9d, 0);
        s32(self, 0xa2, 0);
        s32(self, 0xe1, 0);
        s8(self, 0xa6, 1);
        s32(self, 0x41, company as u32);
        let n = self.ts.i32(P::Company, company, 0x27) + 1;
        self.ts.seti32(P::Company, company, 0x27, n);
        let vt = self.ts.en8(e, 0x2c);
        self.ts.vehicle_init(t, vt);
        if self.ts.en8(e, 0x2b) != 0 {
            let vt2 = self.ts.en8(e, 0x2d);
            self.ts.vehicle_init(t, vt2);
        }
        s8(self, 0x94, 0);
        let (mut tons, mut steam) = (0i32, 0i32);
        let mut c = self.ts.i32(P::Train, t, 0);
        while c != 0 {
            let w = self.ts.vehicle_weight(c);
            tons += w;
            if self.ts.vt8(self.ts.u8(P::Car, c, 8), 0x2e) == 2 {
                steam += w;
            }
            c = i32::from(self.ts.i16(P::Car, c, 2));
        }
        s32(self, 0xaf, tons as u32);
        s32(self, 0xb3, steam as u32);
        if self.ts.flag_570229 == 0 {
            let b = self.ts.u8(P::Train, t, 0xbb) & 0xdf;
            s8(self, 0xbb, b);
        } else {
            todo_hook!(
                self.ts.log,
                "train_create_570229",
                "0x4cdf80 with the option 0x570229 (cargo 0x11 flag, 0x4d14b0)",
                "trains.md 6.1"
            );
        }
        let kind = usize::from(self.ts.en8(e, 0x28)) + 0x1b;
        let terr = i32::from(self.ts.u8(P::Train, t, 0xf3));
        let m_kind = self.ts.effect(kind, company, 0, terr);
        let m_all = self.ts.effect(0x1a, company, 0, terr);
        let price = self.ts.en32(e, 0xc);
        let v = f64::from(m_all) * f64::from(m_kind) * f64::from(price);
        self.ts.seti32(P::Train, t, 8, crate::fp::ftol(v));
        account_move(self.ts, company, 4, v);
        self.ts.train_route_refresh(t);
        let head = self.ts.i32(P::Train, t, 0);
        let he = self.ts.vt_engine(self.ts.u8(P::Car, head, 8));
        let b = self.ts.u8(P::Train, t, 0xbb);
        let no_wire = self.ts.en8(he, 0x28) == 2
            && self.ts.u8(P::Train, t, 0x9c) != 0
            && self.ts.u8(P::Car, head, 0x2a) != 0;
        s8(self, 0xbb, if no_wire { b | 8 } else { b & 0xf7 });
    }

    /// `0x439720(company, k)`: the `k`-th live station of the company (list order).
    fn company_station(&self, company: i32, k: i32) -> i32 {
        let mut k = k;
        for st in self.ids(P::Station) {
            if self.ts.i32(P::Station, st, 0x39) == company
                && self.ts.i32(P::Station, st, 0x45) == 0
            {
                k -= 1;
                if k == 0 {
                    return st;
                }
            }
        }
        0
    }

    /// `train_default_stops(train, from)` (`0x4d3f40`): copies the stops of train `from`, or
    /// gives a new train the company's first two stations (the first two of the world when it
    /// has none) with 2 cargo slots (passengers, or cargo `0x26` with the industry set).
    /// The bytes of a default stop past its cargo list are uninitialized stack memory in the
    /// original (left 0 here).
    pub fn train_default_stops(&mut self, t: i32, from: i32) {
        if from != 0 && self.ts.exists(P::Train, from) {
            for (k, s) in self.stops(from).into_iter().enumerate() {
                self.stop_insert(t, k as i32, Some(s), 0, false, 0, 0);
            }
            return;
        }
        let company = self.ts.i32(P::Train, t, 0x41);
        let nst = self.ts.i32(P::Company, company, 0x172);
        let pick = |ai: &Ai<'_, '_>, k: i32| {
            if nst == 0 {
                crate::pool::nth(&ai.ts.s.stations, k as usize) as i32
            } else {
                ai.company_station(company, k + 1)
            }
        };
        let cargo = if self.hdr8(0x0057_04ac) != 0 { 0x26 } else { 0 };
        let mut s: StopRec = [0; 14];
        s[0..2].copy_from_slice(&(pick(self, 0) as u16).to_le_bytes());
        s[2] = 2;
        s[4] = cargo;
        s[5] = cargo;
        self.stop_insert(t, 0, Some(s), 0, false, 0, 0);
        if nst != 1 {
            s[0..2].copy_from_slice(&(pick(self, 1) as u16).to_le_bytes());
            self.stop_insert(t, 1, Some(s), 0, false, 0, 0);
        }
    }

    /// `train_stop_insert(train, k, stop, station, waypoint, x, y)` (`0x4d11f0`): inserts a stop
    /// at `k` (0 when out of range); without a record, a new stop copies the cargo list of the
    /// previous one. Resets the train's age stamps (`+0xe5`, `+0xdf`, `+0xe9 = 2.0`).
    #[allow(clippy::too_many_arguments)]
    pub fn stop_insert(
        &mut self,
        t: i32,
        k: i32,
        stop: Option<StopRec>,
        station: u16,
        waypoint: bool,
        x: u16,
        y: u16,
    ) {
        let n = i32::from(self.ts.u8(P::Train, t, 0x3b));
        let k = if k < 0 || n < k { 0 } else { k };
        self.ts.setf32(P::Train, t, 0xe9, 2.0);
        let now = self.ts.now;
        self.ts.set32(P::Train, t, 0xe5, now);
        let year = self.ts.year;
        self.ts.set16(P::Train, t, 0xdf, year);
        let mut v = self.stops(t);
        let rec = stop.unwrap_or_else(|| {
            let mut r: StopRec = [0; 14];
            if n == 0 {
                if self.ts.flag_570236 != 0 {
                    todo_hook!(
                        self.ts.log,
                        "stop_insert_570236",
                        "0x4d11f0 first stop with option 0x570236 (cargo list 0x573f0e)",
                        "trains.md 10"
                    );
                } else if self.ts.flag_570229 != 0 {
                    r[3] = 0x30;
                }
            } else {
                let p = &v[((n - 1 + k) % n) as usize];
                r[2..10].copy_from_slice(&p[2..10]);
            }
            r[0..2].copy_from_slice(&station.to_le_bytes());
            r[3] = (r[3] & 0xf7) | (u8::from(waypoint) << 3);
            r[10..12].copy_from_slice(&x.to_le_bytes());
            r[12..14].copy_from_slice(&y.to_le_bytes());
            r
        });
        v.insert(k as usize, rec);
        self.set_stops(t, &v);
        if n == 0 {
            self.ts.set_stop(t, 0, true);
        } else if k <= self.ts.cur_stop(t) {
            let c = self.ts.u8(P::Train, t, 0x3c).wrapping_add(1);
            self.ts.set8(P::Train, t, 0x3c, c);
        }
        self.train_leg_length(t);
        if self.ts.flag_570229 != 0 {
            todo_hook!(
                self.ts.log,
                "stop_cargo_570229",
                "0x4d6330 cargo lists with the option 0x570229",
                "trains.md 10"
            );
        }
    }

    /// `0x439930(company, account, x, category)`: account and category move without cash
    /// (the write-off of a sold or lost train).
    fn write_off(&mut self, company: i32, account: usize, x: f64, cat: usize) {
        let mut r = self.ts.record(P::Company, company);
        for o in [0x4766 + 8 * account, 0x4286 + 8 * cat, 0x46fe + 8 * cat] {
            let v = x + crate::pool::f64_at(&r, o);
            crate::pool::put_f64(&mut r, o, v);
        }
        self.ts.set_record(P::Company, company, &r);
    }

    /// `train_delete(train)` (`0x4cdd50`, command `0x1f`): no refund: account 4 and category 6
    /// lose the purchase price `+8` without cash (`0x439930`); the cars, reservations, route
    /// and stops go.
    pub fn train_delete(&mut self, t: i32) {
        self.ts.log.ran("train_delete");
        self.ts.set8(P::Train, t, 0xf4, 1);
        if self.ts.i32(P::Train, t, 0xab) != 0 {
            self.ts.seti32(P::Train, t, 0xab, 0);
            let b = self.ts.u8(P::Train, t, 0xbb) & 0xef;
            self.ts.set8(P::Train, t, 0xbb, b);
        }
        let company = self.ts.i32(P::Train, t, 0x41);
        let price = self.ts.i32(P::Train, t, 8);
        self.write_off(company, 4, -f64::from(price), 6);
        let n = self.ts.i32(P::Company, company, 0x27) - 1;
        self.ts.seti32(P::Company, company, 0x27, n);
        self.ts.seti32(P::Train, t, 0x41, 0);
        loop {
            let c = self.ts.i32(P::Train, t, 4);
            if c == 0 {
                break;
            }
            self.ts.car_remove(t, c);
        }
        self.ts.set_reserved_range(t, -1, -1);
        self.ts.s.train_extras.retain(|e| e.slot != t as usize);
        self.ts.free(P::Train, t);
    }

    /// `train_set_stop_wait(train, stop, mode)` (`0x4d5830`, command `0x1b`): the wait mode
    /// goes into bits 6..7 of the stop's flags (`+3`).
    pub fn train_set_stop_wait(&mut self, t: i32, k: i32, mode: i8) {
        let f = self.ts.stop8(t, k, 3);
        self.ts
            .set_stop8(t, k, 3, (f & 0x3f) | ((i32::from(mode) << 6) as u8));
    }

    /// `train_set_stop_sell(train, stop, mode)` (`0x4d58e0`, command `0x2f`): bits 4..5 of the
    /// stop's flags; with the option `0x570229` (sandbox) the mode becomes 3.
    pub fn train_set_stop_sell(&mut self, t: i32, k: i32, mode: u8) {
        let mut f = ((mode & 3) << 4) | (self.ts.stop8(t, k, 3) & 0xcf);
        if self.ts.flag_570229 != 0 {
            f |= 0x30;
        }
        self.ts.set_stop8(t, k, 3, f);
    }

    /// `0x4d4750`: the train is working at a station (stopped in state 2, 3, 4, 6 or 9) or
    /// has `+0x94` set.
    fn train_working(&self, t: i32) -> bool {
        let st = self.ts.u8(P::Train, t, 0x9a);
        (self.ts.u8(P::Train, t, 0x99) != 0 && matches!(st, 2 | 3 | 4 | 6 | 9))
            || self.ts.u8(P::Train, t, 0x94) != 0
    }

    /// Command `0x1c` (`cmd_execute` `0x4b0210`): the cargo of each car of stop `k` (every stop
    /// with `0xff`); a train working at a station whose current stop changed forgets its
    /// departure minute and runs `train_arrive` (`0x4d29e0`) again.
    pub fn train_stop_cargo(&mut self, t: i32, k: i32, cargo: [u8; 6]) {
        let working = self.train_working(t);
        let cur = self.ts.cur_stop(t);
        let before: Vec<u8> = (0..14).map(|o| self.ts.stop8(t, cur, o)).collect();
        let n = cargo.iter().filter(|&&c| c != 0xff).count() as u8;
        let stops = if k == 0xff {
            0..i32::from(self.ts.u8(P::Train, t, 0x3b))
        } else {
            k..k + 1
        };
        for s in stops {
            self.ts.set_stop8(t, s, 2, n);
            for (j, c) in cargo.iter().enumerate() {
                self.ts.set_stop8(t, s, 4 + j, *c);
            }
        }
        if working {
            let after: Vec<u8> = (0..14).map(|o| self.ts.stop8(t, cur, o)).collect();
            if before != after {
                self.ts.seti32(P::Train, t, 0x71, 0);
                self.ts.train_arrive(t);
            }
        }
    }

    /// `train_set_class(train, class)` (`0x4d59b0`, command `0x21`): the class goes to `+0xa7`
    /// and, when the train is fast enough (`+0xdb` above `0x537814`), to `+0xa8` (`+4` when the
    /// track ahead belongs to the company), else `+0xa8 = 0`. Not for a crashed train
    /// (`+0xf2`), a broken one (`+0xed`) or one under repair (`+0xbc`).
    pub fn train_set_class(&mut self, t: i32, class: i8) {
        let ts = &mut *self.ts;
        if ts.u8(P::Train, t, 0xf2) != 0
            || ts.u8(P::Train, t, 0xed) != 0
            || ts.u8(P::Train, t, 0xbc) != 0
        {
            return;
        }
        ts.set8(P::Train, t, 0xa7, class as u8);
        if ts.img.f32(0x0053_7814) < ts.f32(P::Train, t, 0xdb) {
            ts.set8(P::Train, t, 0xa8, class as u8);
            let (x, y) = (
                i32::from(ts.i16(P::Train, t, 0xcb)),
                i32::from(ts.i16(P::Train, t, 0xcd)),
            );
            if x > -1 && ts.u8(P::Train, t, 0x9c) != 0 && class != 0 {
                let p = ts.tile_track(x, y);
                let owner = if p == 0 {
                    0
                } else {
                    i32::from(ts.u8(P::Track, p, 2))
                };
                if owner == ts.i32(P::Train, t, 0x41) {
                    ts.set8(P::Train, t, 0xa8, (class as u8).wrapping_add(4));
                }
            }
        } else {
            ts.set8(P::Train, t, 0xa8, 0);
        }
    }

    /// `train_stop_delete(train, k)` (`0x4d5710` -> `0x4d55d0`, command `0x1e`).
    pub fn stop_delete(&mut self, t: i32, k: i32) {
        let n = i32::from(self.ts.u8(P::Train, t, 0x3b));
        if k > n || k < 0 || k >= n {
            return;
        }
        if k == self.ts.cur_stop(t) && n > 1 {
            self.ts.set_stop(t, (self.ts.cur_stop(t) + 1) % n, true);
        }
        if k < self.ts.cur_stop(t) {
            let c = self.ts.u8(P::Train, t, 0x3c).wrapping_sub(1);
            self.ts.set8(P::Train, t, 0x3c, c);
        }
        let mut v = self.stops(t);
        v.remove(k as usize);
        self.set_stops(t, &v);
        self.train_leg_length(t);
        if v.is_empty() {
            self.ts.set8(P::Train, t, 0x3c, 0xff);
            self.train_route_refresh(t);
            self.ts.reserve(t, 1, 1, 7);
        }
        if self.ts.flag_570229 != 0 {
            todo_hook!(
                self.ts.log,
                "stop_cargo_570229",
                "0x4d6330 cargo lists with the option 0x570229",
                "trains.md 10"
            );
        }
    }

    /// `train_replace_engine(train, engine, pay)` (`0x4d4e90`, command `0x20`, AI 7): writes
    /// off the old price, charges the new one to account 4 (and, with `pay`, account 0) and
    /// swaps the engine (and tender) cars at the head of the consist.
    pub fn replace_engine(&mut self, t: i32, e: i32, pay: bool) {
        self.ts.log.ran("train_replace_engine");
        let year = self.ts.year;
        self.ts.set16(P::Train, t, 0xbd, year);
        let company = self.ts.i32(P::Train, t, 0x41);
        let old = self.ts.i32(P::Train, t, 8);
        self.write_off(company, 4, -f64::from(old), 6);
        let kind = usize::from(self.ts.en8(e, 0x28)) + 0x1b;
        let terr = i32::from(self.ts.u8(P::Train, t, 0xf3));
        let m_kind = self.ts.effect(kind, company, 0, terr);
        let m_all = self.ts.effect(0x1a, company, 0, terr);
        let price =
            crate::fp::ftol(f64::from(m_all) * f64::from(m_kind) * f64::from(self.ts.en32(e, 0xc)));
        self.ts.seti32(P::Train, t, 8, price);
        account_move(self.ts, company, 4, f64::from(price));
        if pay {
            account_move(self.ts, company, 0, f64::from(price));
        }
        self.ts.set16(P::Train, t, 0xa0, 0);
        // A locomotive on the track hands its tile and route step to the new one (`0x414220`
        // with a position -> `0x413550` head branch); every car is lifted first (`0x413500`).
        let head = self.ts.i32(P::Train, t, 0);
        let at = (self.ts.u8(P::Car, head, 0x2a) != 0).then(|| {
            (
                self.ts.i16(P::Car, head, 9),
                self.ts.i16(P::Car, head, 0xb),
                self.ts.i16(P::Car, head, 0x11),
            )
        });
        let f9a = self.ts.u8(P::Train, t, 0x9a);
        self.ts.reserve(t, 1, 1, 1);
        let mut c = self.ts.i32(P::Train, t, 0);
        while c != 0 {
            self.ts.car_lift(c);
            c = i32::from(self.ts.i16(P::Car, c, 2));
        }
        self.ts.set_reserved_range(t, -1, -1);
        self.ts.set8(P::Train, t, 0x9c, 0);
        self.ts.set8(P::Train, t, 0xfb, f9a);
        loop {
            let c = self.ts.i32(P::Train, t, 0);
            if c == 0 || self.ts.vt8(self.ts.u8(P::Car, c, 8), 0x2e) > 1 {
                break;
            }
            self.ts.car_remove(t, c);
        }
        for o in [0xcf, 0xd3, 0xd7] {
            self.ts.setf32(P::Train, t, o, 100.0);
        }
        let first = self.ts.i32(P::Train, t, 0);
        let last = self.ts.i32(P::Train, t, 4);
        self.ts.seti32(P::Train, t, 0, 0);
        self.ts.seti32(P::Train, t, 4, 0);
        let vt = self.ts.en8(e, 0x2c);
        self.ts.vehicle_init_at(t, vt, at);
        if self.ts.en8(e, 0x2b) != 0 {
            let vt2 = self.ts.en8(e, 0x2d);
            self.ts.vehicle_init(t, vt2);
        }
        if first != 0 {
            let tail = self.ts.i32(P::Train, t, 4);
            self.ts.set16(P::Car, first, 0, tail as u16);
            self.ts.set16(P::Car, tail, 2, first as u16);
            if last != 0 {
                self.ts.seti32(P::Train, t, 4, last);
            }
        }
        self.ts.train_route_refresh(t);
        let head = self.ts.i32(P::Train, t, 0);
        let he = self.ts.vt_engine(self.ts.u8(P::Car, head, 8));
        let b = self.ts.u8(P::Train, t, 0xbb);
        let no_wire = self.ts.en8(he, 0x28) == 2
            && self.ts.u8(P::Train, t, 0x9c) != 0
            && self.ts.u8(P::Car, head, 0x2a) != 0
            && {
                let (x, y) = (
                    i32::from(self.ts.i16(P::Car, head, 9)),
                    i32::from(self.ts.i16(P::Car, head, 0xb)),
                );
                let p = self.ts.tile_track(x, y);
                p == 0 || self.ts.u8(P::Track, p, 0x23) & 0x10 == 0
            };
        self.ts
            .set8(P::Train, t, 0xbb, if no_wire { b | 8 } else { b & 0xf7 });
    }
}

impl Ts<'_> {
    /// `vehicle_init(car, id, train, type, -1, 0xffff, 0)` (`0x414220`) for a car that is not
    /// first in its train: appended to the car list and, behind another car, placed on the route.
    pub(crate) fn vehicle_init(&mut self, t: i32, vt: u8) {
        self.vehicle_init_at(t, vt, None);
    }

    /// `0x413500(car)`: the car leaves the track (`+0x2a = 0`) and, unless it is hidden
    /// (`+0x45`), its tile structure.
    pub(crate) fn car_lift(&mut self, c: i32) {
        if self.u8(P::Car, c, 0x2a) == 0 {
            return;
        }
        self.set8(P::Car, c, 0x2a, 0);
        if self.u16(P::Car, c, 0x45) == 0 {
            let (x, y) = (
                i32::from(self.i16(P::Car, c, 9)),
                i32::from(self.i16(P::Car, c, 0xb)),
            );
            crate::tilemark::struct_remove(
                self.s,
                x,
                y,
                2,
                i32::from(self.i16(P::Car, c, 6)) as u32,
            );
        }
    }

    /// [`Self::vehicle_init`] with the position argument of `0x414220`: the first car of an
    /// empty train is placed at `at` (`0x413550` head branch) instead of staying off the track.
    pub(crate) fn vehicle_init_at(&mut self, t: i32, vt: u8, at: Option<(i16, i16, i16)>) {
        let size = self.record_size(P::Car);
        let c = self.alloc(P::Car, vec![0u8; size]);
        // The slot's memory keeps the bytes of the car freed there before (`pool_free` does not
        // clear it); the fields set below overwrite most of them.
        if let Some(old) = self.g.freed_cars.remove(&c) {
            let mut rec = self.record(P::Car, c);
            let n = rec.len().min(old.len());
            rec[..n].copy_from_slice(&old[..n]);
            self.set_record(P::Car, c, &rec);
        }
        let kind = self.vt8(vt, 0x2e);
        let set8 = |ai: &mut Ts<'_>, o: usize, v: u8| ai.set8(P::Car, c, o, v);
        let set16 = |ai: &mut Ts<'_>, o: usize, v: u16| ai.set16(P::Car, c, o, v);
        set8(self, 8, vt);
        set16(self, 6, c as u16);
        set8(self, 0x47, u8::from(kind > 1));
        set16(self, 4, t as u16);
        self.seti32(P::Car, c, 0x41, 0);
        set8(self, 0x2a, 0);
        self.seti32(P::Car, c, 0x2d, 0);
        for o in [0xd, 0xf, 0x48, 0x4a] {
            set16(self, o, 0xffff);
        }
        set16(self, 0x45, 0);
        self.seti32(P::Car, c, 0x3d, 0);
        set16(self, 0x4c, 0);
        set8(self, 0x4e, 0);
        self.seti32(P::Car, c, 0x39, 0);
        self.seti32(P::Car, c, 0x34, 0);
        set16(self, 0x32, 0);
        let last = self.i32(P::Train, t, 4);
        set16(self, 0, last as u16);
        set16(self, 2, 0);
        // `+0x2b`: the run-time vehicle serial `0x56506c` (not in the save; 0 after a load).
        let serial = self.g.vehicle_serial;
        self.g.vehicle_serial = serial.wrapping_add(1);
        set16(self, 0x2b, serial as u16);
        set8(self, 0x31, 0);
        if last == 0 {
            self.seti32(P::Train, t, 0, c);
        } else {
            self.set16(P::Car, last, 2, c as u16);
        }
        self.seti32(P::Train, t, 4, c);
        // A car appended behind others goes through `vehicle_place` (`0x413550`, non-head
        // branch) right away: on a train standing on the track it is put on the route behind the
        // car ahead (first seen at day 163 of the Example: a consist change at a stop). The
        // first car of an empty train is skipped (`param_4 == -1`).
        if last != 0 {
            self.place_trailing_car(c);
        } else if at.is_some() {
            self.place_head_car_at(t, at);
        }
    }

    /// `0x4d0fa0(train, car)` + `0x466df0`: unlinks and frees a car (off its tile first).
    pub(crate) fn car_remove(&mut self, t: i32, c: i32) {
        let prev = i32::from(self.i16(P::Car, c, 0));
        let next = i32::from(self.i16(P::Car, c, 2));
        if next != 0 {
            self.set16(P::Car, next, 0, prev as u16);
        }
        if prev != 0 {
            self.set16(P::Car, prev, 2, next as u16);
        }
        if self.i32(P::Train, t, 0) == c {
            self.seti32(P::Train, t, 0, next);
        }
        if self.i32(P::Train, t, 4) == c {
            self.seti32(P::Train, t, 4, prev);
        }
        if self.u8(P::Car, c, 0x2a) != 0 {
            let (x, y) = (
                i32::from(self.i16(P::Car, c, 9)),
                i32::from(self.i16(P::Car, c, 0xb)),
            );
            crate::tilemark::struct_remove(self.s, x, y, 2, c as u32);
            self.set8(P::Car, c, 0x2a, 0);
        }
        // The car's lot (`0x4d0fa0`): back to its industry (`0x4bf450`) when the train works
        // at the lot's own station (stopped in state 2, 3, 4, 6 or 9, or `+0x94`), freed
        // (`0x466a60`) otherwise.
        let lot = self.i32(P::Car, c, 0x2d);
        if lot != 0 {
            let st = self.u8(P::Train, t, 0x9a);
            let working = (self.u8(P::Train, t, 0x99) != 0 && matches!(st, 2 | 3 | 4 | 6 | 9))
                || self.u8(P::Train, t, 0x94) != 0;
            if working && self.u16(P::Train, t, 0x9e) == self.u16(P::Lot, lot, 3) {
                self.lot_back_to_industry(lot);
            } else {
                self.lot_free(lot);
            }
        }
        self.free(P::Car, c);
    }
}
