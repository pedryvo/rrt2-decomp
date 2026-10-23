//! Notification categories of the event history and the per-category notification settings
//! (docs/ui.md "Histórico de eventos e notificações"). This is a feature of the clone, not of the
//! original: the original shows every message it produces. The default is therefore "all on";
//! turning a category off only hides the notice (the line in the activity box and the dialog or
//! newspaper that goes with it), the entry still lands in the history.
//!
//! The categories group the simulation's `GameMessage`s (`rt2-sim/src/message.rs`) and the
//! player's own actions that the window reports. No sound is tied to a message in this build
//! (the sound table of docs/spec/sound.md has no cue for a message), so there is no sound
//! option.

use rt2_sim::message::GameMessage;
use serde::{Deserialize, Serialize};

/// What an entry of the history is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    /// Scenario rules, historical notices and the state of the economy.
    Scenario,
    /// Stocks, bonds, dividends, margin calls and the player's own financial actions.
    Finance,
    /// Towns and industries: first buildings, town bonus offers.
    Towns,
    /// The other companies: starts, connections, mergers, bankruptcies, chairmanships.
    Competitors,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Scenario,
        Category::Finance,
        Category::Towns,
        Category::Competitors,
    ];

    pub fn index(self) -> usize {
        Category::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }

    /// The English name; the key of the string table (`app_strings.rs`).
    pub fn english(self) -> &'static str {
        match self {
            Category::Scenario => "Scenario and economy",
            Category::Finance => "Finance and stocks",
            Category::Towns => "Towns and industries",
            Category::Competitors => "Competitors",
        }
    }

    /// The category of a message the simulation produced.
    pub fn of(m: &GameMessage) -> Category {
        match m {
            GameMessage::Scenario(_)
            | GameMessage::Historical { .. }
            | GameMessage::EconomyChanged { .. } => Category::Scenario,
            GameMessage::FirstOfType { .. } | GameMessage::TownBonusOffer { .. } => Category::Towns,
            GameMessage::CompanyStarted { .. }
            | GameMessage::CompanyConnected { .. }
            | GameMessage::CompanyBankrupt { .. }
            | GameMessage::MergerResult { .. }
            | GameMessage::ChairmanVote { .. }
            | GameMessage::ChairmanChanged { .. }
            | GameMessage::MergerDone { .. } => Category::Competitors,
            GameMessage::StockSplit { .. }
            | GameMessage::MarginWarning { .. }
            | GameMessage::MarginTrade { .. }
            | GameMessage::DividendSuspended { .. }
            | GameMessage::DividendCut { .. } => Category::Finance,
        }
    }
}

/// `[notices]` of `config.toml`: show the notice of each category (default: all, like the
/// original).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notices {
    pub scenario: bool,
    pub finance: bool,
    pub towns: bool,
    pub competitors: bool,
}

impl Default for Notices {
    fn default() -> Self {
        Notices {
            scenario: true,
            finance: true,
            towns: true,
            competitors: true,
        }
    }
}

impl Notices {
    pub fn get(&self, c: Category) -> bool {
        match c {
            Category::Scenario => self.scenario,
            Category::Finance => self.finance,
            Category::Towns => self.towns,
            Category::Competitors => self.competitors,
        }
    }

    pub fn toggle(&mut self, c: Category) {
        let v = match c {
            Category::Scenario => &mut self.scenario,
            Category::Finance => &mut self.finance,
            Category::Towns => &mut self.towns,
            Category::Competitors => &mut self.competitors,
        };
        *v = !*v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_show_everything_and_toggle_one_category() {
        let mut n = Notices::default();
        assert!(Category::ALL.iter().all(|c| n.get(*c)));
        n.toggle(Category::Towns);
        assert!(!n.get(Category::Towns));
        assert!(n.get(Category::Finance));
        n.toggle(Category::Towns);
        assert_eq!(n, Notices::default());
    }

    #[test]
    fn indices_follow_the_order_of_all() {
        for (i, c) in Category::ALL.iter().enumerate() {
            assert_eq!(c.index(), i);
        }
    }

    #[test]
    fn messages_map_to_categories() {
        assert_eq!(
            Category::of(&GameMessage::TownBonusOffer {
                place: 1,
                amount: 5
            }),
            Category::Towns
        );
        assert_eq!(
            Category::of(&GameMessage::CompanyStarted { company: 1 }),
            Category::Competitors
        );
        assert_eq!(
            Category::of(&GameMessage::DividendSuspended { company: 1 }),
            Category::Finance
        );
        assert_eq!(
            Category::of(&GameMessage::EconomyChanged { from: 0, to: 1 }),
            Category::Scenario
        );
    }
}
