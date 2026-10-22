//! Train management of the AI (ai.md section 7): `ai_manage_trains` (`0x4d9210`),
//! `ai_buy_train` (`0x4d90f0`), `ai_replan_train` (`0x4d8f40`) and the stop and consist choice
//! `ai_pick_stop` (`0x4d87e0`).

use super::path::Req;
use super::{Ai, CO_ENGINE_YEAR, CO_REPLAN_YEAR, CO_STATIONS, CO_TRAINS};
use crate::fp::ftol;
use crate::trainstep::P;

/// A stop as built by `ai_pick_stop` (14 bytes: u16 station, u8 cars, u8 flags, cargo list).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stop(pub [u8; 14]);

impl Stop {
    pub fn station(&self) -> i32 {
        i32::from(u16::from_le_bytes([self.0[0], self.0[1]]))
    }
    pub fn is_waypoint(&self) -> bool {
        self.0[3] & 8 != 0
    }
}

impl Ai<'_, '_> {
    fn sta_f32(&self, st: i32, o: usize) -> f32 {
        self.ts.f32(P::Station, st, o)
    }

    /// `ai_manage_trains` (`0x4d9210`, ai.md 7).
    pub fn manage_trains(&mut self) {
        self.ts.log.ran("ai_manage_trains");
        let company = self.company;
        loop {
            let mut own = 0f32;
            let mut foreign = 0f32;
            let mut served = 0f32;
            for st in self.ids(P::Station) {
                if !self.station_usable(st, company) {
                    continue;
                }
                let mine = self.ts.i32(P::Station, st, 0x39) == company;
                let share = self.ts.i32(P::Station, st, 0x4b1);
                for c in 0..44usize {
                    self.ts.setf32(P::Station, st, 0x3fd + 4 * c, 0.0);
                    let sup = f64::from(self.ts.i16(P::Station, st, 0x61 + 2 * c));
                    let dem = f64::from(self.sta_f32(st, 0xb9 + 4 * c));
                    let pot = ((sup * 1.5 + dem) * 0.5) as f32;
                    self.ts.setf32(P::Station, st, 0x34d + 4 * c, pot);
                    if mine {
                        own = (f64::from(pot) + f64::from(own)) as f32;
                    } else if share > 0 {
                        foreign =
                            (0.7 / f64::from(share) * f64::from(pot) + f64::from(foreign)) as f32;
                    }
                }
            }
            for t in self.ids(P::Train) {
                if self.ts.i32(P::Train, t, 0x41) != company || self.ts.u8(P::Train, t, 0x3b) <= 1 {
                    continue;
                }
                for o in [0xcf, 0xd3, 0xd7] {
                    self.ts.setf32(P::Train, t, o, 100.0);
                }
                self.ts.setf32(P::Train, t, 0xdb, 85.0);
                if self.ts.f32(P::Train, t, 0xe9) == 0.0 {
                    self.ts.setf32(P::Train, t, 0xe9, 2.0);
                }
                let w = (1.0 / f64::from(self.ts.f32(P::Train, t, 0xe9))) as f32;
                let n = i32::from(self.ts.u8(P::Train, t, 0x3b));
                for k in 0..n {
                    if self.ts.stop8(t, k, 3) & 8 != 0 {
                        continue;
                    }
                    let st = i32::from(self.ts.stop16(t, k, 0));
                    let mut cnt = 0i32;
                    let ncar = i32::from(self.ts.stop8(t, k, 2));
                    for i in 0..ncar {
                        let c = usize::from(self.ts.stop8(t, k, 4 + i as usize));
                        if c < 0x2c {
                            cnt += 1;
                            let o = 0x3fd + 4 * c;
                            let v = (f64::from(w) + f64::from(self.sta_f32(st, o))) as f32;
                            self.ts.setf32(P::Station, st, o, v);
                        }
                    }
                    served = (f64::from(cnt) * f64::from(w) + f64::from(served)) as f32;
                }
            }
            if own <= 0.0 {
                return;
            }
            let q = f64::from(served) / f64::from(own);
            self.ratio = q as f32;
            if q < 0.5 {
                self.ratio = 0.5;
            }
            // X = min(own * 1.15, f32(own + foreign)).
            let own115 = f64::from(own) * 1.15;
            let sum = f64::from(own) + f64::from(foreign);
            let x = if own115 < sum {
                own115
            } else {
                f64::from(sum as f32)
            };
            let cells = self.co_i32(0x186) + 2 * self.co_i32(0x18a);
            let m = f64::from((cells / 5) as f32);
            let mut a = (x + 2.0) * f64::from(0.2f32);
            if a > m {
                a = m;
            }
            let year = self.ts.year.min(0x834) as i32;
            let target = ftol(220.0 / f64::from(year - 1650) * a + 0.5)
                .max(2)
                .max(self.co_i32(CO_STATIONS));
            if self.perm(0x0057_06b5) && self.co_i32(CO_TRAINS) < target && self.buy_train() {
                continue;
            }
            let year = self.year();
            if year as u16 != self.ts.u16(P::Company, company, CO_REPLAN_YEAR) {
                let pick = self.ids(P::Train).into_iter().find(|&t| {
                    self.ts.i32(P::Train, t, 0x41) == company
                        && year - i32::from(self.ts.u16(P::Train, t, 0xdf)) > 6
                });
                if let Some(t) = pick {
                    self.ts.set16(P::Train, t, 0xdf, year as u16);
                    // The original loops back to the top whatever the result.
                    self.replan_train(t);
                    continue;
                }
                self.ts
                    .set16(P::Company, company, CO_REPLAN_YEAR, year as u16);
            }
            if year as u16 != self.ts.u16(P::Company, company, CO_ENGINE_YEAR) {
                self.ts
                    .set16(P::Company, company, CO_ENGINE_YEAR, year as u16);
                for t in self.ids(P::Train) {
                    if self.ts.i32(P::Train, t, 0x41) == company
                        && year - i32::from(self.ts.u16(P::Train, t, 0xa9)) > 25
                        && self.cash() >= -100_000.0
                    {
                        let _e = self.pick_engine(1);
                        todo_hook!(
                            self.ts.log,
                            "train_replace_engine",
                            "train_replace_engine 0x4d4e90 (new engine on an old train)",
                            "ai.md 7"
                        );
                    }
                }
            }
            self.ratio = 0.0;
            return;
        }
    }

    /// `ai_buy_train` (`0x4d90f0`). The purchase itself (`train_buy` `0x4cdf00` ->
    /// `train_create` `0x4cdf80`) is not ported: after the two stop choices and the engine choice
    /// (the RNG draws of the original) the turn stops buying.
    pub fn buy_train(&mut self) -> bool {
        let Some(a) = self.pick_stop(None, 0) else {
            return false;
        };
        let Some(_b) = self.pick_stop(Some(a), 0) else {
            return false;
        };
        let _e = self.pick_engine(1);
        todo_hook!(
            self.ts.log,
            "ai_train_buy",
            "train_buy 0x4cdf00 / train_create 0x4cdf80 and the stop list (0x4d5710, 0x4d11f0)",
            "ai.md 7; trains.md 6.1"
        );
        false
    }

    /// `ai_replan_train` (`0x4d8f40`): two new stops for an old train. The stop list rewrite
    /// (`0x4d5710`, `0x4d11f0`) is not ported; returns whether the original would have rewritten
    /// the stops (and looped).
    pub fn replan_train(&mut self, t: i32) -> bool {
        if self.ts.u8(P::Train, t, 0x9c) == 0 {
            return false;
        }
        let mode = self.ts.u8(P::Train, t, 0x3d);
        let Some(a) = self.pick_stop(None, mode) else {
            return false;
        };
        let Some(_b) = self.pick_stop(Some(a), mode) else {
            return false;
        };
        let (sx, sy) = if a.is_waypoint() {
            (
                i32::from(i16::from_le_bytes([a.0[10], a.0[11]])),
                i32::from(i16::from_le_bytes([a.0[12], a.0[13]])),
            )
        } else {
            let st = a.station();
            (
                self.ts.i32(P::Station, st, 0xa),
                self.ts.i32(P::Station, st, 0xe),
            )
        };
        let car = self.ts.i32(P::Train, t, 0);
        let cx = i32::from(self.ts.i16(P::Car, car, 9));
        let cy = i32::from(self.ts.i16(P::Car, car, 0xb));
        if !self.route_exists(cx, cy, sx, sy, 0) {
            return false;
        }
        todo_hook!(
            self.ts.log,
            "ai_replan_stops",
            "ai_replan_train 0x4d8f40: delete the stops (0x4d5710) and insert two (0x4d11f0), class 2/3",
            "ai.md 7"
        );
        false
    }

    /// `track_route_exists` (`0x460370`): route-mode search with no company; with `count` set
    /// (`0x566a5c`), counts the path tiles and the foreign ones.
    pub fn route_exists(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, count: i32) -> bool {
        let mut r = Req::route(x1, y1, x2, y2);
        r.count_owner = count;
        self.pathfind(&r).ok
    }

    /// Value of cargo `c` at a station for the stop choice: `(ratio - served/offered) * offered`
    /// (computed in the x87 registers), or `None` when not positive.
    fn stop_value(&self, st: i32, c: usize) -> Option<f64> {
        let pot = self.sta_f32(st, 0x34d + 4 * c);
        if f64::from(pot) <= 0.0 {
            return None;
        }
        let srv = f64::from(self.sta_f32(st, 0x3fd + 4 * c));
        let u = (f64::from(self.ratio) - srv / f64::from(pot)) * f64::from(pot);
        (u > 0.0).then_some(u)
    }

    /// `ai_pick_stop(&out, prev, mode)` (`0x4d87e0`, ai.md 7.1).
    pub fn pick_stop(&mut self, prev: Option<Stop>, mode: u8) -> Option<Stop> {
        self.ts.log.ran("ai_pick_stop");
        let company = self.company;
        let prev_st = prev.filter(|p| !p.is_waypoint()).map(|p| p.station());
        let mut d = 0i32;
        let mut best_score = 0f32;
        let mut best = -1i32;
        loop {
            for st in self.ids(P::Station) {
                if prev.is_some_and(|p| i32::from(u16::from_le_bytes([p.0[0], p.0[1]])) == st) {
                    continue;
                }
                if !self.station_usable(st, company) {
                    continue;
                }
                if prev.is_none() && self.ts.i32(P::Station, st, 0x39) != company {
                    continue;
                }
                // net_min_peer_version() >= 0x232d in a local game.
                if self.ts.i32(P::Station, st, 0x45) != 0 {
                    continue;
                }
                let (sx, sy) = (
                    self.ts.i32(P::Station, st, 0xa),
                    self.ts.i32(P::Station, st, 0xe),
                );
                if let Some(p) = prev {
                    let ps = prev_st.unwrap_or(0);
                    let (px, py) = (
                        self.ts.i32(P::Station, ps, 0xa),
                        self.ts.i32(P::Station, ps, 0xe),
                    );
                    d = Ai::dist(px, py, sx, sy);
                    let c0 = p.0[4];
                    if c0 != 1 && c0 != 0 && d > 0x28 {
                        continue;
                    }
                }
                let mut sum = 0f64;
                for c in 0..0x2c {
                    let Some(u) = self.stop_value(st, c) else {
                        continue;
                    };
                    if d == 0 {
                        sum += u;
                    } else if c == 0 || c == 1 {
                        sum += f64::from(d + 0x3c) * u * 0.0125;
                    } else {
                        sum += u / (f64::from(d + 0x14) * 0.025);
                    }
                }
                let mut s32 = sum as f32;
                if self.ts.i32(P::Station, st, 0x39) != company {
                    s32 = if mode & 3 == 1 {
                        (sum * 0.4) as f32
                    } else {
                        0.0
                    };
                }
                let r = self.ts.rng.range(0x4b, 0x7d);
                let score = f64::from(r) * f64::from(s32) * 0.01;
                if score > f64::from(best_score) {
                    if let Some(ps) = prev_st {
                        let (px, py) = (
                            self.ts.i32(P::Station, ps, 0xa),
                            self.ts.i32(P::Station, ps, 0xe),
                        );
                        if !self.route_exists(sx, sy, px, py, company) {
                            continue;
                        }
                        let (f, t) = (self.route_foreign, self.route_total);
                        if f > (t >> 3) + (t >> 2) || t > d * 2 {
                            continue;
                        }
                    }
                    best = st;
                    best_score = score as f32;
                }
            }
            if best > 0 {
                break;
            }
            if f64::from(self.ratio) >= 5.0 {
                return None;
            }
            self.ratio = (f64::from(self.ratio) + 1.0) as f32;
        }
        // Consist.
        let year = self.ts.year;
        let cars = if year < 0x72b {
            2
        } else if year < 0x73a {
            3
        } else {
            4
        };
        let st = best;
        let d;
        if let Some(ps) = prev_st {
            let (px, py) = (
                self.ts.i32(P::Station, ps, 0xa),
                self.ts.i32(P::Station, ps, 0xe),
            );
            let (sx, sy) = (
                self.ts.i32(P::Station, st, 0xa),
                self.ts.i32(P::Station, st, 0xe),
            );
            d = Ai::dist(px, py, sx, sy);
            if !self.route_exists(px, py, sx, sy, 0) {
                return None;
            }
        } else {
            d = 0;
        }
        let mut vals = [0f32; 4];
        let mut kinds = [0i32; 4];
        let mut cnt = 0usize;
        for c in 0..0x2cusize {
            let Some(u) = self.stop_value(st, c) else {
                continue;
            };
            let mut v = u as f32;
            if d != 0 {
                v = if c == 0 || c == 1 {
                    (f64::from(d + 0x14) * f64::from(v) * 0.025) as f32
                } else {
                    (f64::from(v) / (f64::from(d + 0x14) * 0.025)) as f32
                };
            }
            let mut i = 0usize;
            let mut inserted = false;
            while i < cnt {
                if v > vals[i] && i < 3 {
                    let m = cnt.min(3);
                    vals.copy_within(i..m, i + 1);
                    kinds.copy_within(i..m, i + 1);
                    inserted = true;
                    break;
                }
                i += 1;
            }
            if !inserted && i >= 4 {
                continue;
            }
            vals[i] = v;
            kinds[i] = c as i32;
            cnt = (cnt + 1).min(4);
        }
        let mut n = [cars, 0, 0, 0];
        for i in 1..cnt {
            for _ in 0..2 {
                for j in 0..i {
                    if n[j] > n[i] {
                        let a = f64::from((f64::from(vals[j]) / f64::from(n[j] + 1)) as f32);
                        let b = f64::from((f64::from(vals[i]) / f64::from(n[i] + 1)) as f32);
                        let c = f64::from(vals[j]) / f64::from(n[j]);
                        let dd = f64::from((f64::from(vals[i]) / f64::from(n[i] + 2)) as f32);
                        let m1 = if c > dd { c } else { dd };
                        let m2 = if a > b { a } else { b };
                        if m1 < m2 {
                            n[i] += 1;
                            n[j] -= 1;
                        }
                    }
                }
            }
        }
        if n[1] == cars {
            n[1] = 2;
        } else if n[1] + n[0] == cars && cars == 4 {
            n[0] -= 1;
        }
        let mut out = Stop::default();
        out.0[0..2].copy_from_slice(&(st as u16).to_le_bytes());
        for k in 0..cnt {
            for _ in 0..n[k].max(0) {
                let i = usize::from(out.0[2]);
                if 4 + i < 14 {
                    out.0[4 + i] = kinds[k] as u8;
                }
                out.0[2] = out.0[2].wrapping_add(1);
            }
        }
        Some(out)
    }
}
