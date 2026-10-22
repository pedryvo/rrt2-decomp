//! The train step's view of the world: the save's pools and tiles addressed like the original's
//! memory (record + offset), the data image of the executable and the two RNGs.
//!
//! The ported functions read and write the raw records at the original offsets, so the port can
//! be checked line by line against the disassembly. A record that does not exist reads as zeros
//! (the original would read the stale bytes of a free slot; the paths ported here never do).

use rt2_formats::map::{Pool, Save, Tile};

use crate::data::DataImage;
use crate::effects::{EffectLevels, effect_multiplier};
use crate::hooks::HookLog;
use crate::pool;
use crate::rng::{CrtRng, GameRng};

/// The slot pools the train step touches (`DAT_00574cxx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum P {
    /// `0x574c54`: tile structure chains (7 bytes: type, u32 id, u16 next).
    Struct,
    /// `0x574c60`: stations.
    Station,
    /// `0x574c68`: persons.
    Person,
    /// `0x574c6c`: companies.
    Company,
    /// `0x574c70`: vehicles (cars) of the trains, 0x4f bytes.
    Car,
    /// `0x574c74`: industries.
    Industry,
    /// `0x574c78`: map animation objects (smoke), 0x23 bytes.
    Anim,
    /// `0x574c80`: 5-byte records (train waiting markers, see trains.md 10).
    P80,
    /// `0x574c84`: track pieces, 0x24 bytes.
    Track,
    /// `0x574c64`: trains, 0x14d bytes.
    Train,
    /// `0x574ca8`: territories.
    Territory,
    /// `0x574c7c`: cargo lots (0x15 bytes).
    Lot,
    /// `0x574c58`: station catchment links (0xe bytes: u16 industry, ..., i32 next at +0xa).
    Link,
    /// `0x574c5c`: places (towns).
    Place,
}

/// `DAT_00574b84`-style runtime globals that live outside the save.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Globals {
    /// `0x574b84`: count of trains whose consist changed (`0x4d14b0` refresh pending).
    pub refresh_574b84: i32,
    /// The `restricted` output of `0x4d00c0` is left untouched when the train is stopped by a
    /// breakdown or a crash: `0x4d0400` then reads whatever its stack slot held, which is the
    /// value of the previous call in practice (approximation, trains.md 10.4).
    pub stale_restricted: bool,
    /// `0x5794f0`: a track change with 20 stations or more deferred `station_networks_rebuild`
    /// to the next `sim_month_end` (step 1).
    pub networks_dirty: bool,
    /// Output queue of player-visible messages ([`crate::message`]); never read by the rules.
    pub news: Vec<crate::message::GameMessage>,
}

pub struct Ts<'a> {
    pub s: &'a mut Save,
    pub img: &'a DataImage,
    pub anim_frames: &'a [i32],
    /// Industry types of the active set, fixed up and rebuilt for the year (the runtime
    /// `industry_type_table`).
    pub types: &'a [crate::cargo::IndustryType],
    pub cargo: &'a [crate::cargo::CargoType],
    pub revenue: &'a crate::cargo::RevenueTables,
    /// `game_time_minutes` (`0x5704f4`).
    pub now: u32,
    /// Game year (`0x5704f8`).
    pub year: u16,
    pub rng: &'a mut GameRng,
    pub crt: &'a mut CrtRng,
    pub log: &'a mut HookLog,
    pub g: Globals,
    /// `0x570229` (header byte): no breakdowns for the AI, no crashes, cargo 0x11 rules.
    pub flag_570229: u8,
    /// `0x570236` (header byte): every car weighs as loaded.
    pub flag_570236: u8,
}

pub fn pool_of(s: &Save, p: P) -> &Pool {
    match p {
        P::Struct => &s.structures,
        P::Station => &s.stations,
        P::Person => &s.persons,
        P::Company => &s.companies,
        P::Car => &s.pool_70,
        P::Industry => &s.objects,
        P::Anim => &s.pool_78,
        P::P80 => &s.pool_80,
        P::Track => &s.track_pieces,
        P::Train => &s.trains,
        P::Territory => &s.territories,
        P::Lot => &s.pool_7c,
        P::Link => &s.pool_58,
        P::Place => &s.places,
    }
}

pub fn pool_of_mut(s: &mut Save, p: P) -> &mut Pool {
    match p {
        P::Struct => &mut s.structures,
        P::Station => &mut s.stations,
        P::Person => &mut s.persons,
        P::Company => &mut s.companies,
        P::Car => &mut s.pool_70,
        P::Industry => &mut s.objects,
        P::Anim => &mut s.pool_78,
        P::P80 => &mut s.pool_80,
        P::Track => &mut s.track_pieces,
        P::Train => &mut s.trains,
        P::Territory => &mut s.territories,
        P::Lot => &mut s.pool_7c,
        P::Link => &mut s.pool_58,
        P::Place => &mut s.places,
    }
}

impl Ts<'_> {
    // ---- records ------------------------------------------------------------------------

    pub fn exists(&self, p: P, id: i32) -> bool {
        id > 0 && pool::record(pool_of(self.s, p), id as usize).is_some()
    }

    fn rd(&self, p: P, id: i32, o: usize, n: usize) -> [u8; 4] {
        let mut b = [0u8; 4];
        if id > 0
            && let Some(r) = pool::record(pool_of(self.s, p), id as usize)
        {
            b[..n].copy_from_slice(&r.data[o..o + n]);
        }
        b
    }

    fn wr(&mut self, p: P, id: i32, o: usize, v: &[u8]) {
        let r = pool::record_mut(pool_of_mut(self.s, p), id as usize)
            .unwrap_or_else(|| panic!("write to missing record {p:?}[{id}]"));
        r.data[o..o + v.len()].copy_from_slice(v);
    }

    pub fn u8(&self, p: P, id: i32, o: usize) -> u8 {
        self.rd(p, id, o, 1)[0]
    }
    pub fn i8(&self, p: P, id: i32, o: usize) -> i8 {
        self.u8(p, id, o) as i8
    }
    pub fn u16(&self, p: P, id: i32, o: usize) -> u16 {
        let b = self.rd(p, id, o, 2);
        u16::from_le_bytes([b[0], b[1]])
    }
    pub fn i16(&self, p: P, id: i32, o: usize) -> i16 {
        self.u16(p, id, o) as i16
    }
    pub fn u32(&self, p: P, id: i32, o: usize) -> u32 {
        u32::from_le_bytes(self.rd(p, id, o, 4))
    }
    pub fn i32(&self, p: P, id: i32, o: usize) -> i32 {
        self.u32(p, id, o) as i32
    }
    pub fn f32(&self, p: P, id: i32, o: usize) -> f32 {
        f32::from_bits(self.u32(p, id, o))
    }
    pub fn set8(&mut self, p: P, id: i32, o: usize, v: u8) {
        self.wr(p, id, o, &[v]);
    }
    pub fn set16(&mut self, p: P, id: i32, o: usize, v: u16) {
        self.wr(p, id, o, &v.to_le_bytes());
    }
    pub fn set32(&mut self, p: P, id: i32, o: usize, v: u32) {
        self.wr(p, id, o, &v.to_le_bytes());
    }
    pub fn seti32(&mut self, p: P, id: i32, o: usize, v: i32) {
        self.set32(p, id, o, v as u32);
    }
    pub fn setf32(&mut self, p: P, id: i32, o: usize, v: f32) {
        self.set32(p, id, o, v.to_bits());
    }

    /// The whole record (a copy), for the save/restore of `0x415040`.
    pub fn record(&self, p: P, id: i32) -> Vec<u8> {
        pool::record(pool_of(self.s, p), id as usize)
            .map(|r| r.data.clone())
            .unwrap_or_default()
    }

    pub fn set_record(&mut self, p: P, id: i32, data: &[u8]) {
        let r = pool::record_mut(pool_of_mut(self.s, p), id as usize).expect("record");
        r.data.copy_from_slice(data);
    }

    /// `pool_alloc(pool, data)` (`0x4f1690`).
    pub fn alloc(&mut self, p: P, data: Vec<u8>) -> i32 {
        pool::alloc(pool_of_mut(self.s, p), data).expect("pool grows") as i32
    }

    /// `pool_free` (`0x4f17e0`).
    pub fn free(&mut self, p: P, id: i32) {
        pool::free(pool_of_mut(self.s, p), id as usize);
    }

    pub fn record_size(&self, p: P) -> usize {
        pool_of(self.s, p).record_size
    }

    /// `pool_next_id` (`0x4f1960`).
    pub fn next_id(&self, p: P, id: i32) -> i32 {
        pool::next(pool_of(self.s, p), id as usize) as i32
    }

    /// `pool_nth_id_list_order(list, 0)`.
    pub fn first_id(&self, p: P) -> i32 {
        pool::first(pool_of(self.s, p)) as i32
    }

    // ---- global state block (offset = VA - 0x5704e8, version 0x2348) --------------------

    pub fn state_i32(&self, o: usize) -> i32 {
        let st = &self.s.state;
        i32::from_le_bytes([st[o], st[o + 1], st[o + 2], st[o + 3]])
    }

    pub fn set_state_i32(&mut self, o: usize, v: i32) {
        self.s.state[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// Slot `id` is a live record of the pool (`id < slots` and not free), the test the original
    /// makes before trusting an id read from a record.
    pub fn live(&self, p: P, id: i32) -> bool {
        id > 0 && pool::record(pool_of(self.s, p), id as usize).is_some()
    }

    // ---- tiles (15 bytes in memory: the 10 file bytes, then 5 runtime bytes) ---------------

    /// `DAT_0055e7a4`, `DAT_0055e7a0`.
    pub fn map_w(&self) -> i32 {
        self.s.map.width as i32
    }
    pub fn map_h(&self) -> i32 {
        self.s.map.height as i32
    }

    fn tile(&self, x: i32, y: i32) -> &Tile {
        let w = self.s.map.width;
        &self.s.map.tiles[(y as usize) * w + x as usize]
    }

    fn tile_mut(&mut self, x: i32, y: i32) -> &mut Tile {
        let w = self.s.map.width;
        &mut self.s.map.tiles[(y as usize) * w + x as usize]
    }

    pub fn t8(&self, x: i32, y: i32, o: usize) -> u8 {
        let t = self.tile(x, y);
        match o {
            0 => t.var,
            1 => t.ground,
            2 => t.flags2,
            3 => t.trees,
            4 => t.struct_id,
            5 => t.reserved5,
            6 => t.flags6,
            7 => t.track_id,
            8 => t.reserved8,
            9 => t.region,
            _ => panic!("runtime tile byte {o} not modelled"),
        }
    }

    pub fn set_t8(&mut self, x: i32, y: i32, o: usize, v: u8) {
        let t = self.tile_mut(x, y);
        match o {
            0 => t.var = v,
            1 => t.ground = v,
            2 => t.flags2 = v,
            3 => t.trees = v,
            4 => t.struct_id = v,
            5 => t.reserved5 = v,
            6 => t.flags6 = v,
            7 => t.track_id = v,
            8 => t.reserved8 = v,
            9 => t.region = v,
            _ => panic!("runtime tile byte {o} not modelled"),
        }
    }

    pub fn t16(&self, x: i32, y: i32, o: usize) -> u16 {
        u16::from_le_bytes([self.t8(x, y, o), self.t8(x, y, o + 1)])
    }

    pub fn set_t16(&mut self, x: i32, y: i32, o: usize, v: u16) {
        let b = v.to_le_bytes();
        self.set_t8(x, y, o, b[0]);
        self.set_t8(x, y, o + 1, b[1]);
    }

    /// `tile+7`: track piece id.
    pub fn tile_track(&self, x: i32, y: i32) -> i32 {
        i32::from(self.t16(x, y, 7))
    }

    /// `tile+0 >> 6` is the terrain id; `0x581090[id]` is its class (built at start-up by
    /// `0x471b10`, not in the image): only "is water" is needed here, see `terrain_is_water`.
    pub fn tile_terrain_id(&self, x: i32, y: i32) -> u16 {
        self.t16(x, y, 0) >> 6
    }

    // ---- trains: route ring buffers (+0x45..+0x55, n = +0x59) and stops (+0x37) ----------

    fn extra(&self, train: i32) -> &rt2_formats::map::TrainExtra {
        self.s
            .train_extras
            .iter()
            .find(|e| e.slot == train as usize)
            .unwrap_or_else(|| panic!("train {train} has no extras"))
    }

    fn extra_mut(&mut self, train: i32) -> &mut rt2_formats::map::TrainExtra {
        self.s
            .train_extras
            .iter_mut()
            .find(|e| e.slot == train as usize)
            .unwrap_or_else(|| panic!("train {train} has no extras"))
    }

    fn route_n(&self, train: i32) -> usize {
        self.i32(P::Train, train, 0x59).max(0) as usize
    }

    /// `+0x45[i]` (u16 x).
    pub fn rx(&self, train: i32, i: i32) -> i32 {
        let r = &self.extra(train).route_raw;
        let o = 2 * i as usize;
        i32::from(u16::from_le_bytes([r[o], r[o + 1]]))
    }
    /// `+0x49[i]` (u16 y).
    pub fn ry(&self, train: i32, i: i32) -> i32 {
        let n = self.route_n(train);
        let r = &self.extra(train).route_raw;
        let o = 2 * n + 2 * i as usize;
        i32::from(u16::from_le_bytes([r[o], r[o + 1]]))
    }
    /// `+0x4d[i]` (u8 entry direction).
    pub fn rdin(&self, train: i32, i: i32) -> u8 {
        let n = self.route_n(train);
        self.extra(train).route_raw[4 * n + i as usize]
    }
    /// `+0x51[i]` (u8 exit direction).
    pub fn rdout(&self, train: i32, i: i32) -> u8 {
        let n = self.route_n(train);
        self.extra(train).route_raw[5 * n + i as usize]
    }
    /// `+0x55[i]` (u16 step flags; only the low byte is used: bit 0 half-length tile, bits 1-2
    /// double-track side, bit 3 station stop).
    pub fn rflags(&self, train: i32, i: i32) -> u8 {
        let n = self.route_n(train);
        self.extra(train).route_raw[6 * n + 2 * i as usize]
    }
    pub fn set_rflags(&mut self, train: i32, i: i32, v: u8) {
        let n = self.route_n(train);
        self.extra_mut(train).route_raw[6 * n + 2 * i as usize] = v;
    }
    pub fn set_route_step(&mut self, train: i32, i: i32, x: u16, y: u16, din: u8, dout: u8) {
        let n = self.route_n(train);
        let r = &mut self.extra_mut(train).route_raw;
        let i = i as usize;
        r[2 * i..2 * i + 2].copy_from_slice(&x.to_le_bytes());
        r[2 * n + 2 * i..2 * n + 2 * i + 2].copy_from_slice(&y.to_le_bytes());
        r[4 * n + i] = din;
        r[5 * n + i] = dout;
    }
    /// Raw u16 of `+0x55[i]`.
    pub fn rflags16(&self, train: i32, i: i32) -> u16 {
        let n = self.route_n(train);
        let r = &self.extra(train).route_raw;
        let o = 6 * n + 2 * i as usize;
        u16::from_le_bytes([r[o], r[o + 1]])
    }
    pub fn set_rflags16(&mut self, train: i32, i: i32, v: u16) {
        let n = self.route_n(train);
        let r = &mut self.extra_mut(train).route_raw;
        let o = 6 * n + 2 * i as usize;
        r[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }

    /// Byte `o` of stop `k` (`+0x37 + k*0xe`).
    pub fn stop8(&self, train: i32, k: i32, o: usize) -> u8 {
        self.extra(train).cars_raw[k as usize * 0xe + o]
    }
    pub fn stop16(&self, train: i32, k: i32, o: usize) -> u16 {
        let b = &self.extra(train).cars_raw[k as usize * 0xe + o..];
        u16::from_le_bytes([b[0], b[1]])
    }
    pub fn set_stop8(&mut self, train: i32, k: i32, o: usize, v: u8) {
        self.extra_mut(train).cars_raw[k as usize * 0xe + o] = v;
    }

    /// The current stop index `+0x3c` (signed byte).
    pub fn cur_stop(&self, train: i32) -> i32 {
        i32::from(self.i8(P::Train, train, 0x3c))
    }

    // ---- data tables ----------------------------------------------------------------------

    /// Byte of the vehicle type record `t` (`0x53ddb8 + t*0x39 + o`).
    pub fn vt8(&self, t: u8, o: u32) -> u8 {
        self.img.u8(0x0053_ddb8 + u32::from(t) * 0x39 + o)
    }
    pub fn vt16(&self, t: u8, o: u32) -> u16 {
        self.img.u16(0x0053_ddb8 + u32::from(t) * 0x39 + o)
    }
    pub fn vt32(&self, t: u8, o: u32) -> i32 {
        self.img.i32(0x0053_ddb8 + u32::from(t) * 0x39 + o)
    }
    /// Engine index of vehicle type `t` (`+0x2f`, signed).
    pub fn vt_engine(&self, t: u8) -> i32 {
        i32::from(self.vt8(t, 0x2f) as i8)
    }
    /// Byte of the engine record `e` (`0x53cec0 + e*0x3a + o`).
    pub fn en8(&self, e: i32, o: u32) -> u8 {
        self.img
            .u8(0x0053_cec0u32.wrapping_add((e * 0x3a) as u32) + o)
    }
    pub fn en32(&self, e: i32, o: u32) -> i32 {
        self.img
            .i32(0x0053_cec0u32.wrapping_add((e * 0x3a) as u32) + o)
    }

    // ---- effect_multiplier (0x46b480) -------------------------------------------------------

    /// `effect_multiplier(e, company, person, territory)`: the person defaults to the company's
    /// (`+0x23`) when 0 (cargo.md 5).
    pub fn effect(&self, e: usize, company: i32, person: i32, territory: i32) -> f32 {
        let st = &self.s.state;
        let o = 0x2de + 2 * e;
        let global = i16::from_le_bytes([st[o], st[o + 1]]);
        let mut person = person;
        let company_pct = if company != 0 && self.exists(P::Company, company) {
            if person == 0 {
                person = self.i32(P::Company, company, 0x23);
            }
            Some(self.i16(P::Company, company, 0x1ae + 2 * e))
        } else {
            None
        };
        let person_pct = (person != 0 && self.exists(P::Person, person))
            .then(|| self.i16(P::Person, person, 0x26b + 2 * e));
        let terr_pct = (territory != 0 && self.exists(P::Territory, territory))
            .then(|| self.i16(P::Territory, territory, 0x58 + 2 * e));
        effect_multiplier(EffectLevels {
            global,
            company: company_pct,
            person: person_pct,
            territory: terr_pct,
        })
    }
}

/// `x & 0x8000001f` with the sign fix-up: the C remainder `x % 32` (sign of the dividend).
pub fn ring(x: i32) -> i32 {
    x % 32
}
