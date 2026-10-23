//! The High Scores table (screen A9, `docs/reference/screens.md`).
//!
//! The original keeps its table in `hiscore.dat` next to the executable; that file is not in the
//! install (the game creates it at the first victory) and its layout was never decoded, so the
//! clone keeps its own text file in `~/.local/share/rrt2/hiscore.dat` (`$RRT2_HISCORE` to change
//! it) and never touches the install. Gap: the original's score formula is not in the specs; the
//! clone ranks victories by the company's cash when the scenario ended.

use std::path::{Path, PathBuf};

/// Entries kept, best first.
pub const MAX_ENTRIES: usize = 20;
const MAGIC: &str = "RRT2-HISCORE 1";

/// One finished scenario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub company: String,
    pub scenario: String,
    /// 1 bronze, 2 silver, 3 gold (a loss is not recorded).
    pub medal: u8,
    pub year: u16,
    pub month: u8,
    pub score: i64,
}

/// `$RRT2_HISCORE` or `<data dir>/rrt2/hiscore.dat`.
pub fn file() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("RRT2_HISCORE") {
        return Some(PathBuf::from(p));
    }
    dirs::data_dir().map(|d| d.join("rrt2").join("hiscore.dat"))
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Parses the file; unreadable lines are skipped.
pub fn parse(text: &str) -> Vec<Entry> {
    let mut lines = text.lines();
    if lines.next() != Some(MAGIC) {
        return Vec::new();
    }
    let mut out: Vec<Entry> = lines
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != 6 {
                return None;
            }
            Some(Entry {
                company: f[0].to_owned(),
                scenario: f[1].to_owned(),
                medal: f[2].parse().ok().filter(|m| (1..=3).contains(m))?,
                year: f[3].parse().ok()?,
                month: f[4].parse().ok()?,
                score: f[5].parse().ok()?,
            })
        })
        .collect();
    sort(&mut out);
    out
}

pub fn render(entries: &[Entry]) -> String {
    let mut s = format!("{MAGIC}\n");
    for e in entries {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            clean(&e.company),
            clean(&e.scenario),
            e.medal,
            e.year,
            e.month,
            e.score
        ));
    }
    s
}

fn sort(v: &mut Vec<Entry>) {
    // Stable: equal scores keep the older entry first.
    v.sort_by_key(|e| std::cmp::Reverse(e.score));
    v.truncate(MAX_ENTRIES);
}

/// Adds `e`; returns its 0-based rank, or `None` when it did not make the table.
pub fn insert(list: &mut Vec<Entry>, e: Entry) -> Option<usize> {
    let rank = list.iter().take_while(|o| o.score >= e.score).count();
    if rank >= MAX_ENTRIES {
        return None;
    }
    list.insert(rank, e);
    list.truncate(MAX_ENTRIES);
    Some(rank)
}

pub fn load(path: &Path) -> Vec<Entry> {
    std::fs::read_to_string(path)
        .map(|t| parse(&t))
        .unwrap_or_default()
}

pub fn save(path: &Path, entries: &[Entry]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, render(entries))?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(score: i64) -> Entry {
        Entry {
            company: "B&O".into(),
            scenario: "Baltimore".into(),
            medal: 3,
            year: 1880,
            month: 5,
            score,
        }
    }

    #[test]
    fn insert_keeps_the_table_sorted_and_bounded() {
        let mut v = Vec::new();
        assert_eq!(insert(&mut v, e(100)), Some(0));
        assert_eq!(insert(&mut v, e(300)), Some(0));
        assert_eq!(insert(&mut v, e(200)), Some(1));
        assert_eq!(insert(&mut v, e(200)), Some(2), "ties go after the older");
        assert_eq!(
            v.iter().map(|x| x.score).collect::<Vec<_>>(),
            [300, 200, 200, 100]
        );
        for i in 0..40 {
            insert(&mut v, e(1000 + i));
        }
        assert_eq!(v.len(), MAX_ENTRIES);
        assert_eq!(insert(&mut v, e(1)), None);
    }

    #[test]
    fn render_parse_round_trip_and_garbage_is_ignored() {
        let mut a = e(5);
        a.company = "Tab\tName".into();
        let text = render(&[e(9), a]);
        let back = parse(&text);
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].company, "Tab Name");
        assert_eq!(back[0].score, 9);
        assert!(parse("junk").is_empty());
        assert_eq!(parse(&format!("{MAGIC}\nbad line\n")).len(), 0);
    }

    #[test]
    fn file_round_trip() {
        let dir = std::env::temp_dir().join(format!("rrt2-hs-{}", std::process::id()));
        let p = dir.join("hiscore.dat");
        save(&p, &[e(7)]).unwrap();
        assert_eq!(load(&p), vec![e(7)]);
        std::fs::remove_dir_all(dir).unwrap();
        assert!(load(&p).is_empty());
    }
}
