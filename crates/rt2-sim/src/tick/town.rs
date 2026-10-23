//! `town_year_step` (`0x430a40`, world-growth.md 3.1): the yearly growth of the places over the
//! save pools. The rules are [`crate::growth`]; this module is the world side
//! ([`TownEnv`], [`SiteEnv`]) and the bookkeeping of the step.
//!
//! What the spec leaves open is counted, not guessed silently: `building_can_place`
//! (`0x4087a0`) is a best-effort reading of terrain-track.md 4.3 (footprint cells, river, land
//! and water flags, existing building), `building_create` (`0x409010`/`0x409a30`) writes the
//! record fields of world-growth.md 4.4 but not the terrain marking and the farm textures (one
//! RNG draw per footprint tile of some types), and `building_destroy` (`0x40adb0`) does not
//! return the tiles. Once one of those matters, the RNG stream of the original is lost for the
//! rest of the step (the step is the last RNG consumer of the year).

use super::*;
use crate::growth::{
    BUILDING_TYPES, BuildingRef, GrowthTables, GrowthWorld, KIND_REGION, PickWorld, PlaceGeom,
    RemovalCandidate, SiteEnv, Town, TownEnv, decay_counter, initial_slot_clocks,
    pick_worst_building, site_classes, town_add_building, town_building_stats, town_growth_step,
    town_place_building, type_scarcity_percent,
};
use crate::rng::GameRng;
use crate::tilemark;
use rt2_formats::terrain;

const ST_LAST_YEAR_STEP: usize = 0x1F5; // 0x5706dd u32
const ST_DELIVERIES: usize = 0x227; // 0x57070f i32
const ST_INDUSTRY_COUNT: usize = 0x22B; // 0x570713 i32
const ST_GROWTH: usize = 0xE9; // 0x5705d1 f32
const ST_YEAR_START: usize = 0x20; // 0x570508
const ST_SEEN_TYPE: usize = 0x11B; // 0x570603 u8[53]
const HDR_TYPE_FLAGS: usize = 0x8793; // 0x5702cb, 9 bytes per type
const HDR_CITY_MASK_SET: usize = 0x8974; // 0x5704ac industry set

const PL_FIRST: usize = 2;
const PL_KIND: usize = 0x10;
const BD_PLACE: usize = 2;
const BD_NEXT: usize = 4;
const BD_SHAPE: usize = 6;
const BD_TYPE: usize = 7;
const BD_STATION: usize = 8;

fn place_town(d: &[u8]) -> Town {
    Town {
        kind: d[PL_KIND],
        size_class: d[0x2c],
        growth_index: i32_at(d, 0x2e),
        land_tiles: i32_at(d, 0x32),
        lots_in: u16_at(d, 0x3e),
        lots_out: u16_at(d, 0x40),
        cooldown_until: u32_at(d, 0x42),
        intensity: u16_at(d, 0x46),
        trend: d[0x49] as i8,
    }
}

/// The world as the growth rules see it, for one place.
struct TownWorld<'a> {
    s: &'a mut SimState,
    tab: &'a GrowthTables,
    pick: &'a PickWorld,
    place: usize,
    now: u32,
    year: i32,
    set1: bool,
}

impl TownWorld<'_> {
    fn chain(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut cur = pool::record(&self.s.save.places, self.place)
            .map_or(0, |r| usize::from(u16_at(&r.data, PL_FIRST)));
        while cur != 0 && out.len() <= self.s.save.objects.records.len() {
            out.push(cur);
            cur = pool::record(&self.s.save.objects, cur)
                .map_or(0, |r| usize::from(u16_at(&r.data, BD_NEXT)));
        }
        out
    }

    fn geom(&self) -> PlaceGeom {
        let d = &pool::record(&self.s.save.places, self.place).unwrap().data;
        PlaceGeom {
            id: self.place as u16,
            region: d[PL_KIND] == KIND_REGION,
            size_class: d[0x2c],
            cx: i32::from(i16_at(d, 4)),
            cy: i32::from(i16_at(d, 6)),
            bbox: [
                i32::from(i16_at(d, 8)),
                i32::from(i16_at(d, 10)),
                i32::from(i16_at(d, 12)),
                i32::from(i16_at(d, 14)),
            ],
        }
    }
}

impl TownEnv for TownWorld<'_> {
    /// `town_building_stats` (`0x430400`).
    fn stats(&mut self) -> crate::growth::BuildingStats {
        let refs: Vec<BuildingRef> = self
            .chain()
            .into_iter()
            .filter_map(|b| pool::record(&self.s.save.objects, b))
            .map(|r| BuildingRef {
                ty: r.data[BD_TYPE],
                stations: r.data[industry::OFF_STATIONS] as i8,
            })
            .collect();
        let out = town_building_stats(&refs, self.tab, &self.s.types, self.set1, self.year);
        if let Some(r) = pool::record_mut(&mut self.s.save.places, self.place) {
            put_f32(&mut r.data, 0x12, out.output_rate);
            put_f32(&mut r.data, 0x3a, out.served_rate);
            r.data[0x48] = out.houses_u8;
        }
        out.stats
    }

    /// `town_add_building` (`0x42ffc0`).
    fn add_building(&mut self, rng: &mut GameRng, house: bool) -> bool {
        let d = &pool::record(&self.s.save.places, self.place).unwrap().data;
        let region = d[PL_KIND] == KIND_REGION;
        let mut weights = [0u16; BUILDING_TYPES];
        for (t, w) in weights.iter_mut().enumerate() {
            *w = u16_at(d, 0x4a + 2 * t);
        }
        let mut counts = [0u8; BUILDING_TYPES];
        for b in self.chain() {
            if let Some(r) = pool::record(&self.s.save.objects, b) {
                let t = usize::from(r.data[BD_TYPE]);
                let primary = self.s.types.get(t).is_some_and(|ty| ty.slots[0].mode != 0);
                if t < BUILDING_TYPES && !primary {
                    counts[t] = counts[t].saturating_add(1);
                }
            }
        }
        let geom = self.geom();
        let tab = self.tab;
        let (now, year, place) = (self.now, self.year, self.place);
        let s = &mut *self.s;
        town_add_building(
            rng,
            tab,
            self.pick,
            region,
            &weights,
            &counts,
            house,
            &mut |rng, ty| {
                let mut site = SiteWorld {
                    s: &mut *s,
                    tab,
                    now,
                    year,
                    place,
                };
                town_place_building(rng, tab, &geom, ty, &mut site)
            },
        )
    }

    /// `town_remove_worst_building` (`0x4302c0`).
    fn remove_worst(&mut self, rng: &mut GameRng, house: bool) {
        let chain = self.chain();
        let list: Vec<RemovalCandidate> = chain
            .iter()
            .filter_map(|&b| pool::record(&self.s.save.objects, b))
            .map(|r| RemovalCandidate {
                is_house: usize::from(r.data[BD_TYPE]) == crate::growth::TYPE_HOUSE,
                has_station: u16_at(&r.data, BD_STATION) != 0,
                owned: u16_at(&r.data, 0x97) != 0,
                rating: u16_at(&r.data, industry::OFF_RATING_PCT),
            })
            .collect();
        if let Some(i) = pick_worst_building(rng, &list, house, self.set1) {
            self.s.building_destroy(chain[i]);
        }
    }
}

/// The map as `town_place_building` sees it, with the pools to create the building in.
struct SiteWorld<'a> {
    s: &'a mut SimState,
    tab: &'a GrowthTables,
    now: u32,
    year: i32,
    place: usize,
}

impl SiteWorld<'_> {
    /// What `building_construct` writes into the map after the record is allocated (world-growth
    /// 4.5): structure chain of the footprint tiles (`0x409770`), terrain levelling (`0x409580`),
    /// station links (`0x4072c0`), terrain flags around the anchor (`0x424c10`) and the flat
    /// bit of the redrawn tiles.
    fn mark_terrain(&mut self, slot: usize, shape: usize, geo: &tilemark::Geometry) {
        let (cx, cy) = (i32::from(geo.origin.0), i32::from(geo.origin.1));
        {
            let st = &mut *self.s;
            let img = &st.data.image;
            tilemark::mark_building(&mut st.save, img, shape, slot as u16, cx, cy);
            tilemark::level_terrain(&mut st.save, img, shape, cx, cy);
            if tilemark::cells().any(|(l, c)| tilemark::shape_cell(img, shape, l, c) & 0x40 != 0) {
                todo_hook!(
                    st.log,
                    "building_create_field_textures",
                    "0x409120 saved tile data and 0x429fc0/0x42a3e0/0x42a7d0 field textures (RNG per tile)",
                    "world-growth.md 4.5"
                );
            }
        }
        self.s.link_building(slot);
        let (ax, ay) = (i32::from(geo.anchor.0), i32::from(geo.anchor.1));
        tilemark::recompute_flags(&mut self.s.save, ax - 4, ay - 4, ax + 4, ay + 4);
        let b = geo.bbox.map(i32::from);
        tilemark::recompute_flat(&mut self.s.save, b[0] - 1, b[1] - 1, b[2] + 1, b[3] + 1);
    }

    fn width(&self) -> i32 {
        self.s.save.map.width as i32
    }
    fn height(&self) -> i32 {
        self.s.save.map.height as i32
    }
    fn tile(&self, x: i32, y: i32) -> Option<&rt2_formats::map::Tile> {
        if x < 0 || y < 0 || x >= self.width() || y >= self.height() {
            return None;
        }
        self.s.save.map.tiles.get((y * self.width() + x) as usize)
    }
}

impl SiteEnv for SiteWorld<'_> {
    /// `place_at(x, y, 0)` (`0x430c20`, world-growth.md 3.8).
    fn place_at(&mut self, x: i32, y: i32) -> u16 {
        let order = pool::list_order(&self.s.save.places);
        let mut best_city: Option<(i32, usize)> = None;
        let mut best_region: Option<(i32, usize)> = None;
        for slot in order {
            let Some(r) = pool::record(&self.s.save.places, slot) else {
                continue;
            };
            let d = &r.data;
            let (bx0, by0, bx1, by1) = (
                i32::from(i16_at(d, 8)),
                i32::from(i16_at(d, 10)),
                i32::from(i16_at(d, 12)),
                i32::from(i16_at(d, 14)),
            );
            if x < bx0 || x > bx1 || y < by0 || y > by1 {
                continue;
            }
            let (dx, dy) = (x - i32::from(i16_at(d, 4)), y - i32::from(i16_at(d, 6)));
            let d2 = dx * dx + dy * dy;
            match d[PL_KIND] {
                0 if d2 <= self.tab.dist2[usize::from(d[0x2c]).min(4)]
                    && best_city.is_none_or(|(b, _)| d2 < b) =>
                {
                    best_city = Some((d2, slot));
                }
                1 if best_region.is_none_or(|(b, _)| d2 < b) => best_region = Some((d2, slot)),
                _ => {}
            }
        }
        best_city.or(best_region).map_or(0, |(_, s)| s as u16)
    }

    fn site_classes(&mut self, x: i32, y: i32) -> (i32, i32) {
        let w1 = self.width() + 1;
        let tile = |px: i32, py: i32| self.tile(px, py).map(|t| (t.flags2, t.trees, t.flags6));
        let elev = |ex: i32, ey: i32| {
            self.s
                .save
                .map
                .elevation
                .get((ey * w1 + ex) as usize)
                .map_or(0, |&v| i32::from(v))
        };
        site_classes(&tile, &elev, x, y)
    }

    fn ground_class(&mut self, x: i32, y: i32) -> i32 {
        self.tile(x, y).map_or(0, |t| {
            let id = u16::from(t.var) | (u16::from(t.ground) << 8);
            i32::from(terrain::tables().class_of_id(id >> 6))
        })
    }

    /// `building_can_place` (`0x4087a0`, world-growth.md 4.6), checked against the original by
    /// `crates/rt2-oracle/tests/placement.rs`.
    fn can_place(&mut self, x: i32, y: i32, shape: usize, early: bool) -> bool {
        tilemark::can_place(&self.s.save, &self.s.data.image, x, y, shape, early)
    }

    /// `building_create` (`0x409010` -> `0x409a30`, world-growth.md 4.4).
    fn create(&mut self, rng: &mut GameRng, x: i32, y: i32, shape: usize) {
        let ty = self.tab.shape_type[shape];
        let mut d = vec![0u8; self.s.save.objects.record_size];
        put_u16(&mut d, BD_PLACE, self.place as u16);
        // Footprint geometry (`0x409770`, world-growth.md 4.5): corners per view rotation,
        // anchor, origin cell, region tile and bounding box.
        let geo = tilemark::geometry(&self.s.data.image, shape, x, y, self.width(), self.height());
        for k in 0..4 {
            pool::put_i16(&mut d, 0xa + 2 * k, geo.corner_x[k]);
            pool::put_i16(&mut d, 0x12 + 2 * k, geo.corner_y[k]);
        }
        for (o, v) in [
            (0x1a, geo.anchor.0),
            (0x1c, geo.anchor.1),
            (0x1e, geo.origin.0),
            (0x20, geo.origin.1),
            (0x22, geo.region_tile.0),
            (0x24, geo.region_tile.1),
            (0x26, geo.bbox[0]),
            (0x28, geo.bbox[1]),
            (0x2a, geo.bbox[2]),
            (0x2c, geo.bbox[3]),
        ] {
            pool::put_i16(&mut d, o, v);
        }
        put_u16(&mut d, 0x3e, 0xffff);
        put_u16(&mut d, 0x40, 0xffff);
        d[BD_SHAPE] = shape as u8;
        d[BD_TYPE] = ty;
        put_u16(&mut d, 0x3c, self.year as u16);
        put_f32(&mut d, industry::OFF_RATING, 1.0);
        put_f32(&mut d, industry::OFF_QUALITY, 0.5);
        d[industry::OFF_TERRITORY] = self
            .tile(i32::from(geo.region_tile.0), i32::from(geo.region_tile.1))
            .map_or(0, |t| t.region);
        let clocks = initial_slot_clocks(
            rng,
            &self.s.types[usize::from(ty)],
            self.year,
            self.now,
            false,
        );
        for (k, c) in clocks.iter().enumerate() {
            put_u32(&mut d, industry::OFF_CLOCKS + 4 * k, *c);
        }
        let head =
            pool::record(&self.s.save.places, self.place).map_or(0, |r| u16_at(&r.data, PL_FIRST));
        put_u16(&mut d, BD_NEXT, head);
        let Ok(slot) = pool::alloc(&mut self.s.save.objects, d) else {
            return;
        };
        if let Some(r) = pool::record_mut(&mut self.s.save.objects, slot) {
            put_u16(&mut r.data, 0, slot as u16);
        }
        if let Some(r) = pool::record_mut(&mut self.s.save.places, self.place) {
            put_u16(&mut r.data, PL_FIRST, slot as u16);
        }
        if ty != 0 {
            let n = i32_at(&self.s.save.state, ST_INDUSTRY_COUNT);
            put_u32(&mut self.s.save.state, ST_INDUSTRY_COUNT, (n + 1) as u32);
        }
        let seen = ST_SEEN_TYPE + usize::from(ty);
        if self.s.save.state.get(seen) == Some(&0) {
            self.s.save.state[seen] = 1;
            self.s.log.ran("building_first_of_type_message");
            self.s
                .ts_globals
                .news
                .push(crate::message::GameMessage::FirstOfType {
                    building_type: ty,
                    place: self.place as i32,
                });
        }
        self.mark_terrain(slot, shape, &geo);
        self.s.log.ran("building_create");
    }
}

impl SimState {
    /// `0x4072c0(building)`: links a new building to every station whose catchment reaches it
    /// (`station_catchment_test` `0x407030`): one `pool_58` record per station, chained from
    /// the building (`+0x2f`) and from the station (`+0x3d`).
    pub(super) fn link_building(&mut self, b: usize) {
        let Some(br) = pool::record(&self.save.objects, b) else {
            return;
        };
        let bd = br.data.clone();
        let shape = usize::from(bd[BD_SHAPE]);
        let (bx, by) = (i32::from(i16_at(&bd, 0x1a)), i32::from(i16_at(&bd, 0x1c)));
        let (ox, oy) = (i32::from(i16_at(&bd, 0x1e)), i32::from(i16_at(&bd, 0x20)));
        for st in pool::list_order(&self.save.stations) {
            let Some(sr) = pool::record(&self.save.stations, st) else {
                continue;
            };
            if i32_at(&sr.data, 0x45) != 0 {
                continue;
            }
            let own = i32_at(&sr.data, 4) as usize;
            if bd[BD_TYPE] == 0 && own != b {
                continue;
            }
            let (sx, sy) = (i32_at(&sr.data, 0xa), i32_at(&sr.data, 0xe));
            if sx < 0 {
                continue;
            }
            let img = &self.data.image;
            let radius = img.i32(0x0054_fd9c + 4 * u32::from(sr.data[8]));
            if crate::track::tile_distance(sx, sy, bx, by) > radius + 11 {
                continue;
            }
            let mut min = 9999;
            for (lin, col) in tilemark::cells() {
                if tilemark::shape_cell(img, shape, lin, col) & 0x43 != 0 {
                    min = min.min(crate::track::tile_distance(sx, sy, ox - col, oy - lin));
                }
            }
            if min > radius {
                continue;
            }
            let (next_ind, next_st) = (
                pool::record(&self.save.objects, b).map_or(0, |r| u32_at(&r.data, 0x2f)),
                pool::record(&self.save.stations, st).map_or(0, |r| u32_at(&r.data, 0x3d)),
            );
            let mut rec = vec![0u8; self.save.pool_58.record_size];
            put_u16(&mut rec, 0, b as u16);
            put_u16(&mut rec, 2, st as u16);
            put_u16(&mut rec, 4, min as u16);
            put_u32(&mut rec, 6, next_ind);
            put_u32(&mut rec, 0xa, next_st);
            let Ok(link) = pool::alloc(&mut self.save.pool_58, rec) else {
                return;
            };
            if let Some(r) = pool::record_mut(&mut self.save.objects, b) {
                put_u32(&mut r.data, 0x2f, link as u32);
                r.data[industry::OFF_STATIONS] = r.data[industry::OFF_STATIONS].wrapping_add(1);
            }
            if let Some(r) = pool::record_mut(&mut self.save.stations, st) {
                put_u32(&mut r.data, 0x3d, link as u32);
                let n = i32_at(&r.data, 0x41);
                put_u32(&mut r.data, 0x41, (n + 1) as u32);
            }
        }
    }

    /// `building_destroy(b, 1)` (`0x40adb0`, world-growth.md 3.6).
    fn building_destroy(&mut self, slot: usize) {
        let Some(rec) = pool::record(&self.save.objects, slot) else {
            return;
        };
        let place = usize::from(u16_at(&rec.data, BD_PLACE));
        let next = u16_at(&rec.data, BD_NEXT);
        let real = rec.data[BD_TYPE] != 0 && rec.data[0x2e] == 0;
        let pile = usize::from(u16_at(&rec.data, industry::OFF_PILE));
        let rec_link_head = i32_at(&rec.data, 0x2f);
        if real {
            let n = i32_at(&self.save.state, ST_INDUSTRY_COUNT);
            put_u32(&mut self.save.state, ST_INDUSTRY_COUNT, (n - 1) as u32);
        }
        // Out of the place's list (`0x430da0`).
        let first = pool::record(&self.save.places, place)
            .map_or(0, |r| usize::from(u16_at(&r.data, PL_FIRST)));
        if first == slot {
            if let Some(r) = pool::record_mut(&mut self.save.places, place) {
                put_u16(&mut r.data, PL_FIRST, next);
            }
        } else {
            let mut cur = first;
            let mut guard = 0;
            while cur != 0 && guard <= self.save.objects.records.len() {
                guard += 1;
                let nx = pool::record(&self.save.objects, cur)
                    .map_or(0, |r| usize::from(u16_at(&r.data, BD_NEXT)));
                if nx == slot {
                    if let Some(r) = pool::record_mut(&mut self.save.objects, cur) {
                        put_u16(&mut r.data, BD_NEXT, next);
                    }
                    break;
                }
                cur = nx;
            }
        }
        // The lots of the pile that no station took are freed.
        let mut lot = pile;
        let mut guard = 0;
        while lot != 0 && guard <= self.save.pool_7c.records.len() {
            guard += 1;
            let nx = pool::record(&self.save.pool_7c, lot)
                .map_or(0, |r| usize::from(u16_at(&r.data, industry::LOT_NEXT)));
            let free = pool::record(&self.save.pool_7c, lot)
                .is_some_and(|r| u16_at(&r.data, industry::LOT_STATION) == 0);
            if free {
                pool::free(&mut self.save.pool_7c, lot);
            }
            lot = nx;
        }
        // The station links of the building (`pool_58`, chained from `+0x2f` by link `+6`, and
        // from each station's `+0x3d` by link `+0xa`) are cut and freed.
        let mut link = rec_link_head;
        let mut guard = 0;
        while link > 0 && guard <= 64 {
            guard += 1;
            let Some(lr) = pool::record(&self.save.pool_58, link as usize) else {
                break;
            };
            let (next_ind, station, next_st) = (
                i32_at(&lr.data, 6),
                usize::from(u16_at(&lr.data, 2)),
                i32_at(&lr.data, 0xa),
            );
            if let Some(sr) = pool::record_mut(&mut self.save.stations, station) {
                let head = i32_at(&sr.data, 0x3d);
                if head == link {
                    put_u32(&mut sr.data, 0x3d, next_st as u32);
                } else {
                    let mut cur = head;
                    let mut g2 = 0;
                    while cur > 0 && g2 <= 64 {
                        g2 += 1;
                        let nx = pool::record(&self.save.pool_58, cur as usize)
                            .map_or(0, |r| i32_at(&r.data, 0xa));
                        if nx == link {
                            if let Some(pr) = pool::record_mut(&mut self.save.pool_58, cur as usize)
                            {
                                put_u32(&mut pr.data, 0xa, next_st as u32);
                            }
                            break;
                        }
                        cur = nx;
                    }
                }
                if let Some(sr) = pool::record_mut(&mut self.save.stations, station) {
                    let n = i32_at(&sr.data, 0x41);
                    put_u32(&mut sr.data, 0x41, (n - 1) as u32);
                }
            }
            pool::free(&mut self.save.pool_58, link as usize);
            link = next_ind;
        }
        // Tiles of the footprint (world-growth.md 4.5): the kind-3 structures leave the chains
        // of the bounding box, the saved field tiles come back, the terrain flags of the box
        // and its ring are recomputed.
        let (bbox, origin, saved, shape_ty) = {
            let d = &pool::record(&self.save.objects, slot).expect("record").data;
            (
                [0x26, 0x28, 0x2a, 0x2c].map(|o| i16_at(d, o)),
                (i32::from(i16_at(d, 0x1e)), i32::from(i16_at(d, 0x20))),
                u16_at(d, 0x50),
                d[BD_TYPE],
            )
        };
        tilemark::unmark_building(&mut self.save, slot as u16, bbox);
        if saved != 0 {
            tilemark::restore_saved_tiles(&mut self.save, saved, origin.0, origin.1);
        }
        if matches!(shape_ty, 0xb | 0x10 | 0x21) {
            todo_hook!(
                self.log,
                "building_destroy_field_textures",
                "0x4091f0 repaints the transitions around a field (0x42a7d0)",
                "world-growth.md 4.5"
            );
        }
        let b = bbox.map(i32::from);
        tilemark::recompute_flags(&mut self.save, b[0] - 1, b[1] - 1, b[2] + 1, b[3] + 1);
        if let Some(r) = pool::record_mut(&mut self.save.objects, slot) {
            put_u32(&mut r.data, 0x2f, 0);
            put_u16(&mut r.data, 0x50, 0);
            r.data[industry::OFF_STATIONS] = 0;
            put_u16(&mut r.data, BD_PLACE, 0);
            r.data[industry::OFF_INACTIVE] = 1;
        }
        self.log.ran("building_destroy");
    }

    /// `town_year_step(new_world)` (`0x430a40`, world-growth.md 3.1): `new_world` is the call of
    /// the New Game start (`0x474280`), which keeps every scarcity weight at 100 and grows each
    /// place to its initial size.
    pub(super) fn town_year_step(&mut self, w: &mut World, new_world: bool) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "town_year_step",
                "town_year_step 0x430a40 on a 0x233C file",
                "world-growth.md 3.1"
            );
            return;
        }
        let now = w.minutes;
        let date = date_of(now);
        let last = u32_at(&self.save.state, ST_LAST_YEAR_STEP);
        let dt = (now as u32).wrapping_sub(last) as i32;
        put_u32(&mut self.save.state, ST_LAST_YEAR_STEP, now as u32);
        // Scarcity weights by type (`0x5702cc`, +9 stride).
        let mut count = [0i32; BUILDING_TYPES];
        for slot in pool::list_order(&self.save.objects) {
            if let Some(r) = pool::record(&self.save.objects, slot) {
                let t = usize::from(r.data[BD_TYPE]);
                if t < BUILDING_TYPES {
                    count[t] += 1;
                }
            }
        }
        let mut pick = PickWorld {
            year: i32::from(date.year),
            set1: self.hdr(HDR_CITY_MASK_SET) != 0,
            scarcity: [100; BUILDING_TYPES],
            disabled: [false; BUILDING_TYPES],
        };
        for (t, &n) in count.iter().enumerate() {
            let o = HDR_TYPE_FLAGS + 9 * t;
            pick.scarcity[t] = if new_world {
                100
            } else {
                type_scarcity_percent(n)
            };
            pick.disabled[t] = self.save.header[o] != 0;
            put_u32(&mut self.save.header, o + 1, pick.scarcity[t] as u32);
        }
        let tab = self.data.growth.clone();
        let gw = GrowthWorld {
            growth: f32_at(&self.save.state, ST_GROWTH),
            econ_state: i32_at(&self.save.state, ST_ECON_STATE).clamp(0, 4) as usize,
            viagra: false,
            industry_count: i32_at(&self.save.state, ST_INDUSTRY_COUNT),
            deliveries_total: i32_at(&self.save.state, ST_DELIVERIES),
            year: i32::from(date.year),
            start_year: i32::from(u16_at(&self.save.state, ST_YEAR_START)),
            now: now as u32,
            set1: pick.set1,
        };
        let player_station = pool::list_order(&self.save.stations)
            .into_iter()
            .filter_map(|s| pool::record(&self.save.stations, s))
            .any(|r| r.data[0x45] == 0);
        for place in pool::list_order(&self.save.places) {
            let Some(r) = pool::record_mut(&mut self.save.places, place) else {
                continue;
            };
            if r.data[0x2d] != 0 && !player_station {
                r.data[0x2d] = 0;
            }
            let mut t = place_town(&r.data);
            {
                let mut env = TownWorld {
                    s: &mut *self,
                    tab: &tab,
                    pick: &pick,
                    place,
                    now: now as u32,
                    year: i32::from(date.year),
                    set1: pick.set1,
                };
                town_growth_step(&mut t, &gw, &tab, &mut env, &mut w.rng, dt, new_world);
            }
            if let Some(r) = pool::record_mut(&mut self.save.places, place) {
                put_u32(&mut r.data, 0x2e, t.growth_index as u32);
                put_u16(&mut r.data, 0x46, t.intensity);
                r.data[0x49] = t.trend as u8;
                let a = decay_counter(i32::from(u16_at(&r.data, 0x3e)));
                let b = decay_counter(i32::from(u16_at(&r.data, 0x40)));
                put_u16(&mut r.data, 0x3e, a as u16);
                put_u16(&mut r.data, 0x40, b as u16);
            }
            self.log.ran("town_update");
        }
        let d = decay_counter(gw.deliveries_total);
        put_u32(&mut self.save.state, ST_DELIVERIES, d as u32);
        self.log.ran("town_year_step");
    }
}
