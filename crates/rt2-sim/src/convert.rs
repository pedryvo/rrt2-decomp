//! The loader's conversion of old save versions (`game_load_file` `0x477a60`) to the in-memory
//! layout of version 0x2348, the only one the game writes (`docs/formats/map-gam.md` 1.1 and 9).
//!
//! Every shipped `.MAP`/`.GAM` is version 0x233C. On load the original grows the header, the
//! global state block and seven record types, fills the new fields with defaults, remaps the old
//! terrain ids and reruns its terrain passes; saving then writes version 0x2348. [`upgrade`]
//! reproduces that byte for byte: the result equals what the original saves right after loading
//! the file (0 days advanced) when both start from the same game RNG state (the loader consumes
//! RNG: terrain variants and the passes, `map-gam.md` 9.4).

use rt2_formats::map::{Layout, MAGIC_2348, Pool, Save, TrainExtra};
use rt2_formats::terrain::{GameRng, Node, TerrainGrid, TileBytes};

use crate::World;
use crate::ai::Ai;
use crate::tick::SimState;
use crate::trainstep::P;

/// Version word of the shipped files.
pub const V233C: u32 = 0x233C;

/// Converts a save of an older version (only 0x233C exists in the install) to the 0x2348 layout as
/// the loader `0x477a60` does, consuming the game RNG `rng` like the original. A 0x2348 save is
/// returned unchanged (the loader's version-independent passes are not applied to it).
pub fn upgrade(save: &Save, rng: &mut GameRng) -> Save {
    let v = save.layout.version;
    if v >= MAGIC_2348 {
        return save.clone();
    }
    assert_eq!(v, V233C, "only versions 0x233C and 0x2348 are parsed");
    let mut s = save.clone();
    s.layout = Layout::V2348;
    s.map.version = MAGIC_2348;
    s.header = upgrade_header(&save.header);
    s.state = upgrade_state(&save.state);
    upgrade_tiles(&mut s, rng);
    upgrade_pools(&mut s);
    upgrade_train_extras(&mut s);
    for r in &mut s.rules {
        r.fixed = upgrade_rule_fixed(&r.fixed);
    }
    s.stored_checksum = 0;
    s
}

/// A fresh `_malloc` block as the oracle sees it: the original runs under a debugger there, so
/// the Windows heap fills new blocks with `0xBAADF00D`. The loader leaves some converted bytes
/// unwritten and the saver writes them as they are; reproducing the fill keeps the converted
/// save identical to the oracle's (in an undebugged run they would be leftover heap contents).
fn heap_block(len: usize) -> Vec<u8> {
    const FILL: [u8; 4] = 0xBAAD_F00Du32.to_le_bytes();
    (0..len).map(|i| FILL[i % 4]).collect()
}

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

/// Header block (`0x567b38`, version word included) plus the 0x86 block and the view.
/// `< 0x2341`: 0x878F bytes are read into a zeroed 0x898A area; the 38 nine-byte entries of the
/// old table at `0x5700cd` (file 0x8595) are copied to `0x5702cb` (0x8793) and 15 new entries get
/// the defaults (1, 100, 2). Every load then sets byte 0 of entries 49..52 to 1, and `< 0x2340`
/// clears `0x570227..0x57022a`.
fn upgrade_header(old: &[u8]) -> Vec<u8> {
    const OLD_HDR: usize = 0x8793;
    const NEW_HDR: usize = 4 + 0x898A;
    const TABLE: usize = 0x8793;
    let mut h = vec![0u8; NEW_HDR];
    h[..OLD_HDR].copy_from_slice(&old[..OLD_HDR]);
    put_u32(&mut h, 0, MAGIC_2348);
    for k in 0..53 {
        let e = TABLE + k * 9;
        if k < 38 {
            let src = 0x8595 + k * 9;
            h[e..e + 9].copy_from_slice(&old[src..src + 9]);
        } else {
            h[e] = 1;
            put_u32(&mut h, e + 1, 100);
            put_u32(&mut h, e + 5, 2);
        }
    }
    h[0x8979..0x898E].fill(0);
    for k in 49..53 {
        h[TABLE + k * 9] = 1;
    }
    // `< 0x2340`.
    h[0x86EF..0x86F2].fill(0);
    // Every load: four 32-byte entries at `0x570250` start with 0x7F and nine zero bytes.
    for k in 0..4 {
        let e = 0x8718 + k * 0x20;
        h[e] = 0x7F;
        h[e + 1..e + 11].fill(0);
    }
    let mut out = h;
    out.extend_from_slice(&old[OLD_HDR..]); // 0x86 block + view (0x40)
    out
}

/// Global state block (`game_running` `0x5704e8`): 0x29C8 bytes in 0x233C, 0x3A74 in 0x2348.
/// Three steps of the loader (`< 0x233F`, `< 0x2341`, `< 0x2347`), each inserting zeroed fields.
fn upgrade_state(old: &[u8]) -> Vec<u8> {
    assert_eq!(old.len(), 0x29C8);
    // `< 0x233F`: 0x29C8 -> 0x39CB in a fresh malloc block: tmp+0x2958..+0x3744 and
    // +0x395B..+0x39CB are never written and end up in state+0x2A31..+0x381D and +0x3A38..
    // (see [`heap_block`]).
    let mut tmp = heap_block(0x39CB);
    tmp[..0x29C8].copy_from_slice(old);
    tmp[0xEB..0xEB + 0xCF].copy_from_slice(&old[0xDF..0xDF + 0xCF]);
    tmp[0x1E6..0x1E6 + 0x2772].copy_from_slice(&old[0x1CE..0x1CE + 0x2772]);
    tmp[0x3744..0x3744 + 0x217].copy_from_slice(&old[0x2729..0x2729 + 0x217]);
    tmp[0x4C1..0x4C5].fill(0);
    tmp[0xDD..0xE9].fill(0);
    tmp[0x1DA..0x1E6].fill(0);
    let a = tmp;
    // `< 0x2341`: 0x39CB -> 0x3A74.
    let mut b = vec![0u8; 0x3A74];
    b[..0x111].copy_from_slice(&a[..0x111]);
    b[0x120..0x4D0].copy_from_slice(&a[0x111..0x4C1]);
    b[0x56A..0x3A74].copy_from_slice(&a[0x4C1..0x39CB]);
    // `< 0x2347`: the block from +0xAE moves 0x30 bytes up into `game_running` + 0xDE.
    let mut s = b.clone();
    s[0xDE..0xDE + 0x3956].copy_from_slice(&b[0xAE..0xAE + 0x3956]);
    let v = u32_at(&b, 0x3A00);
    put_u32(&mut s, 0x3A34, v);
    let f = u32_at(&s, 0x3A26) & 0xF7FF_FFFF;
    put_u32(&mut s, 0x3A26, f);
    put_u32(&mut s, 0xDE, 0);
    s
}

/// Tiles: `< 0x233D` maps the old id to its class's first id (`0x552b00`), `< 0x2343` picks a
/// random variant (`0x42a3e0`). The terrain passes run later ([`SimState::load_passes`]).
fn upgrade_tiles(s: &mut Save, rng: &mut GameRng) {
    let (w, h) = (s.map.width, s.map.height);
    let tiles = s.map.tiles.clone();
    let tile = |x: usize, y: usize| {
        let t = &tiles[y * w + x];
        TileBytes {
            b0: t.var,
            b1: t.ground,
            b2: t.flags2,
            b3: t.trees,
            b6: t.flags6,
        }
    };
    let g = TerrainGrid::from_old_ids(w, h, tile, rng);
    for (t, c) in s.map.tiles.iter_mut().zip(&g.cells) {
        t.var = c.v as u8;
        t.ground = (c.v >> 8) as u8;
        t.flags2 = c.flags2;
        t.trees = c.flags3;
        t.flags6 = c.flags6;
    }
}

/// Grows every live record of `pool` to `size` and applies `f`. The pool reader (`0x4f1bc0`)
/// reads the old record size into slots of the new size in a fresh array (`0x4f14c0`), so the
/// tail is heap fill until the converter writes it ([`heap_block`], phase from the array start).
fn grow(pool: &mut Pool, size: usize, mut f: impl FnMut(&mut Vec<u8>)) {
    const FILL: [u8; 4] = 0xBAAD_F00Du32.to_le_bytes();
    pool.record_size = size;
    for r in &mut pool.records {
        let old = r.data.len();
        let base = r.slot * size;
        r.data.extend((old..size).map(|o| FILL[(base + o) % 4]));
        f(&mut r.data);
    }
}

fn upgrade_pools(s: &mut Save) {
    let w = s.map.width;
    let region = |tiles: &[rt2_formats::map::Tile], x: u32, y: u32| -> u8 {
        tiles
            .get(y as usize * w + x as usize)
            .map(|t| t.region)
            .unwrap_or(0)
    };
    // Stations 0x39B -> 0x4C7 (`< 0x233E`): old +0x381..+0x390 move to +0x4AD, +0x61..+0x4AD are
    // cleared; `< 0x2345` clears the u16 at +0x4BE; every load stores the region of the station's
    // tile at +0x4AD.
    let tiles = s.map.tiles.clone();
    grow(&mut s.stations, 0x4C7, |r| {
        let moved: Vec<u8> = r[0x381..0x390].to_vec();
        r[0x4AD..0x4BC].copy_from_slice(&moved);
        r[0x61..0x4AD].fill(0);
        r[0x4BE..0x4C0].fill(0);
        let reg = region(&tiles, u32_at(r, 10), u32_at(r, 0xE));
        put_u32(r, 0x4AD, u32::from(reg));
    });
    // Persons 0x4DC -> 0x576 (`< 0x2341`): +0x48D..+0x4DC move to +0x527, a zeroed gap between.
    grow(&mut s.persons, 0x576, |r| {
        let tail: Vec<u8> = r[0x48D..0x4DC].to_vec();
        r[0x527..0x576].copy_from_slice(&tail);
        r[0x48D..0x527].fill(0);
    });
    // Companies 0x196C -> 0x4DEC (`< 0x233E`) -> 0x4E86 (`< 0x2341`).
    grow(&mut s.companies, 0x4E86, |r| {
        let old: Vec<u8> = r[..0x196C].to_vec();
        let mut a = vec![0u8; 0x4DEC];
        a[..0x196C].copy_from_slice(&old);
        a[0x417F..0x4DEC].copy_from_slice(&old[0xCFF..0x196C]);
        for k in 0..32 {
            let dst = 0x3DF + k * 0x1ED;
            let src = 0x3DF + k * 0x49;
            a[dst..dst + 5].copy_from_slice(&old[src..src + 5]);
            a[dst + 5..dst + 0x1ED].fill(0);
        }
        r[..0x3D0].copy_from_slice(&a[..0x3D0]);
        r[0x3D0..0x46A].fill(0);
        r[0x46A..0x4E86].copy_from_slice(&a[0x3D0..0x4DEC]);
    });
    // Pool `c7c` 0x11 -> 0x15 (`< 0x2344`): two zeroed u16.
    grow(&mut s.pool_7c, 0x15, |r| r[0x11..0x15].fill(0));
    // Trains 0x143 -> 0x14D (`< 0x2346`): +0xFC = 0, +0xF3 = 1.
    grow(&mut s.trains, 0x14D, |r| {
        r[0xFC] = 0;
        r[0xF3] = 1;
    });
    // Places 0x96 -> 0xB4, territories 0x27A -> 0x314 (`< 0x2341`): zeroed tails.
    grow(&mut s.places, 0xB4, |r| r[0x96..0xB4].fill(0));
    grow(&mut s.territories, 0x314, |r| r[0x27A..0x314].fill(0));
    // Map objects (`c74`, every load): +0x42 = 0; +0x9D = region of the owning station (+7 == 0)
    // or of the object's tile.
    let stations = s.stations.clone();
    for r in &mut s.objects.records {
        put_u32(&mut r.data, 0x42, 0);
        let reg = if r.data[7] == 0 {
            let slot = u16::from_le_bytes([r.data[8], r.data[9]]) as usize;
            stations.get(slot).map(|st| st.data[0x4AD]).unwrap_or(0)
        } else {
            let x = i16::from_le_bytes([r.data[0x22], r.data[0x23]]) as i32;
            let y = i16::from_le_bytes([r.data[0x24], r.data[0x25]]) as i32;
            region(&tiles, x as u32, y as u32)
        };
        r.data[0x9D] = reg;
    }
}

/// Train consists and routes: cars 10 -> 14 bytes (`< 0x2347`: byte 3 bit 3 cleared, two zeroed
/// u16), the last route array u8 -> u16 (`< 0x2348`: zeroed, rebuilt by `0x4d14b0`).
fn upgrade_train_extras(s: &mut Save) {
    for e in &mut s.train_extras {
        upgrade_extra(e);
    }
    for r in &mut s.trains.records {
        put_u32(&mut r.data, 0xAB, 0);
    }
}

fn upgrade_extra(e: &mut TrainExtra) {
    let mut cars = Vec::with_capacity(e.cars_raw.len() / 10 * 14);
    for c in e.cars_raw.as_chunks::<10>().0 {
        let mut car = [0u8; 14];
        car[..10].copy_from_slice(c);
        car[3] &= 0xF7;
        // `< 0x233D`: the 6-byte field at +4 used ' ' and '!' where the game now uses ',' and '-'.
        for b in &mut car[4..10] {
            match *b {
                b' ' => *b = b',',
                b'!' => *b = b'-',
                _ => {}
            }
        }
        cars.extend_from_slice(&car);
    }
    e.cars_raw = cars;
    if !e.route_raw.is_empty() {
        let n = e.route_raw.len() / 7;
        let mut route = e.route_raw[..6 * n].to_vec();
        route.extend(std::iter::repeat_n(0u8, 2 * n));
        e.route_raw = route;
    }
}

/// Fixed part of a scenario rule (`rule_read` `0x457c90`, `< 0x2341`): 0x2C8 -> 0x362. Two
/// 0x111-byte blocks at +0x64 grow to 0x15E (zeroed tails), old +0x286..+0x2C8 move to +0x320,
/// and the loader sets the u32 at +0x32E to 1.
fn upgrade_rule_fixed(old: &[u8]) -> Vec<u8> {
    let mut n = vec![0u8; 0x362];
    n[..0x2C8].copy_from_slice(old);
    n[0x320..0x362].copy_from_slice(&old[0x286..0x2C8]);
    for j in 0..2 {
        let d = 0x64 + j * 0x15E;
        let s = 0x64 + j * 0x111;
        n[d..d + 0x111].copy_from_slice(&old[s..s + 0x111]);
        n[d + 0x111..d + 0x15E].fill(0);
    }
    put_u32(&mut n, 0x32E, 1);
    n
}

impl SimState {
    /// The loader's passes after the file is read (`0x477a60` from the train extras on), run on a
    /// save converted by [`upgrade`] from `version`: the route refresh of each train (`0x4d14b0`,
    /// `< 0x2348`), the track tile rebuild of the whole map (`0x4cb090`) and the terrain passes
    /// (`0x4294d0` rivers, `0x428fe0` roads, `0x42a7d0` ground), which see the track overlays.
    pub(crate) fn load_passes(&mut self, w: &mut World, version: u32) {
        self.with_trainstep(w, |ts| {
            if version < MAGIC_2348 {
                let mut t = ts.first_id(P::Train);
                while t != 0 {
                    if ts.i32(P::Train, t, 0x59) != 0 && ts.i32(P::Train, t, 0x65) != 0 {
                        ts.train_route_refresh(t);
                    }
                    t = ts.next_id(P::Train, t);
                }
            }
            difficulty_flags(ts.s);
            let (mw, mh) = (ts.s.map.width, ts.s.map.height);
            let mut ai = Ai::new(ts);
            ai.tile_rebuild(0, 0, mw as i32 - 1, mh as i32 - 1);
            let mut track = vec![false; mw * mh];
            for y in 0..mh {
                for x in 0..mw {
                    track[y * mw + x] = ai.track_overlay(x as i32, y as i32);
                }
            }
            let ts = &mut *ai.ts;
            terrain_passes(ts, &track);
            late_fixups(ts);
            if version < 0x2344 {
                // Every live station in slot order (`0x4f1e30`), by the id in its record.
                let ids: Vec<i32> =
                    ts.s.stations
                        .records
                        .iter()
                        .map(|r| u32_at(&r.data, 0) as i32)
                        .collect();
                for st in ids {
                    station_unlink(ai.ts, st);
                    ai.station_rebuild_links(st);
                }
            }
        });
    }
}

/// The realism options the Load Game wrapper copies into the state (`0x495700` with the editor
/// flag `0x5748f8` clear): from the byte at `0x574872` (trailer +0x32), `0x573d72..0x573d78`.
fn difficulty_flags(s: &mut Save) {
    let opt = s.trailer[0x32];
    let st = &mut s.state;
    let b73 = u8::from((opt & 0x38) >= 0x10);
    let b75 = u8::from((opt & 0x38) >= 8);
    let b78 = opt & 7;
    st[0x388B] = b73;
    st[0x388D] = b75;
    st[0x3890] = b78;
    st[0x388E] = u8::from(b78 >= 2);
    st[0x388F] = u8::from(b78 == 0);
    st[0x388A] = b73;
    st[0x388C] = b75;
}

/// `0x406f60(station)`: clears the station's catchment link chain (`+0x3d`, count `+0x41`) and
/// removes, from each building (`c74`, list order), the first link of its chain (`+0x2f`, count
/// `+0x33`) that points at the station, freeing it in `pool_58`.
fn station_unlink(ts: &mut crate::trainstep::Ts<'_>, st: i32) {
    ts.seti32(P::Station, st, 0x3d, 0);
    ts.seti32(P::Station, st, 0x41, 0);
    let mut b = ts.first_id(P::Industry);
    while b != 0 {
        // `prev` = None: the head field of the building, else the `+6` of the previous link.
        let mut prev: Option<i32> = None;
        let mut l = ts.i32(P::Industry, b, 0x2f);
        while l != 0 {
            if i32::from(ts.u16(P::Link, l, 2)) == st {
                let next = ts.i32(P::Link, l, 6);
                ts.free(P::Link, l);
                match prev {
                    None => ts.seti32(P::Industry, b, 0x2f, next),
                    Some(p) => ts.seti32(P::Link, p, 6, next),
                }
                let c = ts.u8(P::Industry, b, 0x33).wrapping_sub(1);
                ts.set8(P::Industry, b, 0x33, c);
                break;
            }
            prev = Some(l);
            l = ts.i32(P::Link, l, 6);
        }
        b = ts.next_id(P::Industry, b);
    }
}

/// The terrain passes of the loader on the save's tiles, with the track overlays of `0x4cb090`
/// present on the tiles marked in `track`.
fn terrain_passes(ts: &mut crate::trainstep::Ts<'_>, track: &[bool]) {
    let (mw, mh) = (ts.s.map.width, ts.s.map.height);
    {
        {
            let tiles = &ts.s.map.tiles;
            let mut g = TerrainGrid::from_new_ids(mw, mh, |x, y| {
                let t = &tiles[y * mw + x];
                TileBytes {
                    b0: t.var,
                    b1: t.ground,
                    b2: t.flags2,
                    b3: t.trees,
                    b6: t.flags6,
                }
            });
            // The track sprites of `0x4cb090` (layers 4 and 5) are already in the tiles' overlay
            // lists: a tile with overlays takes a blend as a node instead of a new base id.
            for (c, &on) in g.cells.iter_mut().zip(track) {
                if on {
                    c.nodes.insert(
                        0,
                        Node {
                            word: 4,
                            turns: 0,
                            shade: 0,
                        },
                    );
                }
            }
            let mut rng = GameRng(ts.rng.state());
            g.build_overlays(&mut rng);
            *ts.rng = crate::rng::GameRng::new(rng.0);
            // The Load Game wrapper then redraws the territory borders (`0x495700` ->
            // `0x42bf90`).
            let region: Vec<u8> = ts.s.map.tiles.iter().map(|t| t.region).collect();
            let terr = &ts.s.territories;
            g.build_territory_borders(&region, |r| {
                terr.get(usize::from(r)).is_some_and(|t| t.data[0x24] != 0)
            });
            for (t, c) in ts.s.map.tiles.iter_mut().zip(&g.cells) {
                t.var = c.v as u8;
                t.ground = (c.v >> 8) as u8;
                t.flags2 = c.flags2;
                t.trees = c.flags3;
                t.flags6 = c.flags6;
            }
        }
    }
}

/// The fields the saver (`0x477540`) recomputes before writing, with the editor flag
/// `0x5748f8` clear: for each player in the person list (`0x570775` count, `0x570779` ids) the
/// u32 at `+0x54d` is cleared unless `+0x122` is set (then it gets a session value, masked in
/// comparisons); the header's player counts at `0x567c88..0x567c9c` (all, all, 1, with `+0x122`,
/// without, all - 1); the map size copy at `0x567d69`/`0x567d6b`; `0x570235` and the checksum
/// slot `0x57022f` cleared; `0x570233 = (0x570234 != 0)`.
pub(crate) fn saver_refresh(s: &mut Save) {
    const ST_PL_COUNT: usize = 0x28d;
    const ST_PL_LIST: usize = 0x291;
    let n = u32_at(&s.state, ST_PL_COUNT) as i32;
    let mut with = 0i32;
    for k in 0..n.max(0) as usize {
        let o = ST_PL_LIST + 2 * k;
        let id = u16::from_le_bytes([s.state[o], s.state[o + 1]]) as usize;
        let Some(r) = s.persons.records.iter_mut().find(|r| r.slot == id) else {
            continue;
        };
        if r.data.len() < 0x551 {
            continue;
        }
        if u32_at(&r.data, 0x122) == 0 {
            put_u32(&mut r.data, 0x54d, 0);
        } else {
            with += 1;
        }
    }
    if s.layout.version < MAGIC_2348 {
        return;
    }
    let h = &mut s.header;
    for (i, v) in [n, n, 1, with, n - with, n - 1].into_iter().enumerate() {
        put_u32(h, 0x150 + 4 * i, v as u32);
    }
    let (w, ht) = (s.map.width as u16, s.map.height as u16);
    h[0x231..0x233].copy_from_slice(&w.to_le_bytes());
    h[0x233..0x235].copy_from_slice(&ht.to_le_bytes());
    h[0x86FD] = 0;
    h[0x86F7] = 0;
    h[0x86FB] = u8::from(h[0x86FC] != 0);
}

fn i32_at(b: &[u8], off: usize) -> i32 {
    u32_at(b, off) as i32
}

/// The loader's version-independent fixups after the terrain passes (`0x477a60` from
/// `0x478abb`): default view bounds, territory display ids, start/end year sanity, the station
/// bit of each tile, the first car of the first train in the state, and the hard-coded patches of
/// six shipped scenarios.
fn late_fixups(ts: &mut crate::trainstep::Ts<'_>) {
    // `0x573d7e..0x573d8e` all zero: 9999, 9999, -1, -1.
    {
        let st = &mut ts.s.state;
        if (0..4).all(|k| i32_at(st, 0x3896 + 4 * k) == 0) {
            for (k, v) in [9999i32, 9999, -1, -1].into_iter().enumerate() {
                put_u32(st, 0x3896 + 4 * k, v as u32);
            }
        }
    }
    // Territories 1..=count (`0x466830` by slot): a hidden one (`+0x24 == 0`) with no valid
    // display territory (`+0x2a`) shows as 1, a shown one as itself.
    let n = ts.s.territories.count as i32;
    for id in 1..n + 1 {
        if !ts.exists(P::Territory, id) {
            continue;
        }
        if ts.u8(P::Territory, id, 0x24) == 0 {
            let d = i32::from(ts.u8(P::Territory, id, 0x2a));
            if d == 0 || n <= d {
                ts.set8(P::Territory, id, 0x2a, 1);
            }
        } else {
            ts.set8(P::Territory, id, 0x2a, id as u8);
        }
    }
    // Start/end year bounds (`0x5700c5`, `0x5700c9`, header +0x858D/+0x8591) outside 1800..=2100
    // or on the wrong side of the start year (`0x5700c1`) are cleared.
    {
        let h = &mut ts.s.header;
        let start = i32_at(h, 0x8589);
        let a = i32_at(h, 0x858D);
        if !(0x708..=0x834).contains(&a) || start < a {
            put_u32(h, 0x858D, 0);
        }
        let b = i32_at(h, 0x8591);
        if !(0x708..=0x834).contains(&b) || b < start {
            put_u32(h, 0x8591, 0);
        }
    }
    // Tile byte 6 bit 1: a station is on the tile (`station_at` `0x4bf1d0`).
    let (w, h) = (ts.s.map.width, ts.s.map.height);
    for y in 0..h {
        for x in 0..w {
            let on = ts.station_at(x as i32, y as i32) != 0;
            let t = &mut ts.s.map.tiles[y * w + x];
            t.flags6 = if on { t.flags6 | 2 } else { t.flags6 & 0xFD };
        }
    }
    // The first train in list order with cars: its first car (14 bytes) goes to `0x573f0e`.
    if ts.s.trains.count != 0 {
        let t = ts.first_id(P::Train);
        if t != 0 && ts.u8(P::Train, t, 0x3b) != 0 {
            let car: Option<Vec<u8>> =
                ts.s.train_extras
                    .iter()
                    .find(|e| e.slot == t as usize)
                    .map(|e| e.cars_raw[..14].to_vec());
            if let Some(car) = car {
                ts.s.state[0x3A26..0x3A34].copy_from_slice(&car);
            }
        }
    }
    scenario_patches(ts);
}

/// A NUL-terminated string of the executable's data.
fn exe_str(img: &crate::data::DataImage, va: u32) -> String {
    let mut out = String::new();
    let mut a = va;
    loop {
        let b = img.u8(a);
        if b == 0 {
            return out;
        }
        out.push(b as char);
        a += 1;
    }
}

/// `rule_set_condition` (`0x457a30`) on rule `slot` (1-based, file order): new condition text,
/// compiled tokens, and the loop flags (`+0x320`/`+0x2c`/`+0x30` recomputed, `+0x324`/`+0x24`/
/// `+0x28` only ever set).
fn rule_set_condition(s: &mut Save, slot: usize, text: &str) {
    let Some(r) = s.rules.get_mut(slot - 1) else {
        return;
    };
    let Ok((tokens, values)) = crate::scenario::lang::compile(text) else {
        return;
    };
    let (c, p, t) = crate::scenario::lang::loop_flags(&tokens);
    let f = &mut r.fixed;
    put_u32(f, 4, tokens.len() as u32);
    put_u32(f, 0x320, 0);
    put_u32(f, 0x2c, 0);
    put_u32(f, 0x30, 0);
    if t {
        put_u32(f, 0x320, 1);
        put_u32(f, 0x324, 1);
    }
    if c {
        put_u32(f, 0x2c, 1);
        put_u32(f, 0x24, 1);
    }
    if p {
        put_u32(f, 0x30, 1);
        put_u32(f, 0x28, 1);
    }
    r.condition = text.to_string();
    let mut blob = text.as_bytes().to_vec();
    blob.push(0);
    r.raw_blobs[0] = blob;
    r.tokens_a = tokens;
    r.tokens_b = values;
}

/// The hard-coded fixes the loader applies to six shipped scenarios, recognised by map size,
/// player and company counts, start year and rule contents (`0x477a60`, `0x479068..0x47943c`;
/// single player, so the network version tests always pass). The replacement conditions are
/// strings of the executable.
fn scenario_patches(ts: &mut crate::trainstep::Ts<'_>) {
    let img = ts.img;
    let s = &mut *ts.s;
    let (w, h) = (s.map.width, s.map.height);
    let players = i32_at(&s.state, 0x28d);
    let companies = i32_at(&s.state, 0x241);
    let year = u16::from_le_bytes([s.state[0x10], s.state[0x11]]);
    let nrules = s.rules.len();
    let live = |k: usize| k >= 1 && k <= nrules;
    let rf = |s: &Save, k: usize| s.rules[k - 1].fixed.clone();
    if (w, h) == (350, 350) && players == 3 && companies == 3 && year == 0x73a && live(0x1c) {
        let f = rf(s, 0x1c);
        if i32_at(&f, 0x24) != 0 && i32_at(&f, 0x3c) == 0 && f[0x96] != 0 {
            put_u32(&mut s.rules[0x1b].fixed, 0x48, 0);
        }
    }
    if (w, h) == (400, 300)
        && players == 5
        && year == 0x744
        && [0x18, 1, 2, 3, 0xe].iter().all(|&k| live(k))
        && rf(s, 0x18)[0x15d] == b'd'
    {
        for (k, va) in [
            (0x18, 0x5598bc),
            (1, 0x5598e4),
            (2, 0x559964),
            (3, 0x5599e4),
            (0xe, 0x559a1c),
        ] {
            rule_set_condition(s, k, &exe_str(img, va));
            put_u32(&mut s.rules[k - 1].fixed, 0x34, 0);
        }
    }
    if (w, h) == (500, 500) && s.places.count == 0xcb && live(4) {
        let first = crate::pool::list_order(&s.places).first().copied();
        let p0 = first
            .and_then(|slot| s.places.get(slot))
            .map(|r| u16::from_le_bytes([r.data[4], r.data[5]]));
        if p0 == Some(0xed) && i32_at(&rf(s, 4), 4) == 0xb {
            put_u32(&mut s.rules[3].fixed, 0x48, 1);
            put_u32(&mut s.rules[3].fixed, 0x4c, 1);
        }
    }
    if (w, h) == (300, 300)
        && players == 1
        && companies == 1
        && year == 0x737
        && live(0xe)
        && live(7)
        && s.places.get(0x54).is_some()
    {
        let f = rf(s, 0xe);
        if i32_at(&f, 0x24) != 0 && i32_at(&f, 0x3c) != 0 && f[0x69] == 0xef {
            s.rules[0xd].fixed[0x69] = 0x11;
            rule_set_condition(s, 7, &exe_str(img, 0x559a2c));
            if let Some(r) = s.places.records.iter_mut().find(|r| r.slot == 0x54) {
                r.data[0x2d] = 0;
            }
        }
    }
    if (w, h) == (360, 350) && players == 1 && year == 0x7a8 && live(2) && live(0x12) {
        let f = rf(s, 2);
        if i32_at(&f, 0x24) != 0 && i32_at(&f, 0x3c) != 0 && rf(s, 0x12)[0xc3] == 0x14 {
            rule_set_condition(s, 2, &exe_str(img, 0x559a98));
        }
    }
    if (w, h) == (0x113, 0x113)
        && players == 4
        && companies == 4
        && year == 0x7a5
        && [0x17, 2, 3, 4].iter().all(|&k| live(k))
    {
        let f = rf(s, 0x17);
        if i32_at(&f, 0x24) != 0 && i32_at(&f, 0x3c) != 0 && f[0x75] == 10 && f[0x79] == 10 {
            rule_set_condition(s, 2, &exe_str(img, 0x559b48));
            rule_set_condition(s, 3, &exe_str(img, 0x559b94));
            rule_set_condition(s, 3, &exe_str(img, 0x559be0));
        }
    }
}
