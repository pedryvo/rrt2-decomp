//! The financial commands of `cmd_execute` (`0x4b0210`, docs/spec/commands.md 2.1) and what the
//! original's windows decide before sending them (`finance.md` 3.4, 4.4, 6, 7, 14, 15).
//!
//! [`FinCmd`] mirrors the ids and fields of the packets (ids `0x0e`, `0x0f`, `0x10`, `0x11`, `0x12`,
//! `0x13`, `0x14`, `0x15`, `0x22`..`0x25`, `0x2e`); [`FinCmd::from_packet`] reads an original
//! packet. [`World::apply`](crate::World::apply) with [`Command::Finance`](crate::Command)
//! runs the issuing-window checks ([`World::check_fin`]) and then the executor, which, like the
//! original's `cmd_execute`, validates nothing but live ids. [`World::apply_fin_unchecked`] runs
//! the executor alone (what a peer or a replay of a packet does).
//!
//! The vote of a merger and of a chairmanship contest is a two-step affair in the original: the
//! proposer creates a record with the votes of the shareholders, the humans answer with `0x23` /
//! `0x25`, and the count runs when everyone has answered. [`Votes`] holds those records (they are
//! not part of a save). A human that has to answer is listed by [`World::pending_votes`].

use crate::message::VoteTally;
use crate::world::World;

/// Fields of an original financial packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinCmd {
    /// `0x0e` `cmd_stock_trade`: `person_buy_stock` / `person_sell_stock` with the lot of
    /// `person_trade_lot` (`0x4a6e00`). `buy` is `+0xe > 0`. `flag` is `+0xf`: the margin flag of
    /// a purchase (`0x4a7300` `param_3`; docs/spec/commands.md calls it "forced", but it is
    /// `0x4a6f60`'s margin argument and the forced flag of the call is always 0) and the
    /// short-selling flag of a sale (`0x4a73d0` `param_3`).
    StockTrade {
        person: i32,
        company: i32,
        buy: bool,
        flag: bool,
    },
    /// `0x0f` `cmd_resign_chairman`: `company_resign_chairman` (`0x43bf80`).
    ResignChairman { person: i32, company: i32 },
    /// `0x10` `cmd_set_manager`: `company_set_manager(company, manager, 1)` (`0x438780`).
    SetManager { company: i32, manager: i32 },
    /// `0x11` `cmd_stock_issue`: `n > 0` issues `n` shares (`0x439060`), `n <= 0` buys back `-n`
    /// (`0x438f70`).
    StockIssue { company: i32, n: i32 },
    /// `0x12` `cmd_set_dividend`: stores the annual dividend per share (`0x43bfd0`); the `f32`
    /// travels as its bits (`+0xd`) so that commands stay comparable.
    SetDividend { company: i32, dividend_bits: u32 },
    /// `0x13` `cmd_bankruptcy` (`0x43bc30`).
    Bankruptcy { company: i32 },
    /// `0x14` `cmd_bond_issue`: `company_issue_bond(company, principal, term, 1)` (`0x439320`).
    BondIssue {
        company: i32,
        principal: i32,
        term: i32,
    },
    /// `0x15` `cmd_bond_repay`: `company_repay_bond(company, bond, 1)` (`0x4391b0`).
    BondRepay { company: i32, bond: i32 },
    /// `0x22` `cmd_merger_propose` (`0x48b2f0` -> `0x48b4f0`).
    MergerPropose { buyer: i32, target: i32, offer: i32 },
    /// `0x23` `cmd_merger_vote` (`0x48b4b0`): `proposal` is the record id returned by the
    /// proposal ([`Votes`]).
    MergerVote {
        proposal: u32,
        person: i32,
        yes: bool,
    },
    /// `0x24` `cmd_election_start` (`0x42eb40` -> `0x42ed10`).
    ElectionStart { company: i32, challenger: i32 },
    /// `0x25` `cmd_election_vote` (`0x42ecd0`).
    ElectionVote {
        election: u32,
        person: i32,
        yes: bool,
    },
    /// `0x2e` `cmd_chairman_takeover`: `company_chairman_takeover(company, person)` (`0x43c050`).
    ChairmanTakeover { person: i32, company: i32 },
}

fn i32_at(p: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]])
}

impl FinCmd {
    /// `0x12` with the dividend as a number.
    pub fn set_dividend(company: i32, dividend: f32) -> FinCmd {
        FinCmd::SetDividend {
            company,
            dividend_bits: dividend.to_bits(),
        }
    }

    /// The command of an original `cmd_execute` packet (`None` for an id that is not a financial
    /// command).
    pub fn from_packet(raw: &[u8]) -> Option<FinCmd> {
        let mut buf = [0u8; 0x40];
        let n = raw.len().min(0x40);
        buf[..n].copy_from_slice(&raw[..n]);
        let p = &buf[..];
        let id = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
        let b = |o: usize| i32::from(p[o] as i8);
        let ub = |o: usize| i32::from(p[o]);
        Some(match id {
            0x0e => FinCmd::StockTrade {
                person: b(0xc),
                company: ub(0xd),
                buy: (p[0xe] as i8) > 0,
                flag: p[0xf] != 0,
            },
            0x0f => FinCmd::ResignChairman {
                person: b(0xc),
                company: b(0xd),
            },
            0x10 => FinCmd::SetManager {
                company: b(0xc),
                manager: i32::from(i16::from_le_bytes([p[0xd], p[0xe]])),
            },
            0x11 => FinCmd::StockIssue {
                company: b(0xc),
                n: i32_at(p, 0xd),
            },
            0x12 => FinCmd::SetDividend {
                company: b(0xc),
                dividend_bits: i32_at(p, 0xd) as u32,
            },
            0x13 => FinCmd::Bankruptcy { company: b(0xc) },
            0x14 => FinCmd::BondIssue {
                company: b(0xc),
                principal: i32_at(p, 0xd),
                term: b(0x11),
            },
            0x15 => FinCmd::BondRepay {
                company: b(0xc),
                bond: i32::from(i16::from_le_bytes([p[0xd], p[0xe]])),
            },
            0x22 => FinCmd::MergerPropose {
                buyer: ub(0xc),
                target: ub(0xd),
                offer: i32::from(u16::from_le_bytes([p[0xe], p[0xf]])),
            },
            0x23 => FinCmd::MergerVote {
                proposal: i32_at(p, 0xc) as u32,
                person: ub(0x10),
                yes: p[0x11] != 0,
            },
            0x24 => FinCmd::ElectionStart {
                company: ub(0xc),
                challenger: ub(0xd),
            },
            0x25 => FinCmd::ElectionVote {
                election: i32_at(p, 0xc) as u32,
                person: ub(0x10),
                yes: p[0x11] != 0,
            },
            0x2e => FinCmd::ChairmanTakeover {
                person: ub(0xc),
                company: ub(0xd),
            },
            _ => return None,
        })
    }

    /// The packet id of the command.
    pub fn id(&self) -> u32 {
        match self {
            FinCmd::StockTrade { .. } => 0x0e,
            FinCmd::ResignChairman { .. } => 0x0f,
            FinCmd::SetManager { .. } => 0x10,
            FinCmd::StockIssue { .. } => 0x11,
            FinCmd::SetDividend { .. } => 0x12,
            FinCmd::Bankruptcy { .. } => 0x13,
            FinCmd::BondIssue { .. } => 0x14,
            FinCmd::BondRepay { .. } => 0x15,
            FinCmd::MergerPropose { .. } => 0x22,
            FinCmd::MergerVote { .. } => 0x23,
            FinCmd::ElectionStart { .. } => 0x24,
            FinCmd::ElectionVote { .. } => 0x25,
            FinCmd::ChairmanTakeover { .. } => 0x2e,
        }
    }
}

/// Why a financial command was refused. Every variant names the message the original's window
/// shows (LNG id), except the ones the executor itself cannot reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinError {
    /// A company or person id is not live (the only check of `cmd_execute`).
    NoSuchObject,
    /// The scenario forbids the action (permission byte off, LNG 23).
    NotAllowed,
    /// Stock issue: the new issue would sell below US$ 5 (LNG 276, shows the price).
    UnderwritersRefuse { price: i32 },
    /// Stock issue: a stock issue happened less than a year ago (LNG 278).
    IssuedRecently,
    /// Buy-back: there are no publicly held shares (LNG 284).
    NoFloat,
    /// Buy-back: the last `n` shares cannot be bought back (LNG 2705).
    LastShares { n: i32 },
    /// Buy-back: not enough cash for the lot (LNG 360).
    NotEnoughCashBuyback,
    /// Bond issue: 20 bonds are outstanding (LNG 289).
    TooManyBonds,
    /// Bond issue: the credit rating is below the minimum (LNG 290; `have` and `need` are the
    /// rating levels 0..=10).
    RatingTooLow { have: i32, need: i32 },
    /// Bond repayment: cash below principal x 1.02 (LNG 361).
    NotEnoughCashRepay,
    /// Dividend: the board refuses to raise it (LNG 2370).
    DividendRefused,
    /// Bankruptcy: declared less than 5 years ago (LNG 297; the year allowed again).
    BankruptRecently { allowed_from: i32 },
    /// Bankruptcy: the company has not lost money for two full years (LNG 3412).
    BankruptJudge,
    /// Manager hire: the manager is employed elsewhere (`0x438780` does nothing).
    ManagerTaken,
    /// Manager hire: the company cannot pay the signing bonus (LNG 293).
    NotEnoughCashManager,
    /// Stock trade: the purchase is refused (`person_can_buy` `0x4a6f60`).
    CannotBuy,
    /// Stock trade: the sale is refused (`person_can_sell` `0x4a7190`).
    CannotSell,
    /// Merger: fewer than two companies (LNG 387).
    NoOtherCompany,
    /// Merger: the company tried a merger less than a year ago (LNG 388).
    MergerCooldown,
    /// Merger: the buyer cannot pay `shares x offer` (LNG 386).
    NotEnoughCashMerger,
    /// Chairmanship: the person already tried and failed in the last year (LNG 364).
    ElectionCooldown,
    /// The vote record does not exist.
    NoSuchVote,
}

/// What a command did, for the front end (the original shows it in dialogs).
#[derive(Clone, Debug, PartialEq)]
pub enum FinOutcome {
    /// Nothing to report.
    Done,
    /// A stock trade: shares moved (positive bought, negative sold), price per share and the
    /// cash change of the person.
    Trade { shares: i32, price: i32, cash: f64 },
    /// A stock issue (`shares > 0`) or buy-back (`shares < 0`) with the price per share.
    Stock { shares: i32, price: i32 },
    /// A bond was issued at `rate`.
    Bond { rate: f32 },
    /// A merger or chairmanship vote was opened and waits for the humans listed by
    /// [`World::pending_votes`]; `id` is the record id.
    VoteOpen { id: u32 },
    /// The vote ran (every shareholder had answered). `passed` and the tally.
    VoteResult { passed: bool, tally: VoteTally },
}

/// A merger proposal record (`0x574ca0`, finance.md 15): votes are indexed by the position of the
/// person in the person list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergerVoteRec {
    pub id: u32,
    pub buyer: u16,
    pub target: u16,
    /// Offer in US$ per share.
    pub offer: u16,
    pub answered: Vec<bool>,
    pub vote: Vec<bool>,
}

/// A chairmanship contest record (`0x574ca4`, finance.md 15.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElectionRec {
    pub id: u32,
    pub company: u16,
    pub challenger: u16,
    pub answered: Vec<bool>,
    pub vote: Vec<bool>,
}

/// The open merger and election records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Votes {
    pub mergers: Vec<MergerVoteRec>,
    pub elections: Vec<ElectionRec>,
}

impl Votes {
    /// Lowest free id (1-based, as the record pool hands out slots).
    pub fn next_merger_id(&self) -> u32 {
        (1..)
            .find(|i| self.mergers.iter().all(|m| m.id != *i))
            .unwrap()
    }

    pub fn next_election_id(&self) -> u32 {
        (1..)
            .find(|i| self.elections.iter().all(|m| m.id != *i))
            .unwrap()
    }
}

/// One vote that waits for a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingVote {
    /// `true` for a merger, `false` for a chairmanship contest.
    pub merger: bool,
    pub id: u32,
    pub person: i32,
    /// Merger: the buyer company; election: the company whose chair is contested.
    pub company: i32,
    /// Merger: the target company; election: the challenger person.
    pub other: i32,
    /// Merger: the offer per share.
    pub offer: i32,
    /// Shares the person holds of the target (merger) or of the company (election).
    pub shares: i32,
}

/// `0x49d..`: the step of the dividend buttons of `0x449300` in hundredths of a dollar:
/// `max(5, ftol((d * 100 / 6 + 2.5) * 0.2) * 5)` for the current dividend `d`.
pub fn dividend_step(d: f32) -> f32 {
    let x = ((f64::from(d) * f64::from(100.0f32)) * 0.166_666_666_666_666_66 + 2.5) * 0.2;
    let step = (crate::fp::ftol(x) * 5) as f32;
    if step <= 5.0 { 5.0 } else { step }
}

/// A click on "+" (`up`) or "-" of the dividend dialog (`0x449300`, ids `0x693`/`0x694`):
/// the new dividend, or `None` when the board refuses to raise it (LNG 2370). `cap` is the board's
/// limit ([`World::max_dividend_of`]); a dividend already above it keeps its own value as the cap.
pub fn dividend_click(current: f32, cap: f32, up: bool) -> Option<f32> {
    let step = dividend_step(current);
    if up {
        let cap = if cap <= current { current } else { cap };
        // `0x535778` = 0.0100... : the cap must exceed the current value by a cent.
        if f64::from(cap) <= f64::from(current) + f64::from_bits(0x3f84_4673_81d7_dbf5) {
            return None;
        }
        let v = (f64::from(crate::fp::ftol(
            f64::from(current) * 100.0 + f64::from(step) + 0.5,
        )) * 0.01) as f32;
        Some(if cap <= v { cap } else { v })
    } else {
        let v = (f64::from(crate::fp::ftol(
            f64::from(current) * 100.0 - f64::from(step) + 0.5,
        )) * 0.01) as f32;
        Some(if v < 0.0 { 0.0 } else { v })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_fields_follow_commands_md() {
        // 0x14: company 5, principal 500000, term 30 years.
        let mut p = vec![0u8; 0x12];
        p[0] = 0x14;
        p[0xc] = 5;
        p[0xd..0x11].copy_from_slice(&500_000i32.to_le_bytes());
        p[0x11] = 30;
        assert_eq!(
            FinCmd::from_packet(&p),
            Some(FinCmd::BondIssue {
                company: 5,
                principal: 500_000,
                term: 30
            })
        );
        // 0x0e: sale (+0xe = 0xff), short flag set.
        let q = [0x0e, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 0xff, 1];
        assert_eq!(
            FinCmd::from_packet(&q),
            Some(FinCmd::StockTrade {
                person: 2,
                company: 3,
                buy: false,
                flag: true
            })
        );
        assert_eq!(FinCmd::from_packet(&[0x16, 0, 0, 0]), None);
    }

    #[test]
    fn dividend_buttons_step_and_refuse() {
        assert_eq!(dividend_step(0.0), 5.0);
        // 3.00 -> (300/6 + 2.5) * 0.2 = 10.5 -> 10 -> 50 hundredths
        assert_eq!(dividend_step(3.0), 50.0);
        // 0.50 -> (50/6 + 2.5) * 0.2 = 2.17 -> 2 -> 10 hundredths
        assert_eq!(dividend_step(0.5), 10.0);
        assert_eq!(dividend_click(0.5, 2.0, true), Some(0.6));
        assert_eq!(dividend_click(0.5, 2.0, false), Some(0.4));
        assert_eq!(dividend_click(0.0, 2.0, true), Some(0.05));
        assert_eq!(dividend_click(0.02, 2.0, false), Some(0.0));
        // at the cap: refused
        assert_eq!(dividend_click(1.0, 1.0, true), None);
        assert_eq!(dividend_click(1.0, 0.5, true), None);
        // capped to the board's limit
        assert_eq!(dividend_click(0.97, 1.0, true), Some(1.0));
    }
}

// ---- quotes the windows show -----------------------------------------------------------------

/// The stock issue window (`0x447730`): the lot, the quoted price and the amounts of the dialog.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IssueQuote {
    /// `max(1000, shares / 20000 * 1000)`: 5% of the shares.
    pub lot: i32,
    /// `ftol(trade_price(ftol(-0.7 lot)))`.
    pub price: i32,
    /// Underwriting fee rate (`0x535470`, 5%).
    pub fee_rate: f32,
    /// Gross proceeds `lot * price`.
    pub gross: i32,
    /// Fee `gross * 0.05`.
    pub fee: i32,
    /// Why the window refuses, if it does.
    pub refused: Option<FinError>,
}

/// The buy-back window (`0x447bb0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuybackQuote {
    pub lot: i32,
    pub price: i32,
    /// Brokerage rate (`0x5706c9`, 3%).
    pub fee_rate: f32,
    pub gross: i32,
    pub fee: i32,
    pub refused: Option<FinError>,
}

/// The bond window (`0x448250`): the offered bond.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BondQuote {
    /// The window offers 500 000 for 30 years only (commands.md `0x14`).
    pub principal: i32,
    pub term: i32,
    /// `prime + (10 - rating(principal)) * 1%` (`0x4392e0`).
    pub rate: f32,
    /// Credit rating level 0..=10 (0 = F, 10 = AAA) now, and the minimum to issue.
    pub rating: i32,
    pub min_rating: i32,
    pub bonds: usize,
    pub refused: Option<FinError>,
}

/// One outstanding bond with what repaying it takes (`0x4480f0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BondLine {
    pub principal: i32,
    pub maturity_year: i32,
    pub rate: f32,
    /// Prepayment fee, 2% of the principal (LNG 288).
    pub fee: i32,
    pub refused: Option<FinError>,
}

/// The dividend dialog (`0x449300`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DividendInfo {
    pub current: f32,
    /// The board's cap (`0x43edf0`); the dialog never offers less than the current value.
    pub cap: f32,
    pub shares: i32,
    pub permitted: bool,
}

/// What the stock windows show for a person and a company (`stockdtl.win`, `0x4a6e00`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TradeQuote {
    /// Last traded price (`+0x427e`) and the quote of the next share.
    pub last_price: f32,
    pub price: i32,
    pub held: i32,
    pub shares: i32,
    pub free_float: i32,
    pub cash: f64,
    /// Buying power `cash + weighted positions` (`0x4a7a40`).
    pub power: f64,
    /// Lots the original would trade (`0x4a6e00`).
    pub buy_lot: i32,
    pub sell_lot: i32,
    pub can_buy: bool,
    pub can_buy_margin: bool,
    pub can_sell: bool,
    pub can_short: bool,
}

/// A manager of the table `0x54fdad` as the hire dialog shows him.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagerInfo {
    pub index: i32,
    pub salary: i32,
    /// Signing bonus the hire costs (`finance.md` 14.3).
    pub fee: i32,
    /// Employed by a company.
    pub employed: bool,
    /// `(effect id, percent)` pairs.
    pub effects: Vec<(u8, i8)>,
}

impl World {
    fn sim_ref(&self) -> Option<&crate::tick::SimState> {
        self.sim.as_deref()
    }

    /// The stock issue window's numbers for `company`.
    pub fn fin_issue_quote(&self, company: i32) -> Option<IssueQuote> {
        self.sim_ref()?.fc_issue_quote(self.minutes, company, self)
    }

    /// The buy-back window's numbers for `company`.
    pub fn fin_buyback_quote(&self, company: i32) -> Option<BuybackQuote> {
        self.sim_ref()?.fc_buyback_quote(self.minutes, company)
    }

    /// The bond window's numbers for `company`.
    pub fn fin_bond_quote(&self, company: i32) -> Option<BondQuote> {
        self.sim_ref()?.fc_bond_quote(self.minutes, company)
    }

    /// The outstanding bonds of `company` with their repayment fee and the window's refusal.
    pub fn fin_bond_lines(&self, company: i32) -> Vec<BondLine> {
        self.sim_ref()
            .map_or_else(Vec::new, |s| s.fc_bond_lines(self.minutes, company))
    }

    /// The dividend dialog's numbers for `company`.
    pub fn fin_dividend(&self, company: i32) -> Option<DividendInfo> {
        self.sim_ref()?.fc_dividend_info(self.minutes, company)
    }

    /// The stock window's numbers for `person` and `company`.
    pub fn fin_trade_quote(&self, person: i32, company: i32) -> Option<TradeQuote> {
        self.sim_ref()?
            .fc_trade_quote(self.minutes, person, company)
    }

    /// The hire dialog's line of manager `index` for `company` (`None` for a bad index).
    pub fn fin_manager(&self, company: i32, index: i32) -> Option<ManagerInfo> {
        self.sim_ref()?.fc_manager_info(company, index)
    }

    /// Whether the company book offers "Assume chairmanship" (`0x443ab0`): the finance model is
    /// advanced, the scenario allows it and the person holds more than half the shares (or the
    /// editor is on). `contest` is the election button instead (at least 10%).
    pub fn fin_chairman_button(&self, person: i32, company: i32) -> (bool, bool) {
        self.sim_ref()
            .map_or((false, false), |s| s.fc_chairman_buttons(person, company))
    }
}
