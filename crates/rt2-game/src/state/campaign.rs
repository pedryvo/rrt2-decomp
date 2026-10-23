//! The campaign board's medals (docs/reference/screens.md B2): records the medal of a finished
//! campaign scenario and reads them back for the board (`crate::campaign`).

use super::AppState;
use crate::campaign::{self, Progress};

impl AppState {
    fn campaign_dir(&self) -> Option<std::path::PathBuf> {
        self.saves_dir.clone().or_else(crate::paths::saves_dir)
    }

    /// The medals held so far.
    pub(super) fn campaign_progress(&self) -> Progress {
        self.campaign_dir()
            .map_or_else(Progress::default, |d| Progress::load(&d))
    }

    /// The text after a scenario's name on the board: its medal, if any.
    pub(super) fn campaign_medal_text(&self, p: &Progress, set: usize, index: usize) -> String {
        let t = &self.texts;
        match p.medal(set, index) {
            Some(campaign::GOLD) => format!("  [{}]", t.get(2759, "Gold")),
            Some(campaign::SILVER) => format!("  [{}]", t.get(2760, "Silver")),
            Some(campaign::BRONZE) => format!("  [{}]", t.get(2761, "Bronze")),
            _ => String::new(),
        }
    }

    /// A campaign game ended with `rank` (1 bronze .. 3 gold): keep the best medal.
    pub(super) fn record_campaign_medal(&mut self, rank: u8) {
        let Some((set, index)) = self.game().and_then(|g| g.campaign) else {
            return;
        };
        let Some(dir) = self.campaign_dir() else {
            return;
        };
        let mut p = Progress::load(&dir);
        if p.record(set, index, rank) {
            let _ = p.save(&dir);
        }
    }
}
