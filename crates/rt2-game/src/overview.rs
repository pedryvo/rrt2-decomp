//! Data of the Map Overview layers (screen C6, docs/reference/screens.md; manual chapter 4,
//! "Map Overview"): resource supply and demand per building, the supply waiting at each
//! station, the grade, traffic and owner of every track tile.
//!
//! Sources: building boxes `+0x26..+0x2d`, type `+7` and `industry_type_table`
//! (`docs/spec/cargo.md` 7.1: output cargo per slot, the rebuilt input list); station cargo
//! chains `+0x61 + 2c` (`docs/spec/cargo.md` 2); track pieces `docs/spec/terrain-track.md` 1.3
//! (`+2` owner, `+0x12` grade in tenths of a percent) and `docs/spec/trains.md` 4.1 / 11
//! (`+0x17` / `+0x1b` wear, added on every entry of a locomotive).
//!
//! Gap: the original's traffic-density measure and colour ramps are not in the specs; the
//! clone uses the accumulated wear of the track piece (the only per-piece usage counter the
//! simulation keeps) relative to the busiest piece.

use std::collections::BTreeSet;

use rt2_formats::map::Save;
use rt2_sim::cargo::NO_CARGO;
use rt2_sim::data::GameData;

use crate::buildtools::{Rec, rec};

/// A building on the map that supplies or demands one of the selected cargos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker {
    /// Footprint box in tiles (inclusive).
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub supply: bool,
    pub demand: bool,
}

/// Cargo ids the building type `ty` supplies and demands in `year`.
pub fn building_cargos(data: &GameData, ty: u8, year: i32) -> (Vec<u8>, Vec<u8>) {
    let Some(mut it) = data
        .industries
        .first()
        .and_then(|set| set.get(usize::from(ty)))
        .cloned()
    else {
        return (Vec::new(), Vec::new());
    };
    it.rebuild_inputs(year);
    let mut supply: Vec<u8> = it
        .slots
        .iter()
        .filter(|s| {
            i32::from(s.year_from) <= year && year <= i32::from(s.year_to) && s.output != NO_CARGO
        })
        .map(|s| s.output)
        .collect();
    supply.sort_unstable();
    supply.dedup();
    let mut demand: Vec<u8> = it
        .inputs
        .iter()
        .copied()
        .filter(|&c| c != NO_CARGO)
        .collect();
    demand.sort_unstable();
    demand.dedup();
    (supply, demand)
}

/// A building box with the cargos it supplies and demands.
pub type Building = (Marker, Vec<u8>, Vec<u8>);

/// Every non-station building box with its supplied and demanded cargos (`year` = game year).
pub fn buildings(save: &Save, data: &GameData, year: i32) -> Vec<Building> {
    save.objects
        .ordered()
        .filter_map(|o| {
            let b = Rec(&o.data);
            let ty = b.u8(7);
            if ty == 0 {
                return None;
            }
            let (supply, demand) = building_cargos(data, ty, year);
            let m = Marker {
                x0: i32::from(b.i16(0x26)),
                y0: i32::from(b.i16(0x28)),
                x1: i32::from(b.i16(0x2a)),
                y1: i32::from(b.i16(0x2c)),
                supply: false,
                demand: false,
            };
            Some((m, supply, demand))
        })
        .collect()
}

/// Cargos that some building supplies and cargos that some building demands: the list box shows
/// the first in white and the second in yellow.
pub fn cargo_roles(blds: &[Building]) -> (BTreeSet<u8>, BTreeSet<u8>) {
    let (mut s, mut d) = (BTreeSet::new(), BTreeSet::new());
    for (_, su, de) in blds {
        s.extend(su.iter().copied());
        d.extend(de.iter().copied());
    }
    (s, d)
}

/// The buildings that supply or demand any of `selected`.
pub fn resource_markers(blds: &[Building], selected: &BTreeSet<u8>) -> Vec<Marker> {
    if selected.is_empty() {
        return Vec::new();
    }
    blds.iter()
        .filter_map(|(m, su, de)| {
            let mut m = *m;
            m.supply = su.iter().any(|c| selected.contains(c));
            m.demand = de.iter().any(|c| selected.contains(c));
            (m.supply || m.demand).then_some(m)
        })
        .collect()
}

/// Lots waiting at a station over all cargos (`pool_7c` chains from `+0x61 + 2c`).
pub fn station_waiting(save: &Save, st: &Rec, cargos: usize) -> u32 {
    let mut total = 0;
    for c in 0..cargos {
        let mut id = u32::from(st.u16(0x61 + 2 * c));
        let mut n = 0;
        while id != 0 && n < 5000 {
            n += 1;
            id = rec(&save.pool_7c, id as i32).map_or(0, |l| u32::from(l.u16(0x13)));
        }
        total += n;
    }
    total
}

/// `(x, y, lots waiting)` of every station.
pub fn station_statuses(save: &Save, cargos: usize) -> Vec<(i32, i32, u32)> {
    save.stations
        .ordered()
        .map(|s| {
            let r = Rec(&s.data);
            (r.i32(0xa), r.i32(0xe), station_waiting(save, &r, cargos))
        })
        .collect()
}

/// One track tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackTile {
    pub x: i32,
    pub y: i32,
    /// Grade in tenths of a percent (`+0x12`, at most 120).
    pub grade: u8,
    pub wear: f32,
    pub owner: u8,
}

/// All tiles that carry track.
pub fn track_tiles(save: &Save) -> Vec<TrackTile> {
    let map = &save.map;
    let mut out = Vec::new();
    // Pool lookups are linear: index the pieces by slot once.
    let top = save
        .track_pieces
        .records
        .iter()
        .map(|r| r.slot)
        .max()
        .unwrap_or(0);
    let mut by_slot = vec![None; top + 1];
    for r in &save.track_pieces.records {
        by_slot[r.slot] = Some(r);
    }
    for y in 0..map.height {
        for x in 0..map.width {
            let id = map.tile(x, y).track_id;
            if id == 0 {
                continue;
            }
            let Some(p) = by_slot.get(usize::from(id)).copied().flatten() else {
                continue;
            };
            let r = Rec(&p.data);
            let wear = r.f32(0x17).max(0.0) + r.f32(0x1b).max(0.0);
            out.push(TrackTile {
                x: x as i32,
                y: y as i32,
                grade: r.u8(0x12),
                wear: if wear.is_finite() { wear } else { 0.0 },
                owner: r.u8(2),
            });
        }
    }
    out
}

/// Grade bands (tenths of a percent): below 1 %, 1..2, 2..3, 3..5, 5 and more.
pub const GRADE_LIMITS: [u8; 4] = [10, 20, 30, 50];

pub fn grade_band(grade: u8) -> usize {
    GRADE_LIMITS.iter().take_while(|&&l| grade >= l).count()
}

/// Traffic band 0..=4 of a wear value relative to the busiest tile (`None` = unused).
pub fn traffic_band(wear: f32, max: f32) -> Option<usize> {
    if wear <= 0.0 || max <= 0.0 {
        return None;
    }
    let f = wear / max;
    Some(if f < 0.05 {
        0
    } else if f < 0.2 {
        1
    } else if f < 0.45 {
        2
    } else if f < 0.75 {
        3
    } else {
        4
    })
}

/// Number of track tiles each company owns, by company slot.
pub fn track_by_owner(tiles: &[TrackTile]) -> std::collections::BTreeMap<u8, usize> {
    let mut m = std::collections::BTreeMap::new();
    for t in tiles {
        *m.entry(t.owner).or_insert(0) += 1;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_bands_follow_the_limits() {
        assert_eq!(grade_band(0), 0);
        assert_eq!(grade_band(9), 0);
        assert_eq!(grade_band(10), 1);
        assert_eq!(grade_band(29), 2);
        assert_eq!(grade_band(30), 3);
        assert_eq!(grade_band(120), 4);
    }

    #[test]
    fn traffic_is_relative_to_the_busiest_tile() {
        assert_eq!(traffic_band(0.0, 10.0), None);
        assert_eq!(traffic_band(1.0, 0.0), None);
        assert_eq!(traffic_band(10.0, 10.0), Some(4));
        assert_eq!(traffic_band(0.1, 10.0), Some(0));
        assert_eq!(traffic_band(3.0, 10.0), Some(2));
    }

    #[test]
    fn owners_are_counted() {
        let t = |owner| TrackTile {
            x: 0,
            y: 0,
            grade: 0,
            wear: 0.0,
            owner,
        };
        let m = track_by_owner(&[t(1), t(1), t(2)]);
        assert_eq!(m.get(&1), Some(&2));
        assert_eq!(m.get(&2), Some(&1));
    }
}
