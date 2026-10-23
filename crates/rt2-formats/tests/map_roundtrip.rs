//! Every shipped `.MAP`/`.MP2`/`.GAM` loaded and written back unchanged is byte-identical
//! (docs/formats/map-gam.md 1; the editor saves through `Save::to_bytes`). Skipped unless
//! `RT2_DIR` points at an original install.

use std::path::{Path, PathBuf};

use rt2_formats::map::Save;

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(ext.as_str(), "map" | "mp2" | "gam") {
            out.push(p);
        }
    }
}

#[test]
fn every_shipped_map_round_trips_byte_for_byte() {
    let Some(dir) = std::env::var_os("RT2_DIR").map(PathBuf::from) else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut files = Vec::new();
    collect(&dir.join("maps"), &mut files);
    collect(&dir.join("games"), &mut files);
    assert!(!files.is_empty());
    let mut bad = Vec::new();
    for f in &files {
        let raw = std::fs::read(f).unwrap();
        let save = Save::parse(&raw).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let out = save.to_bytes();
        if out != raw {
            bad.push((f.clone(), raw.len(), out.len()));
        }
    }
    eprintln!("{} files checked, {} differ", files.len(), bad.len());
    assert!(bad.is_empty(), "differ: {bad:?}");
}
