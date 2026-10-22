//! Buildings and stations: `building_create` (`0x409010`) -> `building_construct` (`0x409a30`),
//! with the station record (`station_init` `0x4be840`, name `0x42f910`, site class `0x4be430`),
//! the track under and beside a station, the catchment links (`station_rebuild_links`
//! `0x4071b0`) and the supply/demand of the new station (`0x4bebf0`). terrain-track.md 3 and
//! world-growth.md 4.4.
//!
//! The footprint marking, terrain levelling and terrain flags are the shared ones of
//! [`crate::tilemark`] (also used by the town growth).

use crate::ai::Ai;
use crate::fp::ftol;
use crate::tilemark;
use crate::trainstep::P;

use super::{DX, DY, account_move};

/// `building_shape_table` (`0x5457ef + t*0x154`): price (u32 at `+0`), type (`+9`), size
/// (`+10`), direction (`+11`).
const SHAPE: u32 = 0x0054_57ef;
const SHAPE_SIZE: u32 = 0x154;

/// State offsets: industry count (`0x570713`), "first of type" flags (`0x570603`), counter of
/// unnamed stations (`0x5705ca`).
const ST_INDUSTRIES: usize = 0x570713 - 0x5704e8;
const ST_SEEN: usize = 0x570603 - 0x5704e8;
const ST_NONAME: usize = 0x5705ca - 0x5704e8;

/// Arguments of `building_create(x, y, type, ghost, company, place, owner_hint)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewBuilding {
    pub x: i32,
    pub y: i32,
    /// Shape index (`building_shape_table`); for a station `dir + size*8`.
    pub shape: i32,
    /// `+0x2e`: preview ghost (no ledger, no station record).
    pub ghost: bool,
    pub company: i32,
    /// Owning place (`< 1`: the place of the tile, `0x430c20`).
    pub place: i32,
    /// `+0x52` of a station of the player's company (`-1`: from the nearest town).
    pub hint: i32,
}

impl Ai<'_, '_> {
    fn shape_u8(&self, t: i32, o: u32) -> u8 {
        self.ts.img.u8(SHAPE + o + SHAPE_SIZE * t as u32)
    }

    fn st_byte(&self, o: usize) -> u8 {
        self.ts.s.state.get(o).copied().unwrap_or(0)
    }

    /// `0x430c20(x, y, any)`: the place owning a tile: the closest city whose box contains it
    /// within its radius (`0x550408[+0x2c]`), else the closest region whose box contains it.
    pub fn place_at(&self, x: i32, y: i32, any: bool) -> i32 {
        let (mut city, mut region) = (0, 0);
        let (mut bc, mut br) = (99_999, 99_999);
        for p in self.ids(P::Place) {
            let g = |o: usize| i32::from(self.ts.i16(P::Place, p, o));
            let inside = any || (g(8) <= x && x <= g(0xc) && g(0xa) <= y && y <= g(0xe));
            let d2 = (g(6) - y) * (g(6) - y) + (g(4) - x) * (g(4) - x);
            match self.ts.u8(P::Place, p, 0x10) {
                0 => {
                    let r = self
                        .ts
                        .img
                        .i32(0x0055_0408 + 4 * u32::from(self.ts.u8(P::Place, p, 0x2c)));
                    if inside && d2 <= r && d2 < bc {
                        city = i32::from(self.ts.u16(P::Place, p, 0));
                        bc = d2;
                    }
                }
                1 if city == 0 && inside && d2 < br => {
                    region = i32::from(self.ts.u16(P::Place, p, 0));
                    br = d2;
                }
                _ => {}
            }
        }
        if city != 0 { city } else { region }
    }

    /// `0x42fa30(x, y)`: the closest city (slot order; also non-water regions of type 2 with
    /// the industry set `0x5704ac`), `-1` without places.
    pub fn nearest_town(&self, x: i32, y: i32) -> i32 {
        let mut best = -1;
        let mut bd = 99_999;
        let set = self.hdr8(0x0057_04ac) != 0;
        let mut slots: Vec<i32> = self.ids(P::Place);
        slots.sort_unstable();
        for p in slots {
            let kind = self.ts.u8(P::Place, p, 0x10);
            let (px, py) = (
                i32::from(self.ts.i16(P::Place, p, 4)),
                i32::from(self.ts.i16(P::Place, p, 6)),
            );
            let ok = kind == 0 || (set && kind == 2 && !self.is_water(px, py));
            if !ok {
                continue;
            }
            let d = Ai::dist(px, py, x, y);
            if d < bd {
                bd = d;
                best = i32::from(self.ts.u16(P::Place, p, 0));
            }
        }
        best
    }

    /// `building_create` (`0x409010`): allocates the building and runs `building_construct`.
    /// Returns the building id.
    pub fn building_create(&mut self, b: NewBuilding) -> i32 {
        let size = self.ts.record_size(P::Industry);
        let id = self.ts.alloc(P::Industry, vec![0u8; size]);
        self.building_construct(id, b);
        id
    }

    /// `building_construct` (`0x409a30`) for a station or a building without field textures.
    fn building_construct(&mut self, id: i32, b: NewBuilding) {
        self.ts.log.ran("building_create");
        let t = b.shape;
        let kind = self.shape_u8(t, 9);
        self.ts.set8(P::Industry, id, 7, kind);
        let mut place = b.place;
        if !b.ghost {
            if kind != 0 {
                let v = self.ts.state_i32(ST_INDUSTRIES) + 1;
                self.ts.set_state_i32(ST_INDUSTRIES, v);
            }
            if place < 1 {
                place = self.place_at(b.x, b.y, false);
            }
        } else {
            place = 0;
        }
        for k in 0..8 {
            self.ts.seti32(P::Industry, id, 0x6b + 4 * k, 0);
        }
        let company = if b.company == -1 { 0 } else { b.company };
        let set = |ai: &mut Ai<'_, '_>, o: usize, v: &[u8]| {
            for (k, x) in v.iter().enumerate() {
                ai.ts.set8(P::Industry, id, o + k, *x);
            }
        };
        self.ts.set16(P::Industry, id, 2, place as u16);
        self.ts.set16(P::Industry, id, 0, id as u16);
        self.ts.set16(P::Industry, id, 0x97, company as u16);
        self.ts.set8(P::Industry, id, 0x9e, 0);
        self.ts.setf32(P::Industry, id, 0x93, 0.5);
        self.ts.set16(P::Industry, id, 0x50, 0);
        self.ts.set8(P::Industry, id, 6, t as u8);
        self.ts.setf32(P::Industry, id, 0x47, 1.0);
        self.ts.set16(P::Industry, id, 8, 0);
        self.ts.seti32(P::Industry, id, 0x42, 0);
        self.ts.set8(P::Industry, id, 0x46, 0);
        self.ts.seti32(P::Industry, id, 0x99, 0);
        self.ts.set16(P::Industry, id, 0x34, 0);
        self.ts.set16(P::Industry, id, 0x36, 0);
        self.ts.set16(P::Industry, id, 0x3e, 0xffff);
        self.ts.set16(P::Industry, id, 0x40, 0xffff);
        self.ts.set8(P::Industry, id, 0x2e, u8::from(b.ghost));
        self.ts.seti32(P::Industry, id, 0x2f, 0);
        self.ts.set8(P::Industry, id, 0x33, 0);
        self.ts.set16(P::Industry, id, 0x4d, 0);
        let year = self.ts.year;
        self.ts.set16(P::Industry, id, 0x3c, year);
        self.ts.set8(P::Industry, id, 0x4f, 0);
        let hint = if kind == 0 {
            if company == self.player_company() && b.hint != -1 {
                b.hint as u8
            } else {
                let u = self.nearest_town(b.x, b.y);
                if u < 1 {
                    0
                } else {
                    self.ts.u8(P::Place, u, 0x11)
                }
            }
        } else {
            0
        };
        self.ts.set8(P::Industry, id, 0x52, hint);
        let ty = &self.ts.types[usize::from(kind)];
        let clocks = crate::growth::initial_slot_clocks(
            self.ts.rng,
            ty,
            i32::from(year),
            self.ts.now,
            false,
        );
        for (k, c) in clocks.iter().enumerate() {
            self.ts.set32(P::Industry, id, 0x53 + 4 * k, *c);
        }
        set(self, 0x63, &[0; 8]);
        // Geometry, footprint structures and terrain (0x409770, 0x409580).
        let (w, h) = (self.ts.map_w(), self.ts.map_h());
        let geo = tilemark::geometry(self.ts.img, t as usize, b.x, b.y, w, h);
        for k in 0..4 {
            self.ts
                .set16(P::Industry, id, 0xa + 2 * k, geo.corner_x[k] as u16);
            self.ts
                .set16(P::Industry, id, 0x12 + 2 * k, geo.corner_y[k] as u16);
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
            self.ts.set16(P::Industry, id, o, v as u16);
        }
        let region = self.ts.t8(
            i32::from(geo.region_tile.0),
            i32::from(geo.region_tile.1),
            9,
        );
        self.ts.set8(P::Industry, id, 0x9d, region);
        let (ox, oy) = (i32::from(geo.origin.0), i32::from(geo.origin.1));
        tilemark::mark_building(self.ts.s, self.ts.img, t as usize, id as u16, ox, oy);
        tilemark::level_terrain(self.ts.s, self.ts.img, t as usize, ox, oy);
        let bb = geo.bbox.map(i32::from);
        self.dirty(bb[0] - 1, bb[1] - 1, bb[2] + 1, bb[3] + 1);
        if tilemark::cells()
            .any(|(l, c)| tilemark::shape_cell(self.ts.img, t as usize, l, c) & 0x40 != 0)
        {
            todo_hook!(
                self.ts.log,
                "building_field_textures",
                "0x409120 saved tile data and 0x429fc0/0x42a3e0/0x42a7d0 field textures",
                "world-growth.md 4.5"
            );
        }
        if kind == 0 {
            self.station_track(id, b, t, company);
        }
        if !b.ghost {
            if kind == 0 {
                let st = i32::from(self.ts.u16(P::Industry, id, 8));
                self.station_rebuild_links(st);
                self.ts.station_recompute_supply_demand(st);
            } else {
                todo_hook!(
                    self.ts.log,
                    "building_industry_links",
                    "0x4072c0 links and 0x40a610 production of a new industry",
                    "world-growth.md 4.4"
                );
            }
        }
        let (ax, ay) = (i32::from(geo.anchor.0), i32::from(geo.anchor.1));
        crate::tilemark::recompute_flags(self.ts.s, ax - 4, ay - 4, ax + 4, ay + 4);
        // 0x502f60/0x508740 repaint the map: the pending redraw is flushed.
        self.flush_redraw();
        if kind == 0 && !b.ghost {
            let terr = i32::from(self.ts.i8(P::Industry, id, 0x9d));
            let m = self.ts.effect(8, company, 0, terr);
            let price = self.ts.img.i32(SHAPE + SHAPE_SIZE * t as u32);
            let cost = ftol(f64::from(m) * f64::from(price));
            account_move(self.ts, company, 1, f64::from(cost));
            self.ts.seti32(P::Industry, id, 0x99, cost);
        }
        if !b.ghost && self.st_byte(ST_SEEN + usize::from(kind)) == 0 {
            self.ts.s.state[ST_SEEN + usize::from(kind)] = 1;
        }
        if place < 1 {
            self.ts.set16(P::Industry, id, 4, 0);
        } else {
            let head = self.ts.u16(P::Place, place, 2);
            self.ts.set16(P::Industry, id, 4, head);
            self.ts.set16(P::Place, place, 2, id as u16);
        }
    }

    /// `0x4085e0` + `0x408450`/`0x408470`: the `building_create` position of a building of
    /// shape `t` whose anchor cell (flag `0x10`) is at `(sx, sy)`.
    pub fn station_create_xy(&self, t: i32, sx: i32, sy: i32) -> (i32, i32) {
        let (mut ox, mut oy) = (sx, sy);
        'f: for lin in 0..10 {
            for col in 0..10 {
                if tilemark::shape_cell(self.ts.img, t as usize, lin, col) & 0x10 != 0 {
                    ox = sx + col;
                    oy = sy + lin;
                    break 'f;
                }
            }
        }
        let e = tilemark::shape_extents(self.ts.img, t as usize);
        (ox - (e[0] >> 1), oy - (e[1] >> 1))
    }

    /// `0x574414`: the company of the local player (the chairman's company of the human
    /// person); 0 when none.
    pub fn player_company(&self) -> i32 {
        self.ids(P::Company)
            .into_iter()
            .find(|&c| self.ts.i32(P::Company, c, 0x16e) != 0 && self.company_is_human(c))
            .unwrap_or(0)
    }

    /// The station part of `building_construct`: station record, track under the anchor and on
    /// the two tiles beside it (charged to account 3), tile rebuild.
    fn station_track(&mut self, id: i32, b: NewBuilding, t: i32, company: i32) {
        let ox = i32::from(self.ts.i16(P::Industry, id, 0x1e));
        let oy = i32::from(self.ts.i16(P::Industry, id, 0x20));
        let (mut ax, mut ay) = (0, 0);
        'f: for col in 0..10 {
            for lin in 0..10 {
                if tilemark::shape_cell(self.ts.img, t as usize, lin, col) & 0x10 != 0 {
                    ax = ox - col;
                    ay = oy - lin;
                    break 'f;
                }
            }
        }
        if !b.ghost {
            let f6 = self.ts.t8(ax, ay, 6);
            self.ts.set_t8(ax, ay, 6, f6 | 2);
            let size = self.ts.record_size(P::Station);
            let st = self.ts.alloc(P::Station, vec![0u8; size]);
            self.ts.set16(P::Industry, id, 8, st as u16);
            let sz = self.shape_u8(t, 10);
            self.station_init(st, id, sz, ax, ay, company);
            tilemark::struct_add(self.ts.s, ax, ay, 4, st as u32);
            for (lin, col) in tilemark::cells() {
                if tilemark::shape_cell(self.ts.img, t as usize, lin, col) & 0xc != 0 {
                    let (cx, cy) = (ox - col, oy - lin);
                    if cx >= 0 && cx < self.ts.map_w() - 1 && cy >= 0 && cy < self.ts.map_h() - 1 {
                        let v = self.ts.t8(cx, cy, 0);
                        self.ts.set_t8(cx, cy, 0, v | 8);
                    }
                }
            }
        }
        let dir = self.shape_u8(t, 11);
        let back = dir.wrapping_sub(2) & 7;
        let fwd = dir.wrapping_add(2) & 7;
        self.ts.set16(P::Industry, id, 0x3e, ax as u16);
        self.ts.set16(P::Industry, id, 0x40, ay as u16);
        // Electrified when electric track is within 4 tiles (else when half the company's
        // track is electric).
        let mut elec = false;
        for x in ax - 4..=ax + 4 {
            for y in ay - 4..=ay + 4 {
                if self.in_map(x, y) {
                    let p = self.piece(x, y);
                    if p != 0 && self.ts.u8(P::Track, p, 0x23) & 0x10 != 0 {
                        elec = true;
                    }
                }
            }
        }
        if !elec && company != 0 {
            let cells = self.ts.i32(P::Company, company, 0x182);
            if cells != 0 && cells <= self.ts.i32(P::Company, company, 0x18e) * 2 {
                elec = true;
            }
        }
        let saved = self.track_electric;
        self.track_electric = elec;
        self.snapshot_overlays(ax - 1, ay - 1, ax + 1, ay + 1);
        // 0x5746ec: during the AI turn the station track is double.
        let double = self.ai_turn;
        if self.in_map(ax, ay) && !b.ghost {
            self.tile_track_add(ax, ay, (1 << back) | (1 << fwd), 0, 0, company);
            if double {
                self.tile_track_set_double(ax, ay, true);
            }
        }
        let mut total = 0i32;
        for (d, mask) in [(fwd, 1u8 << back), (back, 1u8 << fwd)] {
            let (nx, ny) = (ax + DX[usize::from(d)], ay + DY[usize::from(d)]);
            if !self.in_map(nx, ny) {
                continue;
            }
            let c = self.tile_cost_with(nx, ny, i32::from(back), i32::from(back), company);
            total = total.wrapping_add(c);
            if !b.ghost {
                self.tile_track_add(nx, ny, mask, 0, 0, company);
                if double {
                    self.tile_track_set_double(nx, ny, true);
                }
                let p = self.piece(nx, ny);
                let v = self.ts.i16(P::Track, p, 0x15).wrapping_add((c >> 3) as i16);
                self.ts.set16(P::Track, p, 0x15, v as u16);
            }
        }
        self.track_electric = saved;
        if !b.ghost {
            account_move(self.ts, company, 3, f64::from(total));
        }
        self.tile_rebuild(ax - 1, ay - 1, ax + 1, ay + 1);
        self.overlay_snapshot = None;
    }

    /// `track_tile_cost_with_state` (`0x463a20`) with single track, no electrification and
    /// wooden bridges (the station track), territory bases of the tile.
    fn tile_cost_with(&mut self, x: i32, y: i32, d_in: i32, d_out: i32, company: i32) -> i32 {
        let saved = (
            self.track_single,
            self.track_bridge,
            self.track_electric,
            self.edge_base,
            self.elec_base,
        );
        self.track_single = true;
        self.track_bridge = 1;
        self.track_electric = false;
        let terr = i32::from(self.ts.t8(x, y, 9));
        let m_el = self.ts.effect(0xb, company, 0, terr);
        self.elec_base = ftol(f64::from(m_el) * f64::from(self.ts.img.i32(0x0055_34c0)));
        let m_tr = self.ts.effect(9, company, 0, terr);
        self.edge_base = ftol(f64::from(m_tr) * 2000.0);
        let (c, _) = self.tile_cost(x, y, d_in, d_out, 0, 0, company);
        (
            self.track_single,
            self.track_bridge,
            self.track_electric,
            self.edge_base,
            self.elec_base,
        ) = saved;
        c
    }

    /// `station_init` (`0x4be840`).
    fn station_init(&mut self, st: i32, bld: i32, size: u8, x: i32, y: i32, company: i32) {
        let town = self.nearest_town(x, y);
        let name = if town < 1 {
            let k = self.ts.state_i32(ST_NONAME);
            let idx = (k % 4 + 4) % 4;
            self.ts.set_state_i32(ST_NONAME, k.wrapping_add(1));
            let lng = self.ts.img.u16(0x0055_3998 + 2 * idx as u32);
            self.text(u32::from(lng), "")
        } else {
            self.station_name(town, x, y)
        };
        let mut nb = name.into_bytes();
        nb.truncate(0x1e);
        for k in 0..0x1f {
            self.ts
                .set8(P::Station, st, 0x1a + k, nb.get(k).copied().unwrap_or(0));
        }
        self.ts.seti32(P::Station, st, 0, st);
        self.ts.seti32(P::Station, st, 4, bld);
        self.ts.set8(P::Station, st, 8, size);
        self.ts.seti32(P::Station, st, 0xa, x);
        self.ts.seti32(P::Station, st, 0xe, y);
        self.ts.set16(P::Station, st, 0x4be, 0);
        for o in [0x4bb, 0x4bc, 0x4c0, 0x4bd] {
            self.ts.set8(P::Station, st, o, 0);
        }
        let shape = u32::from(self.ts.u8(P::Industry, bld, 6));
        let (mut l10, mut c10, mut l20, mut c20) = (-1, -1, -1, -1);
        for lin in 0..10 {
            for col in 0..10 {
                let f = tilemark::shape_cell(self.ts.img, shape as usize, lin, col);
                if f & 0x10 != 0 {
                    l10 = lin;
                    c10 = col;
                }
                if f & 0x20 != 0 {
                    c20 = col;
                    l20 = lin;
                }
            }
        }
        self.ts.seti32(P::Station, st, 0x16, l10 - l20 + y);
        self.ts.seti32(P::Station, st, 0x12, x - c20 + c10);
        self.ts.seti32(P::Station, st, 0x39, company);
        self.ts.seti32(P::Station, st, 0x3d, 0);
        self.ts.seti32(P::Station, st, 0x41, 0);
        self.ts.seti32(P::Station, st, 0x4c1, 0);
        self.ts.set16(P::Station, st, 0x4c5, 0);
        self.ts.seti32(P::Station, st, 0x45, 0);
        let (sx, sy) = (
            self.ts.i32(P::Station, st, 0x12),
            self.ts.i32(P::Station, st, 0x16),
        );
        let class = self.site_class(sx, sy);
        self.ts.seti32(P::Station, st, 0x4d, class);
        self.ts.seti32(P::Station, st, 0x59, 0);
        self.ts.seti32(P::Station, st, 0x5d, 0);
        if company != 0 && self.ts.i32(P::Company, company, 0x176) == 0 {
            self.ts.seti32(P::Company, company, 0x176, st);
        }
        for k in 0..0x2c {
            self.ts.seti32(P::Station, st, 0x1c1 + 0xb0 + 4 * k, 0);
            self.ts.seti32(P::Station, st, 0x1c1 + 4 * k, 0);
            self.ts.set8(P::Station, st, 0x321 + k, 0);
        }
        let bits = match size {
            0 => 1,
            1 => 2,
            _ => 4,
        };
        self.ts.seti32(P::Station, st, 0x49, bits);
        let place = self.place_at(x, y, false);
        self.ts.seti32(P::Station, st, 0x53, place);
        let region = self.ts.t8(x, y, 9);
        self.ts.seti32(P::Station, st, 0x4ad, i32::from(region));
        self.ts.set8(P::Industry, bld, 0x9d, region);
        if self.ts.u8(P::Industry, bld, 0x2e) == 0 {
            if company != 0 {
                let v = self.ts.i32(P::Company, company, 0x172) + 1;
                self.ts.seti32(P::Company, company, 0x172, v);
            }
            if place != 0 {
                let v = (self.ts.now as i32).wrapping_add(0x24_ea00);
                self.ts.seti32(P::Place, place, 0x42, v);
            }
        }
        self.ts.seti32(P::Station, st, 0x4b1, 1);
        self.station_networks_rebuild();
    }

    /// A string of the language file with `%1` replaced (empty table: the template text).
    fn text(&self, id: u32, arg: &str) -> String {
        let t = self
            .lng
            .as_ref()
            .and_then(|l| l.get(id).map(str::to_owned))
            .unwrap_or_else(|| "%1".to_owned());
        t.replace("%1", arg)
    }

    /// `0x42f910(town, &name, x, y)`: "Town", "Town Junction", ... (LNG 1034..1039): the bare
    /// name only within the town radius (`0x5503f4[+0x2c] + 1`), each suffix once per town
    /// (`+0x2d` bits); "%1 Adjunct" (`0x5537de`) when all are taken.
    fn station_name(&mut self, town: i32, x: i32, y: i32) -> String {
        let (px, py) = (
            f64::from(f32::from(self.ts.i16(P::Place, town, 4))),
            f64::from(f32::from(self.ts.i16(P::Place, town, 6))),
        );
        let (dx, dy) = (f64::from(x as f32) - px, f64::from(y as f32) - py);
        let d = (dx * dx + dy * dy).sqrt();
        let r = self
            .ts
            .img
            .i32(0x0055_03f4 + 4 * u32::from(self.ts.u8(P::Place, town, 0x2c)));
        let used = self.ts.u8(P::Place, town, 0x2d);
        let tname: String = (0..0x16)
            .map(|k| self.ts.u8(P::Place, town, 0x16 + k))
            .take_while(|&c| c != 0)
            .map(char::from)
            .collect();
        let mut k = 0u32;
        while (k == 0 && f64::from(r + 1) <= d) || used & (1 << k) != 0 {
            k += 1;
            if k > 5 {
                let id = self.ts.img.u16(0x0055_37de);
                self.ts.seti32(P::Place, town, 0x32, 0);
                return self.text(u32::from(id), &tname);
            }
        }
        self.ts.set8(P::Place, town, 0x2d, used | (1 << k));
        let id = self.ts.img.u16(0x0055_37d4 + 2 * k);
        self.text(u32::from(id), &tname)
    }

    /// `0x4be430(x, y)`: the landscape class of a station site (water, desert, mountains,
    /// farmland...) from the tiles up to 5 away.
    fn site_class(&self, x: i32, y: i32) -> i32 {
        let (mut water, mut river, mut steep, mut hilly, mut cls1, mut cls2) =
            (999, 999, 999, 999, 999, 999);
        let (mut c7, mut c910, mut sand, mut crop, mut veg7) = (999, 999, 999, 999, 999);
        let (mut veg_n, mut cnt, mut sum) = (0, 0, 0);
        for r in 0..6 {
            for d in 0..8 {
                let (tx, ty) = (x + DX[d] * r, y + DY[d] * r);
                if !self.in_map(tx, ty) {
                    continue;
                }
                cnt += 1;
                let c = i32::from(self.terrain_class(tx, ty));
                let upd = |v: &mut i32| {
                    if r <= *v {
                        *v = r;
                    }
                };
                if c == 0x12 {
                    upd(&mut water);
                }
                if matches!(c, 1..=4 | 0xc) {
                    upd(&mut sand);
                }
                if c == 1 {
                    upd(&mut cls1);
                }
                if c == 2 {
                    upd(&mut cls2);
                }
                if c == 9 || c == 10 {
                    upd(&mut c910);
                }
                if c == 7 {
                    upd(&mut c7);
                }
                if self.ts.t8(tx, ty, 0) & 1 != 0 {
                    upd(&mut river);
                }
                if self.ts.t8(tx, ty, 6) & 4 != 0 {
                    let v = self.ts.t8(tx, ty, 2) & 0xf;
                    if v == 8 || v == 9 {
                        upd(&mut crop);
                    }
                    if v == 7 {
                        upd(&mut veg7);
                    }
                    veg_n += 1;
                }
                let hs = [0, 1, 2, 3].map(|k| self.corner_h(tx, ty, k));
                let hi = *hs.iter().max().expect("4");
                let lo = *hs.iter().min().expect("4");
                let dh = hi - lo;
                if dh < 0x1a {
                    if dh > 0xc && r <= hilly {
                        hilly = r;
                    }
                } else if r <= steep {
                    steep = r;
                }
                sum += dh;
            }
        }
        let f = sum as f32 / cnt as f32;
        if cls1 < 2 {
            return 0;
        }
        if water < 4 {
            if c7 < 1 {
                return 0xc;
            }
            if hilly < 4 && 5.0 <= f {
                return 6;
            }
            return 5;
        }
        if steep < 3 && 15.0 <= f {
            return 2;
        }
        if cls1 < 5 {
            return 0;
        }
        if cls2 < 4 && veg7 < 9 {
            return 7;
        }
        if river < 4 {
            return if (x + y) & 1 != 0 { 4 } else { 9 };
        }
        if steep < 7 && 11.0 <= f {
            return 3;
        }
        if hilly < 5 && 0.4 < f64::from(veg_n as f32 / cnt as f32) {
            return 1;
        }
        if c910 < 5 && f <= 10.0 {
            return 0xb;
        }
        if 7.0 <= f || sand > 1 { 8 } else { 10 }
    }

    /// `station_rebuild_links` (`0x4071b0`): a catchment link (`pool_58`) from the station to
    /// every live building it reaches, chained from the building (`+0x2f`) and the station
    /// (`+0x3d`).
    pub fn station_rebuild_links(&mut self, st: i32) {
        let bld = self.ts.i32(P::Station, st, 4);
        let size = usize::from(self.ts.u8(P::Station, st, 8));
        let sx = self.ts.i32(P::Station, st, 0xa);
        let sy = self.ts.i32(P::Station, st, 0xe);
        let _ = bld;
        for b in self.ids(P::Industry) {
            if self.ts.u8(P::Industry, b, 0x4f) != 0 {
                continue;
            }
            let cb = crate::track::CatchmentBuilding {
                building_type: usize::from(self.ts.u8(P::Industry, b, 6)),
                centre: (
                    i32::from(self.ts.i16(P::Industry, b, 0x1a)),
                    i32::from(self.ts.i16(P::Industry, b, 0x1c)),
                ),
                origin: (
                    i32::from(self.ts.i16(P::Industry, b, 0x1e)),
                    i32::from(self.ts.i16(P::Industry, b, 0x20)),
                ),
            };
            let kind = self.ts.u8(P::Industry, b, 7);
            if kind == 0 && i32::from(self.ts.u16(P::Industry, b, 8)) != st {
                continue;
            }
            let Some(d) = crate::track::station_catchment(&self.tables.track, (sx, sy), size, &cb)
            else {
                continue;
            };
            let mut r = vec![0u8; self.ts.record_size(P::Link)];
            r[0..2].copy_from_slice(&(b as u16).to_le_bytes());
            r[2..4].copy_from_slice(&(st as u16).to_le_bytes());
            r[4..6].copy_from_slice(&(d as u16).to_le_bytes());
            r[6..10].copy_from_slice(&self.ts.i32(P::Industry, b, 0x2f).to_le_bytes());
            r[10..14].copy_from_slice(&self.ts.i32(P::Station, st, 0x3d).to_le_bytes());
            let l = self.ts.alloc(P::Link, r);
            self.ts.seti32(P::Industry, b, 0x2f, l);
            self.ts.seti32(P::Station, st, 0x3d, l);
            let v = self.ts.u8(P::Industry, b, 0x33).wrapping_add(1);
            self.ts.set8(P::Industry, b, 0x33, v);
            let v = self.ts.i32(P::Station, st, 0x41) + 1;
            self.ts.seti32(P::Station, st, 0x41, v);
        }
    }
}
