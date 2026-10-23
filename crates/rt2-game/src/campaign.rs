//! The medals earned in the campaigns (docs/reference/screens.md B2; scenarios.md 5): which
//! medal the player holds for each of the 18 scenarios of the two campaigns. The original keeps
//! this in the campaign save; the clone keeps one text file next to the saved games, one line
//! per medal: `set index rank` (rank 1 bronze, 2 silver, 3 gold). Only the best medal is kept.

use std::collections::BTreeMap;
use std::path::Path;

/// Medal ranks as stored.
pub const BRONZE: u8 = 1;
pub const SILVER: u8 = 2;
pub const GOLD: u8 = 3;

/// File name inside the saves folder.
pub const FILE: &str = "campaign-medals.txt";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    medals: BTreeMap<(usize, usize), u8>,
}

impl Progress {
    pub fn parse(text: &str) -> Progress {
        let mut p = Progress::default();
        for line in text.lines() {
            let mut it = line.split_whitespace().map(str::parse::<usize>);
            if let (Some(Ok(set)), Some(Ok(index)), Some(Ok(rank))) =
                (it.next(), it.next(), it.next())
                && set < 2
                && index < 18
                && (1..=3).contains(&rank)
            {
                p.record(set, index, rank as u8);
            }
        }
        p
    }

    pub fn render(&self) -> String {
        self.medals
            .iter()
            .map(|((s, i), r)| format!("{s} {i} {r}\n"))
            .collect()
    }

    /// Medal rank of a scenario, if any.
    pub fn medal(&self, set: usize, index: usize) -> Option<u8> {
        self.medals.get(&(set, index)).copied()
    }

    /// Records a medal; true when it is new or better than the one held.
    pub fn record(&mut self, set: usize, index: usize, rank: u8) -> bool {
        let e = self.medals.entry((set, index)).or_insert(0);
        if rank > *e {
            *e = rank;
            true
        } else {
            false
        }
    }

    /// `(gold, silver, bronze)` counts of one campaign.
    pub fn counts(&self, set: usize) -> (usize, usize, usize) {
        let n = |r: u8| {
            self.medals
                .iter()
                .filter(|((s, _), v)| *s == set && **v == r)
                .count()
        };
        (n(GOLD), n(SILVER), n(BRONZE))
    }

    pub fn load(dir: &Path) -> Progress {
        std::fs::read_to_string(dir.join(FILE))
            .map_or_else(|_| Progress::default(), |t| Progress::parse(&t))
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(FILE), self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_best_medal_is_kept_and_the_file_round_trips() {
        let mut p = Progress::default();
        assert!(p.record(0, 3, SILVER));
        assert!(!p.record(0, 3, BRONZE));
        assert!(p.record(0, 3, GOLD));
        assert!(p.record(1, 0, BRONZE));
        assert_eq!(p.medal(0, 3), Some(GOLD));
        assert_eq!(p.counts(0), (1, 0, 0));
        assert_eq!(Progress::parse(&p.render()), p);
    }

    #[test]
    fn junk_lines_are_ignored() {
        let p = Progress::parse("x y z\n0 99 3\n0 1 7\n0 1 2\n");
        assert_eq!(p.medal(0, 1), Some(SILVER));
        assert_eq!(p.counts(0), (0, 1, 0));
    }
}
