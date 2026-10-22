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
