//! Station networks and the train refresh after a track change: `station_networks_rebuild`
//! (`0x46a370`, terrain-track.md "Redes de estações"), `trains_refresh_all` (`0x4d3640` ->
//! `train_route_refresh` `0x4d14b0`) and `trains_flag_route_check` (`0x4d3600`).

use crate::ai::Ai;
use crate::trainstep::P;

/// State-block offsets of the active company list (`0x570729` count, `0x57072d` u16 ids).
const ST_CO_COUNT: usize = 0x241;
const ST_CO_LIST: usize = 0x245;

impl Ai<'_, '_> {
    /// `station_networks_rebuild` (`0x46a370`): numbers the networks (`station+0x57`) by
    /// connectivity in list order, counts the companies that may use each station (`+0x4b1`),
    /// pays the connection bonus of the towns now linked to another town (world-growth.md 6.2),
    /// and sets each company's main network (`+0x17a`) and its size (`+0x1aa`). Ends with the
    /// scenario trigger 4, queued in [`Ai::events`].
    pub fn station_networks_rebuild(&mut self) {
        self.ts.log.ran("station_networks_rebuild");
        let mut counts = [0i32; 100];
        let mut net: u16 = 1;
        let ids = self.ids(P::Station);
        for &s in &ids {
            self.ts.set16(P::Station, s, 0x57, 0);
        }
        for (i, &s) in ids.iter().enumerate() {
            if self.ts.u16(P::Station, s, 0x57) != 0 || self.ts.i32(P::Station, s, 0x45) != 0 {
                continue;
            }
            self.ts.set16(P::Station, s, 0x57, net);
            counts[usize::from(net) % 100] += 1;
            let (sx, sy) = (
                self.ts.i32(P::Station, s, 0xa),
                self.ts.i32(P::Station, s, 0xe),
            );
            for &o in &ids[i + 1..] {
                if self.ts.u16(P::Station, o, 0x57) != 0 {
                    continue;
                }
                let (ox, oy) = (
                    self.ts.i32(P::Station, o, 0xa),
                    self.ts.i32(P::Station, o, 0xe),
                );
                if self.route_exists(sx, sy, ox, oy, 0) {
                    let n = self.ts.u16(P::Station, s, 0x57);
                    self.ts.set16(P::Station, o, 0x57, n);
                    counts[usize::from(n) % 100] += 1;
                }
            }
            net = net.wrapping_add(1);
        }
        let nco = self.ts.state_i32(ST_CO_COUNT).max(0) as usize;
        let cos: Vec<i32> = (0..nco)
            .map(|k| {
                let o = ST_CO_LIST + 2 * k;
                i32::from(u16::from_le_bytes([
                    self.ts.s.state[o],
                    self.ts.s.state[o + 1],
                ]))
            })
            .collect();
        for &s in &ids {
            self.ts.seti32(P::Station, s, 0x4b1, 0);
            if self.ts.i32(P::Station, s, 0x45) == 0 {
                let mut n = 0;
                for &c in &cos {
                    let id = self.ts.i32(P::Company, c, 0);
                    if self.station_usable(s, id) {
                        n += 1;
                    }
                }
                self.ts.seti32(P::Station, s, 0x4b1, n.max(1));
            }
        }
        for (d, &s) in ids.iter().enumerate() {
            let n = self.ts.u16(P::Station, s, 0x57);
            if counts[usize::from(n) % 100] <= 1 || self.ts.i32(P::Station, s, 0x45) != 0 {
                continue;
            }
            let bld = self.ts.i32(P::Station, s, 4);
            let place = i32::from(self.ts.u16(P::Industry, bld, 2));
            if place == 0 {
                continue;
            }
            let bonus = self.ts.i32(P::Place, place, 0x36);
            if bonus == 0 {
                continue;
            }
            for (j, &o) in ids.iter().enumerate() {
                if j == d || self.ts.u16(P::Station, o, 0x57) != n {
                    continue;
                }
                let ob = self.ts.i32(P::Station, o, 4);
                let op = i32::from(self.ts.u16(P::Industry, ob, 2));
                if op == 0 || op == place || self.ts.u8(P::Place, op, 0x10) != 0 {
                    continue;
                }
                let co = self.ts.i32(P::Station, s, 0x39);
                super::ledger_credit(self.ts, co, 4, f64::from(bonus));
                self.ts.seti32(P::Place, place, 0x36, 0);
                break;
            }
        }
        for c in self.ids(P::Company) {
            let main = self.ts.i32(P::Company, c, 0x176);
            if main == 0 {
                self.ts.seti32(P::Company, c, 0x1aa, 0);
                self.ts.seti32(P::Company, c, 0x17a, 0);
            } else {
                let n = self.ts.u16(P::Station, main, 0x57);
                self.ts.seti32(P::Company, c, 0x17a, i32::from(n));
                self.ts
                    .seti32(P::Company, c, 0x1aa, counts[usize::from(n) % 100]);
            }
        }
        self.events.push(4);
    }

    /// `0x4d3640`: [`Ai::train_route_refresh`] for every train (pattern A), then
    /// `0x574b84 = 0`.
    pub fn trains_refresh_all(&mut self) {
        for t in self.ids(P::Train) {
            self.train_route_refresh(t);
        }
        self.ts.g.refresh_574b84 = 0;
    }

    /// `0x4d3600`: every train re-checks its route at its next step (`+0x93 = 1`).
    pub fn trains_flag_route_check(&mut self) {
        for t in self.ids(P::Train) {
            self.ts.set8(P::Train, t, 0x93, 1);
        }
    }

    /// [`Ts::train_route_refresh`].
    pub fn train_route_refresh(&mut self, t: i32) {
        self.ts.train_route_refresh(t);
    }
}

impl crate::trainstep::Ts<'_> {
    /// `train_route_refresh` (`0x4d14b0`): drops waypoint stops left without track, realigns
    /// the route ring (`+0x5d` index, `+0x61` count) to the head position `+0x95`, and sets the
    /// "no catenary" bit `+0xbb & 8` of an electric train.
    pub fn train_route_refresh(&mut self, t: i32) {
        if self.i32(P::Train, t, 0x65) == 0 || self.i32(P::Train, t, 0x61) < 1 {
            return;
        }
        self.set8(P::Train, t, 0x92, 0);
        let car = self.i32(P::Train, t, 0);
        let on_track = self.exists(P::Car, car) && self.u8(P::Car, car, 0x2a) != 0;
        if !on_track {
            todo_hook!(
                self.log,
                "train_route_free",
                "0x4d14b0: free the route of a train that is off the track (0x4627f0)",
                "trains.md 10.7"
            );
        } else {
            let nstops = i32::from(self.u8(P::Train, t, 0x3b));
            for k in (0..nstops).rev() {
                if self.stop8(t, k, 3) & 8 != 0 {
                    let sx = i32::from(self.stop16(t, k, 10) as i16);
                    let sy = i32::from(self.stop16(t, k, 12) as i16);
                    if self.tile_track(sx, sy) == 0 {
                        todo_hook!(
                            self.log,
                            "train_stop_delete",
                            "0x4d14b0: delete a waypoint stop left without track (0x4d55d0)",
                            "trains.md 10.7"
                        );
                    }
                }
            }
            let head = i32::from(self.i16(P::Train, t, 0x95));
            let idx = self.i32(P::Train, t, 0x5d);
            let size = self.i32(P::Train, t, 0x59);
            let adj = if idx < head { size - head } else { -head };
            self.seti32(P::Train, t, 0x5d, head);
            let cnt = self.i32(P::Train, t, 0x61);
            let v = (size - (idx + adj) + cnt) % size;
            self.seti32(P::Train, t, 0x61, v);
        }
        let car = self.i32(P::Train, t, 0);
        let vt = self.u8(P::Car, car, 8);
        let e = self.vt_engine(vt);
        let b = self.u8(P::Train, t, 0xbb);
        if self.en8(e, 0x28) == 2
            && self.u8(P::Train, t, 0x9c) != 0
            && self.u8(P::Car, car, 0x2a) != 0
        {
            let (cx, cy) = (
                i32::from(self.i16(P::Car, car, 9)),
                i32::from(self.i16(P::Car, car, 0xb)),
            );
            let p = self.tile_track(cx, cy);
            if p != 0 && self.u8(P::Track, p, 0x23) & 0x10 != 0 {
                self.set8(P::Train, t, 0xbb, b & 0xf7);
            } else {
                self.set8(P::Train, t, 0xbb, b | 8);
            }
        } else {
            self.set8(P::Train, t, 0xbb, b & 0xf7);
        }
    }
}
