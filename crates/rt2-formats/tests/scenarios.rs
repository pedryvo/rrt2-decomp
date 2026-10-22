//! Scenario rules of every shipped map and save against the condition language of
//! `docs/spec/scenarios.md`: the compiled tokens stored in the file must equal the output of the
//! compiler ported from `0x454060`, every token id must be in the 118-entry table, and every
//! condition must reduce (shape-wise) without an evaluation error. Skipped unless `RT2_DIR` points
//! at an install.

#[path = "scenarios/lang.rs"]
mod lang;

use std::collections::BTreeMap;
use std::path::PathBuf;

use lang::{Ctx, Env, N_ENTRIES, compile, loop_flags, reduce};
use rt2_formats::map::Save;

fn game_dir() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR").map(PathBuf::from)
}

/// Every `.MAP`/`.MP2` under `maps/` and `.GAM` under `games/`.
fn files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["maps", "games"] {
        let Ok(rd) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let ext = p
                .extension()
                .and_then(|x| x.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if matches!(ext.as_str(), "map" | "mp2" | "gam") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Environment that answers every function with a fixed small value (shape check only).
struct Dummy;

impl Env for Dummy {
    fn value(&mut self, _: usize, _: i32, _: i32, _: i32, _: i32, _: bool) -> i32 {
        7
    }
    fn companies(&self) -> Vec<i32> {
        vec![1, 2]
    }
    fn persons(&self) -> Vec<i32> {
        vec![1, 2]
    }
    fn territories(&self) -> Vec<i32> {
        vec![1, 2]
    }
}

fn u32_at(fixed: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(fixed[off..off + 4].try_into().unwrap())
}

/// Offset of a field of the 0x362-byte layout inside the stored fixed part (the 0x2C8 layout of
/// `.MAP` files has the same offsets below 0x64 and 0x9A less at and above 0x320).
fn fixed_off(len: usize, off: usize) -> usize {
    if len == 0x362 || off < 0x64 {
        off
    } else {
        off - 0x9a
    }
}

#[test]
fn every_shipped_rule_compiles_to_its_stored_tokens() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut n_files = 0;
    let mut n_rules = 0;
    let mut used: BTreeMap<usize, usize> = BTreeMap::new();
    let mut triggers: BTreeMap<u32, usize> = BTreeMap::new();
    let mut unknown_ids = Vec::new();
    let mut mismatches = Vec::new();
    let mut shape_errors = Vec::new();
    let mut effects_seen: BTreeMap<usize, usize> = BTreeMap::new();
    let mut max_effect = 0usize;
    for path in files(&dir) {
        let raw = std::fs::read(&path).unwrap();
        let save = Save::parse(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        n_files += 1;
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        for (ri, r) in save.rules.iter().enumerate() {
            n_rules += 1;
            let at = format!("{name} rule {ri}");
            let len = r.fixed.len();
            assert!(len == 0x2c8 || len == 0x362, "{at}: fixed part {len:#x}");
            *triggers.entry(u32_at(&r.fixed, 0x34)).or_default() += 1;
            // tokens: ids in range, count matches +4
            assert_eq!(
                u32_at(&r.fixed, 4) as usize,
                r.tokens_a.len(),
                "{at}: token count"
            );
            for &t in &r.tokens_a {
                if t < 0 || t as usize >= N_ENTRIES {
                    unknown_ids.push((at.clone(), t));
                } else {
                    *used.entry(t as usize).or_default() += 1;
                }
            }
            if std::env::var_os("RT2_DUMP_RULES").is_some() {
                // PlayerId usage
                let stride = if len == 0x362 { 0x15e } else { 0x111 };
                let eff: Vec<(usize, usize, i8)> = (0..2 * stride)
                    .filter(|&j| r.fixed[0x64 + j] != 0)
                    .map(|j| (j / stride, j % stride, r.fixed[0x64 + j] as i8))
                    .collect();
                let g = |o: usize| u32_at(&r.fixed, fixed_off(len, o));
                eprintln!(
                    "FIELDS len={len:#x} 10={} 2c={} 30={} 38={} 3c={} 40={} 44={} 4c={} 60={} 320={} 328={} 32a={} 32e={} b34f={} b350={} b351={} 352={} 356={} 35a={} 35e={}",
                    g(0x10),
                    g(0x2c),
                    g(0x30),
                    g(0x38),
                    g(0x3c),
                    g(0x40),
                    g(0x44),
                    g(0x4c),
                    g(0x60),
                    g(0x320),
                    g(0x328) & 0xffff,
                    g(0x32a),
                    g(0x32e),
                    r.fixed[fixed_off(len, 0x34f)],
                    r.fixed[fixed_off(len, 0x350)],
                    r.fixed[fixed_off(len, 0x351)],
                    g(0x352),
                    g(0x356),
                    g(0x35a),
                    g(0x35e)
                );
                eprintln!(
                    "RULE {at} trig={} flags24/28/324={}/{}/{} one-time={} cond={} eff={eff:?} msg={:?} title={:?}",
                    u32_at(&r.fixed, 0x34),
                    u32_at(&r.fixed, 0x24),
                    u32_at(&r.fixed, 0x28),
                    u32_at(&r.fixed, fixed_off(len, 0x324)),
                    u32_at(&r.fixed, 0x48),
                    r.condition,
                    r.message.chars().take(60).collect::<String>(),
                    r.title
                );
            }
            match compile(&r.condition) {
                Ok((ids, vals)) => {
                    if ids != r.tokens_a || vals != r.tokens_b {
                        mismatches.push(at.clone());
                    }
                }
                Err(e) => mismatches.push(format!("{at}: {e:?}")),
            }
            if r.tokens_a
                .iter()
                .all(|&t| t >= 0 && (t as usize) < N_ENTRIES)
            {
                let ctx = Ctx {
                    company: 1,
                    person: 1,
                    territory: 1,
                    crt_rand: false,
                };
                if let Err(code) = reduce(&mut Dummy, &r.tokens_a, &r.tokens_b, &ctx) {
                    shape_errors.push(format!("{at}: error {code:#x}: {}", r.condition));
                }
                // loop flags the original derives (0x457a30) vs the stored fields
                let (c, p, t) = loop_flags(&r.tokens_a);
                let o24 = u32_at(&r.fixed, 0x24) != 0;
                let o28 = u32_at(&r.fixed, 0x28) != 0;
                let o324 = u32_at(&r.fixed, fixed_off(len, 0x324)) != 0;
                assert!(!c || o24, "{at}: company function without +0x24");
                assert!(!p || o28, "{at}: player function without +0x28");
                assert!(!t || o324, "{at}: territory function without +0x324");
            }
            // effect bytes: 2 choices
            let stride = if len == 0x362 { 0x15e } else { 0x111 };
            for k in 0..2 {
                for j in 0..stride {
                    if r.fixed[0x64 + k * stride + j] != 0 {
                        *effects_seen.entry(j).or_default() += 1;
                        max_effect = max_effect.max(j);
                    }
                }
            }
        }
    }
    eprintln!("{n_files} files, {n_rules} rules");
    eprintln!("trigger kinds: {triggers:?}");
    eprintln!("token ids used: {}", used.len());
    let unused: Vec<_> = (0..N_ENTRIES).filter(|i| !used.contains_key(i)).collect();
    eprintln!("token ids unused: {unused:?}");
    eprintln!("effects: {effects_seen:?}");
    for e in &shape_errors {
        eprintln!("malformed: {e}");
    }
    eprintln!("effect ids set: {} (max {max_effect})", effects_seen.len());
    assert!(n_files > 50, "found only {n_files} files");
    assert!(unknown_ids.is_empty(), "unknown token ids: {unknown_ids:?}");
    assert!(mismatches.is_empty(), "compiler mismatches: {mismatches:?}");
    // Six user-authored rules in the shipped maps are malformed (missing `and`, unbalanced
    // parentheses, ...); the original rejects them at evaluation time with the same error codes
    // (checked against the executable in the oracle test).
    assert_eq!(shape_errors.len(), 6, "shape errors: {shape_errors:?}");
    assert!(max_effect < 0x15e);
}

#[test]
fn word_splitting_and_numbers() {
    use lang::{parse_number, split_words};
    assert_eq!(
        split_words("GameYear>=1900 and (A+B)*3"),
        [
            "GameYear", ">=", "1900", "and", "(", "A", "+", "B", ")", "*", "3"
        ]
    );
    assert_eq!(
        split_words("CompanyCash(IsHighestInClass)"),
        ["CompanyCash", "(", "IsHighestInClass", ")"]
    );
    assert_eq!(split_words("a<=-5"), ["a", "<=", "-", "5"]);
    assert_eq!(parse_number("0x1F"), Some(31));
    assert_eq!(parse_number("123"), Some(123));
    assert_eq!(parse_number("12a"), None);
    assert_eq!(parse_number(""), Some(0));
}
