//! World-mutating construction primitives shared by the computer opponents and the player's
//! commands (docs/spec/terrain-track.md, "Construção"; docs/spec/ai.md 5.4): laying track along a
//! route (`track_lay_route` `0x4cc060`, with the earthwork of `track_level_edge` `0x427ff0` and
//! the tile rebuild `0x4cb090`), the station networks (`station_networks_rebuild` `0x46a370`),
//! station buildings (`building_create` `0x409010` -> `building_construct` `0x409a30`), station
//! improvements (`station_buy_improvements` `0x46aef0`, `ai_station_upgrades` `0x434850`).
//!
//! The functions follow the original one by one on the raw records of the save ([`Ts`]); the
//! construction context is the AI's [`Ai`] (the train-step view plus the track globals
//! `track_single`/`track_bridge_type`/`track_electric`, the pathfinder and the current company),
//! so the AI and the player's [`Command`](crate::command::Command)s run the same code.
//!
//! What only the user interface sees (redraw rectangles `0x417770`/`0x5036d0`, the overlay
//! sprite codes of `0x425960`, sounds, the cost tooltip) is left out; what the save keeps of
//! them (the tile flags derived from the overlay lists) is reproduced.

#![allow(
    clippy::too_many_arguments,
    clippy::needless_range_loop,
    clippy::needless_late_init
)]

macro_rules! todo_hook {
    ($log:expr, $name:literal, $what:literal, $spec:literal) => {
        $log.skip($name, $what, $spec)
    };
}

pub mod bulldoze;
pub mod cmd;
pub mod confiscate;
pub mod improve;
pub mod level;
pub mod network;
pub mod place;
pub mod rights;
pub mod route;
pub mod station;
pub mod tiles;
pub mod train;

use crate::ai::Ai;
use crate::trainstep::{P, Ts};

pub use crate::ai::path::{DX, DY};

/// Corner offsets of a tile in the elevation grid (`0x580fd8`, set by `0x472980`): NW, NE, SE,
/// SW, as offsets from the tile's NW vertex; the stride is `width + 1` (`0x5609ac`).
pub fn corner_offsets(width: i32) -> [i32; 4] {
    [0, 1, width + 2, width + 1]
}

/// `0x4398d0` on the raw company record (finance.md 1): account 0 gains `x` against equity,
/// account `k != 0` gains `x` against cash; the price cache stamp `+0x4250` is cleared.
pub fn account_move(ts: &mut Ts<'_>, company: i32, account: usize, x: f64) {
    let mut r = ts.record(P::Company, company);
    crate::pool::put_u32(&mut r, 0x4250, 0);
    let get = |r: &[u8], o: usize| crate::pool::f64_at(r, o);
    if account == 0 {
        let v = x + get(&r, 0x4766);
        crate::pool::put_f64(&mut r, 0x4766, v);
        let v = get(&r, 0x479e) - x;
        crate::pool::put_f64(&mut r, 0x479e, v);
    } else {
        let o = 0x4766 + 8 * account;
        let v = x + get(&r, o);
        crate::pool::put_f64(&mut r, o, v);
        let v = get(&r, 0x4766) - x;
        crate::pool::put_f64(&mut r, 0x4766, v);
    }
    ts.set_record(P::Company, company, &r);
}

/// `company_ledger_credit` (`0x439970`) on the raw record: category `cat` of the month and of
/// the year, cash and (negated) equity.
pub fn ledger_credit(ts: &mut Ts<'_>, company: i32, cat: usize, x: f64) {
    ts.ledger_credit(company, cat, x);
}

impl Ai<'_, '_> {
    /// Vertex index of corner `c` of tile `(x, y)` in the elevation grid.
    pub fn corner_index(&self, x: i32, y: i32, c: usize) -> i32 {
        let w = self.ts.map_w();
        y * (w + 1) + x + corner_offsets(w)[c & 3]
    }

    /// Height of a vertex (`0x574c28 + i*3`, i16).
    pub fn vertex(&self, i: i32) -> i32 {
        i32::from(
            self.ts
                .s
                .map
                .elevation
                .get(i as usize)
                .copied()
                .unwrap_or(0),
        )
    }

    pub fn set_vertex(&mut self, i: i32, v: i32) {
        if let Some(e) = self.ts.s.map.elevation.get_mut(i as usize) {
            *e = v as i16;
        }
    }

    /// Height of corner `c` of tile `(x, y)`.
    pub fn corner_h(&self, x: i32, y: i32, c: usize) -> i32 {
        self.vertex(self.corner_index(x, y, c))
    }

    /// `track_piece_ptr(tile+7)+0` with the tile in the map (0 outside or without piece).
    pub fn mask_at(&self, x: i32, y: i32) -> u32 {
        if !self.in_map(x, y) {
            return 0;
        }
        self.piece_mask(x, y)
    }
}
