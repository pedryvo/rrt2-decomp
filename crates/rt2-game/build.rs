//! Embeds `assets/lang/*.toml` (docs/i18n.md) into the binary: writes a table of
//! `(file stem, include_str!(file))` so a translation added to the folder is picked up by the
//! next build, and a missing folder simply gives an empty table.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let lang = manifest.join("../../assets/lang");
    println!("cargo:rerun-if-changed={}", lang.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&lang)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "toml"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = String::from("pub static EMBEDDED: &[(&str, &str)] = &[\n");
    for f in &files {
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let abs = f.canonicalize().unwrap_or_else(|_| f.clone());
        println!("cargo:rerun-if-changed={}", abs.display());
        let _ = writeln!(
            out,
            "    ({stem:?}, include_str!({:?})),",
            abs.display().to_string()
        );
    }
    out.push_str("];\n");
    let dest =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets it")).join("lang_embedded.rs");
    std::fs::write(dest, out).expect("write lang_embedded.rs");
}
