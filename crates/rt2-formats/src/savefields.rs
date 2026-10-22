//! Ordered, field-by-field view of a [`Save`] and the comparison of two saves.
//!
//! This is the comparison step of the level-2 "save oracle" (`docs/PLAN.md` section 11,
//! `docs/dynamic-analysis.md` section 5): [`Save::fields`] flattens a parsed file into
//! `(path, value)` pairs in file order and covers **every byte of the file**: decoded values get
//! a name (`state.date`, `places[12].name`), everything else is chunked into raw byte ranges of at
//! most 4 bytes aligned to 4 (`trains[3].raw[0x40..0x44]`). [`diff`] compares two saves, honouring
//! the [`MASKS`] of fields that differ between runs of the original for reasons unrelated to the
//! simulation.
//!
//! Conventions
//! - A path is `entity[index].field`, with the pool slot as index. Offsets in `raw[a..b]` are
//!   relative to the record, block or array the path names (not to the file).
//! - Named fields have the same path in every layout, so 0x233C and 0x2348 saves of the same world
//!   can be compared by them. Raw fields and the few named fields that are layout properties
//!   (record sizes, version word) are flagged [`Field::raw`] and are compared only when both saves
//!   have the same version.
//! - A fixed-size text field is decoded up to its first NUL; the non-zero bytes after it are kept
//!   as raw chunks (all-zero tails are implied). The rule texts (length-prefixed blobs) are
//!   decoded up to the first NUL only.

use crate::map::{CHECKSUM_OFFSET, Pool, Save, TILE_SIZE, TITLE_END, TITLE_OFFSET, c_string};

/// Value of one field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    Int(i64),
    Str(String),
    Bytes(Vec<u8>),
}

impl std::fmt::Display for FieldValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldValue::Int(v) => write!(f, "{v}"),
            FieldValue::Str(s) => write!(f, "{s:?}"),
            FieldValue::Bytes(b) => {
                for (i, x) in b.iter().take(24).enumerate() {
                    if i > 0 && i % 4 == 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{x:02x}")?;
                }
                if b.len() > 24 {
                    write!(f, "..({} bytes)", b.len())?;
                }
                Ok(())
            }
        }
    }
}

/// One field of a save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub path: String,
    pub value: FieldValue,
    /// True for undecoded byte ranges and for layout properties; compared only between saves of
    /// the same version.
    pub raw: bool,
}

#[derive(Clone, Copy)]
enum Kind {
    U8,
    U16,
    U32,
    I32,
    /// NUL-terminated text in a fixed-size field.
    Str,
    /// year u16, month u8, day u8 -> `YYYY-MM-DD`.
    Date,
    /// hour u8, minute u8 -> `HH:MM`.
    Time,
}

/// A decoded sub-field of a record or block. `layout` marks properties of the file version
/// rather than of the world (compared only between saves of the same version).
struct Named {
    off: usize,
    len: usize,
    kind: Kind,
    name: &'static str,
    layout: bool,
}

const fn n(off: usize, len: usize, kind: Kind, name: &'static str) -> Named {
    Named {
        off,
        len,
        kind,
        name,
        layout: false,
    }
}

// Field tables. Offsets are those of `docs/formats/map-gam.md` (sections 1, 5.1, 5.3, 5.4) and
// of the `Save` accessors in `map.rs`; they are the same in both layouts.

const HEADER: &[Named] = &[
    Named {
        off: 0,
        len: 4,
        kind: Kind::U32,
        name: "version",
        layout: true,
    },
    n(4, 0x131 - 4, Kind::Str, "name"),
    n(TITLE_OFFSET, TITLE_END - TITLE_OFFSET, Kind::Str, "title"),
    n(0x231, 2, Kind::U16, "width"),
    n(0x233, 2, Kind::U16, "height"),
    n(CHECKSUM_OFFSET, 4, Kind::U32, "checksum"),
];
const STATE: &[Named] = &[
    n(0x00, 4, Kind::U32, "game_running"),
    n(0x0C, 4, Kind::I32, "game_time_minutes"),
    n(0x10, 4, Kind::Date, "date"),
    n(0x14, 2, Kind::Time, "time"),
    n(0x26, 1, Kind::U8, "game_speed"),
];
const TRAILER: &[Named] = &[n(0x2E, 4, Kind::U32, "game_seed")];
const STATION: &[Named] = &[n(26, 32, Kind::Str, "name")];
const PERSON: &[Named] = &[
    n(8, 0x27 - 8, Kind::Str, "name"),
    n(0x27, 0x120 - 0x27, Kind::Str, "bio"),
];
const COMPANY: &[Named] = &[
    n(0, 4, Kind::U32, "id"),
    n(4, 32, Kind::Str, "name"),
    n(0x148, 4, Kind::U32, "year_148"),
];
const TRAIN: &[Named] = &[
    n(12, 32, Kind::Str, "name"),
    n(0x3B, 1, Kind::U8, "cars"),
    n(0x59, 4, Kind::I32, "route_steps"),
    n(0x65, 4, Kind::I32, "route_flag"),
];
const PLACE: &[Named] = &[
    n(0, 2, Kind::U16, "id"),
    n(4, 2, Kind::U16, "x"),
    n(6, 2, Kind::U16, "y"),
    n(8, 2, Kind::U16, "bbox_x0"),
    n(10, 2, Kind::U16, "bbox_y0"),
    n(12, 2, Kind::U16, "bbox_x1"),
    n(14, 2, Kind::U16, "bbox_y1"),
    n(22, 32, Kind::Str, "name"),
];
const RULE_FIXED: &[Named] = &[n(4, 4, Kind::I32, "tokens")];

fn decode(kind: Kind, b: &[u8]) -> FieldValue {
    let u = |b: &[u8]| b.iter().rev().fold(0u64, |a, &x| (a << 8) | u64::from(x));
    match kind {
        Kind::U8 | Kind::U16 | Kind::U32 => FieldValue::Int(u(b) as i64),
        Kind::I32 => FieldValue::Int(i64::from(i32::from_le_bytes(b[..4].try_into().unwrap()))),
        Kind::Str => {
            let end = b.iter().position(|&x| x == 0).unwrap_or(b.len());
            FieldValue::Str(c_string(&b[..end]))
        }
        Kind::Date => FieldValue::Str(format!(
            "{:04}-{:02}-{:02}",
            u16::from_le_bytes([b[0], b[1]]),
            b[2],
            b[3]
        )),
        Kind::Time => FieldValue::Str(format!("{:02}:{:02}", b[0], b[1])),
    }
}

/// Emits the named fields of `data` and raw chunks (<= 4 bytes, aligned to 4) for the rest.
fn walk(prefix: &str, data: &[u8], named: &[Named], out: &mut Vec<Field>) {
    let raw_chunks = |from: usize, to: usize, skip_zero: bool, out: &mut Vec<Field>| {
        let mut at = from;
        while at < to {
            let end = ((at & !3) + 4).min(to);
            if skip_zero && data[at..end].iter().all(|&x| x == 0) {
                at = end;
                continue;
            }
            out.push(Field {
                path: format!("{prefix}.raw[{at:#x}..{end:#x}]"),
                value: FieldValue::Bytes(data[at..end].to_vec()),
                raw: true,
            });
            at = end;
        }
    };
    let mut pos = 0;
    for f in named {
        if f.off < pos || f.off + f.len > data.len() {
            continue; // table entries are in offset order and inside the record by construction
        }
        raw_chunks(pos, f.off, false, out);
        out.push(Field {
            path: format!("{prefix}.{}", f.name),
            value: decode(f.kind, &data[f.off..f.off + f.len]),
            raw: f.layout,
        });
        pos = f.off + f.len;
        if matches!(f.kind, Kind::Str) {
            // Bytes after the NUL are other data sharing the field (e.g. in place records); keep
            // the non-zero ones as raw chunks.
            let text = &data[f.off..pos];
            let nul = text.iter().position(|&x| x == 0).unwrap_or(text.len());
            raw_chunks((f.off + nul + 1).min(pos), pos, true, out);
        }
    }
    raw_chunks(pos, data.len(), false, out);
}

fn int(path: String, v: i64, raw: bool) -> Field {
    Field {
        path,
        value: FieldValue::Int(v),
        raw,
    }
}

fn pool_fields(name: &str, pool: &Pool, named: &[Named], out: &mut Vec<Field>) {
    let h = |field: &str, v: i64, raw: bool| int(format!("{name}.{field}"), v, raw);
    out.push(h("slots", pool.slots as i64, false));
    out.push(h("record_size", pool.record_size as i64, true));
    out.push(h("linked", i64::from(pool.linked), false));
    out.push(h("field_00", i64::from(pool.field_00), false));
    out.push(h("field_04", i64::from(pool.field_04), false));
    out.push(h("field_0c", i64::from(pool.field_0c), false));
    out.push(h("count", pool.count as i64, false));
    out.push(h("pool_type", i64::from(pool.pool_type), false));
    out.push(h("head", i64::from(pool.head), false));
    out.push(h("tail", i64::from(pool.tail), false));
    out.push(Field {
        path: format!("{name}.bitmap"),
        value: FieldValue::Bytes(pool.bitmap.clone()),
        raw: false,
    });
    for r in &pool.records {
        walk(&format!("{name}[{}]", r.slot), &r.data, named, out);
    }
    for (label, links) in [("next", &pool.next), ("prev", &pool.prev)] {
        for (i, &v) in links.iter().enumerate() {
            out.push(int(format!("{name}.{label}[{i}]"), i64::from(v), false));
        }
    }
}

impl Save {
    /// Every byte of the file as ordered `(path, value)` fields: header, state, trailer, tiles,
    /// elevation, the 15 pools, the train extras and the rules (the order of the file). See the
    /// module documentation for the path conventions.
    pub fn fields(&self) -> Vec<Field> {
        let mut out = Vec::new();
        walk("header", &self.header, HEADER, &mut out);
        walk("state", &self.state, STATE, &mut out);
        walk("trailer", &self.trailer, TRAILER, &mut out);
        let m = &self.map;
        for (i, t) in m.tiles.iter().enumerate() {
            let (x, y) = (i % m.width, i / m.width);
            let b: [u8; TILE_SIZE] = [
                t.var,
                t.ground,
                t.flags2,
                t.trees,
                t.struct_id,
                t.reserved5,
                t.flags6,
                t.track_id,
                t.reserved8,
                t.region,
            ];
            // Bytes 0..2 (`var` + `ground`) are remapped by the loader for versions < 0x233D
            // (`docs/formats/map-gam.md` section 7.1), so their encoding depends on the version.
            out.push(int(
                format!("tiles[{x},{y}].terrain"),
                i64::from(u16::from_le_bytes([b[0], b[1]])),
                true,
            ));
            out.push(Field {
                path: format!("tiles[{x},{y}].rest"),
                value: FieldValue::Bytes(b[2..].to_vec()),
                raw: false,
            });
        }
        for (i, &e) in m.elevation.iter().enumerate() {
            let (x, y) = (i % (m.width + 1), i / (m.width + 1));
            out.push(int(format!("elevation[{x},{y}]"), i64::from(e), false));
        }
        let pools: [(&str, &Pool, &[Named]); 15] = [
            ("structures", &self.structures, &[]),
            ("stations", &self.stations, STATION),
            ("persons", &self.persons, PERSON),
            ("companies", &self.companies, COMPANY),
            ("pool_70", &self.pool_70, &[]),
            ("objects", &self.objects, &[]),
            ("pool_78", &self.pool_78, &[]),
            ("pool_58", &self.pool_58, &[]),
            ("pool_7c", &self.pool_7c, &[]),
            ("pool_80", &self.pool_80, &[]),
            ("track_pieces", &self.track_pieces, &[]),
            ("trains", &self.trains, TRAIN),
            ("places", &self.places, PLACE),
            ("territories", &self.territories, &[]),
            ("pool_88", &self.pool_88, &[]),
        ];
        for (name, pool, named) in pools {
            pool_fields(name, pool, named, &mut out);
        }
        for e in &self.train_extras {
            for (label, data) in [("cars", &e.cars_raw), ("route", &e.route_raw)] {
                // Raw chunks only: the record layout of both arrays depends on the version.
                walk(&format!("trains[{}].{label}", e.slot), data, &[], &mut out);
            }
        }
        out.push(int("rules.count".into(), self.rules.len() as i64, false));
        for (i, r) in self.rules.iter().enumerate() {
            let p = format!("rules[{i}]");
            walk(&format!("{p}.fixed"), &r.fixed, RULE_FIXED, &mut out);
            let text = |name: &str, v: &str| Field {
                path: format!("{p}.{name}"),
                value: FieldValue::Str(v.to_string()),
                raw: false,
            };
            out.push(text("condition", &r.condition));
            for (label, toks) in [("tokens_a", &r.tokens_a), ("tokens_b", &r.tokens_b)] {
                for (j, &v) in toks.iter().enumerate() {
                    out.push(int(format!("{p}.{label}[{j}]"), i64::from(v), false));
                }
            }
            out.push(text("message", &r.message));
            out.push(text("text2", &r.text2));
            out.push(text("text3", &r.text3));
            out.push(text("title", &r.title));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Volatile fields
// ---------------------------------------------------------------------------------------------

/// A field (or byte range of a record) whose value changes between two runs of the original that
/// did the same thing, so a comparison must ignore it unless asked not to.
pub struct Mask {
    /// Save versions the mask applies to (offsets are layout specific).
    pub versions: &'static [u32],
    /// Path of the field, or of the record for a raw range; `*` matches a run of digits.
    pub path: &'static str,
    /// Byte range inside the record named by `path` (matched against `raw[a..b]` chunks), or
    /// `None` when `path` names a whole field.
    pub range: Option<(usize, usize)>,
    /// Why the bytes are volatile.
    pub why: &'static str,
}

/// All known volatile fields, evidence in `docs/dynamic-analysis.md` section 5.4 and the
/// subsection "Que campos mudam" (compared saves written by `advance_save.py` in several sandboxes).
pub const MASKS: &[Mask] = &[
    Mask {
        versions: &[0x233C, 0x2348],
        path: "header.checksum",
        range: None,
        why: "buffer checksum the saver patches in at +0x86F7 (0x477540); it covers all the volatile bytes below",
    },
    Mask {
        versions: &[0x233C, 0x2348],
        path: "header",
        range: Some((0x387, 0x7474)),
        why: "170x170 preview image drawn from the screen at save time (0x477040); depends on the window state",
    },
    // The first dwords of a rule's fixed part are process pointers written raw (values 0x02xxxxxx
    // and 0x01xxxxxx in the samples): +0x00, +0x08, +0x0C and +0x50..+0x60.
    Mask {
        versions: &[0x233C, 0x2348],
        path: "rules[*].fixed",
        range: Some((0x00, 0x04)),
        why: "heap pointer stored raw in the rule record; differs per process and sandbox path",
    },
    Mask {
        versions: &[0x233C, 0x2348],
        path: "rules[*].fixed",
        range: Some((0x08, 0x10)),
        why: "heap pointers stored raw in the rule record; differ per process and sandbox path",
    },
    Mask {
        versions: &[0x233C, 0x2348],
        path: "rules[*].fixed",
        range: Some((0x50, 0x60)),
        why: "heap pointers stored raw in the rule record; differ per process and sandbox path",
    },
    // Train record: a pointer at +0x37..+0x3B (u32 0x02xxxxxx in one sandbox, 0x03xxxxxx in
    // another) and five at +0x45..+0x59 (the heap blocks of the car/route arrays), stored raw.
    Mask {
        versions: &[0x233C, 0x2348],
        path: "trains[*]",
        range: Some((0x37, 0x3B)),
        why: "heap pointer stored raw in the train record; differs per process and sandbox path",
    },
    Mask {
        versions: &[0x233C, 0x2348],
        path: "trains[*]",
        range: Some((0x45, 0x59)),
        why: "five heap pointers stored raw in the train record; differ per process and sandbox path",
    },
    Mask {
        versions: &[0x2348],
        path: "persons[*]",
        range: Some((0x54C, 0x554)),
        why: "new 0x2348 field of the person record holding a heap pointer and a small number; differs per sandbox",
    },
];

fn glob(pat: &str, text: &str) -> bool {
    let (p, t) = (pat.as_bytes(), text.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < p.len() {
        if p[i] == b'*' {
            let start = j;
            while j < t.len() && t[j].is_ascii_digit() {
                j += 1;
            }
            if j == start {
                return false;
            }
            i += 1;
        } else if j < t.len() && p[i] == t[j] {
            i += 1;
            j += 1;
        } else {
            return false;
        }
    }
    j == t.len()
}

/// `base.raw[0xa..0xb]` -> (`base`, a, b).
fn split_raw(path: &str) -> Option<(&str, usize, usize)> {
    let (base, rest) = path.rsplit_once(".raw[")?;
    let rest = rest.strip_suffix(']')?;
    let (a, b) = rest.split_once("..")?;
    let hex = |x: &str| usize::from_str_radix(x.strip_prefix("0x")?, 16).ok();
    Some((base, hex(a)?, hex(b)?))
}

/// The mask that covers `path` in a save of `version`, if any.
pub fn mask_for(version: u32, path: &str) -> Option<&'static Mask> {
    MASKS.iter().find(|m| {
        m.versions.contains(&version)
            && match m.range {
                None => glob(m.path, path),
                Some((lo, hi)) => split_raw(path)
                    .is_some_and(|(base, a, b)| glob(m.path, base) && a < hi && lo < b),
            }
    })
}

// ---------------------------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------------------------

/// One differing field. `a`/`b` are `None` when the field does not exist in that save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub path: String,
    pub a: Option<FieldValue>,
    pub b: Option<FieldValue>,
    /// Why the field is volatile, when a [`MASKS`] entry covers it.
    pub masked: Option<&'static str>,
}

/// Compares two saves field by field, in the order of `a` and then the fields only `b` has.
/// Raw fields are compared only when both saves have the same version. Differences covered by a
/// mask are returned with `masked` set so the caller can count or hide them.
pub fn diff(a: &Save, b: &Save) -> Vec<Difference> {
    use std::collections::HashMap;
    let same_version = a.layout.version == b.layout.version;
    let (fa, fb) = (a.fields(), b.fields());
    let index: HashMap<&str, usize> = fb
        .iter()
        .enumerate()
        .map(|(i, f)| (f.path.as_str(), i))
        .collect();
    let mut seen = vec![false; fb.len()];
    let mut out = Vec::new();
    let mut push = |path: &str, va: Option<&Field>, vb: Option<&Field>| {
        let masked = mask_for(a.layout.version, path)
            .or_else(|| mask_for(b.layout.version, path))
            .map(|m| m.why);
        out.push(Difference {
            path: path.to_string(),
            a: va.map(|f| f.value.clone()),
            b: vb.map(|f| f.value.clone()),
            masked,
        });
    };
    for f in &fa {
        match index.get(f.path.as_str()) {
            Some(&j) => {
                seen[j] = true;
                if (same_version || !(f.raw || fb[j].raw)) && f.value != fb[j].value {
                    push(&f.path, Some(f), Some(&fb[j]));
                }
            }
            None => {
                if same_version || !f.raw {
                    push(&f.path, Some(f), None);
                }
            }
        }
    }
    for (j, f) in fb.iter().enumerate() {
        if !seen[j] && (same_version || !f.raw) {
            push(&f.path, None, Some(f));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Layout, SIZE_OFFSET, STATE_MINUTES};

    /// A minimal save: `w*h` tiles, empty pools, no rules; a given date and checksum.
    fn synthetic_save(layout: &Layout, day: u8) -> Vec<u8> {
        let (w, h) = (3usize, 2usize);
        let mut raw = vec![0u8; layout.tiles_offset()];
        raw[0..4].copy_from_slice(&layout.version.to_le_bytes());
        raw[4..8].copy_from_slice(b"Test");
        raw[SIZE_OFFSET..SIZE_OFFSET + 2].copy_from_slice(&(w as u16).to_le_bytes());
        raw[SIZE_OFFSET + 2..SIZE_OFFSET + 4].copy_from_slice(&(h as u16).to_le_bytes());
        let state = layout.state_offset();
        raw[state + STATE_MINUTES..state + STATE_MINUTES + 4]
            .copy_from_slice(&(904_826_583i32 + 1440 * i32::from(day)).to_le_bytes());
        raw[state + 0x10..state + 0x14].copy_from_slice(&[0x5E, 0x07, 2, 4 + day]);
        raw[state + 0x14..state + 0x16].copy_from_slice(&[19, 3]);
        let seed = layout.game_seed_offset();
        raw[seed..seed + 4].copy_from_slice(&183_274u32.to_le_bytes());
        for i in 0..w * h {
            raw.extend_from_slice(&[0x40, 0x2d, 0, 3, 0, 0, 0, 0, 0, 1 + i as u8]);
        }
        raw.resize(raw.len() + 2 * (w + 1) * (h + 1), 0);
        for _ in 0..15 {
            for v in [1i32, 4, 0, 1, 0, 1, 0, 0] {
                raw.extend_from_slice(&v.to_le_bytes());
            }
            raw.extend_from_slice(&[0, 0, 0, 0, 1]);
        }
        raw.extend_from_slice(&0i32.to_le_bytes());
        raw
    }

    fn fields_of(raw: &[u8]) -> Vec<Field> {
        Save::parse(raw).unwrap().fields()
    }

    fn real(diffs: Vec<Difference>) -> Vec<String> {
        diffs
            .into_iter()
            .filter(|d| d.masked.is_none())
            .map(|d| d.path)
            .collect()
    }

    #[test]
    fn walk_covers_every_byte_exactly_once() {
        let data: Vec<u8> = (0..29).collect();
        let table = [
            n(3, 2, Kind::U16, "a"),
            n(10, 4, Kind::U32, "b"),
            n(14, 5, Kind::Str, "c"),
        ];
        let mut out = Vec::new();
        walk("r", &data, &table, &mut out);
        let mut covered = vec![0u8; data.len()];
        for f in &out {
            let (lo, hi) = match split_raw(&f.path) {
                Some((_, a, b)) => (a, b),
                None => {
                    let t = table
                        .iter()
                        .find(|t| f.path == format!("r.{}", t.name))
                        .unwrap();
                    (t.off, t.off + t.len)
                }
            };
            assert!(hi - lo <= 4 || f.path.ends_with(".c"));
            for c in &mut covered[lo..hi] {
                *c += 1;
            }
        }
        assert!(covered.iter().all(|&c| c == 1), "{covered:?}");
        // Raw chunks are aligned to 4 bytes.
        assert!(out.iter().any(|f| f.path == "r.raw[0x0..0x3]"));
        assert!(out.iter().any(|f| f.path == "r.raw[0x5..0x8]"));
    }

    #[test]
    fn text_fields_keep_non_zero_bytes_after_the_nul() {
        let mut data = vec![0u8; 16];
        data[..2].copy_from_slice(b"Ab");
        data[9] = 0x55;
        let mut out = Vec::new();
        walk("r", &data, &[n(0, 12, Kind::Str, "name")], &mut out);
        let paths: Vec<&str> = out.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["r.name", "r.raw[0x8..0xc]", "r.raw[0xc..0x10]"]);
        assert_eq!(out[0].value, FieldValue::Str("Ab".into()));
        assert_eq!(out[1].value, FieldValue::Bytes(vec![0, 0x55, 0, 0]));
    }

    #[test]
    fn fields_have_unique_stable_paths() {
        let fields = fields_of(&synthetic_save(&Layout::V2348, 0));
        let mut paths: Vec<&str> = fields.iter().map(|f| f.path.as_str()).collect();
        let n = paths.len();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), n);
        let get = |p: &str| fields.iter().find(|f| f.path == p).map(|f| &f.value);
        assert_eq!(
            get("state.date"),
            Some(&FieldValue::Str("1886-02-04".into()))
        );
        assert_eq!(get("state.time"), Some(&FieldValue::Str("19:03".into())));
        assert_eq!(get("trailer.game_seed"), Some(&FieldValue::Int(183_274)));
        assert_eq!(get("header.name"), Some(&FieldValue::Str("Test".into())));
        assert_eq!(get("rules.count"), Some(&FieldValue::Int(0)));
        assert!(get("tiles[2,1].rest").is_some());
        assert!(get("header.raw[0x4..0x8]").is_none()); // inside the named `name`
        assert!(get("header.raw[0x0..0x4]").is_none()); // the version word is named
    }

    #[test]
    fn equal_saves_have_no_differences() {
        for l in [Layout::V233C, Layout::V2348] {
            let raw = synthetic_save(&l, 0);
            let s = Save::parse(&raw).unwrap();
            assert!(diff(&s, &s).is_empty());
        }
    }

    #[test]
    fn a_later_day_changes_only_the_state() {
        for l in [Layout::V233C, Layout::V2348] {
            let a = Save::parse(&synthetic_save(&l, 0)).unwrap();
            let b = Save::parse(&synthetic_save(&l, 1)).unwrap();
            let d = diff(&a, &b);
            assert_eq!(real(d.clone()), ["state.game_time_minutes", "state.date"]);
            assert_eq!(d[1].a, Some(FieldValue::Str("1886-02-04".into())));
            assert_eq!(d[1].b, Some(FieldValue::Str("1886-02-05".into())));
        }
    }

    #[test]
    fn volatile_bytes_are_flagged_as_masked() {
        let l = Layout::V2348;
        let mut raw = synthetic_save(&l, 0);
        raw[CHECKSUM_OFFSET] = 0xAA; // checksum
        raw[0x400] = 0x11; // preview
        let a = Save::parse(&synthetic_save(&l, 0)).unwrap();
        let b = Save::parse(&raw).unwrap();
        let d = diff(&a, &b);
        assert_eq!(d.len(), 2);
        assert!(d.iter().all(|x| x.masked.is_some()), "{d:?}");
        assert!(real(d).is_empty());
        // Just outside the preview is not masked.
        raw[0x400] = 0;
        raw[0x7474] = 1;
        let c = Save::parse(&raw).unwrap();
        assert_eq!(
            real(diff(&a, &c)),
            ["header.raw[0x7474..0x7478]".to_string()]
        );
    }

    #[test]
    fn cross_version_compares_named_fields_only() {
        let a = Save::parse(&synthetic_save(&Layout::V233C, 0)).unwrap();
        let b = Save::parse(&synthetic_save(&Layout::V2348, 0)).unwrap();
        assert!(real(diff(&a, &b)).is_empty());
        // A raw byte that differs is ignored across versions...
        let mut raw = synthetic_save(&Layout::V2348, 0);
        raw[0x8800] = 7;
        let c = Save::parse(&raw).unwrap();
        assert!(real(diff(&a, &c)).is_empty());
        // ... and reported between saves of the same version.
        let b = Save::parse(&synthetic_save(&Layout::V2348, 0)).unwrap();
        assert_eq!(real(diff(&b, &c)), ["header.raw[0x8800..0x8804]"]);
        // A named field differs in both cases.
        let d = Save::parse(&synthetic_save(&Layout::V2348, 3)).unwrap();
        assert_eq!(real(diff(&a, &d)).len(), 2);
    }

    #[test]
    fn mask_paths_match_records_by_digits() {
        assert!(glob("rules[*].fixed", "rules[12].fixed"));
        assert!(!glob("rules[*].fixed", "rules[].fixed"));
        assert!(!glob("rules[*].fixed", "rules[1a].fixed"));
        assert_eq!(
            split_raw("persons[1].raw[0x54c..0x550]"),
            Some(("persons[1]", 0x54c, 0x550))
        );
        let hit = |v, p| mask_for(v, p).is_some();
        assert!(hit(0x2348, "persons[1].raw[0x54c..0x550]"));
        assert!(hit(0x2348, "persons[7].raw[0x550..0x554]"));
        assert!(!hit(0x2348, "persons[1].raw[0x548..0x54c]"));
        assert!(!hit(0x233C, "persons[1].raw[0x54c..0x550]"));
        assert!(hit(0x2348, "rules[3].fixed.raw[0x58..0x5c]"));
        assert!(!hit(0x2348, "rules[3].fixed.raw[0x10..0x14]"));
        assert!(!hit(0x2348, "trains[3].raw[0x3c..0x40]"));
        assert!(hit(0x2348, "trains[3].raw[0x34..0x38]"));
        assert!(hit(0x2348, "trains[3].raw[0x58..0x59]"));
        assert!(!hit(0x2348, "trains[3].route.raw[0x58..0x59]"));
    }

    /// `RT2_SAVE_2348` is a ':'-separated list of save files written by the original.
    #[test]
    fn original_saves_diff_against_themselves() {
        let Some(list) = std::env::var_os("RT2_SAVE_2348") else {
            eprintln!("RT2_SAVE_2348 not set; skipping");
            return;
        };
        let saves: Vec<Save> = std::env::split_paths(&list)
            .map(|p| Save::parse(&std::fs::read(&p).unwrap()).unwrap())
            .collect();
        for s in &saves {
            assert_eq!(s.layout.version, 0x2348);
            assert!(diff(s, s).is_empty());
        }
        let mut dates = std::collections::BTreeSet::new();
        for s in &saves {
            dates.insert((s.date.year, s.date.month, s.date.day, s.map.name.clone()));
        }
        // Two saves of the same world on different days differ in the date.
        for (i, a) in saves.iter().enumerate() {
            for b in &saves[i + 1..] {
                if a.map.name == b.map.name && a.date != b.date {
                    let paths = real(diff(a, b));
                    assert!(paths.contains(&"state.date".to_string()), "{paths:?}");
                }
            }
        }
    }
}
