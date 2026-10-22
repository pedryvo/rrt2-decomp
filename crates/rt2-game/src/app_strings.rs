//! The clone's own UI texts (docs/i18n.md, "Textos do clone"): English defaults whose ids are
//! `APP_FIRST + index`, outside the `default2.lng` range. The list is append-only: an id is
//! never reused or reordered, because translation files refer to it. `%1`..`%9` are replaced by
//! arguments (`Texts::af`).

/// Id of the first entry.
pub const APP_FIRST: u32 = 100_000;

pub static APP_EN: &[&str] = &[
    "Menu",
    "Save",
    "Language",
    "Saved %1",
    "Cannot save: %1",
    "%1  (saved)",
    "rrt2 %1   |   UI scale %2%  (Ctrl +/- to change)   |   %3",
    "%1 %2, %3",
    "Market",
    "Stations",
    "Industries",
    "Not available",
    "Back",
    "Close",
    "No game state to show.",
    "Nothing to show.",
    "Left / Right: company   Esc: close",
    "Backspace: back   Esc: close",
    "Click a row for details   Wheel, PgUp, PgDn: scroll   Esc: close",
    "Paused",
    "Speed:",
    "Cash",
    "Dividend",
    "Book Value",
    "Rating",
    "This year",
    "Last year",
    "Engine",
    "Cars",
    "Route",
    "Speed",
    "Age",
    "Vehicle",
    "Cargo",
    "Station",
    "Wait",
    "Owner",
    "Improvements",
    "Cargo waiting",
    "Waiting",
    "Level",
    "Delivered",
    "Kind",
    "Houses",
    "Growth",
    "Shipped",
    "Location",
    "Industry",
    "Built",
    "Place",
    "Received",
    "Output",
    "Per year",
    "Produced",
    "Input",
    "Stock",
    "No such company.",
    "Founded",
    "Tender",
    "Restaurant car",
    "Caboose",
    "Car",
    "Purchased",
    "Status",
    "stopped",
    "running",
    "Weight",
    "Condition",
    "broken down",
    "empty",
    "loaded",
    "waypoint",
    "Consist",
    "Linked buildings",
    "none",
    "built",
    "industry",
    "houses",
    "Supply and demand are not decoded for the old 0x233C layout.",
    "Extent",
    "Growth index",
    "Intensity",
    "Trend",
    "Land tiles",
    "Connection bonus",
    "Lots delivered (yr)",
    "Lots shipped (yr)",
    "No industries belong to this place.",
    "TODO: population and cargo demand per place are not decoded in the save.",
    "Region",
    "Tiles",
    "Cities and regions",
    "Type",
    "Price paid",
    "Profitability",
    "Profit last year",
    "Linked stations",
    "Lots received (yr)",
    "This building produces no cargo this year.",
    "Production",
    "Inputs",
    "This save uses the old 0x233C record layout: only names and positions are decoded, the other fields show \"-\". TODO: record conversion on load.",
    "Train %1",
    "Engine %1",
    "Vehicle %1",
    "%1 car",
    "half full",
    "full",
    "no wait",
    "TODO: profit, cargo carried per stop and the schedule status are not decoded in the save yet.",
    "TODO: territory effects, company rights and goodwill are not decoded here.",
    "code %1",
    "cargo %1",
    "Improvement bit %1",
    "Class %1",
    "Type %1",
    "Building %1",
    "%1 mph",
    "%1 years",
    "%1 t",
    "* = human chairman   Click a row for the company report   Esc: close",
    "Game started: %1.",
    "Companies",
    "Messages",
    "The annual report for %1 is ready.",
    "%1 would be founded with %2 shares at %3 (founder buys %4 for %5). TODO: company creation is not in the simulation yet.",
    "This tool is not available yet: construction is still being built into the simulation. (Placeholder panel.)",
    "Main Menu",
    "No scenario rules in this game.",
    "This scenario has no victory conditions.",
    "Defeat",
    "Total capital",
    "You buy",
    "You need at least %1 in personal cash and purchasing power to start a company.",
    "start of year",
    "end of year",
    "start of month",
    "end of month",
    "track or station change",
    "company founded",
    "checked during play",
    "You (human player)",
    "Computer player %1",
    "TODO: these options are kept but not applied to the simulation yet.",
    "Classic Campaign",
    "The Second Century",
    "Starting bonus",
    "TODO: the bonuses of this campaign are not decoded.",
    "TODO: the high scores list is not built yet.",
    "TODO: the credits are not built yet.",
    "Income",
    "Balance",
    "Goals",
];

/// The id of an English default.
pub fn id_of(english: &str) -> Option<u32> {
    APP_EN
        .iter()
        .position(|e| *e == english)
        .map(|i| APP_FIRST + i as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `.a("...")` / `.af("...")` call with a one-line literal names a known default.
    #[test]
    fn literals_used_in_the_sources_are_in_the_table() {
        for src in [
            include_str!("info.rs"),
            include_str!("state.rs"),
            include_str!("state/info_ui.rs"),
            include_str!("text.rs"),
        ] {
            for call in [".a(\"", ".af(\""] {
                for part in src.split(call).skip(1) {
                    let mut text = String::new();
                    let mut chars = part.chars();
                    while let Some(c) = chars.next() {
                        match c {
                            '\\' => match chars.next() {
                                Some('n') => text.push('\n'),
                                Some(o) => text.push(o),
                                None => break,
                            },
                            '"' => break,
                            c => text.push(c),
                        }
                    }
                    if text == "Menu" || !text.is_empty() {
                        assert!(id_of(&text).is_some(), "missing app string {text:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn defaults_are_unique() {
        for (i, a) in APP_EN.iter().enumerate() {
            assert_eq!(id_of(a), Some(APP_FIRST + i as u32), "{a:?} is duplicated");
        }
    }
}
