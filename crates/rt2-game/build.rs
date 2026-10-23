//! Embeds `assets/lang/*.toml` (docs/i18n.md) into the binary: writes a table of
//! `(file stem, include_str!(file))` so a translation added to the folder is picked up by the
//! next build, and a missing folder simply gives an empty table.

use std::fmt::Write as _;
use std::path::PathBuf;

/// Short git commit of the checkout (`RRT2_GIT_COMMIT`, "-dirty" when modified), or "unknown"
/// outside a git checkout (a source tarball). `RRT2_GIT_COMMIT` in the environment wins (CI).
fn git_commit() -> String {
    if let Ok(v) = std::env::var("RRT2_GIT_COMMIT")
        && !v.is_empty()
    {
        return v;
    }
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    match git(&["rev-parse", "--short=9", "HEAD"]) {
        Some(h) if !h.is_empty() => {
            let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
                .is_some_and(|s| !s.is_empty());
            if dirty { format!("{h}-dirty") } else { h }
        }
        _ => "unknown".to_owned(),
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=RRT2_GIT_COMMIT");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
    println!("cargo:rustc-env=RRT2_GIT_COMMIT={}", git_commit());
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
