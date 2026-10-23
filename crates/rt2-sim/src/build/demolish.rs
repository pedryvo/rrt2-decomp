//! Station removal (docs/spec/terrain-track.md 3.4, docs/spec/scenarios.md 4.2.1):
//! `station_remove` (`0x4bf4e0`, called by `building_destroy(building, 1)` `0x40adb0` for a
//! station building) and the map-wide structure removal `tile_struct_remove_all` (`0x42e410`).
//!
//! The building part of `0x40adb0` (place list, tiles, flags) is [`SimState::building_destroy`]
//! (`tick/town.rs`), which calls [`Ai::station_remove`] for a building with a station (`+8`).
//!
//! [`SimState::building_destroy`]: crate::tick::SimState

use crate::ai::Ai;
use crate::trainstep::P;

use super::{account_move, ledger_credit};

impl Ai<'_, '_> {
    /// `0x42e410(kind, id)`: row by row, the first `(kind, id)` structure of each tile's chain
    /// is unlinked and freed; on a tile that lost one, `tile+2 & 0x10` is recomputed (set while
    /// a kind-3 structure is left in the chain).
    pub fn tile_struct_remove_all(&mut self, kind: u8, id: u32) {
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        for y in 0..h {
            for x in 0..w {
                let mut link: Option<i32> = None;
                let mut cur = i32::from(self.ts.t16(x, y, 4));
                let mut hit = 0;
                while cur != 0 {
                    if self.ts.u32(P::Struct, cur, 1) == id && self.ts.st_type(cur) == kind {
                        hit = cur;
                        break;
                    }
                    link = Some(cur);
                    cur = self.ts.st_next(cur);
                }
                if hit == 0 {
                    continue;
                }
                let nx = self.ts.st_next(hit);
                self.ts.free(P::Struct, hit);
                match link {
                    None => self.ts.set_t16(x, y, 4, nx as u16),
                    Some(s) => self.ts.set16(P::Struct, s, 5, nx as u16),
                }
                let mut c = i32::from(self.ts.t16(x, y, 4));
                let mut building = false;
                while c != 0 {
                    if self.ts.st_type(c) == crate::trainstep::anim::STRUCT_BUILDING {
                        building = true;
                        break;
                    }
                    c = self.ts.st_next(c);
                }
                let f = self.ts.t8(x, y, 2);
                self.ts
                    .set_t8(x, y, 2, if building { f | 0x10 } else { f & 0xef });
            }
        }
    }

    /// `station_remove(station)` (`0x4bf4e0`). A station already removed (`+0x45`) is left
    /// alone. A real station (building `+0x2e == 0`) with an owner takes one off the company's
    /// station count (`+0x172`) and writes the building's value (`+0x99`) off account 1 and
    /// into category 5 (no refund); the catchment links go (`0x406f60`) and the networks are
    /// renumbered (`0x46a370`). Then the station structures leave the map (`0x42e410(4, id)`),
    /// the station tile loses `tile+6 & 2`, the station is marked removed (`+0x45 = 1`; the
    /// record stays), a company whose main station (`+0x176`) it was gets another of its
    /// stations, every train loses its stops at the station (`0x4d5710`), and the networks are
    /// renumbered again. (The detail window of the station, `0x466230`, and the station list
    /// `0x498dd0` are interface only.)
    ///
    /// Two quirks of the original are kept: the main-station search reuses the company loop's
    /// index (the company loop resumes after the index of the station found, or after the
    /// station count), and once a train lost a stop, every lower stop index refreshes the
    /// train again (`ai_replan_train` `0x4d8f40` for a computer company, then
    /// `train_route_refresh` `0x4d14b0`).
    pub fn station_remove(&mut self, st: i32) {
        if self.ts.i32(P::Station, st, 0x45) != 0 {
            return;
        }
        self.ts.log.ran("station_remove");
        let b = self.ts.i32(P::Station, st, 4);
        if self.ts.u8(P::Industry, b, 0x2e) == 0 {
            let co = self.ts.i32(P::Station, st, 0x39);
            if co != 0 {
                let n = self.ts.i32(P::Company, co, 0x172);
                self.ts.seti32(P::Company, co, 0x172, n - 1);
                let v = -f64::from(self.ts.i32(P::Industry, b, 0x99));
                account_move(self.ts, co, 1, v);
                ledger_credit(self.ts, co, 5, v);
            }
            crate::convert::station_unlink(self.ts, st);
            self.station_networks_rebuild();
        }
        self.tile_struct_remove_all(crate::trainstep::anim::STRUCT_STATION, st as u32);
        let (sx, sy) = (
            self.ts.i32(P::Station, st, 0xa),
            self.ts.i32(P::Station, st, 0xe),
        );
        let f = self.ts.t8(sx, sy, 6);
        self.ts.set_t8(sx, sy, 6, f & 0xfd);
        self.ts.seti32(P::Station, st, 0x45, 1);
        let cos = self.ids(P::Company);
        let sts = self.ids(P::Station);
        let mut i = 0usize;
        while i < cos.len() {
            let c = cos[i];
            if self.ts.i32(P::Company, c, 0x176) == st {
                self.ts.seti32(P::Company, c, 0x176, 0);
                let cid = self.ts.i32(P::Company, c, 0);
                let mut j = 0usize;
                while j < sts.len() {
                    let s = sts[j];
                    if self.ts.exists(P::Station, s)
                        && self.ts.i32(P::Station, s, 0x45) == 0
                        && self.ts.i32(P::Station, s, 0x39) == cid
                        && s != st
                    {
                        self.ts.seti32(P::Company, c, 0x176, s);
                        break;
                    }
                    j += 1;
                }
                i = j;
            }
            i += 1;
        }
        for t in self.ids(P::Train) {
            let mut found = false;
            let n = i32::from(self.ts.u8(P::Train, t, 0x3b));
            for k in (0..n).rev() {
                let live = k < i32::from(self.ts.u8(P::Train, t, 0x3b));
                let hit = live
                    && i32::from(self.ts.stop16(t, k, 0)) == st
                    && self.ts.stop8(t, k, 3) & 8 == 0;
                if hit {
                    self.stop_delete(t, k);
                    found = true;
                }
                if found {
                    let co = self.ts.i32(P::Train, t, 0x41);
                    if !self.ts.company_is_human(co) {
                        let id = self.ts.i32(P::Train, t, 0x3d);
                        let saved = self.company;
                        if saved < 1 {
                            self.company = co;
                        }
                        self.replan_train(id);
                        self.company = saved;
                    }
                    self.ts.train_route_refresh(t);
                }
            }
        }
        self.station_networks_rebuild();
    }
}

impl Ai<'_, '_> {
    /// Pass 0 of the water fill (`0x490490` mode 3) on a land tile: for the tile and each
    /// neighbour its piece connects to (`piece+0` bits, `0x537b48`/`0x537b68`), the trains
    /// holding a reservation on the piece (`piece+3` -> `pool_80+1`) are deleted.
    pub fn flood_delete_trains(&mut self, x: i32, y: i32) {
        let p = self.piece(x, y);
        if p == 0 {
            return;
        }
        let mask = self.ts.u8(P::Track, p, 0);
        if mask == 0 {
            return;
        }
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        for k in -1i32..8 {
            let (tx, ty) = if k == -1 {
                (x, y)
            } else {
                if mask & (1 << k) == 0 {
                    continue;
                }
                let (nx, ny) = (x + super::DX[k as usize], y + super::DY[k as usize]);
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                (nx, ny)
            };
            loop {
                let q = self.piece(tx, ty);
                if q == 0 {
                    break;
                }
                let r = i32::from(self.ts.u16(P::Track, q, 3));
                if r == 0 {
                    break;
                }
                let t = i32::from(self.ts.u16(P::P80, r, 1));
                self.train_delete(t);
            }
        }
    }
}
