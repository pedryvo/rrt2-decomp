//! Field-by-field comparison of two saves (`.GAM`/`.MP2`, versions 0x233C and 0x2348): the
//! comparison step of the level-2 save oracle (`docs/dynamic-analysis.md` section 5).
//!
//! Exit code 0 when the saves are equal after masking the volatile fields, 1 when they differ.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use rt2_formats::map::Save;
use rt2_formats::savefields::{Difference, FieldValue, diff};

#[derive(Parser)]
#[command(about = "Compare two saves field by field (exit 1 when they differ)")]
struct Args {
    a: PathBuf,
    b: PathBuf,
    /// Do not hide the volatile fields (checksum, preview, heap pointers)
    #[arg(long)]
    no_mask: bool,
    /// Only report fields whose path starts with this prefix, e.g. `state.` or `trains[3]`
    #[arg(long)]
    only: Option<String>,
    /// Machine-readable output
    #[arg(long)]
    json: bool,
}

/// Group key: the entity a path belongs to (`trains[3]`, `state`, `tiles`).
fn entity(path: &str) -> &str {
    match path.find('.') {
        Some(i) => &path[..i],
        None => path.split('[').next().unwrap_or(path),
    }
}

fn load(path: &PathBuf) -> anyhow::Result<Save> {
    let raw = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Save::parse(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn json_value(v: &Option<FieldValue>) -> String {
    match v {
        None => "null".into(),
        Some(FieldValue::Int(i)) => i.to_string(),
        Some(FieldValue::Str(s)) => json_str(s),
        Some(b @ FieldValue::Bytes(_)) => json_str(&b.to_string()),
    }
}

fn shown(v: &Option<FieldValue>) -> String {
    v.as_ref().map_or("<absent>".into(), |v| v.to_string())
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Args::parse();
    let (a, b) = (load(&args.a)?, load(&args.b)?);
    let all = diff(&a, &b);
    let masked = all.iter().filter(|d| d.masked.is_some()).count();
    let shown_diffs: Vec<&Difference> = all
        .iter()
        .filter(|d| args.no_mask || d.masked.is_none())
        .filter(|d| args.only.as_ref().is_none_or(|p| d.path.starts_with(p)))
        .collect();

    // Group by entity, in order of first appearance.
    let mut groups: Vec<(&str, Vec<&Difference>)> = Vec::new();
    for d in &shown_diffs {
        let key = entity(&d.path);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(d),
            None => groups.push((key, vec![d])),
        }
    }

    let mut out = String::new();
    let versions = (a.layout.version, b.layout.version);
    if args.json {
        let _ = writeln!(out, "{{");
        let _ = writeln!(
            out,
            "  \"version_a\": {}, \"version_b\": {},",
            versions.0, versions.1
        );
        let _ = writeln!(
            out,
            "  \"differences\": {}, \"entities\": {}, \"masked\": {},",
            shown_diffs.len(),
            groups.len(),
            masked
        );
        let _ = writeln!(out, "  \"fields\": [");
        for (i, d) in shown_diffs.iter().enumerate() {
            let mask = d.masked.map_or("null".into(), json_str);
            let _ = writeln!(
                out,
                "    {{\"path\": {}, \"a\": {}, \"b\": {}, \"masked\": {}}}{}",
                json_str(&d.path),
                json_value(&d.a),
                json_value(&d.b),
                mask,
                if i + 1 < shown_diffs.len() { "," } else { "" }
            );
        }
        let _ = writeln!(out, "  ]\n}}");
    } else {
        if versions.0 != versions.1 {
            let _ = writeln!(
                out,
                "note: versions differ ({:#x} vs {:#x}); only named fields are compared",
                versions.0, versions.1
            );
        }
        for (key, diffs) in &groups {
            let _ = writeln!(out, "{key} ({} fields)", diffs.len());
            for d in diffs {
                let tag = if d.masked.is_some() { "  [masked]" } else { "" };
                let _ = writeln!(out, "  {}: {} -> {}{tag}", d.path, shown(&d.a), shown(&d.b));
            }
        }
        let mut sections: Vec<(&str, usize)> = Vec::new();
        for d in &shown_diffs {
            let sec = d.path.split(['.', '[']).next().unwrap_or(&d.path);
            match sections.iter_mut().find(|(k, _)| *k == sec) {
                Some((_, c)) => *c += 1,
                None => sections.push((sec, 1)),
            }
        }
        let list: Vec<String> = sections.iter().map(|(k, c)| format!("{k}={c}")).collect();
        if !list.is_empty() {
            let _ = writeln!(out, "by section: {}", list.join(" "));
        }
        let _ = writeln!(
            out,
            "{} differing fields in {} entities ({} volatile fields {})",
            shown_diffs.len(),
            groups.len(),
            masked,
            if args.no_mask { "shown" } else { "masked" }
        );
    }
    let _ = std::io::stdout().write_all(out.as_bytes());
    Ok(if shown_diffs.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
