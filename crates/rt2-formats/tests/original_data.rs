//! Checks against an original game install. Skipped unless `RT2_DIR` points at one.

use std::path::PathBuf;

use rt2_formats::pk2::Pk2;

fn game_dir() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR").map(PathBuf::from)
}

#[test]
fn every_pk2_parses_and_covers_the_whole_file() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    for name in ["RT2.PK2", "RT2_06.PK2", "RT2_09GO.PK2", "RT2_11PL.PK2"] {
        let raw = std::fs::read(dir.join("data").join(name)).unwrap();
        let pk2 = Pk2::parse(&raw).unwrap();
        let end = pk2
            .entries()
            .iter()
            .map(|e| pk2.range(e).end)
            .max()
            .unwrap();
        assert_eq!(end, raw.len(), "{name}: entries should end exactly at EOF");
    }
}

#[test]
fn default_language_file_parses() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raw = std::fs::read(dir.join("default2.lng")).unwrap();
    let lng = rt2_formats::lng::Lng::parse(&raw).unwrap();
    assert_eq!(lng.get(13), Some("Railroad Tycoon II - Platinum Edition"));
    assert_eq!(lng.table_entry("CARGO_TYPES", 0), Some("passenger"));
    assert_eq!(lng.strings.len(), 2807);
}

#[test]
fn known_names_resolve_in_rt2_pk2() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raw = std::fs::read(dir.join("data/RT2_11PL.PK2")).unwrap();
    let pk2 = Pk2::parse(&raw).unwrap();
    assert_eq!(pk2.entries()[0], *pk2.find_name("setup.imb").unwrap());
}

const PK2_NAMES: [&str; 4] = ["RT2.PK2", "RT2_06.PK2", "RT2_09GO.PK2", "RT2_11PL.PK2"];

/// Every entry of every PK2 as (archive name, entry hash, contents).
fn all_blobs(dir: &std::path::Path) -> Vec<(&'static str, u32, Vec<u8>)> {
    let mut out = Vec::new();
    for name in PK2_NAMES {
        let raw = std::fs::read(dir.join("data").join(name)).unwrap();
        let pk2 = Pk2::parse(&raw).unwrap();
        for e in pk2.entries() {
            out.push((name, e.hash, pk2.contents(e).to_vec()));
        }
    }
    out
}

/// A blob counts as LAD when it parses to the last byte with at least one record and every
/// record has a plausible kind. Everything else in the PK2s is audio, IMB layouts, palettes, LUTs.
fn parse_lad(blob: &[u8]) -> Option<rt2_formats::lad::Lad<'_>> {
    let (lad, used) = rt2_formats::lad::Lad::parse_with_len(blob).ok()?;
    let plausible = used == blob.len()
        && !lad.records.is_empty()
        && lad.records.iter().all(|r| matches!(r.kind(), 1 | 3));
    plausible.then_some(lad)
}

#[test]
fn lad_blobs_parse_exactly_and_decode() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut found = 0;
    let mut images = 0;
    for (archive, hash, blob) in all_blobs(&dir) {
        let Some(lad) = parse_lad(&blob) else {
            continue;
        };
        found += 1;
        for rec in &lad.records {
            for img in rec.groups.iter().flat_map(|g| &g.images) {
                img.decode()
                    .unwrap_or_else(|e| panic!("{archive} {hash:08x}: {e}"));
                images += 1;
            }
        }
    }
    // 1975 (RT2) + 12 (RT2_06) + 3 (RT2_11PL): every LAD blob, including the 80 that the
    // heuristic Python parser only parsed partially.
    assert_eq!(found, 1990);
    assert_eq!(images, 30952);
}

#[test]
fn sprite_palettes_resolve_by_name() {
    use rt2_formats::palette::{PaletteBank, palette_name};
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raw = std::fs::read(dir.join("data/RT2.PK2")).unwrap();
    let pk2 = Pk2::parse(&raw).unwrap();
    for id in 0..713 {
        let entry = pk2
            .find_name(&palette_name(id))
            .unwrap_or_else(|| panic!("missing {}", palette_name(id)));
        let bank = PaletteBank::parse(pk2.contents(entry)).unwrap();
        assert!(!bank.palettes.is_empty());
    }
    assert!(pk2.find_name(&palette_name(713)).is_none());
    // Banks 713 and up live in the patch archives; look up across all four.
    let blobs = all_blobs(&dir);
    let hashes: std::collections::HashSet<u32> = blobs.iter().map(|b| b.1).collect();
    let mut checked = 0;
    for (_, _, blob) in &blobs {
        let Some(lad) = parse_lad(blob) else {
            continue;
        };
        for rec in lad.records.iter().filter(|r| r.uses_palette()) {
            assert!(
                hashes.contains(&rt2_formats::pk2::name_hash(&palette_name(
                    rec.palette_id()
                ))),
                "palette {} missing",
                rec.palette_id()
            );
            checked += 1;
        }
    }
    assert!(checked > 10_000);
}

#[test]
fn every_map_and_save_parses() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut count = 0;
    for sub in ["maps", "games"] {
        for entry in std::fs::read_dir(dir.join(sub)).unwrap() {
            let path = entry.unwrap().path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_uppercase);
            let wanted = matches!(
                (sub, ext.as_deref()),
                ("maps", Some("MAP")) | ("games", Some("GAM"))
            );
            if !wanted {
                continue;
            }
            let raw = std::fs::read(&path).unwrap();
            let map = rt2_formats::map::Map::parse(&raw)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!((150..=500).contains(&map.width), "{}", path.display());
            count += 1;
        }
    }
    assert_eq!(count, 33, "31 maps + 2 saves");
}

#[test]
fn britain_water_fraction_is_plausible() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raw = std::fs::read(dir.join("maps").join("Britain.MAP")).unwrap();
    let map = rt2_formats::map::Map::parse(&raw).unwrap();
    assert_eq!((map.width, map.height), (300, 300));
    let water = map.water_fraction();
    assert!((0.3..0.9).contains(&water), "water fraction {water}");
    assert!(map.tiles.iter().all(|t| t.region >= 1));
}

fn read_game_file(dir: &std::path::Path, sub: &str, name: &str) -> Vec<u8> {
    std::fs::read(dir.join(sub).join(name)).unwrap()
}

#[test]
fn every_map_and_save_parses_with_state_to_the_last_byte() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut count = 0;
    for sub in ["maps", "games"] {
        for entry in std::fs::read_dir(dir.join(sub)).unwrap() {
            let path = entry.unwrap().path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_uppercase);
            if !matches!(ext.as_deref(), Some("MAP" | "GAM")) {
                continue;
            }
            let raw = std::fs::read(&path).unwrap();
            let save = rt2_formats::map::Save::parse(&raw)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(
                i64::from(save.game_time_minutes),
                save.minutes_from_date(),
                "{}",
                path.display()
            );
            assert_eq!(save.places.count, save.places.records.len());
            count += 1;
        }
    }
    assert_eq!(count, 33);
}

#[test]
fn tutorial_save_state() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let save =
        rt2_formats::map::Save::parse(&read_game_file(&dir, "games", "Tutorial.GAM")).unwrap();
    assert_eq!(save.date.year, 1870);
    assert_eq!((save.date.month, save.date.day), (2, 4));
    assert_eq!((save.hour, save.minute), (19, 3));
    assert_eq!(save.game_time_minutes, 904_826_583);
    assert_eq!(save.game_speed, 3);
    assert_eq!(save.game_seed, 183_274);
    let places = save.place_list();
    assert_eq!(places.len(), 103);
    let london = &places[0];
    assert_eq!(london.name, "London");
    assert_eq!((london.x, london.y), (260, 237));
    assert_eq!(london.bbox, [249, 226, 271, 248]);
    let names: Vec<_> = save.company_list().into_iter().map(|c| c.name).collect();
    assert_eq!(
        names,
        [
            "Great Western",
            "Liverpool & Manchester",
            "Middleton Colliery"
        ]
    );
    assert_eq!(save.rules.len(), 5);
    assert!(save.stations.records.is_empty() && save.trains.records.is_empty());
}

#[test]
fn example_save_state() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let save =
        rt2_formats::map::Save::parse(&read_game_file(&dir, "games", "Example save game.GAM"))
            .unwrap();
    assert_eq!(save.date.year, 1875);
    assert_eq!((save.date.month, save.date.day), (12, 10));
    assert_eq!((save.hour, save.minute), (5, 21));
    assert_eq!(save.game_time_minutes, 907_656_801);
    let companies = save.company_list();
    assert_eq!(companies.len(), 5);
    assert_eq!(companies[4].name, "Nottingham Northern");
    assert_eq!(companies[3].name, "CSX");
    let stations = save.station_names();
    assert_eq!(&stations[..3], ["Oxford", "Slough", "London"]);
    assert_eq!(stations.len(), 10);
    assert_eq!(save.train_names()[0], "Slough Bullet");
    assert_eq!(save.train_extras.len(), 9);
    assert_eq!(save.person_list()[0].name, "Isambard Kingdom Brunel");
    // Track pieces are what the tiles' `track_id` points at.
    let used: std::collections::BTreeSet<u8> = save
        .map
        .tiles
        .iter()
        .map(|t| t.track_id)
        .filter(|&t| t != 0)
        .collect();
    assert_eq!(used.len(), save.track_pieces.records.len());
}

#[test]
fn bo_scenario_places_and_seed() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let save =
        rt2_formats::map::Save::parse(&read_game_file(&dir, "maps", "#01B&O~1.MAP")).unwrap();
    assert_eq!(save.game_seed, 16451);
    assert_eq!(save.date.year, 1830);
    let places = save.place_list();
    let baltimore = places.iter().find(|p| p.name == "Baltimore").unwrap();
    assert_eq!((baltimore.x, baltimore.y), (72, 53));
    assert_eq!(baltimore.bbox, [65, 46, 79, 60]);
    // Every tile `struct_id` indexes the structure pool.
    let ids: std::collections::BTreeSet<u8> = save
        .map
        .tiles
        .iter()
        .map(|t| t.struct_id)
        .filter(|&t| t != 0)
        .collect();
    assert_eq!(ids.len(), save.structures.records.len());
    assert!(
        ids.iter()
            .all(|&i| save.structures.get(usize::from(i)).is_some())
    );
}
