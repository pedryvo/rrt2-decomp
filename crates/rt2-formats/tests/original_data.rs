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
