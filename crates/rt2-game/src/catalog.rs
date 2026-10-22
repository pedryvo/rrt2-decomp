//! Lists of scenarios and saved games.

use std::path::{Path, PathBuf};

use rt2_formats::map::{Map, Save};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A scenario map of the install (`maps/*.MAP`, `*.MP2`).
    Map,
    /// A game shipped with the install (`games/*.GAM`).
    Original,
    /// One of our saves.
    Own,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub title: String,
    pub source: Source,
}

fn extension_is(p: &Path, exts: &[&str]) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Files of a folder with one of the extensions, sorted by name (never in directory order).
fn files(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && extension_is(p, exts))
        .collect();
    out.sort();
    out
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The display title of a map: the scenario title, else the short name, else the file name.
pub fn map_title(map: &Map, path: &Path) -> String {
    // Some scenario titles end in a marker character (` @`) that is not part of the name.
    let clean = |s: &str| s.trim().trim_end_matches('@').trim().to_owned();
    [clean(&map.title), clean(&map.name)]
        .into_iter()
        .find(|s| !s.is_empty() && s.chars().all(|c| !c.is_control()))
        .unwrap_or_else(|| stem(path))
}

pub fn list_maps(install: &Path) -> Vec<Entry> {
    files(&install.join("maps"), &["map", "mp2"])
        .into_iter()
        .map(|path| {
            let title = std::fs::read(&path)
                .ok()
                .and_then(|raw| Map::parse(&raw).ok())
                .map(|m| map_title(&m, &path))
                .unwrap_or_else(|| stem(&path));
            Entry {
                path,
                title,
                source: Source::Map,
            }
        })
        .collect()
}

pub fn list_saves(install: &Path, own: Option<&Path>) -> Vec<Entry> {
    let mut out: Vec<Entry> = own
        .map(|d| files(d, &["gam"]))
        .unwrap_or_default()
        .into_iter()
        .map(|path| Entry {
            title: stem(&path),
            path,
            source: Source::Own,
        })
        .collect();
    out.extend(
        files(&install.join("games"), &["gam"])
            .into_iter()
            .map(|path| Entry {
                title: stem(&path),
                path,
                source: Source::Original,
            }),
    );
    out
}

/// Parses a file as a save, or as a plain map when it has no game state.
pub fn load_any(raw: &[u8]) -> anyhow::Result<(Map, Option<Save>)> {
    match Save::parse(raw) {
        Ok(save) => Ok((save.map.clone(), Some(save))),
        Err(_) => Ok((Map::parse(raw)?, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_is_sorted_and_filters_extensions() {
        let dir = std::env::temp_dir().join(format!("rrt2-cat-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("maps")).unwrap();
        for n in ["b.MAP", "a.mp2", "c.txt"] {
            std::fs::write(dir.join("maps").join(n), b"junk").unwrap();
        }
        let l = list_maps(&dir);
        let names: Vec<_> = l.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            names,
            vec!["a", "b"],
            "unparseable maps fall back to the file name"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn own_saves_come_before_the_originals() {
        let base = std::env::temp_dir().join(format!("rrt2-saves-{}", std::process::id()));
        let (inst, own) = (base.join("inst"), base.join("own"));
        std::fs::create_dir_all(inst.join("games")).unwrap();
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(inst.join("games/Tutorial.GAM"), b"x").unwrap();
        std::fs::write(own.join("mine.GAM"), b"x").unwrap();
        let l = list_saves(&inst, Some(&own));
        assert_eq!(
            l.iter().map(|e| e.source).collect::<Vec<_>>(),
            vec![Source::Own, Source::Original]
        );
        std::fs::remove_dir_all(base).unwrap();
    }
}
