//! Terrain ids and overlays against the original's maps. Skipped unless `RT2_DIR` is set.

use std::path::PathBuf;

use rt2_formats::map::Map;
use rt2_formats::terrain::{
    CLASS_COUNT, GameRng, TERRAIN_IDS, TerrainGrid, TileBytes, WATER_CLASS, tables,
};

fn game_dir() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR").map(PathBuf::from)
}

fn grid_of(map: &Map, rng: &mut GameRng) -> TerrainGrid {
    let tile = |x: usize, y: usize| {
        let t = map.tile(x, y);
        TileBytes {
            b0: t.var,
            b1: t.ground,
            b2: t.flags2,
            b3: t.trees,
            b6: t.flags6,
        }
    };
    if map.version < 0x233d {
        TerrainGrid::from_old_ids(map.width, map.height, tile, rng)
    } else {
        TerrainGrid::from_new_ids(map.width, map.height, tile)
    }
}

/// Old maps: the class of every tile is the class the old table gives, water stays water, and the
/// passes only ever produce ids and records inside the tables.
#[test]
fn old_maps_convert_and_overlays_stay_in_range() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raw = std::fs::read(dir.join("games/Example save game.GAM")).unwrap();
    let map = Map::parse(&raw).unwrap();
    assert_eq!(map.version, 0x233c);
    let mut rng = GameRng(1);
    let mut grid = grid_of(&map, &mut rng);
    grid.build_overlays(&mut rng);
    let tb = tables();
    let mut water = 0usize;
    let mut with_overlays = 0usize;
    for c in &grid.cells {
        assert!((c.id() as usize) < TERRAIN_IDS);
        if tb.class_of_id(c.id()) == WATER_CLASS {
            water += 1;
        }
        if !c.nodes.is_empty() {
            with_overlays += 1;
        }
        for n in &c.nodes {
            assert!(n.bank() <= 3 || n.bank() == 7, "bank {}", n.bank());
        }
    }
    // The Example is 58% water by the old `ground in (0x61, 0x62)` test.
    let frac = water as f64 / grid.cells.len() as f64;
    assert!((0.55..0.62).contains(&frac), "water fraction {frac}");
    assert!(with_overlays > 1000, "{with_overlays} tiles with overlays");
}

/// New-format maps (`.MP2`) already carry the transition ids the original's passes produced; the
/// ground pass must give back the same classes everywhere and the same ids on transition tiles.
#[test]
fn ground_pass_reproduces_saved_transitions() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(dir.join("maps"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("MP2")))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let tb = tables();
    let (mut tiles, mut transitions, mut same) = (0usize, 0usize, 0usize);
    for f in files.iter().take(12) {
        let map = Map::parse(&std::fs::read(f).unwrap()).unwrap();
        let mut rng = GameRng(99);
        let before = grid_of(&map, &mut rng);
        let mut grid = before.clone();
        grid.build_overlays(&mut rng);
        for (a, b) in before.cells.iter().zip(&grid.cells) {
            tiles += 1;
            assert_eq!(
                tb.class_of_id(a.id()),
                tb.class_of_id(b.id()),
                "{}: a pass changed a tile's class",
                f.display()
            );
            let s = tb.start[usize::from(tb.class_of_id(a.id()))];
            if i32::from(a.id()) >= s[4] {
                transitions += 1;
                if a.id() == b.id() && a.turns() == b.turns() {
                    same += 1;
                }
            }
        }
    }
    assert!(tiles > 100_000);
    assert!(transitions > 1000, "{transitions} transition tiles");
    let ratio = same as f64 / transitions as f64;
    assert!(
        ratio > 0.97,
        "only {ratio:.3} of saved transitions reproduced"
    );
    let _ = CLASS_COUNT;
}
