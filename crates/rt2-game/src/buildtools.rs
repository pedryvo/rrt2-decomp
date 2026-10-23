//! Construction and train-purchase logic behind the player's tools (no UI, no GPU).
//!
//! Everything the issuing windows of the original check before they send a command lives here
//! (docs/spec/commands.md 3.2 to 3.9): the scenario permissions, the track rights of the
//! territory, the engine list and its price, the station placement, the improvement table. The
//! commands themselves are the simulation's ([`rt2_sim::build::cmd::BuildCmd`], applied with
//! `World::apply_build`); a *preview* is the same command applied to a clone of the world, so
//! the path, the cost and the refusal are exactly what the real command would do, and the real
//! world is never touched (the clone shares nothing with it).
//!
//! The module only depends on `rt2-sim` and `rt2-formats` so the integration test
//! (`tests/e2e_build.rs`) can include it without the window.

#![allow(dead_code)]

use rt2_formats::map::{Pool, Save};
use rt2_sim::World;
use rt2_sim::build::cmd::{BuildCmd, CmdError};
use rt2_sim::data::GameData;
use rt2_sim::effects::{EffectLevels, effect_multiplier};
use rt2_sim::tilemark;

/// `0x5704e8`: first byte of the global state block (`Save::state`).
const ST_BASE: u32 = 0x0057_04e8;
/// `0x56 7b38` is the header base; the scenario option bytes live in the header.
const HDR_BASE: u32 = 0x0056_7b38;

pub const CO_CHAIRMAN: usize = 0x23;
pub const CO_ACTIVE: usize = 0x16e;
pub const CO_CASH: usize = 0x4766;
const PE_HUMAN: usize = 0x122;

// Offsets of the permission bytes, `0x5706a9 + i` (commands.md 3.9).
pub const PERM_RETIRE_TRAIN: u32 = 0x0057_06b4;
pub const PERM_BUY_TRAIN: u32 = 0x0057_06b5;
pub const PERM_LAY_TRACK: u32 = 0x0057_06b6;
pub const PERM_BUILD_STATION: u32 = 0x0057_06b7;
pub const PERM_BUILD_SUPPORT: u32 = 0x0057_06b8;
pub const PERM_BUY_INDUSTRY: u32 = 0x0057_06b9;

/// A little-endian view of a pool record.
#[derive(Clone, Copy)]
pub struct Rec<'a>(pub &'a [u8]);

impl Rec<'_> {
    pub fn u8(&self, o: usize) -> u8 {
        self.0.get(o).copied().unwrap_or(0)
    }
    pub fn u16(&self, o: usize) -> u16 {
        u16::from_le_bytes([self.u8(o), self.u8(o + 1)])
    }
    pub fn i16(&self, o: usize) -> i16 {
        self.u16(o) as i16
    }
    pub fn i32(&self, o: usize) -> i32 {
        i32::from_le_bytes([self.u8(o), self.u8(o + 1), self.u8(o + 2), self.u8(o + 3)])
    }
    pub fn u32(&self, o: usize) -> u32 {
        self.i32(o) as u32
    }
    pub fn f64(&self, o: usize) -> f64 {
        let mut b = [0u8; 8];
        for (k, v) in b.iter_mut().enumerate() {
            *v = self.u8(o + k);
        }
        let v = f64::from_le_bytes(b);
        if v.is_finite() { v } else { 0.0 }
    }
    pub fn f32(&self, o: usize) -> f32 {
        f32::from_bits(self.u32(o))
    }
}

pub fn rec(pool: &Pool, slot: i32) -> Option<Rec<'_>> {
    if slot <= 0 {
        return None;
    }
    pool.get(slot as usize).map(|r| Rec(&r.data))
}

/// Header byte at a virtual address of the original's data (`0x567b38 + i`).
pub fn hdr8(save: &Save, va: u32) -> u8 {
    save.header
        .get(va.wrapping_sub(HDR_BASE) as usize)
        .copied()
        .unwrap_or(0)
}

/// State-block byte at a virtual address (`0x5704e8 + i`).
pub fn st8(save: &Save, va: u32) -> u8 {
    save.state
        .get(va.wrapping_sub(ST_BASE) as usize)
        .copied()
        .unwrap_or(0)
}

pub fn st_i16(save: &Save, va: u32) -> i16 {
    let o = va.wrapping_sub(ST_BASE) as usize;
    match save.state.get(o..o + 2) {
        Some(b) => i16::from_le_bytes([b[0], b[1]]),
        None => 0,
    }
}

pub fn st_i32(save: &Save, va: u32) -> i32 {
    let o = va.wrapping_sub(ST_BASE) as usize;
    match save.state.get(o..o + 4) {
        Some(b) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        None => 0,
    }
}

/// True for the layout the simulation reads (`0x2348`): the old `0x233C` records are not
/// converted on load yet, and a construction command on them would read garbage.
pub fn playable(save: &Save) -> bool {
    save.layout.version >= 0x2348
}

/// The local player's company: the active company whose chairman is a human person
/// (`0x574414`; `Ai::player_company`).
pub fn human_company(save: &Save) -> Option<i32> {
    save.companies
        .ordered()
        .filter(|c| Rec(&c.data).i32(CO_ACTIVE) != 0)
        .find(|c| {
            let chair = Rec(&c.data).i32(CO_CHAIRMAN);
            rec(&save.persons, chair).is_some_and(|p| p.u8(PE_HUMAN) != 0)
        })
        .map(|c| c.slot as i32)
}

/// Account 0 of the company: the cash (`finance.md` 2).
pub fn cash(save: &Save, company: i32) -> f64 {
    rec(&save.companies, company).map_or(0.0, |c| c.f64(CO_CASH))
}

/// A scenario permission byte (commands.md 3.9); 1 = allowed.
pub fn permitted(save: &Save, va: u32) -> bool {
    st8(save, va) != 0
}

/// `effect_multiplier(e, company, 0, territory)` (`0x46b480`, cargo.md 5).
pub fn effect(save: &Save, e: usize, company: i32, territory: i32) -> f32 {
    let global = st_i16(save, 0x0057_07c6 + 2 * e as u32);
    let co = rec(&save.companies, company);
    let company_pct = co.map(|c| c.i16(0x1ae + 2 * e));
    let person_pct = co
        .and_then(|c| rec(&save.persons, c.i32(CO_CHAIRMAN)))
        .map(|p| p.i16(0x26b + 2 * e));
    let terr_pct = rec(&save.territories, territory).map(|t| t.i16(0x58 + 2 * e));
    effect_multiplier(EffectLevels {
        global,
        company: company_pct,
        person: person_pct,
        territory: terr_pct,
    })
}

/// `effect_sum` of the engine availability test (`0x4683a0`): the modifier ids `0x7f + e` and
/// `0xf3 + e` are signed sums (positive forces, negative forbids), not multipliers.
fn effect_sum(save: &Save, e: usize, company: i32) -> i32 {
    let mut v = i32::from(st_i16(save, 0x0057_07c6 + 2 * e as u32));
    if let Some(c) = rec(&save.companies, company) {
        v += i32::from(c.i16(0x1ae + 2 * e));
        if let Some(p) = rec(&save.persons, c.i32(CO_CHAIRMAN)) {
            v += i32::from(p.i16(0x26b + 2 * e));
        }
    }
    v
}

/// Territory (region byte) of a tile.
pub fn territory_at(save: &Save, x: i32, y: i32) -> i32 {
    if x < 0 || y < 0 || x as usize >= save.map.width || y as usize >= save.map.height {
        return 0;
    }
    i32::from(save.map.tile(x as usize, y as usize).region)
}

/// Rights bit of the company in a territory: bit 0 trains, 1 track, 2 buildings, 3 stations
/// (`company + 0x479 + t * 0x1ed`).
pub fn rights(save: &Save, company: i32, territory: i32) -> u8 {
    rec(&save.companies, company).map_or(0, |c| c.u8(0x479 + territory.max(0) as usize * 0x1ed))
}

// ---- track ------------------------------------------------------------------------------------

/// Largest bridge type available (`max_bridge_type` `0x469ef0`, commands.md 3.2): by year
/// (`< 1840` 1, `< 1880` 2, else 3).
// TODO(terrain-track.md 6): the scenario option `+0x1bf` (0 = by year, 1..5 = fixed limit) is
// not located in the save header yet.
pub fn max_bridge_type(year: i32) -> i32 {
    match year {
        ..1840 => 1,
        1840..1880 => 2,
        _ => 3,
    }
}

/// `electric_track_available` (`0x469f30`): some electric engine (kind 2) is available to the
/// company.
pub fn electric_track_available(
    data: &GameData,
    save: &Save,
    company: i32,
    year: i32,
    month: i32,
) -> bool {
    (0..data.engines.len() as i32).any(|e| {
        data.engines[e as usize].kind == 2 && engine_available(data, save, e, company, year, month)
    })
}

/// Why the track tool refuses before sending a command (LNG ids in the comments).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// LNG 23: "That action is not allowed in this scenario."
    NotAllowed,
    /// LNG 337: no company.
    NoCompany,
    /// LNG 2266: no track rights in the territory.
    NoRights,
    /// LNG 341: cash is not positive.
    NoCash,
    /// LNG 535: a station cannot be placed here.
    BadStationSite,
    /// LNG 394: cash below the cost.
    CannotAfford,
    /// A refusal of the simulation's command.
    Sim(CmdError),
}

/// Checks of the Lay Track tool's window (`0x495ea0`, commands.md 3.2) for a drag ending at
/// `(x2, y2)`; the territory is the one of the tiles at both ends.
pub fn check_track(save: &Save, company: i32, ends: [(i32, i32); 2]) -> Result<(), Refusal> {
    if !permitted(save, PERM_LAY_TRACK) {
        return Err(Refusal::NotAllowed);
    }
    if rec(&save.companies, company).is_none() {
        return Err(Refusal::NoCompany);
    }
    for (x, y) in ends {
        if rights(save, company, territory_at(save, x, y)) & 2 == 0 {
            return Err(Refusal::NoRights);
        }
    }
    Ok(())
}

// ---- preview ----------------------------------------------------------------------------------

/// What a command would do, found by running it on a clone of the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub result: Result<i32, CmdError>,
    /// What the company's cash falls by (negative when it gains).
    pub cost: f64,
    /// Tiles whose track, structure or ground changed.
    pub tiles: Vec<(i32, i32)>,
}

/// Runs `cmd` on a copy of `world` (`World::apply_build`) and reports the result, the cash it
/// would cost and the tiles it would change.
pub fn preview(world: &World, company: i32, cmd: BuildCmd) -> Preview {
    let Some(before) = world.save_ref() else {
        return Preview {
            result: Err(CmdError::NoSuchObject),
            cost: 0.0,
            tiles: Vec::new(),
        };
    };
    let cash0 = cash(before, company);
    let mut w = world.clone();
    let result = w.apply_build(cmd);
    let Some(after) = w.save_ref() else {
        return Preview {
            result,
            cost: 0.0,
            tiles: Vec::new(),
        };
    };
    let width = before.map.width;
    // A tile changed when its structure or track piece changed (the earthwork around a path
    // also moves other fields of the neighbours, which a preview must not paint).
    let piece = |s: &Save, id: u8| s.track_pieces.get(usize::from(id)).map(|r| r.data.clone());
    let tiles = before
        .map
        .tiles
        .iter()
        .zip(&after.map.tiles)
        .enumerate()
        .filter(|(_, (a, b))| {
            a.track_id != b.track_id
                || a.struct_id != b.struct_id
                || (a.track_id != 0 && piece(before, a.track_id) != piece(after, b.track_id))
        })
        .map(|(i, _)| ((i % width) as i32, (i / width) as i32))
        .collect();
    Preview {
        result,
        cost: cash0 - cash(after, company),
        tiles,
    }
}

// ---- stations ---------------------------------------------------------------------------------

/// The three station sizes (`improvement_table` rows 0..2, shapes `size * 8 + orientation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationSize {
    Small,
    Medium,
    Large,
}

impl StationSize {
    pub const ALL: [StationSize; 3] = [StationSize::Small, StationSize::Medium, StationSize::Large];

    pub fn index(self) -> i32 {
        self as i32
    }

    /// Catchment radius in tiles (terrain-track.md 3: 2, 4 and 8).
    pub fn radius(self) -> i32 {
        [2, 4, 8][self as usize]
    }

    pub fn shape(self, orientation: i32) -> i32 {
        self.index() * 8 + (orientation & 7)
    }

    pub fn from_index(i: usize) -> StationSize {
        Self::ALL[i.min(2)]
    }
}

/// `building_shape_table` (`0x5457ef`, `0x154` bytes per shape): base price `+0`, type `+9`,
/// track direction `+0xb` (the orientation of the shape).
pub fn shape_price(data: &GameData, shape: i32) -> i32 {
    data.image.i32(0x0054_57ef + 0x154 * shape as u32)
}

pub fn shape_dir(data: &GameData, shape: i32) -> u8 {
    data.image.u8(0x0054_57fa + 0x154 * shape as u32)
}

/// `0x4085e0` + `0x408450`/`0x408470`: the `building_create` position of a station shape whose
/// anchor cell (flag `0x10`) is on tile `(sx, sy)` (`Ai::station_create_xy`).
pub fn station_create_xy(data: &GameData, shape: i32, sx: i32, sy: i32) -> (i32, i32) {
    let (mut ox, mut oy) = (sx, sy);
    'f: for lin in 0..10 {
        for col in 0..10 {
            if tilemark::shape_cell(&data.image, shape as usize, lin, col) & 0x10 != 0 {
                ox = sx + col;
                oy = sy + lin;
                break 'f;
            }
        }
    }
    let e = tilemark::shape_extents(&data.image, shape as usize);
    (ox - (e[0] >> 1), oy - (e[1] >> 1))
}

/// The tiles of the footprint (cells with flags `& 0x3c3`) of a station shape placed with its
/// anchor on `(sx, sy)`.
pub fn station_footprint(data: &GameData, shape: i32, sx: i32, sy: i32) -> Vec<(i32, i32)> {
    let (x, y) = station_create_xy(data, shape, sx, sy);
    let e = tilemark::shape_extents(&data.image, shape as usize);
    let (ox, oy) = ((e[0] >> 1) + x, (e[1] >> 1) + y);
    tilemark::cells()
        .filter(|&(lin, col)| {
            tilemark::shape_cell(&data.image, shape as usize, lin, col) & 0x3c3 != 0
        })
        .map(|(lin, col)| (ox - col, oy - lin))
        .collect()
}

/// The command that builds a station of `shape` with its anchor tile on `(sx, sy)`.
pub fn station_command(
    data: &GameData,
    company: i32,
    shape: i32,
    sx: i32,
    sy: i32,
    style: i32,
) -> BuildCmd {
    let (x, y) = station_create_xy(data, shape, sx, sy);
    BuildCmd::Build {
        company,
        x,
        y,
        shape,
        hint: style,
    }
}

/// What a station of `size` would cost at a tile: `effect_multiplier(8) * base price`.
pub fn station_price(
    data: &GameData,
    save: &Save,
    company: i32,
    shape: i32,
    x: i32,
    y: i32,
) -> i32 {
    let m = effect(save, 8, company, territory_at(save, x, y));
    rt2_sim::fp::ftol(f64::from(m) * f64::from(shape_price(data, shape)))
}

/// Tries the eight orientations of `size` on tile `(sx, sy)` and returns the first that the
/// simulation accepts, with its preview. `forced` limits the test to one orientation.
#[allow(clippy::too_many_arguments)]
pub fn find_station(
    world: &World,
    data: &GameData,
    company: i32,
    size: StationSize,
    sx: i32,
    sy: i32,
    style: i32,
    forced: Option<i32>,
) -> (i32, Preview) {
    let mut last = None;
    let range: Vec<i32> = match forced {
        Some(o) => vec![o & 7],
        None => (0..8).collect(),
    };
    for o in range {
        let shape = size.shape(o);
        let p = preview(
            world,
            company,
            station_command(data, company, shape, sx, sy, style),
        );
        if p.result.is_ok() {
            return (o, p);
        }
        last = Some((o, p));
    }
    last.expect("at least one orientation")
}

// ---- improvements -----------------------------------------------------------------------------

/// `improvement_table` (`0x5500c8`, 30 rows of `0x1b` bytes): cost `+7`, first year `+0xb`, last
/// year `+0xf`, enabled `+0x13` (terrain-track.md 3.4).
const IMP_TABLE: u32 = 0x0055_00c8;
const IMP_ROW: u32 = 0x1b;
pub const IMP_ROWS: u32 = 30;

/// `improvement_available(i)` (`0x468310`).
pub fn improvement_available(data: &GameData, save: &Save, i: u32, year: i32, month: i32) -> bool {
    if hdr8(save, 0x0057_04ac) != 0 && !matches!(i, 0..=4 | 10 | 11 | 0x15) {
        return false;
    }
    let img = &data.image;
    let first = img.i32(IMP_TABLE + IMP_ROW * i + 0xb);
    let last = img.i32(IMP_TABLE + IMP_ROW * i + 0xf);
    let started = year > first || (year == first && month >= 9);
    started && year <= last && img.i32(IMP_TABLE + IMP_ROW * i + 0x13) != 0
}

pub fn improvement_base_cost(data: &GameData, i: u32) -> i32 {
    data.image.i32(IMP_TABLE + IMP_ROW * i + 7)
}

/// One row of the Buy dialog (`buildbuy.win`).
#[derive(Clone, Debug, PartialEq)]
pub struct Improvement {
    pub row: u32,
    pub cost: i32,
    pub owned: bool,
    pub available: bool,
    /// Rows already bought that exclude this one (the original's exclusion bits).
    pub excluded: bool,
}

/// The purchasable improvements of a station (rows `3..=11` and `15..=21`: 16 of the 19
/// items; the three station sizes are rows 0..2 and the size change is not in the simulation
/// yet, `station_size_change` hook). A row is `excluded` when the station already has one of
/// the bits of its exclusion mask (`row + 0x17`).
pub fn improvements(
    data: &GameData,
    save: &Save,
    station: i32,
    year: i32,
    month: i32,
) -> Vec<Improvement> {
    let Some(st) = rec(&save.stations, station) else {
        return Vec::new();
    };
    let mask = st.u32(0x49);
    let owner = st.i32(0x39);
    let terr = st.i32(0x4ad);
    let m = effect(save, 8, owner, terr);
    (3..IMP_ROWS)
        .filter(|&i| data.image.i32(IMP_TABLE + IMP_ROW * i + 0x13) != 0 || i == 22)
        .filter(|&i| !matches!(i, 12..=14 | 22))
        .map(|i| {
            let cost = rt2_sim::fp::ftol(f64::from(m) * f64::from(improvement_base_cost(data, i)));
            let excl = data.image.u32(IMP_TABLE + IMP_ROW * i + 0x17);
            Improvement {
                row: i,
                cost,
                owned: mask & (1 << i) != 0,
                available: improvement_available(data, save, i, year, month),
                excluded: mask & excl != 0,
            }
        })
        .collect()
}

// ---- engines ----------------------------------------------------------------------------------

/// `engine_available(e, company)` (`0x4683a0`, trains.md 2.4).
pub fn engine_available(
    data: &GameData,
    save: &Save,
    e: i32,
    company: i32,
    year: i32,
    month: i32,
) -> bool {
    let id = if e < 0x3e { e + 0x7f } else { e + 0xf3 } as usize;
    let m = effect_sum(save, id, company);
    if m < 0 {
        return false;
    }
    if m > 0 {
        return true;
    }
    let Some(en) = data.engines.get(e as usize) else {
        return false;
    };
    if !en.enabled || st8(save, 0x0057_0667 + e as u32) == 0 {
        return false;
    }
    let (first, last) = engine_years(data, e);
    (i32::from(first) < year || (year == i32::from(first) && month > 5)) && year <= i32::from(last)
}

/// First and last year of the locomotive vehicle of engine `e` (`engine+0x2c`, vehicle
/// `+0x1c/+0x1e`).
pub fn engine_years(data: &GameData, e: i32) -> (u16, u16) {
    let v = data.image.u8(0x0053_cec0 + 0x3a * e as u32 + 0x2c);
    let o = 0x0053_ddb8 + u32::from(v) * 0x39;
    (data.image.u16(o + 0x1c), data.image.u16(o + 0x1e))
}

/// One line of the Purchase Train window.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineRow {
    pub index: i32,
    /// Short name of the locomotive vehicle (`cr..`), e.g. `sroc_`.
    pub vehicle: String,
    /// 0 steam, 1 diesel, 2 electric.
    pub kind: u8,
    pub first_year: u16,
    pub top_mph: i32,
    pub power: i32,
    pub reliability: i32,
    pub price: i32,
    pub maintenance: i32,
    pub fuel: i32,
}

/// The engines the company can buy, ordered by the first year of the locomotive (trains.md
/// 3.5). Price and maintenance carry the modifiers of `0x4e3f90` (trains.md 6.3).
pub fn engine_rows(
    data: &GameData,
    save: &Save,
    company: i32,
    year: i32,
    month: i32,
) -> Vec<EngineRow> {
    let mut v: Vec<EngineRow> = (0..data.engines.len() as i32)
        .filter(|&e| engine_available(data, save, e, company, year, month))
        .map(|e| {
            let en = &data.engines[e as usize];
            let kind = usize::from(en.kind);
            let m_price = effect(save, 0x1a, company, 1) * effect(save, 0x1b + kind, company, 1);
            let m_maint = effect(save, 0x15, company, 1) * effect(save, 0x1b + kind, company, 1);
            let vt = data.image.u8(0x0053_cec0 + 0x3a * e as u32 + 0x2c);
            let name = (0..5u32)
                .map(|k| data.image.u8(0x0053_ddb8 + u32::from(vt) * 0x39 + k))
                .take_while(|&b| b != 0)
                .map(|b| b as char)
                .collect();
            EngineRow {
                index: e,
                vehicle: name,
                kind: en.kind,
                first_year: engine_years(data, e).0,
                top_mph: en.top_mph,
                power: en.power,
                reliability: en.reliability_class,
                price: rt2_sim::fp::ftol(f64::from(m_price) * f64::from(en.price)),
                maintenance: rt2_sim::fp::ftol(f64::from(m_maint) * f64::from(en.maintenance)),
                fuel: en.fuel,
            }
        })
        .collect();
    v.sort_by_key(|r| (r.first_year, r.index));
    v
}

/// Station slots owned by `company`, in list order (the stops a train of the company can use).
pub fn company_stations(save: &Save, company: i32) -> Vec<i32> {
    save.stations
        .ordered()
        .filter(|s| Rec(&s.data).i32(0x39) == company)
        .map(|s| s.slot as i32)
        .collect()
}

/// Where a track drag that starts or ends on tile `(x, y)` really starts or ends. A station is
/// drawn as a building that stands up from the ground, so a press on what the player sees as the
/// station lands on a tile around its footprint box (objects `+0x26..+0x2d`, `Info::pick`); that
/// press means the station's own tile, where its track is (the manual: "click at one endpoint
/// of your track", Manual.html, Lay Track). Other tiles stay as they are.
pub fn snap_to_station(save: &Save, x: i32, y: i32) -> (i32, i32) {
    for o in save.objects.ordered() {
        let b = Rec(&o.data);
        if b.u8(7) != 0 {
            continue;
        }
        let (x0, y0, x1, y1) = (
            i32::from(b.i16(0x26)) - 1,
            i32::from(b.i16(0x28)) - 1,
            i32::from(b.i16(0x2a)) + 1,
            i32::from(b.i16(0x2c)) + 1,
        );
        if x < x0 || x > x1 || y < y0 || y > y1 {
            continue;
        }
        if let Some(t) = station_tile(save, i32::from(b.u16(8))) {
            return t;
        }
    }
    (x, y)
}

/// A station's tile.
pub fn station_tile(save: &Save, station: i32) -> Option<(i32, i32)> {
    rec(&save.stations, station).map(|s| (s.i32(0xa), s.i32(0xe)))
}

/// Whether the track piece under a station's anchor tile is double and electrified (`piece +0x23`
/// bits 0 and 4; the station window's derived bits, `stationview.rs`).
pub fn station_track_flags(save: &Save, station: i32) -> (bool, bool) {
    let Some((x, y)) = station_tile(save, station) else {
        return (false, false);
    };
    let (w, h) = (save.map.width as i32, save.map.height as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return (false, false);
    }
    let id = i32::from(save.map.tile(x as usize, y as usize).track_id);
    match rec(&save.track_pieces, id) {
        Some(p) => {
            let f = p.u8(0x23);
            (f & 1 != 0, f & 0x10 != 0)
        }
        None => (false, false),
    }
}

/// A stop of a train's route (`train+0x37`, 14 bytes each; trains.md 10, commands.md 2.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop {
    pub station: i32,
    pub waypoint: bool,
    pub cargo: Vec<u8>,
    /// 0 do not wait, 1 half load, 2 full load (`+3 >> 6`).
    pub wait: u8,
    pub x: i32,
    pub y: i32,
}

/// The stops of a train, read from the save.
pub fn train_stops(save: &Save, train: i32) -> Vec<Stop> {
    let Some(t) = rec(&save.trains, train) else {
        return Vec::new();
    };
    let n = usize::from(t.u8(0x3b));
    let Some(extra) = save.train_extras.iter().find(|e| e.slot == train as usize) else {
        return Vec::new();
    };
    (0..n)
        .filter_map(|k| {
            let r = extra.cars_raw.get(k * 14..k * 14 + 14)?;
            let n = usize::from(r[2]).min(6);
            Some(Stop {
                station: i32::from(u16::from_le_bytes([r[0], r[1]])),
                waypoint: r[3] & 8 != 0,
                cargo: r[4..4 + n].to_vec(),
                wait: r[3] >> 6,
                x: i32::from(i16::from_le_bytes([r[10], r[11]])),
                y: i32::from(i16::from_le_bytes([r[12], r[13]])),
            })
        })
        .collect()
}

/// Trains of a company, in list order.
pub fn company_trains(save: &Save, company: i32) -> Vec<i32> {
    save.trains
        .ordered()
        .filter(|t| Rec(&t.data).i32(0x41) == company)
        .map(|t| t.slot as i32)
        .collect()
}

/// Cash of the company at a glance for the refusal checks of the issuing windows.
pub fn can_afford(save: &Save, company: i32, price: f64) -> bool {
    cash(save, company) >= price
}

/// `engine_name_ids` (`0x5534f4`, u16 per engine): the LNG id of the engine's name.
pub fn engine_name_id(data: &GameData, e: i32) -> u32 {
    u32::from(data.image.u16(0x0055_34f4 + 2 * e as u32))
}

// ---- industries -------------------------------------------------------------------------------

/// `0x573d78`: the industrial model; 2 is Expert, the only one that allows buying (commands.md 3.6).
pub fn industrial_model(save: &Save) -> i32 {
    st_i32(save, 0x0057_3d78) & 0xff
}

/// What the industry window offers for a building (commands.md 3.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trade {
    Buy,
    Sell,
    /// Nothing: model, permission, type or owner rule it out.
    None,
}

/// `building_price(id, company)` (`0x40bae0`): the paid value when owned, else
/// `effect_multiplier(0xd) * base price of the shape`.
pub fn building_price(data: &GameData, save: &Save, company: i32, building: i32) -> i32 {
    let Some(b) = rec(&save.objects, building) else {
        return 0;
    };
    if b.u16(0x97) != 0 && b.i32(0x99) != 0 {
        return b.i32(0x99);
    }
    let terr = i32::from(b.u8(0x9d));
    let m = effect(save, 0xd, company, terr);
    rt2_sim::fp::ftol(f64::from(m) * f64::from(shape_price(data, i32::from(b.u8(6)))))
}

/// The industry window's Buy / Sell button: model Expert, permission `0x5706b9`, a buyable
/// type (`0x535c39 + type * 0x27`), and the owner (none: buy, the player: sell).
pub fn industry_trade(data: &GameData, save: &Save, company: i32, building: i32) -> Trade {
    let Some(b) = rec(&save.objects, building) else {
        return Trade::None;
    };
    let kind = u32::from(b.u8(7));
    if industrial_model(save) != 2
        || !permitted(save, PERM_BUY_INDUSTRY)
        || data.image.u8(0x0053_5c39 + kind * 0x27) != 1
    {
        return Trade::None;
    }
    match i32::from(b.u16(0x97)) {
        0 => Trade::Buy,
        c if c == company => Trade::Sell,
        _ => Trade::None,
    }
}

// ---- territory rights -------------------------------------------------------------------------

/// `territory_rights_price(company, territory, bit)` (`0x440fb0`, commands.md 3.7). The
/// goodwill (`0x43eca0`) is approximated by the company's standing in the territory
/// (`company + 0x47a + t * 0x1ed`).
// TODO(commands.md 3.7): the general goodwill term of `company_goodwill` is not specified well
// enough to port; the price is exact only for companies whose general goodwill equals their
// standing in the territory.
pub fn rights_price(save: &Save, company: i32, territory: i32, bit: usize) -> i32 {
    let Some(t) = rec(&save.territories, territory) else {
        return 0;
    };
    let base = 1000.0 * f64::from(t.i16(0x3b + 2 * bit));
    let g = rec(&save.companies, company).map_or(0, |c| {
        i32::from(c.u16(0x47a + territory.max(0) as usize * 0x1ed)).clamp(0, 10_000)
    });
    let f = f64::from((15_000 - g) as f32 * 0.0001);
    let y = if f <= 1.0 {
        f * f
    } else {
        1.0 + 3.0 * (f * f - 1.0)
    };
    (rt2_sim::fp::ftol(base * y) / 1000) * 1000
}

/// The mask of the `0x2a` command: all four rights unless the territory sells them one by one.
pub fn rights_mask(save: &Save, territory: i32, bit: usize) -> u8 {
    match rec(&save.territories, territory) {
        Some(t) if t.u8(0x43) != 0 => 1 << bit,
        _ => 0xf,
    }
}
