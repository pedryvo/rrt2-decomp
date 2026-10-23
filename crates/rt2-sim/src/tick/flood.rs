//! Territory flooding (scenario effect 233, docs/spec/scenarios.md 4.2.1): `0x4c8a30(territory,
//! v)` finds the lowest tile of the territory and runs the water fill `0x490490(x, y, 0x12, 3, 0,
//! level)` in its mode 3, which also clears what stands on the flooded tiles.
//!
//! Mode 3 of `0x490490` (the editor's modes 0..2 are [`crate::editor::fill`]):
//!
//! 1. 4-neighbour flood from the start tile (stack, neighbours W, N, E, S) over the tiles whose
//!    four corners sum to at most `4 * level`;
//! 2. with `0x574b80`/`0x574b78` raised (the track removal skips the network rebuild), three
//!    passes over the marked tiles, row by row: pass 0 deletes the trains holding a
//!    reservation on the track of a land tile or of the neighbours its piece connects to; pass 1
//!    destroys the station of a land tile (`building_destroy(b, 1)`); pass 2 bulldozes the
//!    track (`0x4286a0`), destroys the building (`0x408e70`), clears the river (`0x429ea0`) and
//!    the road (`0x429e30`) of a land tile, clears the river of a water tile, then paints the
//!    tile water (`0x42a3e0` forced, only when it is not water yet) and sets its four corners
//!    to `level` (`0x4260c0`);
//! 3. the rivers of the whole map are rebuilt (`0x4294d0`), then the station networks
//!    (`0x46a370`) and the train routes (`0x4d3640`); the coast of the box is cleaned
//!    (`0x426d30`) and its ground blends redone (`0x42a7d0(x0 - 2, y0 - 2, w + 5, h + 5)`).

use super::*;
use crate::rng::GameRng;
use crate::tilemark;

/// Terrain class of water.
const WATER: u8 = 0x12;

fn corner_sum(s: &Save, x: i32, y: i32) -> i32 {
    let ew = s.map.width as i32 + 1;
    let e = |x: i32, y: i32| i32::from(s.map.elevation[(y * ew + x) as usize]);
    e(x, y) + e(x + 1, y) + e(x, y + 1) + e(x + 1, y + 1)
}

fn class_at(s: &Save, x: i32, y: i32) -> u8 {
    crate::editor::class_at(s, x, y)
}

/// `0x429ea0(x, y, 0, 0)`: clears the river bit and re-orients the river tiles of the 3x3
/// box (`0x429ce0(x, y, 0)`); without the redraw argument nothing else.
fn river_clear(s: &mut Save, x: i32, y: i32) {
    let w = s.map.width;
    let i = y as usize * w + x as usize;
    if s.map.tiles[i].var & 1 == 0 {
        return;
    }
    s.map.tiles[i].var &= 0xfe;
    let (mw, mh) = (s.map.width as i32, s.map.height as i32);
    for ny in y - 1..=y + 1 {
        for nx in x - 1..=x + 1 {
            if nx >= 0
                && ny >= 0
                && nx < mw
                && ny < mh
                && s.map.tiles[ny as usize * w + nx as usize].var & 1 != 0
            {
                crate::editor::river_orient(s, nx, ny, false);
            }
        }
    }
}

impl SimState {
    /// `0x4c8a30(territory, v)`: the tile of the territory (`tile+9`) with the lowest sum of
    /// its four corners (the first in row order on a tie) is flooded to `sum / 4 + v` (the
    /// division rounds toward zero).
    pub(super) fn flood_territory(
        &mut self,
        minutes: i32,
        rng: &mut GameRng,
        crt: &mut crate::rng::CrtRng,
        territory: i32,
        amount: i32,
    ) {
        let (w, h) = (self.save.map.width as i32, self.save.map.height as i32);
        let mut best = 999_999;
        let mut at = None;
        for y in 0..h {
            for x in 0..w {
                if i32::from(self.save.map.tiles[(y * w + x) as usize].region) != territory {
                    continue;
                }
                let v = corner_sum(&self.save, x, y);
                if v < best {
                    best = v;
                    at = Some((x, y));
                }
            }
        }
        let Some((x, y)) = at else {
            return;
        };
        self.log.ran("scenario_flood");
        self.water_fill(minutes, rng, crt, x, y, best / 4 + amount);
    }

    /// `0x490490(x, y, 0x12, 3, 0, level)`: the water fill of a flood (module docs).
    pub(super) fn water_fill(
        &mut self,
        minutes: i32,
        rng: &mut GameRng,
        crt: &mut crate::rng::CrtRng,
        x: i32,
        y: i32,
        level: i32,
    ) {
        let (w, h) = (self.save.map.width as i32, self.save.map.height as i32);
        // The overlay lists as loaded, before any tile changes.
        let ground = self.take_ground_layer();
        self.ground_layer = Some(ground);
        let borders = crate::fields::border_layer(&self.save);
        let limit = level * 4;
        let mut mark = vec![0u8; (w * h) as usize];
        mark[(y * w + x) as usize] = 1;
        let mut stack = vec![(x, y)];
        while let Some((cx, cy)) = stack.pop() {
            for k in 0..4 {
                let (nx, ny) = match k {
                    0 if cx > 0 => (cx - 1, cy),
                    1 if cy > 0 => (cx, cy - 1),
                    2 if cx < w - 1 => (cx + 1, cy),
                    3 if cy < h - 1 => (cx, cy + 1),
                    _ => continue,
                };
                let i = (ny * w + nx) as usize;
                if mark[i] != 0 {
                    continue;
                }
                if corner_sum(&self.save, nx, ny) <= limit {
                    mark[i] = 1;
                    stack.push((nx, ny));
                } else {
                    mark[i] = 2;
                }
            }
        }
        let river_before: Vec<bool> = self.save.map.tiles.iter().map(|t| t.var & 1 != 0).collect();
        let mut bbox: Option<[i32; 4]> = None;
        for pass in 0..3 {
            for ty in 0..h {
                for tx in 0..w {
                    if mark[(ty * w + tx) as usize] != 1 {
                        continue;
                    }
                    let water = class_at(&self.save, tx, ty) == WATER;
                    if water {
                        if pass != 2 {
                            continue;
                        }
                        river_clear(&mut self.save, tx, ty);
                    } else {
                        match pass {
                            0 => {
                                self.with_trainstep_parts(minutes, rng, crt, |ts| {
                                    crate::ai::Ai::new(ts).flood_delete_trains(tx, ty);
                                });
                                continue;
                            }
                            1 => {
                                if let Some(st) = tilemark::first_struct(&self.save, tx, ty, 4) {
                                    let b = pool::record(&self.save.stations, st as usize)
                                        .map_or(0, |r| i32_at(&r.data, 4));
                                    if b > 0 {
                                        self.building_destroy_with(
                                            rng,
                                            Some((minutes, &mut *crt)),
                                            b as usize,
                                        );
                                    }
                                }
                                continue;
                            }
                            _ => {
                                if self.save.map.tiles[(ty * w + tx) as usize].track_id != 0
                                    || self.save.map.tiles[(ty * w + tx) as usize].reserved8 != 0
                                {
                                    self.with_trainstep_parts(minutes, rng, crt, |ts| {
                                        let mut ai = crate::ai::Ai::new(ts);
                                        ai.suppress_networks = true;
                                        let mut removed = 0;
                                        ai.track_bulldoze_tile(tx, ty, 0, None, &mut removed);
                                    });
                                }
                                if let Some(b) = tilemark::first_struct(&self.save, tx, ty, 3)
                                    && pool::record(&self.save.objects, b as usize).is_some()
                                {
                                    self.building_destroy_with(
                                        rng,
                                        Some((minutes, &mut *crt)),
                                        b as usize,
                                    );
                                }
                                river_clear(&mut self.save, tx, ty);
                                crate::editor::lay_road(&mut self.save, tx, ty, false);
                            }
                        }
                    }
                    // LAB_004909f4
                    let b = bbox.get_or_insert([tx, ty, tx, ty]);
                    b[0] = b[0].min(tx);
                    b[1] = b[1].min(ty);
                    b[2] = b[2].max(tx);
                    b[3] = b[3].max(ty);
                    if class_at(&self.save, tx, ty) != WATER {
                        crate::editor::paint_tile(&mut self.save, rng, tx, ty, usize::from(WATER));
                        self.dirty.add(tx - 1, ty - 1, tx - 1, ty - 1);
                    }
                    let ew = w + 1;
                    for (cx, cy) in [(tx, ty), (tx + 1, ty), (tx + 1, ty + 1), (tx, ty + 1)] {
                        self.save.map.elevation[(cy * ew + cx) as usize] =
                            level.clamp(-0xff, 0x1fe) as i16;
                    }
                }
            }
        }
        // `0x4294d0(0, 0, W, H)`: the river overlays of the whole map are rebuilt; a tile that
        // loses or gets one refreshes its overlay flags (`0x4258e0`).
        let track = self.track_mask(0, 0, w - 1, h - 1);
        let ground = self.take_ground_layer();
        for ty in 0..h {
            for tx in 0..w {
                let i = (ty * w + tx) as usize;
                let t = &self.save.map.tiles[i];
                let river = t.var & 1 != 0;
                if !river && !river_before[i] {
                    continue;
                }
                let any =
                    river || ground[i] || borders[i] || t.trees & 0x20 != 0 || track.get(tx, ty);
                let t = &mut self.save.map.tiles[i];
                tilemark::overlay_flags(t, any);
            }
        }
        self.ground_layer = Some(ground);
        self.with_trainstep_parts(minutes, rng, crt, |ts| {
            let mut ai = crate::ai::Ai::new(ts);
            ai.station_networks_rebuild();
            ai.trains_refresh_all();
        });
        if let Some([x0, y0, x1, y1]) = bbox {
            crate::editor::coast_cleanup(&mut self.save, rng, x0 - 1, y0 - 1, x1 + 1, y1 + 1);
            self.flood_blend(rng, [x0 - 2, y0 - 2, x1 - x0 + 5, y1 - y0 + 5], &borders);
            // `0x417770(x0 - 3, y0 - 3, x1 + 3, y1 + 3)`: the deferred redraw (`0x417c40`) also
            // refreshes the flat bit (`0x417880`) of the new corners; the light pass is left to
            // the saver like every pending redraw.
            self.dirty.add(x0 - 3, y0 - 3, x1 + 3, y1 + 3);
            tilemark::recompute_flat(&mut self.save, x0 - 3, y0 - 3, x1 + 3, y1 + 3);
        }
    }

    /// `0x42a7d0(x, y, w, h)` after the fill ([`crate::fields::ground_rect_with`]), with the
    /// territory borders the lists held before the tiles turned to water.
    fn flood_blend(&mut self, rng: &mut GameRng, r: [i32; 4], borders: &[bool]) {
        let track = self.track_mask(r[0] - 6, r[1] - 6, r[0] + r[2] + 6, r[1] + r[3] + 6);
        let mut ground = self.take_ground_layer();
        let mut ctx = crate::fields::FieldCtx {
            dirty: &mut self.dirty,
            ground: &mut ground,
            track: &track,
        };
        crate::fields::ground_rect_with(
            &mut self.save,
            &mut ctx,
            rng,
            r[0],
            r[1],
            r[2],
            r[3],
            Some(borders),
        );
        self.ground_layer = Some(ground);
    }
}
