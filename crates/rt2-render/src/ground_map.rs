//! Which texture a tile gets, and which track record a piece draws.
//!
//! The ground mapping is PROVISIONAL, see [`ground_texture`]. The track mapping is exact
//! (`docs/formats/sprite-catalog.md` section 4.2).

use rt2_formats::map::Tile;

/// Deterministic per-tile hash used to vary sprites and scatter trees.
pub fn tile_hash(x: usize, y: usize, salt: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ salt;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// A run of `ground` bytes drawn with a run of "pure" `ground.imb` records (the interior
/// textures, not the transition tiers with transparent water noise).
///
/// Reading of the contact sheet (`sprite-catalog.md` section 4.1): the bank is a sequence of
/// families, each a few pure records followed by two transition records, e.g. light sand 0-2,
/// ochre sand 13-18, dirt 21-26, ochre 29-37, yellow-olive 40-60, olive-green 63-77, green 80-94,
/// deep green 97-109, snow 112-116, wheat 119-123, swamp 170-177, dark rock 180-183, brown rock
/// 186-189, grey rock 192-195, beige rock 198-201.
struct Cluster {
    ground: (u8, u8),
    records: (u16, u16),
}

const CLUSTERS: [Cluster; 17] = [
    Cluster {
        ground: (0x00, 0x00),
        records: (0, 2),
    },
    Cluster {
        ground: (0x01, 0x05),
        records: (13, 18),
    },
    Cluster {
        ground: (0x06, 0x0b),
        records: (29, 37),
    },
    Cluster {
        ground: (0x0c, 0x10),
        records: (52, 60),
    },
    Cluster {
        ground: (0x11, 0x17),
        records: (63, 77),
    },
    Cluster {
        ground: (0x18, 0x1a),
        records: (40, 60),
    },
    Cluster {
        ground: (0x1b, 0x20),
        records: (63, 77),
    },
    Cluster {
        ground: (0x21, 0x25),
        records: (66, 77),
    },
    Cluster {
        ground: (0x26, 0x2f),
        records: (80, 94),
    },
    Cluster {
        ground: (0x30, 0x3f),
        records: (97, 99),
    },
    Cluster {
        ground: (0x40, 0x42),
        records: (112, 116),
    },
    Cluster {
        ground: (0x43, 0x43),
        records: (170, 177),
    },
    Cluster {
        ground: (0x44, 0x4f),
        records: (119, 123),
    },
    Cluster {
        ground: (0x50, 0x5a),
        records: (186, 189),
    },
    Cluster {
        ground: (0x5b, 0x5c),
        records: (198, 201),
    },
    Cluster {
        ground: (0x5d, 0x60),
        records: (192, 195),
    },
    Cluster {
        ground: (0x61, 0xff),
        records: (80, 94),
    },
];

/// `ground.imb` record for a tile.
///
/// PROVISIONAL. The original picks the record with `DAT_005804ac[terrain id]`, built in `0x471b10`,
/// where the terrain id is the 10-bit value `tile[0..2] >> 6` (that is `ground << 2 | var >> 6`).
/// That table is not decoded yet (`sprite-catalog.md` section 10, open question 1). Evidence that
/// ids are ordered like the records: `ground 0x00` only ever uses `var >> 6` 0..2 (ids 0-2, the
/// three pure light-sand records, never id 3 = the transparent transition) and `0x04` is almost
/// all `var >> 6 = 3` (id 19 = the water-noise record) at the shoreline. Beyond the first
/// families the offset between id and record is unknown, so each run of `ground` bytes is spread
/// monotonically over the pure records of the family it looks like (neighbouring ids give
/// neighbouring, similar textures, so the map stays smooth). Water ids 0x61/0x62 do not use
/// `ground.imb` at all (they use the ocean bank).
// TODO(sprite-catalog.md section 10.1): replace by the real id -> record table once decoded.
pub fn ground_texture(tile: &Tile) -> u16 {
    let g = tile.ground;
    let c = CLUSTERS
        .iter()
        .find(|c| (c.ground.0..=c.ground.1).contains(&g))
        .unwrap_or(&CLUSTERS[8]);
    let first_id = c.ground.0 as u32 * 4;
    let ids = (c.ground.1 as u32 - c.ground.0 as u32 + 1) * 4;
    let id = g as u32 * 4 + (tile.var >> 6) as u32;
    let (lo, hi) = (c.records.0 as u32, c.records.1 as u32);
    (lo + (id - first_id) * (hi - lo + 1) / ids).min(hi) as u16
}

/// Ocean variant 0..4 (the four turns of one animation frame): the water terrain ids are
/// `0x185..=0x188`, i.e. `ground 0x61` with `var >> 6 = 1..=3` and `ground 0x62` with 0.
pub fn water_variant(tile: &Tile) -> u16 {
    let id = (tile.ground as u16) << 2 | (tile.var >> 6) as u16;
    id.wrapping_sub(0x185) & 3
}

/// Quarter turns (0..4) the tile's texture is rotated by: `tile[0] >> 4 & 3`
/// (`sprite-catalog.md` section 2.1).
pub fn texture_turns(tile: &Tile) -> u8 {
    (tile.var >> 4) & 3
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

    fn tile(ground: u8, var: u8) -> Tile {
        Tile {
            var,
            ground,
            flags2: 0,
            trees: 0,
            struct_id: 0,
            reserved5: 0,
            flags6: 0,
            track_id: 0,
            reserved8: 0,
            region: 1,
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
    fn ground_records_stay_inside_the_bank() {
        for g in 0..=0x62u8 {
            for v in [0u8, 0x40, 0x80, 0xc0] {
                let r = ground_texture(&tile(g, v));
                assert!(r < 221, "ground {g:#x} -> {r}");
            }
        }
    }

    #[test]
    fn neighbouring_ids_give_neighbouring_records() {
        // The grass plateau of Britain (ground 0x2d, ids 0xb4..=0xb7) stays inside the green family.
        let recs: Vec<u16> = (0..4)
            .map(|v| ground_texture(&tile(0x2d, v << 6)))
            .collect();
        assert!(recs.windows(2).all(|w| w[0] <= w[1]));
        assert!(recs.iter().all(|r| (80..=94).contains(r)), "{recs:?}");
        // Light sand never selects the transparent transition records 3 and 4.
        for v in 0..4 {
            assert!(ground_texture(&tile(0x00, v << 6)) <= 2);
        }
    }

    #[test]
    fn water_variants_cover_the_four_ids() {
        assert_eq!(water_variant(&tile(0x61, 0x40)), 0);
        assert_eq!(water_variant(&tile(0x61, 0x80)), 1);
        assert_eq!(water_variant(&tile(0x61, 0xc0)), 2);
        assert_eq!(water_variant(&tile(0x62, 0x00)), 3);
    }

    #[test]
    fn texture_turns_use_bits_four_and_five() {
        assert_eq!(texture_turns(&tile(0, 0x30)), 3);
        assert_eq!(texture_turns(&tile(0, 0xc1)), 0);
    }
}
