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
    /// A merger vote ran (`0x4015a0`, `docs/spec/finance.md` 15.1): LNG 24 "Merger Succeeds" or
    /// 25 "Merger Fails" plus the tally (LNG 63 "Votes in favor", 64 "Votes against"); the names
    /// are the ones at the time of the vote (the target is gone after a merger).
    MergerResult {
        buyer: i32,
        target: i32,
        buyer_name: String,
        target_name: String,
        offer: i32,
        passed: bool,
        tally: VoteTally,
    },
    /// A chairmanship contest ran (`0x42ee00`, finance.md 15.3): LNG 26 / 28 and the tally.
    ChairmanVote {
        company: i32,
        challenger: i32,
        company_name: String,
        challenger_name: String,
        passed: bool,
        tally: VoteTally,
    },
    /// A person took over the chair (`0x43c050`, LNG 27 "%1 is new chairman of %2!").
    ChairmanChanged {
        company: i32,
        person: i32,
        person_name: String,
        company_name: String,
    },
    /// A merger went through (`0x43c3c0`, LNG 168 "%1 takes over %2!").
    MergerDone {
        buyer: i32,
        buyer_name: String,
        target_name: String,
    },
    /// The broker called the margin (`0x4a7a40`): LNG 2270 (`month` 0..11 names the deadline).
    MarginWarning { person: i32, month: u8 },
    /// The broker sold `shares` of `company` (LNG 2271) or bought them to cover a short position
    /// (2272), or the short squeeze of LNG 2273 forced a cover (`squeeze`).
    MarginTrade {
        person: i32,
        company: i32,
        company_name: String,
        shares: i32,
        kind: MarginTradeKind,
    },
    /// The board suspended the dividend (LNG 2274).
    DividendSuspended { company: i32 },
    /// The board cut the dividend to the `f32` with bits `to_bits` per share (LNG 2713).
    DividendCut { company: i32, to_bits: u32 },
}

/// What a broker's margin trade was ([`GameMessage::MarginTrade`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginTradeKind {
    /// Long position sold (LNG 2271).
    Sold,
    /// Short position covered (LNG 2272).
    Covered,
    /// Short squeeze (LNG 2273).
    Squeeze,
}

/// One side of a vote count (`0x4015a0`, `0x42ee00`): the persons that hold shares of the target
/// and voted that way with their positions, the "other investors" (the free float that votes by
/// the public's mood) and the total.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteSide {
    /// `(person, shares)` in the order of the person list.
    pub voters: Vec<(i32, i32)>,
    pub public: i32,
    pub total: i32,
}

/// The two columns of the vote dialog: "Votes in favor" (LNG 63) and "Votes against" (LNG 64).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteTally {
    pub yes: VoteSide,
    pub no: VoteSide,
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
