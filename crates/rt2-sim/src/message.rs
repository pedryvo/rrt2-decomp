//! The output queue of player-visible messages and the end-of-game outcome.
//!
//! The original shows these as dialogs, newspaper headlines and status lines
//! (`0x469320`, `0x4a42f0`, `0x4510a0`, `0x46aa90`). The simulation never reads the queue back:
//! it is an output only, drained by the front end with [`World::take_messages`], so it cannot
//! affect determinism (the queue is not part of the state comparison either).

use crate::scenario::Message;

/// What the front end should show, in the order the original raised it.
///
/// Texts of scenario rules arrive already substituted (`docs/spec/scenarios.md` 2.4); the other
/// variants carry ids and numbers and the front end formats them with the LNG string named in
/// each variant's documentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameMessage {
    /// A scenario rule message (`docs/spec/scenarios.md` 2.4): `kind` is dialog, newspaper or
    /// two-button choice; `trigger` is the trigger code of the rule that produced it, and
    /// `label1`/`label2` are the button labels (newspaper: subtitle). A choice cannot be
    /// answered yet: the engine runs the effect set of button 0 (documented limitation).
    Scenario(Message),
    /// Year-end historical notice (`docs/spec/time.md` 7.5): `lng` is 2630 (1877, Tool and Die
    /// stopped accepting iron), 2851 (1879), 419 (1900), 417 (1910) or 420 (1960).
    Historical { year: i32, lng: u16 },
    /// The economic state changed (`docs/spec/cargo.md` 9, newspaper `0x46aa90`): states 0..=4.
    EconomyChanged { from: u8, to: u8 },
    /// First building of a type in the game (`docs/spec/world-growth.md` 4.4, LNG 94 near a
    /// town, 95 without one). `place` is the place the building was created in.
    FirstOfType { building_type: u8, place: i32 },
    /// A town offers a bonus to the first railroad that connects it (`docs/spec/world-tick.md`
    /// 3.6, LNG 226: "%1 has offered a bonus of %2 ...").
    TownBonusOffer { place: i32, amount: i32 },
    /// An AI company started (`docs/spec/ai.md` 5.3, LNG 166).
    CompanyStarted { company: i32 },
    /// An AI company connected two places (`docs/spec/ai.md` 5.3, LNG 240); the places are 0
    /// when the plan was not recorded.
    CompanyConnected { company: i32, from: i32, to: i32 },
    /// A company went bankrupt (`docs/spec/ai.md` 10, LNG 182).
    CompanyBankrupt { company: i32 },
    /// A stock split with the `ratio` shares per share (`docs/spec/finance.md` 8, LNG 169).
    StockSplit { company: i32, ratio: i32 },
}

/// End of the game (`Scenario::game_over` state 1..=4, `docs/spec/scenarios.md` 3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Bronze,
    Silver,
    Gold,
    /// Defeat; [`World::game_over_other_won`] tells whether another player won.
    Lose,
}
