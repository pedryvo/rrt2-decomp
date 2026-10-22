//! Which textures a tile gets, and which track record a piece draws.
//!
//! The ground mapping is the original's (`rt2_formats::terrain`, `docs/formats/sprite-catalog.md`
//! section 4.1); so is the track mapping (section 4.2).

use rt2_formats::terrain::{self, Cell, Node, tables};

/// Deterministic per-tile hash used to vary sprites and scatter trees.
pub fn tile_hash(x: usize, y: usize, salt: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ salt;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Marks "no overlay" in an overlay code.
pub const NO_OVERLAY: u32 = u32::MAX;

/// Stack an overlay code samples: bits 14-15 of the code.
pub const STACK_GROUND: u32 = 0;
/// `riveran.imb`; the shader adds the animation frame (5 frames x 18 pieces).
pub const STACK_RIVER: u32 = 1;
/// `road.imb`, after the 90 river layers of the "extra" stack.
pub const STACK_ROAD: u32 = 2;
/// `grbeacha.imb`; the shader adds the animation frame (32 frames x 8 pieces).
pub const STACK_BEACH: u32 = 3;

/// Ground records 5..=12 are the shore pieces the original replaces with the animated
/// `grbeacha.imb` bank (`0x511a90`: `(record - 5) + (counter & 31) * 8`).
const SHORE_RECORDS: std::ops::RangeInclusive<u16> = 5..=12;

/// Records of `ground.imb`; record numbers from here up are the ocean variants.
pub const GROUND_RECORDS: u16 = terrain::GROUND_RECORDS;

/// Packs an overlay node for the shader: `layer | turns << 12 | stack << 14`. `None` for layers the
/// viewer does not draw (territory borders).
pub fn overlay_code(node: &Node) -> Option<u32> {
    let rec = node.record();
    let (stack, layer) = match node.bank() {
        terrain::BANK_GROUND if SHORE_RECORDS.contains(&rec) => (STACK_BEACH, rec - 5),
        terrain::BANK_GROUND => (STACK_GROUND, rec),
        terrain::BANK_RIVER => (STACK_RIVER, rec),
        terrain::BANK_ROAD => (STACK_ROAD, rec),
        _ => return None,
    };
    Some(u32::from(layer) | u32::from(node.turns & 3) << 12 | stack << 14)
}

/// What a terrain cell is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellLook {
    /// Record of `ground.imb` for the base texture; `None` for water.
    pub ground: Option<u16>,
    /// Variant 0..4 of the ocean texture for water; the shader adds the animation frame.
    pub water: Option<u16>,
    /// Quarter turns of the base texture.
    pub turns: u8,
    /// Overlay codes (first = on top), [`NO_OVERLAY`] = unused slot.
    pub overlays: [u32; 4],
}

/// Looks of a terrain cell: the base texture picked by `0x5804ac[id]` and the overlay list.
pub fn cell_look(cell: &Cell) -> CellLook {
    let rec = tables().record_of_id(cell.id());
    let mut overlays = [NO_OVERLAY; 4];
    let mut n = 0;
    for node in &cell.nodes {
        if let Some(code) = overlay_code(node)
            && n < overlays.len()
        {
            overlays[n] = code;
            n += 1;
        }
    }
    if rec >= GROUND_RECORDS {
        CellLook {
            ground: None,
            water: Some(rec - GROUND_RECORDS),
            turns: 0,
            overlays,
        }
    } else {
        CellLook {
            ground: Some(rec),
            water: None,
            turns: cell.turns(),
            overlays,
        }
    }
}

/// `(edge mask, tracks.imb record)` for the 50 valid masks (`sprite-catalog.md` section 4.2,
/// `edge_shape_table` `0x5365e8` followed by `shape_record` `0x536594`). `trackd.imb` (double
/// track) uses the same index.
pub const TRACK_RECORDS: [(u8, u8); 50] = [
    (1, 0),
    (2, 9),
    (4, 52),
    (8, 57),
    (9, 2),
    (16, 39),
    (17, 1),
    (18, 41),
    (19, 43),
    (21, 63),
    (25, 4),
    (32, 44),
    (33, 3),
    (34, 10),
    (35, 11),
    (36, 53),
    (38, 12),
    (42, 38),
    (49, 5),
    (50, 45),
    (51, 7),
    (64, 21),
    (66, 23),
    (68, 22),
    (69, 19),
    (70, 25),
    (72, 24),
    (76, 26),
    (81, 37),
    (84, 50),
    (85, 6),
    (98, 46),
    (100, 55),
    (102, 28),
    (128, 29),
    (132, 54),
    (136, 30),
    (137, 32),
    (138, 51),
    (140, 58),
    (144, 40),
    (145, 42),
    (152, 59),
    (153, 8),
    (162, 64),
    (168, 20),
    (170, 13),
    (196, 56),
    (200, 31),
    (204, 27),
];

/// Record of `tracks.imb` / `trackd.imb` for an edge mask, `None` for masks the game never builds.
pub fn track_record(mask: u8) -> Option<u8> {
    TRACK_RECORDS
        .binary_search_by_key(&mask, |&(m, _)| m)
        .ok()
        .map(|i| TRACK_RECORDS[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(id: u16, low: u16) -> Cell {
        Cell {
            v: id << 6 | low,
            ..Default::default()
        }
    }

    #[test]
    fn track_table_is_sorted_and_covers_every_valid_mask() {
        assert!(TRACK_RECORDS.windows(2).all(|w| w[0].0 < w[1].0));
        // The 50 masks of terrain-track.md 1.4 (42 common + 8 T junctions).
        let valid = [
            1, 2, 4, 8, 16, 32, 64, 128, 9, 17, 18, 19, 25, 33, 34, 35, 36, 38, 49, 50, 51, 66, 68,
            70, 72, 76, 85, 98, 100, 102, 132, 136, 137, 140, 144, 145, 152, 153, 170, 196, 200,
            204, 21, 42, 69, 81, 84, 138, 162, 168,
        ];
        assert_eq!(valid.len(), 50);
        for m in valid {
            assert!(track_record(m).is_some(), "mask {m}");
        }
        assert_eq!(track_record(3), None);
        assert_eq!(track_record(0), None);
        // Records are unique and inside the 69-record bank.
        let mut recs: Vec<u8> = TRACK_RECORDS.iter().map(|&(_, r)| r).collect();
        recs.sort_unstable();
        recs.dedup();
        assert_eq!(recs.len(), 50);
        assert!(recs.iter().all(|&r| r < 65));
    }

    #[test]
    fn ground_ids_pick_the_tables_record() {
        // Lush grass (class 6) ids 209..=214 are records 86..=91; ids 215..=217 the rare ones.
        assert_eq!(cell_look(&cell(209, 0)).ground, Some(86));
        assert_eq!(cell_look(&cell(214, 0)).ground, Some(91));
        assert_eq!(cell_look(&cell(0, 0)).ground, Some(0));
        // Turns come from the tile's own bits 4-5.
        assert_eq!(cell_look(&cell(209, 0x30)).turns, 3);
    }

    #[test]
    fn water_ids_use_the_ocean_variants() {
        // Class 18 starts at id 590; its four own ids are ocean variants 0..4.
        for (i, id) in (590u16..594).enumerate() {
            let l = cell_look(&cell(id, 0));
            assert_eq!(l.ground, None);
            assert_eq!(l.water, Some(i as u16));
        }
    }

    #[test]
    fn overlay_codes_select_the_stack() {
        // Ground piece 3, one turn.
        let n = Node {
            word: 3 << 6,
            turns: 1,
            shade: 0,
        };
        assert_eq!(overlay_code(&n), Some(3 | 1 << 12));
        // Ground record 6 is a shore piece, drawn from the animated beach bank.
        let n = Node {
            word: 6 << 6,
            turns: 2,
            shade: 0,
        };
        assert_eq!(overlay_code(&n), Some(1 | 2 << 12 | STACK_BEACH << 14));
        // River and road nodes carry their bank in bits 3-5.
        let river = Node {
            word: 4 << 6 | 0x12,
            turns: 0,
            shade: 0,
        };
        assert_eq!(overlay_code(&river), Some(4 | STACK_RIVER << 14));
        let road = Node {
            word: 9 << 6 | 0x1b,
            turns: 3,
            shade: 0,
        };
        assert_eq!(overlay_code(&road), Some(9 | 3 << 12 | STACK_ROAD << 14));
        // Territory borders are not drawn.
        let t = Node {
            word: 1 << 6 | 0x09,
            turns: 0,
            shade: 0,
        };
        assert_eq!(overlay_code(&t), None);
    }
}
