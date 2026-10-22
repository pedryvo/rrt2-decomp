//! Differential tests against the original executable (skipped when absent).

use rt2_formats::pk2::{fold_name_byte, name_hash};
use rt2_oracle::Oracle;

fn oracle() -> Option<Oracle> {
    let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    });
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return None;
    }
    Some(Oracle::load_file(&path).expect("load exe"))
}

/// `FUN_004f0d70`: `mov al, cl; ...; ret`, so ECX = byte, result in AL.
const FOLD: u32 = 0x004f_0d70;

#[test]
fn fold_matches_original_for_all_bytes() {
    let Some(mut o) = oracle() else { return };
    for c in 0..=255u8 {
        let got = o.call_thiscall(FOLD, u32::from(c), &[]).unwrap() as u8;
        assert_eq!(got, fold_name_byte(c), "fold({c:#04x})");
    }
}

#[test]
fn unhooked_import_is_reported() {
    let Some(mut o) = oracle() else { return };
    let imp = o.image().imports[0].clone();
    // Call straight into the first IAT slot's stub.
    let slot = o.image().image_base + imp.iat_rva;
    let stub = o.read_u32(slot).unwrap();
    let err = o.call_cdecl(stub, &[]).unwrap_err().to_string();
    assert!(err.contains(&format!("{}!{}", imp.dll, imp.name)), "{err}");
}

#[test]
fn snapshot_restores_data() {
    let Some(mut o) = oracle() else { return };
    let data = o
        .image()
        .sections
        .iter()
        .find(|s| s.name == ".data")
        .unwrap()
        .clone();
    let addr = o.image().image_base + data.rva;
    let before = o.read_u32(addr).unwrap();
    o.write_u32(addr, before ^ 0xdead_beef).unwrap();
    o.restore().unwrap();
    assert_eq!(o.read_u32(addr).unwrap(), before);
}

#[test]
fn name_hash_still_matches_known_value() {
    assert_eq!(name_hash("setup.imb"), 0x0c9b_172f);
}
