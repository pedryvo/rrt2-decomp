//! Track and station construction of the AI (ai.md 5.4 and 5.5): `ai_connect_town`
//! (`0x435060`), `ai_build_town_station` (`0x434a40`), `ai_connect_station` (`0x434940`) and
//! `ai_build_line` (`0x435790`). In simulation mode they price the work (path cost from
//! `track_pathfind` in build mode plus the raw cost of the station types); the real mode lays
//! the track (`track_lay_route` `0x4cc060`) and builds the stations (`0x409010`).

use super::path::{DX, DY, Req};
use super::site::Site;
use super::{Ai, CO_BOX, CO_ELECTRIC};
use crate::trainstep::P;

/// Saved `tile+2 & 0x10` bits of the cells marked while a path is searched.
type Marks = Vec<(i32, i32, u8)>;

impl Ai<'_, '_> {
    fn type_cost(&self, t: u32) -> i32 {
        self.ts.img.i32(0x0054_57ef + 0x154 * t)
    }

    /// `max_bridge_type` (`0x469ef0`, terrain-track.md 2.2).
    pub fn max_bridge_type(&self) -> i32 {
        let opt = self.hdr8(0x0056_7b38 + 0x1bf);
        let _ = opt;
        // Scenario option +0x1bf of 0x574bc8 is not in the save; the year rule applies.
        if self.st8(0x0057_3d7a) != 0 {
            return 3;
        }
        let y = self.year();
        if y < 1840 {
            1
        } else if y < 1880 {
            2
        } else {
            3
        }
    }

    fn mark(&mut self, marks: &mut Marks, x: i32, y: i32) {
        let b = self.ts.t8(x, y, 2);
        self.ts.set_t8(x, y, 2, b | 0x10);
        marks.push((x, y, (b & 0x10) >> 4));
    }

    fn unmark(&mut self, marks: Marks) {
        for (x, y, bit) in marks.into_iter().rev() {
            let b = self.ts.t8(x, y, 2);
            self.ts.set_t8(x, y, 2, ((bit & 1) << 4) | (b & 0xef));
        }
    }

    /// Marks the footprint cells (`& 3`) of type `t` anchored at `(sx, sy)`.
    fn mark_footprint(&mut self, marks: &mut Marks, t: u32, sx: i32, sy: i32) {
        let (ox, oy) = self.footprint_origin(t, sx, sy);
        for row in 0..10 {
            for col in 0..10 {
                if self.footprint(t, row, col) & 3 != 0 {
                    self.mark(marks, ox - col, oy - row);
                }
            }
        }
    }

    /// The `k`-th (1-based) station of the town's surroundings (`0x431330`): in the town box
    /// and closer than 9 to its center.
    pub fn town_station(&self, town: i32, k: i32) -> i32 {
        let mut k = k;
        for st in self.ids(P::Station) {
            if self.station_near_town(town, st) {
                k -= 1;
                if k == 0 {
                    return st;
                }
            }
        }
        0
    }

    /// `0x431280`: number of stations near the town.
    pub fn town_station_count(&self, town: i32) -> i32 {
        self.ids(P::Station)
            .into_iter()
            .filter(|&st| self.station_near_town(town, st))
            .count() as i32
    }

    /// Station `st` in the town box (`+8..+0xe`) and at a distance `< 9.0` from the center.
    pub fn station_near_town(&self, town: i32, st: i32) -> bool {
        let sx = self.ts.i32(P::Station, st, 0xa);
        let sy = self.ts.i32(P::Station, st, 0xe);
        let b = |o: usize| i32::from(self.ts.i16(P::Place, town, o));
        if !(b(8) <= sx && sx <= b(0xc) && b(0xa) <= sy && sy <= b(0xe)) {
            return false;
        }
        let dx = f64::from(sx as f32) - f64::from(b(4) as f32);
        let dy = f64::from(sy as f32) - f64::from(b(6) as f32);
        (dx * dx + dy * dy).sqrt() < 9.0
    }

    /// `ai_connect_town(town, simulate, &cost)` (`0x435060`, ai.md 5.4).
    pub fn connect_town(&mut self, town: i32, simulate: bool, cost: &mut i32) -> bool {
        let (size, size2) = self.station_size(town);
        let tx = i32::from(self.ts.i16(P::Place, town, 4));
        let ty = i32::from(self.ts.i16(P::Place, town, 6));
        let x0 = self.co_i32(CO_BOX);
        let y0 = self.co_i32(CO_BOX + 4);
        let x1 = self.co_i32(CO_BOX + 8);
        let y1 = self.co_i32(CO_BOX + 12);
        let st = if self.town_station_count(town) != 0 {
            self.town_station(town, 1)
        } else {
            0
        };
        let company = self.company;
        let mut boxes: Vec<(i32, i32, i32, i32)> = Vec::new();
        loop {
            if y1 < y0 {
                return false;
            }
            let mut best = 1_999_999_999i32;
            let mut pick: Option<(i32, i32)> = None;
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if !self.in_map(x, y) || self.piece(x, y) == 0 {
                        continue;
                    }
                    if self.piece_owner(x, y) != company {
                        continue;
                    }
                    let (dx, dy) = (x - tx, y - ty);
                    let mut d = dx * dx + dy * dy;
                    if self.ts.t8(x, y, 6) & 2 != 0 {
                        d -= d >> 4;
                    }
                    if d < best
                        && !boxes
                            .iter()
                            .any(|&(a, b, c, e)| a <= x && x <= c && b <= y && y <= e)
                    {
                        best = d;
                        pick = Some((x, y));
                    }
                }
            }
            let Some((bx, by)) = pick else {
                return false;
            };
            let r = if st == 0 {
                self.build_town_station(town, bx, by, size, size2, simulate, cost)
            } else {
                let sx = self.ts.i32(P::Station, st, 0xa);
                let sy = self.ts.i32(P::Station, st, 0xe);
                self.connect_station(sx, sy, bx, by, simulate, cost)
            };
            if r {
                return true;
            }
            boxes.push((bx - 5, by - 5, bx + 5, by + 5));
            if boxes.len() >= 3 {
                return false;
            }
        }
    }

    /// `ai_connect_station` (`0x434940`): path from an existing station to the network.
    pub fn connect_station(
        &mut self,
        sx: i32,
        sy: i32,
        tx: i32,
        ty: i32,
        simulate: bool,
        cost: &mut i32,
    ) -> bool {
        if simulate {
            let mut r = Req::route(sx, sy, tx, ty);
            r.build = true;
            r.company = self.company;
            r.want_path = true;
            let f = self.pathfind(&r);
            *cost = f.cost;
            return f.ok;
        }
        let electric = self.co_electric();
        let bridge = if self.max_bridge_type() >= 3 { 3 } else { 1 };
        self.track_lay_route(sx, sy, tx, ty, -1, -1, electric, bridge)
    }

    fn co_electric(&self) -> bool {
        self.company != 0 && self.ts.u8(P::Company, self.company, CO_ELECTRIC) != 0
    }

    /// `ai_build_town_station(town, x, y, size, size', simulate, &cost)` (`0x434a40`).
    #[allow(clippy::too_many_arguments)]
    pub fn build_town_station(
        &mut self,
        town: i32,
        tx: i32,
        ty: i32,
        size: i32,
        size2: i32,
        simulate: bool,
        cost: &mut i32,
    ) -> bool {
        let cx = i32::from(self.ts.i16(P::Place, town, 4));
        let cy = i32::from(self.ts.i16(P::Place, town, 6));
        let company = self.company;
        let mut attempt = 0;
        loop {
            let (site, dir) = loop {
                attempt += 1;
                let (dx, dy) = (cx - tx, cy - ty);
                let mut c = dx.abs() < dy.abs();
                if attempt == 2 {
                    c = !c;
                }
                let dir: u8 = if !c {
                    if tx <= cx { 6 } else { 2 }
                } else if ty <= cy {
                    0
                } else {
                    4
                };
                let _ = self.station_size(town);
                if let Some(s) = self.find_station_site(town, company, dir, size, size2) {
                    break (s, dir);
                }
                if attempt > 1 {
                    return false;
                }
            };
            let t = site.building_type();
            let mut marks = Marks::new();
            self.mark_footprint(&mut marks, t, site.x, site.y);
            let t2 = u32::from(site.dir.wrapping_sub(4) & 7) + size2 as u32 * 8;
            self.mark_footprint(&mut marks, t2, site.x, site.y);
            let back = dir.wrapping_sub(4) & 7;
            if simulate {
                let (fx, fy) = (
                    site.x + DX[usize::from(back)],
                    site.y + DY[usize::from(back)],
                );
                self.mark(&mut marks, fx, fy);
            }
            let r = if !simulate {
                let electric = self.co_electric();
                let bridge = self.max_bridge_type();
                self.track_lay_route(
                    site.x,
                    site.y,
                    tx,
                    ty,
                    i32::from(back),
                    -1,
                    electric,
                    bridge,
                )
            } else {
                let mut r = Req::route(site.x, site.y, tx, ty);
                r.build = true;
                r.start_dir = i32::from(dir);
                r.company = company;
                r.want_path = true;
                let f = self.pathfind(&r);
                *cost = f.cost;
                if f.ok {
                    *cost = cost.wrapping_add(self.type_cost(t));
                }
                f.ok
            };
            self.unmark(marks);
            if r {
                if !simulate {
                    self.build_station(site);
                }
                return true;
            }
            if attempt > 1 {
                return false;
            }
        }
    }

    /// `ai_build_line(a, b, simulate, &cost)` (`0x435790`, ai.md 5.5).
    pub fn build_line(&mut self, a: i32, b: i32, simulate: bool, cost: Option<&mut i32>) -> bool {
        let towns = [a, b];
        let pos = |ai: &Ai<'_, '_>, t: i32| {
            (
                i32::from(ai.ts.i16(P::Place, t, 4)),
                i32::from(ai.ts.i16(P::Place, t, 6)),
            )
        };
        let (ax, ay) = pos(self, a);
        let (bx, by) = pos(self, b);
        let dir0: u8 = if (ax - bx).abs() < (ay - by).abs() {
            if by <= ay { 0 } else { 4 }
        } else if bx <= ax {
            6
        } else {
            2
        };
        let dir1 = dir0.wrapping_sub(4) & 7;
        let dirs = [dir0, dir1];
        let company = self.company;
        let mut sites: [Option<Site>; 2] = [None, None];
        let mut size2 = [0i32; 2];
        for i in 0..2 {
            let (s, s2) = self.station_size(towns[i]);
            size2[i] = s2;
            let Some(site) = self.find_station_site(towns[i], company, dirs[i], s, s2) else {
                return false;
            };
            sites[i] = Some(site);
            if i == 1 {
                let s0 = sites[0].expect("site");
                let dx = f64::from(site.x as f32) - f64::from(s0.x as f32);
                let dy = f64::from(site.y as f32) - f64::from(s0.y as f32);
                if (dx * dx + dy * dy).sqrt() < 8.0 {
                    return false;
                }
            }
        }
        let s0 = sites[0].expect("site");
        let s1 = sites[1].expect("site");
        let mut marks = Marks::new();
        for (i, s) in [s0, s1].into_iter().enumerate() {
            self.mark_footprint(&mut marks, s.building_type(), s.x, s.y);
            let t2 = u32::from(s.dir.wrapping_sub(4) & 7) + size2[i] as u32 * 8;
            self.mark_footprint(&mut marks, t2, s.x, s.y);
            if simulate {
                let back = dirs[i].wrapping_sub(4) & 7;
                let (fx, fy) = (s.x + DX[usize::from(back)], s.y + DY[usize::from(back)]);
                self.mark(&mut marks, fx, fy);
            }
        }
        let r = if !simulate {
            let electric = self.co_electric();
            let bridge = self.max_bridge_type();
            self.track_lay_route(
                s0.x,
                s0.y,
                s1.x,
                s1.y,
                i32::from(dir1),
                i32::from(dir1),
                electric,
                bridge,
            )
        } else {
            let mut r = Req::route(s0.x, s0.y, s1.x, s1.y);
            r.build = true;
            r.start_dir = i32::from(dir0);
            r.end_dir = i32::from(dir1);
            r.company = company;
            r.want_path = true;
            let f = self.pathfind(&r);
            if f.ok
                && let Some(c) = cost
            {
                *c = f
                    .cost
                    .wrapping_add(self.type_cost(s0.building_type()))
                    .wrapping_add(self.type_cost(s1.building_type()));
            }
            f.ok
        };
        self.unmark(marks);
        if !r {
            return false;
        }
        if !simulate {
            self.build_station(s0);
            self.build_station(s1);
        }
        true
    }

    /// `track_lay_route` (`0x4cc060`) with the AI's arguments: not ported (the track is not
    /// laid); the turn goes on as if it succeeded.
    #[allow(clippy::too_many_arguments)]
    pub fn track_lay_route(
        &mut self,
        _x1: i32,
        _y1: i32,
        _x2: i32,
        _y2: i32,
        _d1: i32,
        _d2: i32,
        electric: bool,
        bridge: i32,
    ) -> bool {
        // 0x4cc060 writes the track globals from its arguments.
        self.track_electric = electric;
        self.track_bridge = bridge;
        todo_hook!(
            self.ts.log,
            "track_lay_route",
            "track_lay_route 0x4cc060: lay the path, earthwork, ledger (RNG of the tile rebuild)",
            "ai.md 5.4; terrain-track.md 2.3"
        );
        true
    }

    /// The real station of `0x434a40`/`0x435790`: `building_create` (`0x409010`) of type
    /// `dir + size*8`, `ai_station_upgrades` (`0x434850`) and the news. Not ported.
    fn build_station(&mut self, _site: Site) {
        todo_hook!(
            self.ts.log,
            "ai_station_build",
            "building_create 0x409010 of the station, ai_station_upgrades 0x434850, news LNG 239",
            "ai.md 5.4; terrain-track.md 3.1"
        );
    }
}
