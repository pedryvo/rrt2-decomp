//! `re/names/sprite_catalog.tsv` against an original game install (skipped without `RT2_DIR`).
//! The catalogue itself holds no game data, only hashes, names and classifications.

use std::collections::HashMap;
use std::path::PathBuf;

use rt2_formats::lad::Lad;
use rt2_formats::pk2::{Pk2, name_hash};

fn catalog() -> Vec<(u32, String, String)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../re/names/sprite_catalog.tsv");
    let text = std::fs::read_to_string(path).unwrap();
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let mut f = l.split('\t');
            let hash = u32::from_str_radix(f.next().unwrap(), 16).unwrap();
            (
                hash,
                f.next().unwrap().to_string(),
                f.next().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn catalog_rows_are_well_formed() {
    let rows = catalog();
    assert!(rows.len() > 1900);
    let mut seen = std::collections::HashSet::new();
    for (hash, name, category) in &rows {
        assert!(seen.insert(*hash), "duplicate hash {hash:08x}");
        assert!(!category.is_empty());
        if name != "-" {
            assert_eq!(name_hash(name), *hash, "{name} does not hash to its row");
        }
    }
}

#[test]
fn catalog_covers_every_lad_bank() {
    let Some(dir) = std::env::var_os("RT2_DIR").map(PathBuf::from) else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut lads: HashMap<u32, usize> = HashMap::new();
    for name in ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"] {
        let raw = std::fs::read(dir.join("data").join(name)).unwrap();
        let pk2 = Pk2::parse(&raw).unwrap();
        for e in pk2.entries() {
            let blob = pk2.contents(e);
            if let Ok((lad, used)) = Lad::parse_with_len(blob)
                && used == blob.len()
                && !lad.records.is_empty()
                && lad.records.iter().all(|r| matches!(r.kind(), 1 | 3))
            {
                lads.insert(e.hash, lad.records.len());
            }
        }
    }
    let rows = catalog();
    assert_eq!(rows.len(), lads.len());
    for (hash, name, _) in &rows {
        assert!(
            lads.contains_key(hash),
            "{name} ({hash:08x}) is not a LAD bank"
        );
    }
}
