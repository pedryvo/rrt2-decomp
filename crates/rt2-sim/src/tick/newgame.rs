//! New Game from a scenario map (`0x479b20`, docs/spec/world-tick.md 10): load the map, keep the
//! options the setup screen chose, set up the players and start the world (`0x474280`).

use rt2_formats::map::{MAGIC_2348, Save};

use super::SimState;
use crate::calendar::Date;
use crate::data::GameData;
use crate::fp::ftol;
use crate::pool;
use crate::world::World;

/// What the setup screens hand to the scenario start. The original keeps these in process
/// globals that survive the map load: [`NewGameOptions::trailer`] is the 0xB6-byte block at
/// `0x574840` (saved before the loader and restored after it), [`NewGameOptions::block86`] the
/// 0x86-byte block at `0x566d00` (idem, `0x479b20`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGameOptions {
    /// `0x574840`: `+0` number of AI players, `+0x22` start year override (u16, 0 = the map's),
    /// `+0x2e` `game_seed`, `+0x32` realism options (bits 0-2 and 3-5), `+0x36 + 4*i` player
    /// slot `i` (character, human index or -1, AI index, flags bit 0 = in use).
    pub trailer: Vec<u8>,
    /// `0x566d00`: campaign state (`+0x16` scenario index, `+0x1a` campaign running, `+0x3d` set).
    pub block86: Vec<u8>,
    /// File name without extension, copied to the header name field `0x567b5b`.
    pub name: String,
    /// `0x570234`: 1 when started from the campaign list (`0x410de0`), 0 from New Scenario.
    pub campaign: bool,
    /// CRT `rand` state when `0x479b20` starts.
    pub crt_seed: u32,
    /// `default2.lng`, where `0x473590` takes the name and biography of each player's persona
    /// (ids at `0x551ea7`/`0x551ea9 + 0x23 * persona`). Without it the map's texts stay.
    pub lng: Option<std::sync::Arc<rt2_formats::lng::Lng>>,
}

impl NewGameOptions {
    /// The options of an existing save: its trailer and campaign block (for replaying a start the
    /// original made, e.g. from its snapshot).
    pub fn from_save(s: &Save, name: &str, campaign: bool, crt_seed: u32) -> NewGameOptions {
        let b = s.layout.header_len;
        NewGameOptions {
            trailer: s.trailer.clone(),
            block86: s.header[b..b + 0x86].to_vec(),
            name: name.to_string(),
            campaign,
            crt_seed,
            lng: None,
        }
    }
}

/// `_strncpy(dst, text, dst.len())`: the LNG text as Latin-1 bytes, the rest zero-filled.
fn strncpy(dst: &mut [u8], text: &str) {
    dst.fill(0);
    for (d, c) in dst.iter_mut().zip(text.chars()) {
        *d = c as u32 as u8;
    }
}

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
fn i32_at(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

impl World {
    /// Starts a new game on scenario `map` as `0x479b20` does, with the game RNG at `seed` when
    /// it starts (`rng_seed` just before: a clock value for the campaign, `game_seed` for New
    /// Scenario). The player is not in a company yet: that is the Start New Company flow.
    pub fn new_game(map: &Save, options: &NewGameOptions, data: &GameData, seed: u32) -> World {
        let mut rng = rt2_formats::terrain::GameRng(seed);
        let mut save = if map.layout.version < MAGIC_2348 {
            crate::convert::upgrade(map, &mut rng)
        } else {
            let mut s = map.clone();
            crate::convert::every_load(&mut s);
            s
        };
        // The setup's option block replaces the map's after the loader (`0x479b20`).
        save.trailer = options.trailer.clone();
        save.game_seed = u32::from_le_bytes(options.trailer[0x2e..0x32].try_into().unwrap());
        let mut w = World::from_loaded(&save, data, rng.0, options.crt_seed);
        let mut sim = w.sim.take().expect("from_loaded builds the sim");
        sim.load_passes(&mut w, map.layout.version);
        sim.new_game_start(&mut w, options);
        w.sim = Some(sim);
        w
    }
}

impl SimState {
    fn new_game_start(&mut self, w: &mut World, o: &NewGameOptions) {
        self.players_setup(o, w.minutes);
        // `0x479b20`: the campaign block comes back after the load.
        let b = self.save.layout.header_len;
        self.save.header[b..b + 0x86].copy_from_slice(&o.block86);
        self.world_start(w);
        // Header name field (`0x567b5b`, header +0x23).
        let h = &mut self.save.header;
        let n = o.name.len().min(0x20);
        h[0x23..0x23 + n].copy_from_slice(&o.name.as_bytes()[..n]);
        h[0x23 + n] = 0;
        // `0x573f1c`: more than one network player.
        put_u32(&mut self.save.state, 0x3A34, 0);
        for k in [6u8, 0, 2] {
            self.event_trigger(w, k);
        }
        // `0x573d2d[i] = engine_available(i, 0)` for the 0x42 engines.
        let d = crate::calendar::minutes_to_date(w.minutes);
        for e in 0..0x42usize {
            let v = self.engine_available(e, i32::from(d.year), i32::from(d.month));
            self.save.state[0x3845 + e] = u8::from(v);
        }
        self.save.header[0x86FC] = u8::from(o.campaign);
        // The first redraw of the map (`0x417c40`, seen once at the end of the campaign start).
        self.redraw_map();
    }

    /// `0x417c40`: the deferred redraw of the map's dirty rectangle. The flat bit (`0x417880`)
    /// is already kept up to date where the elevation changes; this is the light pass.
    pub(crate) fn redraw_map(&mut self) {
        if let Some([x0, y0, x1, y1]) = self.dirty.0.take() {
            crate::tilemark::recompute_light(&mut self.save, x0, y0, x1, y1);
        }
    }

    /// `0x473590` (single player): the player records of the map take the slots of the option
    /// block; each company resets its territory standing (`0x43ddb0`) and four counters; the
    /// territories and stations lose their per-game counters.
    fn players_setup(&mut self, o: &NewGameOptions, minutes: i32) {
        let st = &self.save.state;
        let n = i32_at(st, 0x28d).max(0) as usize;
        let ids: Vec<usize> = (0..n)
            .map(|k| u16::from_le_bytes([st[0x291 + 2 * k], st[0x292 + 2 * k]]) as usize)
            .collect();
        for (i, &p) in ids.iter().enumerate().rev() {
            let e = 0x36 + 4 * i;
            let (ch, slot, flags) = (
                self.save.trailer[e] as i8,
                self.save.trailer[e + 1] as i8,
                self.save.trailer[e + 3],
            );
            let img = &self.data.image;
            let Some(r) = pool::record_mut(&mut self.save.persons, p) else {
                continue;
            };
            if flags & 1 != 0 {
                put_u32(&mut r.data, 0x122, u32::from(slot >= 0));
                put_u32(&mut r.data, 0x126, if slot >= 0 { 0 } else { u32::MAX });
                let t = 0x0055_1e89 + 0x23 * i32::from(ch) as u32;
                r.data[0x132] = img.u8(t);
                r.data[0x52f] = ch as u8;
                // Name (`+8`, 30 bytes) and biography (`+0x27`, 250 bytes) of the persona from
                // the LNG (`_strncpy`: zero-filled), each followed by a terminator.
                if let Some(lng) = &o.lng {
                    let name = img.u16(0x0055_1ea7 + 0x23 * i32::from(ch) as u32);
                    let bio = img.u16(0x0055_1ea9 + 0x23 * i32::from(ch) as u32);
                    strncpy(&mut r.data[8..0x26], lng.get(u32::from(name)).unwrap_or(""));
                    r.data[0x26] = 0;
                    strncpy(
                        &mut r.data[0x27..0x121],
                        lng.get(u32::from(bio)).unwrap_or(""),
                    );
                    r.data[0x121] = 0;
                } else {
                    crate::todo_hook!(
                        self.log,
                        "player_persona_texts",
                        "0x473590: persona name and biography from default2.lng (no LNG given)",
                        "world-tick.md 10"
                    );
                }
                put_u32(&mut r.data, 0x555, 0);
            } else {
                self.player_remove(minutes, p);
            }
            if let Some(r) = pool::record_mut(&mut self.save.persons, p) {
                r.data[0x534..0x544].fill(0);
            }
        }
        let st = &self.save.state;
        let nc = i32_at(st, 0x241).max(0) as usize;
        let cos: Vec<usize> = (0..nc)
            .map(|k| u16::from_le_bytes([st[0x245 + 2 * k], st[0x246 + 2 * k]]) as usize)
            .collect();
        for c in cos {
            self.company_reset_territories(c);
            if let Some(r) = pool::record_mut(&mut self.save.companies, c) {
                r.data[0x4219..0x4229].fill(0);
            }
        }
        for r in &mut self.save.territories.records {
            r.data[0x2b..0x3b].fill(0);
            r.data[0x52..0x58].fill(0);
            r.data[0x58..0x58 + 0x2bc].fill(0);
        }
        for r in &mut self.save.stations.records {
            r.data[0x4bb] = 0;
        }
    }

    /// `0x43ddb0`: the company's block of each territory (list order, `+0x479 + id * 0x1ed`):
    /// standing `+0x47a` = 5000, or `2 * territory.+0x4e - 5000` when that is set; the rights
    /// `+0x479` = all four (editor, or a territory without per-right flags `+0x43` whose
    /// `+0x26` is set) or bit `i` per right flag `+0x26 + i`; the rest (`+0x47e`, 0x1e8 bytes)
    /// zeroed. Then `+0x4229 = 5000` and the two "every territory" bytes `+0x422d` (right 1 in
    /// all) and `+0x422e` (right 0 in all).
    fn company_reset_territories(&mut self, company: usize) {
        let editor = self.save.state[0x3892] != 0;
        let terr: Vec<(usize, i16, u8, [u8; 4])> = pool::list_order(&self.save.territories)
            .into_iter()
            .filter_map(|t| pool::record(&self.save.territories, t))
            .map(|r| {
                let d = &r.data;
                let id = i32_at(d, 0).max(0) as usize;
                (
                    id,
                    i16_at(d, 0x4e),
                    d[0x43],
                    [d[0x26], d[0x27], d[0x28], d[0x29]],
                )
            })
            .collect();
        let Some(r) = pool::record_mut(&mut self.save.companies, company) else {
            return;
        };
        let d = &mut r.data;
        for &(id, v, per_right, flags) in &terr {
            let b = 0x479 + id * 0x1ed;
            if b + 0x1ed > d.len() {
                continue;
            }
            let standing = if v == 0 {
                5000
            } else {
                v.wrapping_mul(2).wrapping_sub(5000)
            };
            d[b + 1..b + 3].copy_from_slice(&standing.to_le_bytes());
            d[b + 5..b + 5 + 0x1e8].fill(0);
            d[b] = if editor {
                0xf
            } else if per_right == 0 {
                if flags[0] != 0 { 0xf } else { 0 }
            } else {
                (0..4)
                    .filter(|&i| flags[i] != 0)
                    .fold(0, |a, i| a | (1 << i))
            };
        }
        put_u32(d, 0x4229, 5000);
        let all = |bit: u8| {
            terr.iter()
                .all(|&(id, ..)| d.get(0x479 + id * 0x1ed).is_some_and(|f| f & bit != 0))
        };
        let (r1, r0) = (all(2), all(1));
        d[0x422d] = u8::from(r1);
        d[0x422e] = u8::from(r0);
    }

    /// `0x40b820` for every building in list order: a live one (`+0x4f == 0`) clears `+0x63` and
    /// `+0x67`, restarts its production clocks (`0x40bb40`: `rng(0, 100)` per slot unless
    /// decorative, `+0x2e`), recounts its piles (`0x40a610(b, 0)`), takes the year in `+0x3c`
    /// and clears the yearly counters of its station (`0x4beb00`).
    fn buildings_set_date(&mut self, w: &mut World, year: i32, minutes: i32) {
        use crate::industry;
        for slot in pool::list_order(&self.save.objects) {
            let Some(rec) = pool::record(&self.save.objects, slot) else {
                continue;
            };
            if rec.data[industry::OFF_INACTIVE] != 0 {
                continue;
            }
            let mut data = rec.data.clone();
            data[0x63..0x6b].fill(0);
            let ty = usize::from(data[industry::OFF_TYPE]);
            let decorative = data[0x2e] != 0;
            let clocks = crate::growth::initial_slot_clocks(
                &mut w.rng,
                &self.types[ty],
                year,
                minutes as u32,
                decorative,
            );
            for (k, c) in clocks.iter().enumerate() {
                put_u32(&mut data, industry::OFF_CLOCKS + 4 * k, *c);
            }
            let types = std::mem::take(&mut self.types);
            let effects = self.effect_table();
            let env = industry::Env {
                types: &types,
                now: minutes,
                year,
                basic_model: self.save.state[super::ST_BASIC_MODEL] != 0,
                sandbox: self.sandbox(),
                overtime: false,
                econ_factor: 1.0,
                effects: &effects,
            };
            industry::recount_piles(&mut data, &self.save.pool_7c, &env);
            self.types = types;
            data[0x3c..0x3e].copy_from_slice(&(year as u16).to_le_bytes());
            let station = usize::from(u16::from_le_bytes([data[8], data[9]]));
            if let Some(r) = pool::record_mut(&mut self.save.objects, slot) {
                r.data = data;
            }
            if station != 0
                && let Some(s) = pool::record_mut(&mut self.save.stations, station)
            {
                s.data[0x59..0x61].fill(0);
                for o in [0x4b5usize, 0x4b7, 0x4b9] {
                    s.data[o..o + 2].fill(0);
                }
            }
        }
        self.log.ran("building_set_date");
    }

    /// `0x474280`: the world start of a new game.
    fn world_start(&mut self, w: &mut World) {
        // Speed at least 3 (`0x57050e`, u32 read).
        if i32_at(&self.save.state, 0x26) < 3 {
            put_u32(&mut self.save.state, 0x26, 3);
            w.speed = 3;
            w.speed_resume = 3;
        }
        let opt = self.save.trailer[0x32];
        {
            let st = &mut self.save.state;
            st[0x388A] = u8::from((opt & 0x38) >= 0x10);
            st[0x388C] = u8::from((opt & 0x38) >= 8);
            st[0x3890] = opt & 7;
            st[0x388E] = u8::from((opt & 7) >= 2);
            st[0x388F] = u8::from(opt & 7 == 0);
            let v = [st[0x38B6], st[0x38B7]];
            st[0x391A..0x391C].copy_from_slice(&v);
            st[0x388B] = st[0x388A];
            st[0x388D] = st[0x388C];
        }
        // Start date override: the option year with the map's start month, day and time
        // (`0x570508`: state +0x20 year, +0x22 month, +0x23 day, +0x24 hour, +0x25 minute).
        let year = u16::from_le_bytes([self.save.trailer[0x22], self.save.trailer[0x23]]);
        if year != 0 {
            let st = &self.save.state;
            let d = Date {
                year,
                month: st[0x22],
                day: st[0x23],
                hour: st[0x24],
                minute: st[0x25],
            };
            self.game_set_date(w, d);
        }
        self.towns_recompute_extent();
        self.territories_recount_land();
        self.town_year_step(w, true);
        self.week_end(w);
        for r in &mut self.save.companies.records {
            put_u32(&mut r.data, 0x4248, 1.0f32.to_bits());
        }
        let k1 = self.data.image.f64(0x0053_5390);
        let k2 = self.data.image.f64(0x0053_75d8);
        let slots: Vec<usize> = self.save.persons.records.iter().map(|r| r.slot).collect();
        for p in slots {
            let a = w.rng.range(-0x7fff, 0x7fff);
            let b = w.rng.range(-0x50, 0x50) + 0x3c;
            let Some(r) = pool::record_mut(&mut self.save.persons, p) else {
                continue;
            };
            put_u32(&mut r.data, 0x548, a as u32);
            let ch = r.data[0x52f] as i8;
            let base = self
                .data
                .image
                .i32(0x0055_1e8b + 0x23 * i32::from(ch) as u32);
            let v = ftol((f64::from(b) + f64::from(200 - base) * k1) * k2);
            let v = if v < 0x8ca00 { v } else { 0x8ca00 };
            let v = if v > 0x11940 { v } else { 0x11940 };
            put_u32(&mut r.data, 0x551, 0);
            let adj = (4 - i32::from((opt >> 3) & 7) - i32::from(opt & 7)) * 0x50;
            put_u32(&mut r.data, 0x267, (v + adj) as u32);
        }
        let st = &mut self.save.state;
        st[0x1F9..0x1F9 + 0x2c].fill(0);
        let mut e = 0x38CF;
        while e <= 0x392E {
            let b = st[e] & 0xfe;
            let name = &st[e - 0x15..];
            let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
            st[e] = (b & 0xfd) | (u8::from(len > 1) << 1);
            e += 0x18;
        }
    }
}

fn i16_at(b: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([b[off], b[off + 1]])
}

impl SimState {
    /// `place_at(x, y, 1)` (`0x430c20` without the extent test): the nearest city (`+0x10 == 0`)
    /// within the squared radius of its size (`0x550408[+0x2c]`), else the nearest region.
    fn place_at_any(&self, x: i32, y: i32) -> usize {
        let (mut best_c, mut best_r) = (99_999i32, 99_999i32);
        let (mut city, mut region) = (0usize, 0usize);
        for slot in pool::list_order(&self.save.places) {
            let Some(r) = pool::record(&self.save.places, slot) else {
                continue;
            };
            let d = &r.data;
            let (dx, dy) = (i32::from(i16_at(d, 4)) - x, i32::from(i16_at(d, 6)) - y);
            let d2 = dy * dy + dx * dx;
            match d[0x10] {
                0 => {
                    let lim = self.data.image.i32(0x0055_0408 + 4 * u32::from(d[0x2c]));
                    if d2 <= lim && d2 < best_c {
                        city = slot;
                        best_c = d2;
                    }
                }
                1 if city == 0 && d2 < best_r => {
                    region = slot;
                    best_r = d2;
                }
                _ => {}
            }
        }
        if city != 0 { city } else { region }
    }

    /// `towns_recompute_extent` (`0x430e70`): each place's land tile count (`+0x32`, not water
    /// nor class 8) and bounding box (`+8..+0xe`) over the tiles it owns (`place_at(x, y, 1)`);
    /// a place without tiles gets the whole map.
    fn towns_recompute_extent(&mut self) {
        if self.save.places.count == 0 {
            return;
        }
        let order = pool::list_order(&self.save.places);
        for &p in &order {
            if let Some(r) = pool::record_mut(&mut self.save.places, p) {
                put_u32(&mut r.data, 0x32, 0);
                for (o, v) in [(8, 9999i16), (10, 9999), (12, -1), (14, -1)] {
                    r.data[o..o + 2].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
        let (w, h) = (self.save.map.width, self.save.map.height);
        let tb = rt2_formats::terrain::tables();
        for x in 0..w {
            for y in 0..h {
                let p = self.place_at_any(x as i32, y as i32);
                if p == 0 {
                    continue;
                }
                let t = &self.save.map.tiles[y * w + x];
                let class = tb.class_of_id(u16::from(t.ground) << 2 | u16::from(t.var >> 6));
                let Some(r) = pool::record_mut(&mut self.save.places, p) else {
                    continue;
                };
                let d = &mut r.data;
                if class != 18 && class != 8 {
                    let n = i32_at(d, 0x32) + 1;
                    put_u32(d, 0x32, n as u32);
                }
                let (x, y) = (x as i16, y as i16);
                if x < i16_at(d, 8) {
                    d[8..10].copy_from_slice(&x.to_le_bytes());
                }
                if y < i16_at(d, 10) {
                    d[10..12].copy_from_slice(&y.to_le_bytes());
                }
                if i16_at(d, 12) < x {
                    d[12..14].copy_from_slice(&x.to_le_bytes());
                }
                if i16_at(d, 14) < y {
                    d[14..16].copy_from_slice(&y.to_le_bytes());
                }
            }
        }
        for &p in &order {
            if let Some(r) = pool::record_mut(&mut self.save.places, p)
                && i16_at(&r.data, 8) == 9999
            {
                r.data[8..10].fill(0);
                r.data[10..12].fill(0);
                r.data[12..14].copy_from_slice(&(w as i16 - 1).to_le_bytes());
                r.data[14..16].copy_from_slice(&(h as i16 - 1).to_le_bytes());
            }
        }
    }

    /// `territories_recount_land` (`0x4c8970`): land tiles per region (tile byte 9) into each
    /// territory's `+0x48` (by its id at `+0`) and their total into `0x573e04`.
    fn territories_recount_land(&mut self) {
        let tb = rt2_formats::terrain::tables();
        let mut n = [0i32; 32];
        for t in &self.save.map.tiles {
            let class = tb.class_of_id(u16::from(t.ground) << 2 | u16::from(t.var >> 6));
            if class != 18 {
                n[usize::from(t.region) & 31] += 1;
            }
        }
        let total: i32 = n.iter().sum();
        put_u32(&mut self.save.state, 0x391C, total as u32);
        for r in &mut self.save.territories.records {
            let id = i32_at(&r.data, 0) as usize & 31;
            put_u32(&mut r.data, 0x48, n[id] as u32);
        }
    }
}

impl SimState {
    /// `game_set_date` (`0x46b650`): sets the clock and restarts every dated counter: the start
    /// and economy dates, the growth level (`0x5705d1 = 0x570223 * 0x5350b8`), the prime rate
    /// (0.08) and economic state (2), each company (`0x43dca0`), person (`0x4a7960`), train
    /// (`0x4d4660`), building (`0x40b820`) and place (`0x4317f0`), the track pieces' counters,
    /// the engine availability bytes and the "type seen" flags.
    pub(super) fn game_set_date(&mut self, w: &mut World, d: Date) {
        let minutes = d.to_minutes();
        self.set_clock(w, minutes);
        let mut date = [0u8; 4];
        date[..2].copy_from_slice(&d.year.to_le_bytes());
        date[2] = d.month;
        date[3] = d.day;
        let tod = [d.hour, d.minute];
        let k = self.data.image.f64(0x0053_50b8);
        let level = i32_at(&self.save.header, 0x86EB);
        let growth = (f64::from(level) * k) as f32;
        {
            let st = &mut self.save.state;
            st[0x10..0x14].copy_from_slice(&date);
            st[0x14..0x16].copy_from_slice(&tod);
            put_u32(st, 0x0C, minutes as u32);
            put_u32(st, 0xE9, growth.to_bits());
            for o in [0x237usize, 0x16, 0x20] {
                st[o..o + 4].copy_from_slice(&date);
            }
            for o in [0x23Busize, 0x1A, 0x24] {
                st[o..o + 2].copy_from_slice(&tod);
            }
            put_u32(st, 0x23D, 0);
            st[0x2D5] = 0;
            put_u32(st, 0x04, 0);
            put_u32(st, 0x08, 0);
            put_u32(st, 0x1E9, 0x3da3_d70a);
            put_u32(st, 0x22F, 2);
            put_u32(st, 0x233, 0);
            put_u32(st, 0x1F5, minutes as u32);
        }
        let year = i32::from(d.year);
        let date_u32 = u32::from_le_bytes(date);
        let date_w = d.year;
        for c in pool::list_order(&self.save.companies) {
            let Some(r) = pool::record_mut(&mut self.save.companies, c) else {
                continue;
            };
            let x = &mut r.data;
            put_u32(x, 0x148, year as u32);
            put_u32(x, 0x425c, minutes as u32);
            x[0x470..0x472].copy_from_slice(&date_w.to_le_bytes());
            x[0x46e..0x470].fill(0);
            put_u32(x, 0x16a, 0);
            put_u32(x, 0x158, 0);
            x[0x472..0x474].fill(0);
            put_u32(x, 0x424c, 0);
            put_u32(x, 0x154, 0);
            put_u32(x, 0x14c, 0);
            x[0x4245..0x4247].fill(0);
            x[0x4243..0x4245].fill(0);
            put_u32(x, 0x46a, 0);
            put_u32(x, 0x34, 0);
            let q = self.company_quote(minutes, c, true);
            let Some(r) = pool::record_mut(&mut self.save.companies, c) else {
                continue;
            };
            let x = &mut r.data;
            put_u32(x, 0x3c, (q as f32).to_bits());
            put_u32(x, 0x14c, 0);
            put_u32(x, 0x15c, 0xf);
            // `0x4a49f0(1800, 1, 1, 0, 0)`: 1800-01-01 00:00.
            x[0x160..0x162].copy_from_slice(&0x708u16.to_le_bytes());
            x[0x162] = 1;
            x[0x163] = 1;
            x[0x164] = 0;
            x[0x165] = 0;
            let t = w.rng.range(0x1e, 0x5a) * 0x5a0 + minutes;
            put_u32(x, 0x19e, 0);
            put_u32(x, 0x1a2, 0);
            put_u32(x, 0x1a6, 0);
            put_u32(x, 0x17e, t as u32);
            x[0x4286..0x4286 + 0x68].fill(0);
        }
        for p in pool::list_order(&self.save.persons) {
            let v = w.rng.range(0x3c, 0x1c2) * 0x5a0;
            if let Some(r) = pool::record_mut(&mut self.save.persons, p) {
                put_u32(&mut r.data, 0x267, v as u32);
            }
        }
        for t in pool::list_order(&self.save.trains) {
            let Some(r) = pool::record_mut(&mut self.save.trains, t) else {
                continue;
            };
            let x = &mut r.data;
            x.copy_within(0x10d..0x12d, 0x11d);
            x.copy_within(0xfd..0x10d, 0x10d);
            x[0xfd..0x10d].fill(0);
            x[0xbd..0xbf].copy_from_slice(&date_w.to_le_bytes());
            x[0xa9..0xab].copy_from_slice(&date_w.to_le_bytes());
            put_u32(x, 0xbf, 0);
            put_u32(x, 0xc3, 0);
            put_u32(x, 0x71, 0);
            put_u32(x, 0x2f, 0);
            put_u32(x, 0x6d, minutes as u32);
            put_u32(x, 0xc7, minutes as u32);
        }
        self.buildings_set_date(w, year, minutes);
        for p in pool::list_order(&self.save.places) {
            if let Some(r) = pool::record_mut(&mut self.save.places, p) {
                let x = &mut r.data;
                put_u32(x, 0x36, 0);
                x[0x3e..0x42].fill(0);
                put_u32(x, 0x2e, 100);
                put_u32(x, 0x42, (minutes + 0x1d_8800) as u32);
            }
            put_u32(&mut self.save.state, 0x227, 0);
        }
        for r in &mut self.save.track_pieces.records {
            put_u32(&mut r.data, 0x1f, 0);
            put_u32(&mut r.data, 0x1b, 0);
        }
        for e in 0..0x42usize {
            let v = self.engine_available(e, year, i32::from(d.month));
            self.save.state[0x3845 + e] = u8::from(v);
        }
        for t in 0..53u32 {
            let from = self.data.image.i32(0x0053_5c44 + 0x27 * t);
            self.save.state[0x11B + t as usize] = u8::from(year >= from);
        }
        let _ = date_u32;
    }
}
