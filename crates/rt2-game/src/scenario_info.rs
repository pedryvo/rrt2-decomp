//! What the menus show about a scenario file: title, description, size, start date, the
//! briefing texts and the bronze / silver / gold / loss goals, all read from the map file and its
//! rules (docs/spec/scenarios.md 2.2, 5 and 6; docs/formats/map-gam.md 1 and 5.6).
//!
//! There is no goal table: the goals of a scenario are ordinary rules whose first effect set holds
//! `Game Over - Win 1/2/3/Loss` (effects 1 to 4, `scenarios.md` 5). The briefing window is the
//! text of the rules with trigger 6 (`scenarios.md` 2.2) and the medal texts are the messages the
//! player gets when the rule fires. The condition is shown as the original editor stores it.

use rt2_formats::map::Save;
use rt2_sim::scenario::{MessageKind, Rule};

/// Header offset of the free-text scenario description (`map-gam.md` 1). The block runs up to the
/// 170x170 preview image at `0x387`, so the text is at most `0x387 - 0x25a` bytes.
const DESCRIPTION: std::ops::Range<usize> = 0x25a..0x387;

/// Medal of a goal rule (`scenarios.md` 5: effects 1, 2, 3 and 4 of the first effect set).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Medal {
    Bronze,
    Silver,
    Gold,
    Loss,
}

impl Medal {
    /// The medal of effect number `e` (1..=4), if it is one.
    pub fn from_effect(e: usize) -> Option<Medal> {
        match e {
            1 => Some(Medal::Bronze),
            2 => Some(Medal::Silver),
            3 => Some(Medal::Gold),
            4 => Some(Medal::Loss),
            _ => None,
        }
    }
}

/// One goal rule: the condition that ends the game with `medal`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Goal {
    pub medal: Medal,
    /// The rule's comment/title (`Gold`, `Silver`, `Loser`...).
    pub title: String,
    /// The condition text of the rule, as stored.
    pub condition: String,
    /// The message shown when the rule fires (empty when it has none).
    pub message: String,
    /// Trigger of the rule (`scenarios.md` 2.2): when it is evaluated.
    pub trigger: u8,
}

/// A briefing page: a rule with trigger 6 (`scenarios.md` 2.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Briefing {
    pub title: String,
    pub text: String,
}

/// A news item the rules would print (newspaper / dialog messages that are not goals).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventPreview {
    pub kind: MessageKind,
    pub headline: String,
    pub text: String,
    pub trigger: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioInfo {
    pub title: String,
    pub description: String,
    pub width: usize,
    pub height: usize,
    pub year: u16,
    pub month: u8,
    pub day: u8,
    /// Persons (players) in the file; the first is the human, the others are computer players.
    pub persons: usize,
    /// Companies already founded in the file (a scenario with none asks the player to found
    /// one, `finance.md` 17).
    pub companies: usize,
    pub briefing: Vec<Briefing>,
    pub goals: Vec<Goal>,
    pub events: Vec<EventPreview>,
}

/// Latin-1 text up to the first NUL, with CR LF folded to LF.
fn c_text(raw: &[u8]) -> String {
    raw.iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect::<String>()
        .replace("\r\n", "\n")
}

impl ScenarioInfo {
    pub fn from_save(save: &Save, raw: &[u8], fallback_title: &str) -> ScenarioInfo {
        let description = raw
            .get(DESCRIPTION)
            .map(c_text)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let mut info = ScenarioInfo {
            title: fallback_title.to_owned(),
            description,
            width: save.map.width,
            height: save.map.height,
            year: save.date.year,
            month: save.date.month,
            day: save.date.day,
            persons: save.persons.ordered().count(),
            companies: save.companies.ordered().count(),
            briefing: Vec::new(),
            goals: Vec::new(),
            events: Vec::new(),
        };
        for r in &save.rules {
            info.add_rule(&Rule::from_file(r));
        }
        info
    }

    fn add_rule(&mut self, r: &Rule) {
        let medal = r
            .effects
            .first()
            .and_then(|set| {
                (1..=4)
                    .filter(|&e| set.get(e).copied().unwrap_or(0) != 0)
                    .find_map(Medal::from_effect)
            })
            .filter(|_| !r.date_mode);
        if let Some(medal) = medal {
            self.goals.push(Goal {
                medal,
                title: r.title.trim().to_owned(),
                condition: r.condition.trim().to_owned(),
                message: r.message.trim().to_owned(),
                trigger: r.trigger,
            });
        } else if r.trigger == 6 {
            if r.message.trim().len() > 1 {
                self.briefing.push(Briefing {
                    title: r.title.trim().to_owned(),
                    text: r.message.trim().to_owned(),
                });
            }
        } else if r.trigger != 7 && r.message.trim().len() > 1 {
            self.events.push(EventPreview {
                kind: r.message_kind,
                headline: if r.message_kind == MessageKind::Newspaper {
                    r.message.trim().to_owned()
                } else {
                    r.title.trim().to_owned()
                },
                text: if r.message_kind == MessageKind::Newspaper {
                    r.label1.trim().to_owned()
                } else {
                    r.message.trim().to_owned()
                },
                trigger: r.trigger,
            });
        }
    }

    /// The goals in medal order (gold first, loss last).
    pub fn goals_by_medal(&self) -> Vec<&Goal> {
        let mut v: Vec<&Goal> = self.goals.iter().collect();
        v.sort_by_key(|g| std::cmp::Reverse(g.medal));
        // Loss last.
        v.sort_by_key(|g| g.medal == Medal::Loss);
        v
    }

    /// The briefing as one text: every trigger-6 page, in rule order, separated by blank lines.
    pub fn briefing_text(&self) -> String {
        let parts: Vec<&str> = self
            .briefing
            .iter()
            .map(|b| b.text.as_str())
            .filter(|t| !t.is_empty())
            .collect();
        if parts.is_empty() {
            self.description.clone()
        } else {
            parts.join("\n\n")
        }
    }
}

/// A condition of the scenario language (`scenarios.md` 3) spaced out for reading:
/// `CompanyLoadsHauledLifetime>=12` becomes `Company Loads Hauled Lifetime >= 12`.
pub fn humanize_condition(cond: &str) -> String {
    let mut out = String::with_capacity(cond.len() + 16);
    let mut prev = ' ';
    for c in cond.chars() {
        if c.is_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            prev = ' ';
            continue;
        }
        let cmp = matches!(c, '>' | '<' | '=');
        let prev_cmp = matches!(prev, '>' | '<' | '=');
        if (cmp != prev_cmp && prev != ' ' && prev != '(' && c != ')')
            || (c.is_uppercase() && prev.is_lowercase())
        {
            out.push(' ');
        }
        out.push(c);
        prev = c;
    }
    out.trim().to_owned()
}

/// Word-wraps `text` to lines of at most `max` characters (blank lines kept, words longer than a
/// line broken). Widths are an estimate: the UI has no text measurement.
pub fn wrap_text(text: &str, max: usize) -> Vec<String> {
    let max = max.max(8);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let mut len = 0usize;
        for word in para.split_whitespace() {
            let wl = word.chars().count();
            if len > 0 && len + 1 + wl > max {
                out.push(std::mem::take(&mut line));
                len = 0;
            }
            if wl > max {
                // Break a very long word.
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(max) {
                    if len > 0 {
                        out.push(std::mem::take(&mut line));
                    }
                    line = chunk.iter().collect();
                    len = chunk.len();
                }
                continue;
            }
            if len > 0 {
                line.push(' ');
                len += 1;
            }
            line.push_str(word);
            len += wl;
        }
        out.push(line);
    }
    while out.len() > 1 && out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions_are_spaced_for_reading() {
        assert_eq!(
            humanize_condition("CompanyLoadsHauledLifetime>=12"),
            "Company Loads Hauled Lifetime >= 12"
        );
        assert_eq!(
            humanize_condition("GameYear=1840  and TerritoryId=2"),
            "Game Year = 1840 and Territory Id = 2"
        );
        assert_eq!(
            humanize_condition("CompanyCash(IsHighestInClass)"),
            "Company Cash(Is Highest In Class)"
        );
    }

    #[test]
    fn wrapping_keeps_paragraphs_and_breaks_long_words() {
        let t = wrap_text("one two three four\n\nfive", 9);
        assert_eq!(t, vec!["one two", "three", "four", "", "five"]);
        let l = wrap_text("abcdefghijkl", 8);
        assert_eq!(l, vec!["abcdefgh", "ijkl"]);
        assert_eq!(wrap_text("", 10), vec![String::new()]);
    }

    #[test]
    fn effects_one_to_four_are_the_medals() {
        assert_eq!(Medal::from_effect(3), Some(Medal::Gold));
        assert_eq!(Medal::from_effect(4), Some(Medal::Loss));
        assert_eq!(Medal::from_effect(5), None);
        assert!(Medal::Gold > Medal::Bronze);
    }

    fn rule(trigger: u8, effects: &[usize], message: &str, title: &str) -> Rule {
        let mut r = Rule {
            trigger,
            message: message.to_owned(),
            title: title.to_owned(),
            condition: "GameYear>=1850".to_owned(),
            ..Rule::default()
        };
        r.effects = [vec![0i8; 350], vec![0i8; 350]];
        for &e in effects {
            r.effects[0][e] = 1;
        }
        r
    }

    fn empty() -> ScenarioInfo {
        ScenarioInfo {
            title: "T".into(),
            description: String::new(),
            width: 1,
            height: 1,
            year: 1850,
            month: 1,
            day: 1,
            persons: 1,
            companies: 0,
            briefing: vec![],
            goals: vec![],
            events: vec![],
        }
    }

    #[test]
    fn rules_sort_into_goals_briefing_and_events() {
        let mut i = empty();
        i.add_rule(&rule(6, &[], "Welcome.", "Victory Conditions"));
        i.add_rule(&rule(3, &[3], "Gold!", "Gold"));
        i.add_rule(&rule(1, &[1], "Bronze.", "Bronze"));
        i.add_rule(&rule(2, &[4], "Lost.", "Loser"));
        i.add_rule(&rule(4, &[], "A town grows.", "Growth"));
        i.add_rule(&rule(7, &[], "Status: 3 of 5", "Status"));
        assert_eq!(i.briefing.len(), 1);
        assert_eq!(i.events.len(), 1, "status lines (trigger 7) are not events");
        let order: Vec<Medal> = i.goals_by_medal().iter().map(|g| g.medal).collect();
        assert_eq!(order, vec![Medal::Gold, Medal::Bronze, Medal::Loss]);
        assert_eq!(i.briefing_text(), "Welcome.");
    }

    #[test]
    fn briefing_falls_back_to_the_description() {
        let mut i = empty();
        i.description = "A map.".into();
        assert_eq!(i.briefing_text(), "A map.");
    }

    /// Real data: needs the game install (`RT2_DIR`), skipped without it.
    #[test]
    fn the_first_scenario_has_three_medals_and_a_loss() {
        let Some(dir) = std::env::var_os("RT2_DIR") else {
            return;
        };
        let path = std::path::Path::new(&dir).join("maps").join("#01B&O~1.MAP");
        let Ok(raw) = std::fs::read(path) else {
            return;
        };
        let save = Save::parse(&raw).unwrap();
        let info = ScenarioInfo::from_save(&save, &raw, "B&O");
        let medals: Vec<Medal> = info.goals_by_medal().iter().map(|g| g.medal).collect();
        assert_eq!(
            medals,
            vec![Medal::Gold, Medal::Silver, Medal::Bronze, Medal::Loss]
        );
        assert!(info.briefing_text().starts_with("The Baltimore and Ohio"));
        assert_eq!((info.width, info.height, info.year), (150, 150, 1830));
        assert!(info.description.starts_with("Begin your railroad"));
    }
}
