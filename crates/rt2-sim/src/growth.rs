//! World growth: towns, regions, buildings and industries (docs/spec/world-growth.md).
//!
//! The yearly growth pass (`0x430a40` -> `0x430540` per place), the building type pick
//! (`0x42ffc0`), the site search (`0x42fcb0`), the removal pick (`0x4302c0`), the industry
//! rating (`0x40b450`) and a few small helpers. The game state (building lists, tiles) is not
//! modelled here: the rules that need it take small environment traits so that the same code
//! runs against the real structures later and against stubs in the differential tests
//! (`crates/rt2-oracle/tests/growth.rs`).
//!
//! The original computes in the x87 at 53-bit precision; `f64` arithmetic reproduces it as long
//! as every store to a C `float` is rounded with [`f32r`] and every `float -> int` conversion
//! goes through [`ftol`] (see [`crate::fp`]).

use crate::cargo::IndustryType;
use crate::exe::{Exe, ExeError};
use crate::fp::{f32r, ftol};
use crate::rng::GameRng;

/// Industry/building type count (`0x535c38`, 0x27-byte records) and per-type constants.
pub const BUILDING_TYPES: usize = 53;
/// Type 0 is the player's station building.
pub const TYPE_STATION: usize = 0;
/// Type 0x12: housing.
pub const TYPE_HOUSE: usize = 0x12;
/// Type 0x1c: port (tries four shapes, up to 10500 attempts).
pub const TYPE_PORT: usize = 0x1c;
/// Building shapes in `0x5457ef` (0x154-byte records).
pub const SHAPES: usize = 0x7a;

const ADDR_BUILDING_TYPES: u32 = 0x0053_5c38;
const ADDR_TARGET: u32 = 0x0053_a464;
const ADDR_TREND: u32 = 0x0053_a48c;
const ADDR_RADIUS_CAP: u32 = 0x0055_03f4;
const ADDR_DIST2: u32 = 0x0055_0408;
const ADDR_BOUNTY: u32 = 0x0053_a4b0;
const ADDR_ECON_TOWN: u32 = 0x0055_04e4;
const ADDR_ECON_REGION: u32 = 0x0055_04f8;
const ADDR_SHAPES: u32 = 0x0054_57ef;

/// One record of `building_type_table` (`0x535c38`, 0x27 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingType {
    /// `+2`: base pick weight in percent (100 in every record).
    pub weight: i32,
    /// `+0x0a`: size in "building units" (what counts towards a town's industry target).
    pub size: u16,
    /// `+0x0c`: first year the type may be founded.
    pub first_year: i32,
    /// `+0x10..+0x13`: forest class range `[lo, hi]` and slope class range `[lo, hi]`.
    pub site: [i8; 4],
    /// `+0x14`: allowed on terrain class `c` (0..18).
    pub allowed: [bool; 19],
}

impl BuildingType {
    fn from_raw(r: &[u8]) -> Self {
        let i32at = |o: usize| i32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
        let mut allowed = [false; 19];
        for (i, a) in allowed.iter_mut().enumerate() {
            *a = r[0x14 + i] != 0;
        }
        BuildingType {
            weight: i32at(2),
            size: u16::from_le_bytes([r[0xa], r[0xb]]),
            first_year: i32at(0xc),
            site: [r[0x10] as i8, r[0x11] as i8, r[0x12] as i8, r[0x13] as i8],
            allowed,
        }
    }

    /// `true` when the type has no forest/slope constraint (`0x42fcb0`, first disjunct).
    pub fn unconstrained(&self) -> bool {
        self.site[0] <= 0 && self.site[1] > 1 && self.site[2] <= 0 && self.site[3] > 1
    }
}

/// Everything the growth rules read from the executable's `.data`.
#[derive(Clone, Debug)]
pub struct GrowthTables {
    pub types: Vec<BuildingType>,
    /// `0x53a464` f32: target base per city size class and pass (`class * 2 + pass`).
    pub target: [f32; 10],
    /// `0x53a48c` f32[9]: region trend factor for trend -2..=6.
    pub trend: [f32; 9],
    /// `0x5503f4`: maximum placement radius per city size class.
    pub radius_cap: [i32; 5],
    /// `0x550408`: squared catchment radius per city size class.
    pub dist2: [i32; 5],
    /// `0x53a4b0`: connection bounty per city size class.
    pub bounty: [i32; 5],
    /// `0x5504e4` / `0x5504f8`: growth multiplier per economy state (city / region).
    pub econ_town: [f32; 5],
    pub econ_region: [f32; 5],
    /// `0x5457ef`: purchase price per shape.
    pub shape_price: Vec<i32>,
    /// `0x5457f8`: building type per shape.
    pub shape_type: Vec<u8>,
}

fn arr<const N: usize>(exe: &Exe, va: u32) -> Result<[f32; N], ExeError> {
    Ok(exe.f32s(va, N)?.try_into().expect("length N"))
}

fn iarr<const N: usize>(exe: &Exe, va: u32) -> Result<[i32; N], ExeError> {
    let mut o = [0i32; N];
    for (i, v) in o.iter_mut().enumerate() {
        *v = exe.u32_at(va + 4 * i as u32)? as i32;
    }
    Ok(o)
}

impl GrowthTables {
    pub fn load(exe: &Exe) -> Result<Self, ExeError> {
        let types = (0..BUILDING_TYPES)
            .map(|t| {
                Ok(BuildingType::from_raw(
                    exe.read(ADDR_BUILDING_TYPES + 0x27 * t as u32, 0x27)?,
                ))
            })
            .collect::<Result<Vec<_>, ExeError>>()?;
        let mut shape_price = Vec::new();
        let mut shape_type = Vec::new();
        for s in 0..SHAPES as u32 {
            let rec = ADDR_SHAPES + 0x154 * s;
            shape_price.push(exe.u32_at(rec)? as i32);
            shape_type.push(exe.read(rec + 9, 1)?[0]);
        }
        Ok(GrowthTables {
            types,
            target: arr(exe, ADDR_TARGET)?,
            trend: arr(exe, ADDR_TREND)?,
            radius_cap: iarr(exe, ADDR_RADIUS_CAP)?,
            dist2: iarr(exe, ADDR_DIST2)?,
            bounty: iarr(exe, ADDR_BOUNTY)?,
            econ_town: arr(exe, ADDR_ECON_TOWN)?,
            econ_region: arr(exe, ADDR_ECON_REGION)?,
            shape_price,
            shape_type,
        })
    }
}

// ---- 1. town growth ----------------------------------------------------------------------------

/// Kind of a place record (`+0x10`): 0 city, 1 region, 2 label.
pub const KIND_CITY: u8 = 0;
pub const KIND_REGION: u8 = 1;

/// The fields of a place (`list_towns`, 180 bytes) that the growth step reads or writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Town {
    pub kind: u8,
    /// `+0x2c`: size class 0..4 (Village .. Megalopolis).
    pub size_class: u8,
    /// `+0x2e`: relative growth index in percent (50..300; 100 = neutral).
    pub growth_index: i32,
    /// `+0x32`: land tiles of the place (regions).
    pub land_tiles: i32,
    /// `+0x3e` / `+0x40`: lots delivered to / shipped from the place this year (decay 0.8/year).
    pub lots_in: u16,
    pub lots_out: u16,
    /// `+0x42`: no removals before this game minute.
    pub cooldown_until: u32,
    /// `+0x46`: growth intensity in percent (80..120 city, 95..105 region).
    pub intensity: u16,
    /// `+0x49`: region trend -2..=6.
    pub trend: i8,
}

/// Counts returned by `town_building_stats` (`0x430400`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildingStats {
    /// Houses (type 0x12 with industry set 0).
    pub houses: i32,
    /// Every other building of the place (industries, stations, big houses).
    pub others: i32,
    /// Sum of the `size` of those.
    pub size_sum: i32,
}

/// What the growth step needs from the world.
pub trait TownEnv {
    /// `0x430400`.
    fn stats(&mut self) -> BuildingStats;
    /// `0x42ffc0(town, _, house)`: `true` when a building was created.
    fn add_building(&mut self, rng: &mut GameRng, house: bool) -> bool;
    /// `0x4302c0(town, house)`.
    fn remove_worst(&mut self, rng: &mut GameRng, house: bool);
    /// The live `0x570713` (buildings created or destroyed by the places grown before change
    /// it during the year step); `None` uses [`GrowthWorld::industry_count`].
    fn industry_count(&mut self) -> Option<i32> {
        None
    }
}

/// World values read by the growth step.
#[derive(Clone, Copy, Debug)]
pub struct GrowthWorld {
    /// `0x5705d1`: growth factor (f32).
    pub growth: f32,
    /// `0x570717`: economy state 0..4.
    pub econ_state: usize,
    /// `0x574c14`: "Viagra" cheat.
    pub viagra: bool,
    /// `0x570713`: number of industries (buildings with type != 0 and not decorative).
    pub industry_count: i32,
    /// `0x57070f`: lots delivered in the current accounting window.
    pub deliveries_total: i32,
    /// Year of `game_date` and of the start date `0x570508`.
    pub year: i32,
    pub start_year: i32,
    /// `game_time_minutes`.
    pub now: u32,
    /// `0x5704ac != 0`: expansion industry set (no houses).
    pub set1: bool,
}

const GROWTH_P0: f64 = 1.1810279687551883e-7; // f32 at 0x535380
const GROWTH_P1: f64 = 4.1335979972245696e-7; // f32 at 0x53536c
const CHURN: f64 = 1.2400793650793651e-8; // f64 at 0x535370
const MILLION: f64 = 1_000_000.0;

/// `town_update` (`0x430540`): the per-place growth step. `dt` is the number of game minutes
/// since the previous pass (one year in the original); `initial` is the world-creation call
/// (`0x430a40(1)`), which uses `dt = 10000` and always grows to the target.
///
/// RNG calls, in order (spec section 3.2): the intensity re-roll, then per pass (industries,
/// then houses) the shrink draws, the churn draw and the growth draws; `env` makes the rest.
pub fn town_growth_step(
    t: &mut Town,
    w: &GrowthWorld,
    tab: &GrowthTables,
    env: &mut dyn TownEnv,
    rng: &mut GameRng,
    dt: i32,
    initial: bool,
) {
    if t.kind != KIND_CITY && t.kind != KIND_REGION {
        return;
    }
    let mut dt = dt;
    let mut initial = initial;
    if initial {
        dt = 10000;
    }
    if initial || !(80..=120).contains(&t.intensity) {
        t.intensity = if t.kind == KIND_CITY {
            rng.range(80, 120)
        } else {
            rng.range(95, 105)
        } as u16;
    }
    for pass in [1usize, 0] {
        if w.set1 && pass == 0 {
            continue;
        }
        // Target (an int, stored as a float afterwards).
        let target: i32 = if t.kind == KIND_CITY {
            let g = f64::from(w.growth);
            let k = f64::from(tab.target[usize::from(t.size_class) * 2 + pass]);
            let eco = f64::from(tab.econ_town[w.econ_state]);
            let v = ftol(((g * k) * eco) * (f64::from(t.intensity) * 0.01));
            if w.viagra && pass == 0 {
                v.wrapping_mul(2)
            } else {
                v
            }
        } else {
            if pass == 0 {
                continue;
            }
            if !(-2..=6).contains(&t.trend) {
                t.trend = 0;
            }
            let tr = f64::from(tab.trend[(i32::from(t.trend) + 2) as usize]);
            let g = f64::from(w.growth);
            let eco = f64::from(tab.econ_region[w.econ_state]);
            let v = ((tr * ((g + 1.0) * 0.5)) * eco) * (f64::from(t.intensity) * 0.01);
            ftol((v * f64::from(t.land_tiles)) * 0.01)
        };
        let target_f = f64::from(target as f32);
        let dt_f = f64::from(dt as f32);
        let mut removed = false;
        let mut n_target;
        let mut p: f64;
        // The locals the original keeps from its last `0x430400` call.
        let mut last_houses;
        let mut last_others;
        // Shrink loop (and the first evaluation of the target).
        loop {
            let st = env.stats();
            let (cur, n_ind) = if pass == 0 {
                (st.houses, st.houses)
            } else {
                (st.size_sum, st.others)
            };
            last_houses = st.houses;
            last_others = n_ind;
            let industry_count = env.industry_count().unwrap_or(w.industry_count);
            let x1 = if industry_count != 0 {
                f64::from(st.houses + n_ind) / f64::from(industry_count)
            } else {
                0.0
            };
            let mut x2 = if w.deliveries_total != 0 {
                f64::from(i32::from(t.lots_out) + i32::from(t.lots_in))
                    / (f64::from(w.deliveries_total) * 2.0)
            } else {
                x1
            };
            if t.kind == KIND_REGION {
                x2 *= 1.5;
            }
            let ratio = if x1 <= 0.0 { 1.0 } else { (x2 / x1).min(16.0) };
            t.growth_index = ftol(ratio * 20.0 + 80.0).clamp(50, 300);
            let years = w.year - w.start_year;
            if years < 12 {
                t.growth_index = (t.growth_index - 100)
                    .wrapping_mul(years)
                    .wrapping_add(1200)
                    / 15;
            }
            n_target = ftol((f64::from(t.growth_index) * 0.01) * target_f);
            if t.kind == KIND_CITY && n_target <= 1 {
                n_target = 1;
            }
            if initial && n_target > 5 {
                n_target -= 1;
            }
            p = f32r((f64::from(n_target - cur) / f64::from(cur.max(1))) * dt_f * GROWTH_P0);
            if initial {
                if p >= 0.0 {
                    break;
                }
                initial = false;
            }
            if p >= 0.0 || w.now < t.cooldown_until {
                break;
            }
            let r = rng.range(0, 1_000_000);
            if -p * MILLION <= f64::from(r) {
                break;
            }
            env.remove_worst(rng, pass == 0);
            removed = true;
        }
        // Spontaneous removal ("churn").
        let a = f32r(f64::from(dt) * CHURN);
        if !initial && w.now >= t.cooldown_until {
            let n = if pass == 0 { last_houses } else { last_others };
            if n > 4 {
                let r = rng.range(0, 1_000_000);
                if f64::from(r) < (f64::from(n) * a) * MILLION {
                    env.remove_worst(rng, pass == 0);
                    continue;
                }
            }
        }
        if removed || p <= 0.0 {
            continue;
        }
        // Growth loop.
        loop {
            if !initial {
                let r = rng.range(0, 1_000_000);
                if p * MILLION <= f64::from(r) {
                    break;
                }
            }
            if !env.add_building(rng, pass == 0) {
                break;
            }
            let st = env.stats();
            let cur = if pass == 0 { st.houses } else { st.size_sum };
            p = f32r((f64::from(n_target - cur) / f64::from(cur.max(1))) * dt_f * GROWTH_P1);
            if p <= 0.0 {
                break;
            }
        }
    }
}

// ---- 2. building statistics, type pick, site search, removal pick --------------------------------

/// What `town_building_stats` (`0x430400`) reads of one building of the place's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingRef {
    /// `+7`.
    pub ty: u8,
    /// `+0x33`: stations linked to it.
    pub stations: i8,
}

/// Outputs of `0x430400` besides [`BuildingStats`]: the place's `+0x12`, `+0x3a` and `+0x48`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TownStatsOut {
    pub stats: BuildingStats,
    /// `+0x12`: sum of the yearly rates of every valid primary slot of the place's buildings.
    pub output_rate: f32,
    /// `+0x3a`: the same, only for buildings with linked stations.
    pub served_rate: f32,
    /// `+0x48`: houses clamped to 255.
    pub houses_u8: u8,
}

/// `town_building_stats` (`0x430400`). `buildings` is the place's list in list order,
/// `industries` the `0x541ae0` records of the active set.
pub fn town_building_stats(
    buildings: &[BuildingRef],
    tab: &GrowthTables,
    industries: &[IndustryType],
    set1: bool,
    year: i32,
) -> TownStatsOut {
    let mut s = BuildingStats::default();
    let (mut all, mut served) = (0.0f32, 0.0f32);
    for b in buildings {
        if usize::from(b.ty) == TYPE_HOUSE && !set1 {
            s.houses += 1;
        } else {
            s.others += 1;
            s.size_sum += i32::from(tab.types[usize::from(b.ty)].size);
        }
        for sl in &industries[usize::from(b.ty)].slots {
            if sl.output != 0x7f
                && sl.mode != 0
                && i32::from(sl.year_from) <= year
                && year <= i32::from(sl.year_to)
            {
                all = f32r(f64::from(all) + f64::from(sl.rate)) as f32;
                if b.stations != 0 {
                    served = f32r(f64::from(served) + f64::from(sl.rate)) as f32;
                }
            }
        }
    }
    TownStatsOut {
        stats: s,
        output_rate: all,
        served_rate: served,
        houses_u8: s.houses.min(255) as u8,
    }
}

/// World values for the type pick.
#[derive(Clone, Debug)]
pub struct PickWorld {
    pub year: i32,
    pub set1: bool,
    /// `0x5702cc` (+9 stride): scarcity weight in percent per type (recomputed every pass).
    pub scarcity: [i32; BUILDING_TYPES],
    /// `0x5702cb` (+9 stride): type disabled by the scenario.
    pub disabled: [bool; BUILDING_TYPES],
}

/// `town_add_building` (`0x42ffc0`). `weights` is the place's `u16[53]` at `+0x4a`; `counts[t]`
/// the number of buildings of type `t` already in the place (primary producers count as 0);
/// `region` is `kind == 1`. `place(rng, t)` is `0x42fcb0`.
///
/// RNG: `rng(0, n-1)` to pick a candidate, `rng(0, limit)` to test its weight, repeated up to
/// 5000 times (spec 4.1).
#[allow(clippy::too_many_arguments)]
pub fn town_add_building(
    rng: &mut GameRng,
    tab: &GrowthTables,
    world: &PickWorld,
    region: bool,
    weights: &[u16; BUILDING_TYPES],
    counts: &[u8; BUILDING_TYPES],
    house: bool,
    place: &mut dyn FnMut(&mut GameRng, usize) -> bool,
) -> bool {
    let mut excluded = [false; BUILDING_TYPES];
    excluded[TYPE_STATION] = true;
    if !house {
        if !world.set1 {
            excluded[TYPE_HOUSE] = true;
        }
    } else if !world.set1 {
        return place(rng, TYPE_HOUSE);
    }
    let mut d = [0i32; BUILDING_TYPES];
    for t in 0..BUILDING_TYPES {
        if weights[t] == 0 || world.year < tab.types[t].first_year || world.disabled[t] {
            excluded[t] = true;
        }
        if !excluded[t] {
            d[t] = ftol(
                (f64::from(tab.types[t].weight) * 0.01)
                    * (f64::from(world.scarcity[t]) * 0.01)
                    * f64::from(weights[t]),
            );
        }
    }
    let mut cands: Vec<usize> = (0..BUILDING_TYPES).filter(|&t| !excluded[t]).collect();
    if cands.is_empty() {
        return false;
    }
    let mut budget = 3000i32;
    let mut limit = 0x400i32;
    let mut k = 0i32;
    loop {
        let idx = rng.range(0, cands.len() as i32 - 1) as usize;
        let t = cands[idx];
        if (region || i32::from(counts[t]) <= k) && rng.range(0, limit) < d[t] {
            if place(rng, t) {
                return true;
            }
            cands.remove(idx);
            if cands.is_empty() {
                return false;
            }
        }
        budget -= 1;
        if budget == 0 && limit > 8 {
            limit >>= 2;
            budget = 500;
        }
        k = (3000 - budget) >> 10;
        if budget == 0 {
            return false;
        }
    }
}

/// `building_shape_pick` (`0x40ba00`): the shape (index into `0x5457ef`) used for a new building
/// of type `ty`. Some types draw a random one of up to 24 rotations/variants.
pub fn shape_pick(rng: &mut GameRng, shape_type: &[u8], ty: usize, randomize: bool) -> usize {
    let ty = if ty > 0x30 { TYPE_HOUSE } else { ty };
    let varied = matches!(
        ty,
        10 | 11 | 0x10 | 0x12 | 0x1c | 0x1d | 0x1e | 0x21 | 0x27..=0x29
    );
    let n = if randomize && varied {
        rng.range(1, 0x18)
    } else {
        1
    };
    let mut idx = 0usize;
    for _ in 0..n {
        idx = (idx + 1) % SHAPES;
        while usize::from(shape_type[idx]) != ty {
            idx = (idx + 1) % SHAPES;
        }
    }
    idx
}

/// The map as `town_place_building` sees it.
pub trait SiteEnv {
    /// `0x430c20(x, y, 0)`: id of the place owning tile `(x, y)` (0 = none).
    fn place_at(&mut self, x: i32, y: i32) -> u16;
    /// `0x42fb10`: forest and slope class of the site.
    fn site_classes(&mut self, x: i32, y: i32) -> (i32, i32);
    /// Terrain class (`0x581090[tile >> 6]`) of the tile.
    fn ground_class(&mut self, x: i32, y: i32) -> i32;
    /// `0x4087a0`.
    fn can_place(&mut self, x: i32, y: i32, shape: usize, early: bool) -> bool;
    /// `0x409010`.
    fn create(&mut self, rng: &mut GameRng, x: i32, y: i32, shape: usize);
}

/// The place fields `town_place_building` reads.
#[derive(Clone, Copy, Debug)]
pub struct PlaceGeom {
    pub id: u16,
    pub region: bool,
    pub size_class: u8,
    /// `+4,+6`: centre.
    pub cx: i32,
    pub cy: i32,
    /// `+8..+0xe`: bounding box `x0, y0, x1, y1`.
    pub bbox: [i32; 4],
}

/// `town_place_building` (`0x42fcb0`): searches a site for a building of type `ty` and creates
/// it. Up to 300 attempts (900 for types 0x27..0x29, 10500 for ports, x3 for regions).
///
/// RNG: the shape pick (`rng(1, 24)` for some types), then per attempt 4 radius draws, 2 sign
/// draws (cities) or 2 box draws (regions), and `rng(0, 100)` on snow ground.
pub fn town_place_building(
    rng: &mut GameRng,
    tab: &GrowthTables,
    p: &PlaceGeom,
    ty: usize,
    env: &mut dyn SiteEnv,
) -> bool {
    let mut attempts: i32 = match ty {
        TYPE_PORT => 0x2904,
        0x27..=0x29 => 900,
        _ => 300,
    };
    if p.region {
        attempts *= 3;
    }
    let shape = shape_pick(rng, &tab.shape_type, ty, true);
    let mut radius = i32::from(ty == TYPE_HOUSE);
    let row = &tab.types[ty];
    while attempts != 0 {
        if attempts & 7 == 7 {
            radius += 1;
            let cap = tab.radius_cap[usize::from(p.size_class)] + 1;
            if cap < radius {
                radius = cap;
            }
        }
        let (x, y) = if !p.region {
            let r1 = rng.range(0, radius);
            let r2 = rng.range(0, radius);
            let mut dx = ftol(f64::from(r1 * r2).sqrt());
            let r3 = rng.range(0, radius);
            let r4 = rng.range(0, radius);
            let mut dy = ftol(f64::from(r3 * r4).sqrt());
            if rng.range(0, 1) == 0 {
                dx = -dx;
            }
            let x = p.cx + dx;
            if rng.range(0, 1) == 0 {
                dy = -dy;
            }
            (x, p.cy + dy)
        } else {
            let x = rng.range(p.bbox[0], p.bbox[2]);
            let y = rng.range(p.bbox[1], p.bbox[3]);
            (x, y)
        };
        let ok = i32::from(env.place_at(x, y)) == i32::from(p.id) && {
            if row.unconstrained() {
                true
            } else {
                let (c1, c2) = env.site_classes(x, y);
                let g = env.ground_class(x, y);
                i32::from(row.site[0]) <= c1
                    && c1 <= i32::from(row.site[1])
                    && i32::from(row.site[2]) <= c2
                    && c2 <= i32::from(row.site[3])
                    && row.allowed.get(g as usize).copied().unwrap_or(false)
                    && (g != 8 || rng.range(0, 100) < 0x29)
            }
        };
        if ok {
            if ty == TYPE_PORT {
                for k in 0..4 {
                    if env.can_place(x, y, 0x54 + k, false) {
                        env.create(rng, x, y, 0x54 + k);
                        return true;
                    }
                }
            } else if env.can_place(x, y, shape, attempts > 300) {
                env.create(rng, x, y, shape);
                return true;
            }
        }
        attempts -= 1;
    }
    false
}

/// One building of a place as `town_remove_worst_building` (`0x4302c0`) sees it.
#[derive(Clone, Copy, Debug)]
pub struct RemovalCandidate {
    pub is_house: bool,
    /// `+8 != 0`: has a station (a player's station building).
    pub has_station: bool,
    /// `+0x97 != 0`: owned by a company.
    pub owned: bool,
    /// `+0x4b`.
    pub rating: u16,
}

/// `town_remove_worst_building` (`0x4302c0`): index (in list order) of the building to demolish.
/// `house` selects the class: houses when set, everything else when clear. One
/// `rng(0x58, 0x70)` per eligible building.
pub fn pick_worst_building(
    rng: &mut GameRng,
    list: &[RemovalCandidate],
    house: bool,
    set1: bool,
) -> Option<usize> {
    let mut best = 999_999.0f32;
    let mut pick = None;
    for (i, b) in list.iter().enumerate() {
        if b.has_station || b.owned || set1 || b.is_house != house {
            continue;
        }
        let v = rng.range(0x58, 0x70).wrapping_mul(i32::from(b.rating));
        if (v as f32) < best {
            best = v as f32;
            pick = Some(i);
        }
    }
    pick
}

/// Site classes of `0x42fb10`: `forest` and `slope` (0..2) from the tile bytes around `(x, y)`.
///
/// `tile(px, py)` returns `(b2, b3, b6)` of the tile or `None` outside the map, `elev(x, y)` the
/// corner height at grid position `(x, y)`.
pub fn site_classes(
    tile: &dyn Fn(i32, i32) -> Option<(u8, u8, u8)>,
    elev: &dyn Fn(i32, i32) -> i32,
    x: i32,
    y: i32,
) -> (i32, i32) {
    let (mut weight_sum, mut tree_sum, mut slope_sum) = (0i32, 0i32, 0i32);
    for r in 0..3i32 {
        let w = 0x20 - r * 8;
        for k in 0..4 {
            if r == 0 && k >= 1 {
                continue;
            }
            let (px, py) = match k {
                0 => (x - r, y),
                1 => (x, y + r),
                2 => (x + r, y),
                _ => (x, y - r),
            };
            if let Some((b2, b3, b6)) = tile(px, py) {
                weight_sum += w;
                if b6 & 4 != 0 && matches!(b2 & 0xf, 1 | 2) {
                    tree_sum += i32::from(b3 & 0x1f) * w;
                }
                let dz = elev(x, y) - elev(x + 1, y + 1);
                slope_sum += dz.abs() * w;
            }
        }
    }
    let pct = (tree_sum * 100) / weight_sum;
    let c1 = if pct < 4 { 0 } else { i32::from(pct > 20) + 1 };
    let sl = (slope_sum * 100) / weight_sum;
    let c2 = if sl > 180 { i32::from(sl > 500) + 1 } else { 0 };
    (c1, c2)
}

// ---- 3. industry rating, purchase ----------------------------------------------------------------

/// `slot_valid` (`0x4e2330`): is the output slot active in `year`.
pub fn slot_valid(
    sl: &crate::cargo::Slot,
    year: i32,
    cargo_on: &dyn Fn(u8) -> bool,
    sandbox: bool,
    peer_version: i32,
) -> bool {
    let (i1, i2) = (&sl.inputs[0], &sl.inputs[1]);
    if sl.output == 0x7f && i1.cargo == 0x7f {
        return false;
    }
    if year < i32::from(sl.year_from) || i32::from(sl.year_to) < year {
        return false;
    }
    if sl.output != 0x7f && !cargo_on(sl.output) {
        return false;
    }
    let optional = |th: f32| f64::from(th) > 0.01 && peer_version > 0x2332;
    if i1.cargo != 0x7f && !cargo_on(i1.cargo) && !optional(i1.threshold) && !sandbox {
        return false;
    }
    if i2.cargo != 0x7f && !cargo_on(i2.cargo) && !optional(i2.threshold) && !sandbox {
        return false;
    }
    true
}

/// Inputs of the yearly rating (`0x40b450`) of one building.
#[derive(Clone)]
pub struct RatingInput<'a> {
    pub ty: &'a IndustryType,
    pub year: i32,
    pub cargo_on: &'a dyn Fn(u8) -> bool,
    pub sandbox: bool,
    pub peer_version: i32,
    /// `+0x34`: lots received this year.
    pub received: u16,
    /// `+0x63` / `+0x67`: lots delivered to a demanding station / produced, per output slot.
    pub delivered: [i8; 4],
    pub produced: [i8; 4],
    /// `+0x93`: smoothed rating.
    pub old: f32,
    /// Purchase price of the building's shape (`0x5457ef`).
    pub price: i32,
}

/// Outputs of the yearly rating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RatingOut {
    /// `+0x93`.
    pub smoothed: f32,
    /// `+0x4b`: `ftol(smoothed * 100 + 50)` (before the computer-owner bonus).
    pub rating: u16,
    /// `+0x36`: profit / 1000.
    pub profit_k: i16,
    /// The yearly profit (credited to the owner's ledger category 3).
    pub profit: i32,
}

/// `industry_year_step` (`0x40b450`) for a building that is not company-owned (the owner bonus
/// for computer companies is `rating < 100 ? (rating + 100) / 2 : rating + 5`, applied by the
/// caller through [`computer_owner_rating`]).
pub fn industry_rating(i: &RatingInput) -> RatingOut {
    let sum_d: i32 = i.delivered.iter().map(|&v| i32::from(v)).sum();
    let sum_p: i32 = i.produced.iter().map(|&v| i32::from(v)).sum();
    let ratio: f32 = if sum_p == 0 {
        if sum_d == 0 { 1.0 } else { 1.5 }
    } else {
        let q = f32r(f64::from(sum_d) / f64::from(sum_p)) as f32;
        if q >= 0.0 {
            if q > 2.0 { 2.0 } else { q }
        } else {
            0.0
        }
    };
    let activity = f32r(f64::from(i.received) + f64::from(sum_d)) as f32;
    let mut cap = 0.0f32;
    for sl in &i.ty.slots {
        if !slot_valid(sl, i.year, i.cargo_on, i.sandbox, i.peer_version) {
            continue;
        }
        if sl.mode == 0 {
            let add = if sl.output == 0x7f {
                2.5
            } else if sl.output == 0x11 {
                3.0
            } else {
                4.0
            };
            cap = f32r(f64::from(cap) + add) as f32;
        } else {
            cap = f32r(f64::from(sl.rate) * 1.1 + f64::from(cap)) as f32;
            if sl.inputs[0].cargo != 0x7f {
                cap = f32r(f64::from(cap) + 0.6) as f32;
            }
        }
    }
    if cap <= 0.5 {
        cap = 0.5;
    } else if cap > 3.0 {
        cap = f32r((f64::from(cap) - 3.0) * 0.5 + 3.0) as f32;
    }
    let u = f64::from(activity) / f64::from(cap);
    let u = if u >= 0.0 { u.min(2.0) } else { 0.0 };
    let new = (u + f64::from(ratio)) * 0.5;
    let old = f64::from(i.old);
    let smoothed = if new <= old {
        old * 0.7 + new * 0.3
    } else {
        old * 0.55 + new * 0.45
    };
    let smoothed = f32r(smoothed) as f32;
    let rating = ftol(f64::from(smoothed) * 100.0 + 50.0) as u16;
    let profit = ftol((f64::from(i.price) * (f64::from(i32::from(rating) - 100) * 0.01)) * 0.25);
    RatingOut {
        smoothed,
        rating,
        profit_k: (profit / 1000) as i16,
        profit,
    }
}

/// Rating adjustment for buildings owned by a computer-controlled company (`0x40b719`).
pub fn computer_owner_rating(r: u16) -> u16 {
    if r < 100 { (r + 100) / 2 } else { r + 5 }
}

/// Purchase price of an unowned building (`building_price` `0x40bae0`): `ftol(m * base)` with
/// `m = effect_multiplier(0xd, company, 0, territory)`; owned buildings return their stored price.
pub fn building_price(base: i32, effect: f64) -> i32 {
    ftol(effect * f64::from(base))
}

// ---- 4. monthly town bounty --------------------------------------------------------------------

/// Threshold of the monthly draw that may open a connection bounty (`sim_month_end` `0x4a5320`):
/// `rng(0, 10000) < threshold`. `None` outside months 1..=10.
pub fn bounty_draw_threshold(month: u8, year: i32, map_width: i32) -> Option<i32> {
    if month >= 11 {
        return None;
    }
    let d = (year - 1860).abs();
    Some(((50 - d) * (54300 / map_width)) / 70)
}

/// `game_growth_factor`: the value written to `0x5705d1` at every year end (`0x4a4d00`):
/// `(1 + rate)^(year - start_year + 1) * level% / 100`, with the rate from `0x57022b` (f32).
pub fn growth_factor(rate: f32, years_since_start: i32, level_percent: i32) -> f32 {
    let p = crate::fp::pow(f64::from(rate) + 1.0, f64::from(years_since_start + 1));
    f32r(p * f64::from(level_percent) * 0.01) as f32
}

/// A place as `town_bounty_offer` (`0x431660`) sees it.
#[derive(Clone, Copy, Debug)]
pub struct OfferPlace {
    pub kind: u8,
    /// `+0x36`: pending bounty (0 = none).
    pub bounty: i32,
    pub size_class: u8,
    pub cx: i32,
    pub cy: i32,
    pub bbox: [i32; 4],
}

/// `town_bounty_offer` (`0x431660`, runs when the monthly draw of `0x4a5320` succeeds): picks
/// the place that opens a connection bounty. `places` is `list_towns` in list order, `stations`
/// the `(x, y)` of every station. Up to 500 draws of `rng(0, n - 1)`; the place must be a city
/// without a pending bounty and without a station within 9 tiles. Returns the place index and
/// the bounty.
pub fn town_bounty_offer(
    rng: &mut GameRng,
    tab: &GrowthTables,
    places: &[OfferPlace],
    stations: &[(i32, i32)],
) -> Option<(usize, i32)> {
    if places.is_empty() {
        return None;
    }
    for _ in 0..500 {
        let i = rng.range(0, places.len() as i32 - 1) as usize;
        let p = &places[i];
        if p.kind != KIND_CITY || p.bounty > 0 {
            continue;
        }
        let near = stations
            .iter()
            .filter(|&&(sx, sy)| {
                p.bbox[0] <= sx && sx <= p.bbox[2] && p.bbox[1] <= sy && sy <= p.bbox[3] && {
                    let dx = f64::from(sx - p.cx);
                    let dy = f64::from(sy - p.cy);
                    (dx * dx + dy * dy).sqrt() < 9.0
                }
            })
            .count();
        if near == 0 {
            return Some((i, tab.bounty[usize::from(p.size_class)]));
        }
    }
    None
}

// ---- 5. yearly bookkeeping of `world_growth_step` (0x430a40) -------------------------------------------

/// Scarcity weight (`0x5702cc`, percent) of a building type that has `count` buildings in the
/// whole world: `500 / (count + 1)` below 3 buildings, 100 otherwise. The world-creation call
/// (`initial`) keeps all weights at 100.
pub fn type_scarcity_percent(count: i32) -> i32 {
    if count < 3 { 500 / (count + 1) } else { 100 }
}

/// Yearly decay of the delivery counters (`+0x3e`, `+0x40` of each place and `0x57070f`):
/// `ftol(v * 0.8 + 0.5)`.
pub fn decay_counter(v: i32) -> i32 {
    ftol(f64::from(v) * 0.8 + 0.5)
}

/// `building_init_slot_clocks` (`0x40bb40`): the four production clocks (`+0x53`) of a new
/// building. Each starts at `now` (or at the first minute of the slot's first year when that
/// year is still ahead); real buildings (`decorative == false`) then draw `rng(0, 100)` per slot
/// (4 draws, empty slots included) and start that percentage of one production period earlier,
/// so that new industries do not all produce on the same day.
pub fn initial_slot_clocks(
    rng: &mut GameRng,
    ty: &IndustryType,
    year: i32,
    now: u32,
    decorative: bool,
) -> [u32; 4] {
    let mut out = [0u32; 4];
    for (k, sl) in ty.slots.iter().enumerate() {
        let mut clock = now;
        if i32::from(sl.year_from) > year {
            clock = (i32::from(sl.year_from) * 483_840) as u32;
        }
        if !decorative {
            let m: f32 = match sl.output {
                1 => 1.1,
                0 => 1.25,
                _ => 1.42,
            };
            let rate = if f64::from(sl.rate) > 0.1 {
                sl.rate
            } else {
                1.0
            };
            let r = rng.range(0, 100);
            let back = ((f64::from(r) * (483_840.0 / f64::from(rate))) * f64::from(0.01f32))
                * f64::from(m);
            clock = ftol(f64::from(clock as f32) - back) as u32;
        }
        out[k] = clock;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scarcity_and_decay() {
        assert_eq!(
            [0, 1, 2, 3, 9].map(type_scarcity_percent),
            [500, 250, 166, 100, 100]
        );
        assert_eq!(
            [0, 1, 2, 10, 100, 1000].map(decay_counter),
            [0, 1, 2, 8, 80, 800]
        );
    }

    #[test]
    fn bounty_threshold() {
        assert_eq!(bounty_draw_threshold(11, 1860, 400), None);
        // 1860, map 400 wide: 50 * (54300 / 400 = 135) / 70 = 96.
        assert_eq!(bounty_draw_threshold(5, 1860, 400), Some(96));
        // 50 years away the chance reaches zero, beyond it goes negative (never drawn).
        assert_eq!(bounty_draw_threshold(5, 1910, 400), Some(0));
        assert!(bounty_draw_threshold(5, 1950, 400).unwrap() < 0);
    }
}
