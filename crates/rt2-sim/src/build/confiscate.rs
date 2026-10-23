//! Territory confiscation and destruction (scenario effects 55..59 and 299..303,
//! docs/spec/scenarios.md 4.2.1): the track (`0x437d40` -> `0x437ab0`), the trains (`0x437db0`),
//! the stations (`0x437f50`) and the other buildings (`0x4380a0`) of one company, or of every
//! company of the active list (`company < 1`), inside one territory (`0` = the whole map).
//!
//! Confiscation (`destroy == false`) only clears the owner; the trains are always deleted
//! (`train_delete` `0x4cdd50`). Destruction removes the track with `tile_track_remove`
//! (`0x4162b0`) after deleting the trains holding a reservation on it, and destroys the station
//! buildings (`building_destroy(b, 1)` `0x40adb0`, not on this view: counted as a missing hook).

use crate::ai::Ai;
use crate::scenario::Confiscate;
use crate::trainstep::P;

impl Ai<'_, '_> {
    /// The companies an effect hits: `company_ptr(company)`, or the active list in order.
    fn confiscate_targets(&self, company: i32) -> Vec<i32> {
        if company < 1 {
            self.active_companies()
        } else if self.ts.exists(P::Company, company) {
            vec![company]
        } else {
            Vec::new()
        }
    }

    /// The effect body of `rule_evaluate` (`0x455f30`): `All` runs the four in the order track,
    /// trains, stations, buildings.
    pub fn territory_confiscate(
        &mut self,
        company: i32,
        territory: i32,
        what: Confiscate,
        destroy: bool,
    ) {
        match what {
            Confiscate::All => {
                self.confiscate_track(company, territory, destroy);
                self.confiscate_trains(company, territory);
                self.confiscate_stations(company, territory, destroy);
                self.confiscate_buildings(company, territory);
            }
            Confiscate::Track => self.confiscate_track(company, territory, destroy),
            Confiscate::Trains => self.confiscate_trains(company, territory),
            Confiscate::Stations => self.confiscate_stations(company, territory, destroy),
            Confiscate::Buildings => self.confiscate_buildings(company, territory),
        }
    }

    /// `0x437d40(company, territory, destroy)`: `0x437ab0` per company. The redraw of every
    /// track tile (`0x4cb090`) only runs with the view option `0x574bc8+0x54`, which is not
    /// part of the game state (assumed off).
    fn confiscate_track(&mut self, company: i32, territory: i32, destroy: bool) {
        for c in self.confiscate_targets(company) {
            self.confiscate_track_of(c, territory, destroy);
        }
    }

    /// `0x437ab0`: row by row, every tile whose piece belongs to `c` (and lies in `territory`
    /// when nonzero, tile `+9`) loses its owner (`piece+2 = 0`), or with `destroy` the trains
    /// with a reservation on the piece (`piece+3` -> `pool_80+1`) and then the track
    /// (`tile_track_remove(tile, 0, c, 1)`).
    fn confiscate_track_of(&mut self, c: i32, territory: i32, destroy: bool) {
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        for y in 0..h {
            for x in 0..w {
                let p = self.piece(x, y);
                let owner = if p == 0 {
                    0
                } else {
                    i32::from(self.ts.u8(P::Track, p, 2))
                };
                if owner != c || (territory != 0 && i32::from(self.ts.t8(x, y, 9)) != territory) {
                    continue;
                }
                if !destroy {
                    self.ts.set8(P::Track, p, 2, 0);
                    continue;
                }
                loop {
                    let p = self.piece(x, y);
                    if p == 0 {
                        break;
                    }
                    let r = i32::from(self.ts.u16(P::Track, p, 3));
                    if r == 0 {
                        break;
                    }
                    let t = i32::from(self.ts.u16(P::P80, r, 1));
                    self.train_delete(t);
                }
                let mut removed = 0;
                self.tile_track_remove(x, y, 0, c, true, &mut removed);
            }
        }
    }

    /// `0x437db0(company, territory)`: the trains of the list from the last to the first; with
    /// a territory only a train on the map (`+0x9c`) whose locomotive (`+0`, car pool `+9`/`+0xb`)
    /// stands in it. Each goes through `train_delete(+0x3d)`.
    fn confiscate_trains(&mut self, company: i32, territory: i32) {
        for c in self.confiscate_targets(company) {
            let trains = self.ids(P::Train);
            for &t in trains.iter().rev() {
                if !self.ts.live(P::Train, t) || self.ts.i32(P::Train, t, 0x41) != c {
                    continue;
                }
                if territory != 0 {
                    if self.ts.u8(P::Train, t, 0x9c) == 0 {
                        continue;
                    }
                    let car = self.ts.i32(P::Train, t, 0);
                    let x = i32::from(self.ts.i16(P::Car, car, 9));
                    let y = i32::from(self.ts.i16(P::Car, car, 0xb));
                    if i32::from(self.ts.t8(x, y, 9)) != territory {
                        continue;
                    }
                }
                let id = self.ts.i32(P::Train, t, 0x3d);
                self.train_delete(id);
            }
        }
    }

    /// `0x437f50(company, territory, destroy)`: the stations of the list from the last to the
    /// first owned by `c` (`+0x39`) and in `territory` (`+0x4ad`, 0 = any): the building loses
    /// its owner (`+0x97 = 0`) and so does the station, or with `destroy` the building goes
    /// (`building_destroy(b, 1)`).
    fn confiscate_stations(&mut self, company: i32, territory: i32, destroy: bool) {
        for c in self.confiscate_targets(company) {
            let stations = self.ids(P::Station);
            for &s in stations.iter().rev() {
                if self.ts.i32(P::Station, s, 0x39) != c
                    || (territory != 0 && self.ts.i32(P::Station, s, 0x4ad) != territory)
                {
                    continue;
                }
                let b = self.ts.i32(P::Station, s, 4);
                if destroy {
                    todo_hook!(
                        self.ts.log,
                        "confiscate_destroy_station",
                        "building_destroy(b, 1) 0x40adb0 of a station (effects 299/302)",
                        "scenarios.md 4.2.1"
                    );
                } else {
                    if self.ts.exists(P::Industry, b) {
                        self.ts.set16(P::Industry, b, 0x97, 0);
                    }
                    self.ts.seti32(P::Station, s, 0x39, 0);
                }
            }
        }
    }

    /// `0x4380a0(company, territory)`: the buildings of the list from the last to the first
    /// owned by `c` (`+0x97`), placed (`+7`) and in `territory` (`+0x9d`, 0 = any) lose the
    /// owner.
    fn confiscate_buildings(&mut self, company: i32, territory: i32) {
        for c in self.confiscate_targets(company) {
            let buildings = self.ids(P::Industry);
            for &b in buildings.iter().rev() {
                if i32::from(self.ts.i16(P::Industry, b, 0x97)) == c
                    && self.ts.u8(P::Industry, b, 7) != 0
                    && (territory == 0 || i32::from(self.ts.i8(P::Industry, b, 0x9d)) == territory)
                {
                    self.ts.set16(P::Industry, b, 0x97, 0);
                }
            }
        }
    }
}
