//! The clone's own saves (`~/.local/share/rrt2/saves`): file naming, the metadata that sits
//! next to each save, the autosave rotation and thumbnails (docs/ui.md "Opcoes e saves").
//!
//! A save is the original's `.GAM` bytes (`World::to_save`, so the game can be opened by the
//! original too) plus two sidecars with the same stem: `<stem>.toml` (what the load dialog
//! shows: scenario, company, game date, wall-clock time of the save) and `<stem>.png` (a small
//! terrain thumbnail, the radar's colours). The sidecars are optional: a `.GAM` without them
//! (copied by hand, or one of the original's) still lists and loads, just with less to show.
//!
//! All of this is I/O and wall-clock time, so it lives in `rt2-game`, never in `rt2-sim`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const QUICK_STEM: &str = "quicksave";
pub const AUTO_PREFIX: &str = "autosave-";
/// Version of the sidecar format.
pub const META_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveKind {
    #[default]
    Manual,
    Quick,
    Auto,
}

/// What the load dialog shows about a save.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveMeta {
    pub version: u32,
    pub kind: SaveKind,
    pub scenario: String,
    pub company: String,
    pub year: u16,
    pub month: u8,
    pub day: u8,
    /// The company's cash in dollars, if there is a company.
    pub cash: Option<i64>,
    /// Unix time of the save (UTC).
    pub saved_at: u64,
}

/// A small picture: `w * h` RGB triples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumbnail {
    pub w: u32,
    pub h: u32,
    pub rgb: Vec<[u8; 3]>,
}

/// A listed save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveEntry {
    pub path: PathBuf,
    pub stem: String,
    pub meta: Option<SaveMeta>,
    /// Modification time (Unix), used when there is no sidecar.
    pub modified: u64,
}

impl SaveEntry {
    pub fn kind(&self) -> SaveKind {
        self.meta.as_ref().map_or_else(
            || {
                if self.stem == QUICK_STEM {
                    SaveKind::Quick
                } else if autosave_seq(&self.stem).is_some() {
                    SaveKind::Auto
                } else {
                    SaveKind::Manual
                }
            },
            |m| m.kind,
        )
    }

    /// When it was saved: the sidecar's time, else the file's.
    pub fn when(&self) -> u64 {
        self.meta
            .as_ref()
            .map(|m| m.saved_at)
            .filter(|&t| t > 0)
            .unwrap_or(self.modified)
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A file name the player typed, made safe: letters, digits, space and `-_.()` only, trimmed,
/// at most 60 characters; empty becomes `save`.
pub fn sanitize_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.()#&',!+@=".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    let cut: String = cleaned.chars().take(60).collect();
    let cut = cut.trim().to_owned();
    if cut.is_empty() { "save".into() } else { cut }
}

pub fn gam_path(dir: &Path, stem: &str) -> PathBuf {
    dir.join(format!("{stem}.GAM"))
}

fn meta_path(dir: &Path, stem: &str) -> PathBuf {
    dir.join(format!("{stem}.toml"))
}

fn thumb_path(dir: &Path, stem: &str) -> PathBuf {
    dir.join(format!("{stem}.png"))
}

/// Writes `bytes` through a temporary file so a crash never leaves half a save.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).with_context(|| tmp.display().to_string())?;
    std::fs::rename(&tmp, path).with_context(|| path.display().to_string())
}

/// Writes a save and its sidecars into `dir` (created if missing).
pub fn write_save(
    dir: &Path,
    stem: &str,
    bytes: &[u8],
    meta: &SaveMeta,
    thumb: Option<&Thumbnail>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    let path = gam_path(dir, stem);
    write_atomic(&path, bytes)?;
    let toml = toml::to_string_pretty(meta).context("save metadata")?;
    write_atomic(&meta_path(dir, stem), toml.as_bytes())?;
    match thumb {
        Some(t) => write_atomic(&thumb_path(dir, stem), &encode_thumbnail(t)?)?,
        None => {
            let _ = std::fs::remove_file(thumb_path(dir, stem));
        }
    }
    Ok(path)
}

fn encode_thumbnail(t: &Thumbnail) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, t.w, t.h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header()?;
        let flat: Vec<u8> = t.rgb.iter().flat_map(|p| p.iter().copied()).collect();
        w.write_image_data(&flat)?;
    }
    Ok(out)
}

/// The thumbnail next to a save, if there is a readable one.
pub fn read_thumbnail(path: &Path) -> Option<Thumbnail> {
    let png_path = path.with_extension("png");
    let file = std::fs::File::open(png_path).ok()?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgb || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let rgb = buf[..info.buffer_size()].as_chunks::<3>().0.to_vec();
    (rgb.len() == (info.width * info.height) as usize).then_some(Thumbnail {
        w: info.width,
        h: info.height,
        rgb,
    })
}

/// The `.GAM` files of `dir`, newest first (ties by name, so the order never depends on the
/// directory order). A missing folder is an empty list.
pub fn list(dir: &Path) -> Vec<SaveEntry> {
    let mut out: Vec<SaveEntry> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("gam"))
        })
        .map(|path| {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let meta = std::fs::read_to_string(meta_path(dir, &stem))
                .ok()
                .and_then(|t| toml::from_str::<SaveMeta>(&t).ok());
            let modified = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            SaveEntry {
                path,
                stem,
                meta,
                modified,
            }
        })
        .collect();
    out.sort_by(|a, b| b.when().cmp(&a.when()).then_with(|| a.stem.cmp(&b.stem)));
    out
}

/// Deletes a save and its sidecars.
pub fn delete(entry: &SaveEntry) -> Result<()> {
    std::fs::remove_file(&entry.path).with_context(|| entry.path.display().to_string())?;
    let _ = std::fs::remove_file(entry.path.with_extension("toml"));
    let _ = std::fs::remove_file(entry.path.with_extension("png"));
    Ok(())
}

/// The sequence number of an autosave stem (`autosave-0007` -> 7).
pub fn autosave_seq(stem: &str) -> Option<u32> {
    stem.strip_prefix(AUTO_PREFIX)?.parse().ok()
}

pub fn autosave_stem(seq: u32) -> String {
    format!("{AUTO_PREFIX}{seq:04}")
}

/// The sequence number the next autosave takes: one past the highest on disk.
pub fn next_autosave_seq(dir: &Path) -> u32 {
    list(dir)
        .iter()
        .filter_map(|e| autosave_seq(&e.stem))
        .max()
        .map_or(1, |m| m + 1)
}

/// Deletes the oldest autosaves so that at most `keep` remain (never fewer than one, so the
/// autosave just written survives a `keep` of 0). Returns the stems deleted, oldest first.
pub fn prune_autosaves(dir: &Path, keep: usize) -> Vec<String> {
    let mut autos: Vec<(u32, SaveEntry)> = list(dir)
        .into_iter()
        .filter_map(|e| autosave_seq(&e.stem).map(|s| (s, e)))
        .collect();
    autos.sort_by_key(|(s, _)| *s);
    let excess = autos.len().saturating_sub(keep.max(1));
    let mut gone = Vec::new();
    for (_, e) in autos.into_iter().take(excess) {
        if delete(&e).is_ok() {
            gone.push(e.stem);
        }
    }
    gone
}

/// Writes an autosave and prunes the rotation to `keep`. Returns the path written.
pub fn autosave(
    dir: &Path,
    keep: usize,
    bytes: &[u8],
    meta: &SaveMeta,
    thumb: Option<&Thumbnail>,
) -> Result<PathBuf> {
    let stem = autosave_stem(next_autosave_seq(dir));
    let path = write_save(dir, &stem, bytes, meta, thumb)?;
    prune_autosaves(dir, keep);
    Ok(path)
}

/// Game months elapsed between two `(year, month)` points (`month` 1..=12).
pub fn months_between(from: (u16, u8), to: (u16, u8)) -> i32 {
    let m = |(y, mo): (u16, u8)| i32::from(y) * 12 + i32::from(mo);
    m(to) - m(from)
}

/// Civil date and time (UTC) of a Unix time: `(year, month, day, hour, minute)`.
pub fn civil(unix: u64) -> (i64, u32, u32, u32, u32) {
    let days = (unix / 86_400) as i64;
    let rem = (unix % 86_400) as u32;
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, rem / 3600, rem % 3600 / 60)
}

/// `2026-10-23 14:05` (UTC; the date has no time zone database behind it).
pub fn format_time(unix: u64) -> String {
    if unix == 0 {
        return String::new();
    }
    let (y, m, d, h, mi) = civil(unix);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rrt2-saves-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn meta(t: u64, kind: SaveKind) -> SaveMeta {
        SaveMeta {
            version: META_VERSION,
            kind,
            scenario: "Baltimore & Ohio".into(),
            company: "Rocha Railroad".into(),
            year: 1876,
            month: 3,
            day: 4,
            cash: Some(-12_345),
            saved_at: t,
        }
    }

    #[test]
    fn civil_dates_are_right() {
        assert_eq!(civil(0), (1970, 1, 1, 0, 0));
        assert_eq!(civil(1_000_000_000), (2001, 9, 9, 1, 46));
        assert_eq!(civil(1_709_210_096), (2024, 2, 29, 12, 34));
        assert_eq!(format_time(1_000_000_000), "2001-09-09 01:46");
        assert_eq!(format_time(0), "");
    }

    #[test]
    fn file_names_are_made_safe() {
        assert_eq!(sanitize_stem("  My game  "), "My game");
        assert_eq!(sanitize_stem("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(sanitize_stem("a/b\\c:d"), "a_b_c_d");
        assert_eq!(sanitize_stem("..."), "save");
        assert_eq!(sanitize_stem(""), "save");
        assert_eq!(sanitize_stem(&"x".repeat(200)).len(), 60);
        assert!(!sanitize_stem("../x").contains('/'));
    }

    #[test]
    fn a_save_round_trips_with_its_sidecars() {
        let dir = tmp("rt");
        let thumb = Thumbnail {
            w: 2,
            h: 2,
            rgb: vec![[1, 2, 3], [4, 5, 6], [7, 8, 9], [250, 251, 252]],
        };
        let m = meta(1_700_000_000, SaveKind::Manual);
        let p = write_save(&dir, "Mine", b"GAMBYTES", &m, Some(&thumb)).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"GAMBYTES");
        let l = list(&dir);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].stem, "Mine");
        assert_eq!(l[0].meta.as_ref(), Some(&m));
        assert_eq!(read_thumbnail(&l[0].path), Some(thumb));
        delete(&l[0]).unwrap();
        assert!(list(&dir).is_empty());
        assert!(!dir.join("Mine.png").exists() && !dir.join("Mine.toml").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_save_without_sidecars_still_lists() {
        let dir = tmp("bare");
        std::fs::write(dir.join("old.GAM"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let l = list(&dir);
        assert_eq!(l.len(), 1);
        assert!(l[0].meta.is_none());
        assert_eq!(l[0].kind(), SaveKind::Manual);
        assert!(read_thumbnail(&l[0].path).is_none());
        // A missing folder is an empty list, not an error.
        assert!(list(&dir.join("nope")).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn listing_is_newest_first_and_stable() {
        let dir = tmp("order");
        for (name, t) in [("b", 100), ("a", 100), ("c", 300), ("d", 200)] {
            write_save(&dir, name, b"x", &meta(t, SaveKind::Manual), None).unwrap();
        }
        let names: Vec<_> = list(&dir).into_iter().map(|e| e.stem).collect();
        assert_eq!(names, ["c", "d", "a", "b"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn autosaves_rotate_and_keep_the_newest() {
        let dir = tmp("rot");
        // A manual save and the quick save are never touched by the rotation.
        write_save(&dir, "Mine", b"m", &meta(1, SaveKind::Manual), None).unwrap();
        write_save(&dir, QUICK_STEM, b"q", &meta(2, SaveKind::Quick), None).unwrap();
        for i in 0..7u64 {
            autosave(&dir, 3, &[i as u8], &meta(10 + i, SaveKind::Auto), None).unwrap();
        }
        let mut autos: Vec<_> = list(&dir)
            .into_iter()
            .filter(|e| autosave_seq(&e.stem).is_some())
            .map(|e| e.stem)
            .collect();
        autos.sort();
        assert_eq!(autos, ["autosave-0005", "autosave-0006", "autosave-0007"]);
        // The kept ones are the newest content.
        assert_eq!(std::fs::read(dir.join("autosave-0007.GAM")).unwrap(), [6]);
        assert!(dir.join("Mine.GAM").exists() && dir.join("quicksave.GAM").exists());
        assert!(!dir.join("autosave-0001.toml").exists());
        assert_eq!(next_autosave_seq(&dir), 8);
        // Lowering the limit prunes on the next autosave; 0 still keeps one.
        assert_eq!(prune_autosaves(&dir, 1), ["autosave-0005", "autosave-0006"]);
        assert_eq!(prune_autosaves(&dir, 0), Vec::<String>::new());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn months_between_counts_across_years() {
        assert_eq!(months_between((1876, 11), (1877, 2)), 3);
        assert_eq!(months_between((1876, 5), (1876, 5)), 0);
        assert_eq!(months_between((1876, 5), (1876, 4)), -1);
    }

    #[test]
    fn a_failed_write_leaves_no_partial_file() {
        let dir = tmp("atomic");
        // The target is a directory: the rename fails and the temp file is not mistaken for a save.
        std::fs::create_dir_all(dir.join("x.GAM")).unwrap();
        assert!(write_save(&dir, "x", b"data", &meta(1, SaveKind::Manual), None).is_err());
        assert!(list(&dir).iter().all(|e| e.path.is_file()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
