//! The player's financial actions as dialogs on the eagle frame (docs/spec/commands.md 2.1,
//! finance.md 3.4, 4.4, 6, 7, 14, 15): issue and repay bonds, issue and buy back stock, the
//! dividend, the merger, bankruptcy, the managers, the chairman's block and the personal trading
//! of shares, plus the vote the shareholder is asked to cast.
//!
//! Every number comes from `rt2_sim::World` (`fin_*` quotes) and every change goes through
//! [`rt2_sim::fincmd::FinCmd`] with `World::apply_fin`, which runs the checks of the original's
//! windows first: a refusal (`FinError`) is shown with the LNG text of that window. The texts are
//! the original's (`default2.lng`) except the few the clone adds (`app_strings`).

use rt2_sim::fincmd::{FinCmd, FinError, FinOutcome, PendingVote, dividend_click};
use rt2_sim::message::{GameMessage, MarginTradeKind, VoteSide, VoteTally};
use rt2_ui::{Kind, Ui, game_font};

use super::super::{AppState, Screen};
use super::{Act, Dialog, E_TEXT, Message, at};
use crate::info::money;
use crate::text::{id as lng, subst};

pub(in crate::state) const A_FIN_OK: u32 = 6000;
pub(in crate::state) const A_FIN_NO: u32 = 6001;
pub(in crate::state) const A_FIN_MINUS: u32 = 6002;
pub(in crate::state) const A_FIN_PLUS: u32 = 6003;
pub(in crate::state) const A_FIN_MINUS10: u32 = 6004;
pub(in crate::state) const A_FIN_PLUS10: u32 = 6005;
pub(in crate::state) const A_FIN_PREV: u32 = 6006;
pub(in crate::state) const A_FIN_NEXT: u32 = 6007;
pub(in crate::state) const A_FIN_BUY: u32 = 6008;
pub(in crate::state) const A_FIN_MARGIN: u32 = 6009;
pub(in crate::state) const A_FIN_SELL: u32 = 6010;
pub(in crate::state) const A_FIN_SHORT: u32 = 6011;
/// The buttons of the company book (`info_ui`): one id per action.
pub(in crate::state) const A_FINACT: u32 = 6100;

/// The actions of the company book's finance buttons, in button order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::state) enum FinAct {
    Bond,
    Repay,
    Stock,
    Buyback,
    Dividend,
    Merger,
    Bankrupt,
    Manager,
    Resign,
    Assume,
    Election,
    Trade,
}

const ACTS: [FinAct; 12] = [
    FinAct::Bond,
    FinAct::Repay,
    FinAct::Stock,
    FinAct::Buyback,
    FinAct::Dividend,
    FinAct::Merger,
    FinAct::Bankrupt,
    FinAct::Manager,
    FinAct::Resign,
    FinAct::Assume,
    FinAct::Election,
    FinAct::Trade,
];

impl FinAct {
    pub(in crate::state) fn from_name(n: &str) -> Option<FinAct> {
        Some(match n {
            "bond" => FinAct::Bond,
            "repay" => FinAct::Repay,
            "stock" => FinAct::Stock,
            "buyback" => FinAct::Buyback,
            "dividend" => FinAct::Dividend,
            "merger" => FinAct::Merger,
            "bankrupt" => FinAct::Bankrupt,
            "manager" => FinAct::Manager,
            "resign" => FinAct::Resign,
            "assume" => FinAct::Assume,
            "election" => FinAct::Election,
            "trade" => FinAct::Trade,
            _ => return None,
        })
    }
}

/// The open financial dialog.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::state) enum FinDlg {
    Bond {
        company: i32,
    },
    Stock {
        company: i32,
    },
    Buyback {
        company: i32,
    },
    Dividend {
        company: i32,
        value: f32,
    },
    Merger {
        company: i32,
        target: i32,
        offer: i32,
    },
    Bankrupt {
        company: i32,
    },
    Repay {
        company: i32,
        bond: usize,
    },
    Resign {
        company: i32,
    },
    Assume {
        company: i32,
    },
    Election {
        company: i32,
    },
    Manager {
        company: i32,
        pick: usize,
    },
    Trade {
        company: i32,
    },
    /// Margin purchase (`short == false`) or short sale: the confirmation of LNG 2262 / 2263.
    TradeConfirm {
        company: i32,
        short: bool,
    },
    Vote(PendingVote),
}

/// What a dialog shows: title, body lines and the buttons `(text, action, enabled)`.
struct View {
    title: String,
    lines: Vec<String>,
    buttons: Vec<(String, u32, bool)>,
}

fn clean(s: &str) -> String {
    s.replace("\\n", "\n").trim().to_owned()
}

/// Replaces each `%s` in turn (the `printf` markers of LNG 2270..2272).
fn subst_s(template: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut it = args.iter();
    let mut rest = template;
    while let Some(i) = rest.find("%s") {
        out.push_str(&rest[..i]);
        out.push_str(it.next().map_or("", String::as_str));
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

fn int(v: i64) -> String {
    money(v as f64).replace('$', "")
}

impl AppState {
    // ---- data -------------------------------------------------------------------------------

    fn fin_world(&self) -> Option<&rt2_sim::World> {
        self.game().map(|g| &g.world)
    }

    /// The local human person (the first human, as `Info::founder`).
    pub(in crate::state) fn human_person(&self) -> Option<i32> {
        let save = self.fin_world()?.save_ref()?;
        crate::info::founder(save)
            .filter(|f| f.human)
            .map(|f| f.person)
    }

    fn cname(&self, c: i32) -> String {
        self.with_info(|i| i.company_name(c.max(0) as u32))
            .unwrap_or_default()
    }

    fn pname(&self, p: i32) -> String {
        self.with_info(|i| i.person_name(p.max(0) as u32))
            .unwrap_or_default()
    }

    fn crec(&self, c: i32) -> Option<Vec<u8>> {
        let save = self.fin_world()?.save_ref()?;
        save.companies
            .get(c.max(0) as usize)
            .map(|r| r.data.clone())
    }

    fn crec_i32(&self, c: i32, o: usize) -> i32 {
        self.crec(c).map_or(0, |d| {
            i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
        })
    }

    fn crec_f64(&self, c: i32, o: usize) -> f64 {
        self.crec(c).map_or(0.0, |d| {
            f64::from_le_bytes(d[o..o + 8].try_into().unwrap_or([0; 8]))
        })
    }

    /// The other active companies, in list order (the merger's targets).
    fn merger_targets(&self, buyer: i32) -> Vec<i32> {
        let Some(save) = self.fin_world().and_then(|w| w.save_ref()) else {
            return Vec::new();
        };
        save.companies
            .ordered()
            .filter(|r| {
                r.slot as i32 != buyer
                    && r.data.get(0x16e..0x172).is_some_and(|b| b != [0, 0, 0, 0])
            })
            .map(|r| r.slot as i32)
            .collect()
    }

    fn rating_name(&self, level: i32) -> String {
        self.texts
            .get(1020 + level.clamp(0, 10) as u32, "")
            .trim()
            .to_owned()
    }

    /// The window's text for a refusal.
    pub(in crate::state) fn fin_error_text(&self, e: FinError) -> String {
        let t = &self.texts;
        let s = match e {
            FinError::NotAllowed => t.get(23, "That action is not allowed in this scenario."),
            FinError::UnderwritersRefuse { price } => subst(
                &t.get(276, "The price of the new stock issue would be %1."),
                &[money(f64::from(price))],
            ),
            FinError::IssuedRecently => {
                t.get(278, "You've already issued stock within the last year.")
            }
            FinError::NoFloat => t.get(284, "There are no publicly held shares available."),
            FinError::LastShares { n } => subst(
                &t.get(2705, "You can't buy back the last %1 shares."),
                &[int(i64::from(n))],
            ),
            FinError::NotEnoughCashBuyback => t.get(360, "The company does not have enough money."),
            FinError::TooManyBonds => subst(
                &t.get(289, "You have %1 bonds outstanding."),
                &[rt2_sim::finance::MAX_BONDS.to_string()],
            ),
            FinError::RatingTooLow { have, need } => subst(
                &t.get(290, "Your credit rating is '%1'; it must be at least '%2'."),
                &[self.rating_name(have), self.rating_name(need)],
            ),
            FinError::NotEnoughCashRepay => {
                t.get(361, "You don't have enough cash to repay this bond.")
            }
            FinError::DividendRefused => t.get(
                2370,
                "The board refuses to let you raise the dividend rate.",
            ),
            FinError::BankruptRecently { allowed_from } => subst(
                &t.get(
                    297,
                    "You declared bankruptcy in %1.  You may not declare it again until %2.",
                ),
                &[(allowed_from - 5).to_string(), allowed_from.to_string()],
            ),
            FinError::BankruptJudge => t.get(
                3412,
                "The bankruptcy judge won't let you declare bankruptcy.",
            ),
            FinError::ManagerTaken => t.a("That manager already works for another company."),
            FinError::NotEnoughCashManager => t.get(293, "Your company doesn't have enough money."),
            FinError::CannotBuy => t.a("You cannot buy shares of this company now."),
            FinError::CannotSell => t.a("You cannot sell shares of this company now."),
            FinError::NoOtherCompany => t.get(387, "There are no other companies to merge with!"),
            FinError::MergerCooldown => t.get(
                388,
                "Your company has already made one unsuccessful merger attempt recently.",
            ),
            FinError::NotEnoughCashMerger => {
                t.get(386, "Your company doesn't have that much money!")
            }
            FinError::ElectionCooldown => t.get(
                364,
                "You've already tried unsuccessfully to change jobs once in the last year.",
            ),
            FinError::NoSuchObject | FinError::NoSuchVote => t.a("That object no longer exists."),
        };
        clean(&s)
    }

    fn fin_message(&mut self, body: String) {
        let ok = self.texts.get(lng::OK, "OK");
        self.push_dialog(Dialog::Message(Message {
            title: String::new(),
            body,
            buttons: vec![(ok, Act::Close)],
        }));
    }

    // ---- opening ----------------------------------------------------------------------------

    /// The buttons of the company book for `slot`: `(label, action id)`.
    pub(in crate::state) fn fin_buttons(&self, slot: u32) -> Vec<(String, u32)> {
        let (Some(w), Some(person)) = (self.fin_world(), self.human_person()) else {
            return Vec::new();
        };
        let c = slot as i32;
        let t = &self.texts;
        let own = self.player() == Some(c);
        let mut out: Vec<FinAct> = Vec::new();
        if own {
            out.push(FinAct::Bond);
            if self.crec_i32(c, 0x40) > 0 {
                out.push(FinAct::Repay);
            }
            out.extend([
                FinAct::Stock,
                FinAct::Buyback,
                FinAct::Dividend,
                FinAct::Merger,
                FinAct::Bankrupt,
                FinAct::Manager,
                FinAct::Resign,
            ]);
        } else {
            let (assume, election) = w.fin_chairman_button(person, c);
            out.push(FinAct::Trade);
            if assume {
                out.push(FinAct::Assume);
            }
            if election {
                out.push(FinAct::Election);
            }
        }
        out.into_iter()
            .map(|a| {
                let text = match a {
                    FinAct::Bond => t.get(313, "Issue Bonds"),
                    FinAct::Repay => t.a("Repay Bond"),
                    FinAct::Stock => t.get(315, "Issue Stock"),
                    FinAct::Buyback => t.get(317, "Buy Back Stock"),
                    FinAct::Dividend => t.get(319, "Change Dividend"),
                    FinAct::Merger => t.get(321, "Attempt Merger"),
                    FinAct::Bankrupt => t.get(323, "Declare Bankruptcy"),
                    FinAct::Manager => t.get(325, "View/Hire Other Manager"),
                    FinAct::Resign => t.get(327, "Resign as chairman"),
                    FinAct::Assume => t.get(329, "Assume chairmanship"),
                    FinAct::Election => t.get(331, "Call for special chairman election"),
                    FinAct::Trade => t.a("Trade Shares"),
                };
                let id = A_FINACT + ACTS.iter().position(|&x| x == a).unwrap_or(0) as u32;
                (clean(&text), id)
            })
            .collect()
    }

    /// A finance button of the company book (or a screenshot name): the window's checks, then
    /// the dialog or the refusal.
    pub(in crate::state) fn fin_act(&mut self, act: FinAct, company: i32) {
        let Some(w) = self.fin_world() else { return };
        let Some(person) = self.human_person() else {
            return;
        };
        let refusal: Option<FinError> = match act {
            FinAct::Bond => w.fin_bond_quote(company).and_then(|q| q.refused),
            FinAct::Stock => w.fin_issue_quote(company).and_then(|q| q.refused),
            FinAct::Buyback => w.fin_buyback_quote(company).and_then(|q| q.refused),
            FinAct::Dividend => w
                .fin_dividend(company)
                .filter(|d| !d.permitted)
                .map(|_| FinError::NotAllowed),
            FinAct::Merger => {
                let target = self.merger_targets(company).first().copied().unwrap_or(0);
                w.check_fin(&FinCmd::MergerPropose {
                    buyer: company,
                    target,
                    offer: 0,
                })
                .err()
                .filter(|e| {
                    matches!(
                        e,
                        FinError::NotAllowed | FinError::NoOtherCompany | FinError::MergerCooldown
                    )
                })
                .or_else(|| {
                    self.merger_targets(company)
                        .is_empty()
                        .then_some(FinError::NoOtherCompany)
                })
            }
            FinAct::Bankrupt => w.check_fin(&FinCmd::Bankruptcy { company }).err(),
            FinAct::Resign => w
                .check_fin(&FinCmd::ResignChairman { person, company })
                .err(),
            FinAct::Assume => w
                .check_fin(&FinCmd::ChairmanTakeover { person, company })
                .err(),
            FinAct::Election => w
                .check_fin(&FinCmd::ElectionStart {
                    company,
                    challenger: person,
                })
                .err(),
            FinAct::Repay | FinAct::Manager | FinAct::Trade => None,
        };
        if let Some(e) = refusal {
            let text = self.fin_error_text(e);
            return self.fin_message(text);
        }
        let d = match act {
            FinAct::Bond => FinDlg::Bond { company },
            FinAct::Stock => FinDlg::Stock { company },
            FinAct::Buyback => FinDlg::Buyback { company },
            FinAct::Dividend => FinDlg::Dividend {
                company,
                value: w.fin_dividend(company).map_or(0.0, |d| d.current),
            },
            FinAct::Merger => {
                let target = self.merger_targets(company).first().copied().unwrap_or(0);
                let base = w
                    .fin_trade_quote(person, target)
                    .map_or(1, |q| q.price.max(1));
                FinDlg::Merger {
                    company,
                    target,
                    offer: base + 1,
                }
            }
            FinAct::Bankrupt => FinDlg::Bankrupt { company },
            FinAct::Repay => {
                if w.fin_bond_lines(company).is_empty() {
                    let text = self.texts.a("You have no bonds outstanding.");
                    return self.fin_message(text);
                }
                FinDlg::Repay { company, bond: 0 }
            }
            FinAct::Resign => FinDlg::Resign { company },
            FinAct::Assume => FinDlg::Assume { company },
            FinAct::Election => FinDlg::Election { company },
            FinAct::Manager => FinDlg::Manager { company, pick: 0 },
            FinAct::Trade => FinDlg::Trade { company },
        };
        self.push_dialog(Dialog::Fin(d));
    }

    /// The action ids of the company book's buttons.
    pub(in crate::state) fn fin_book_activated(&mut self, action: u32, company: u32) -> bool {
        let Some(&act) = ACTS.get(action.wrapping_sub(A_FINACT) as usize) else {
            return false;
        };
        let c = if company == 0 {
            self.player().unwrap_or(0)
        } else {
            company as i32
        };
        self.fin_act(act, c);
        true
    }

    /// Screenshots: a financial dialog by name (`fin-bond`, ...) for the player's company, or the
    /// first other company for the ones that make no sense on one's own (`fin-trade`,
    /// `fin-assume`, `fin-election`).
    pub(in crate::state) fn show_fin_dialog(&mut self, name: &str) -> bool {
        let Some(n) = name.strip_prefix("fin-") else {
            return false;
        };
        let Some(own) = self.player() else {
            return false;
        };
        let other = self.merger_targets(own).first().copied().unwrap_or(own);
        if n == "vote" {
            let Some(person) = self.human_person() else {
                return false;
            };
            self.push_dialog(Dialog::Fin(FinDlg::Vote(PendingVote {
                merger: true,
                id: 1,
                person,
                company: other,
                other: own,
                offer: 80,
                shares: self
                    .fin_world()
                    .map_or(0, |w| w.fin_trade_quote(person, own).map_or(0, |q| q.held)),
            })));
            return true;
        }
        let Some(act) = FinAct::from_name(n) else {
            return false;
        };
        let company = if matches!(act, FinAct::Trade | FinAct::Assume | FinAct::Election) {
            other
        } else {
            own
        };
        self.fin_act(act, company);
        // A refusal opens a message: the shot shows it, as the player would see it.
        true
    }

    // ---- views ------------------------------------------------------------------------------

    fn ok_cancel(&self, ok: &str) -> Vec<(String, u32, bool)> {
        vec![
            (ok.to_owned(), A_FIN_OK, true),
            (
                self.texts.get(lng::CANCEL, "Cancel"),
                super::A_DLG_CLOSE,
                true,
            ),
        ]
    }

    fn fin_view(&self, d: &FinDlg) -> Option<View> {
        let t = &self.texts;
        let w = self.fin_world()?;
        let ok = t.get(lng::OK, "OK");
        let person = self.human_person()?;
        Some(match d {
            FinDlg::Bond { company } => {
                let q = w.fin_bond_quote(*company)?;
                let year = i32::from(w.date().year) + q.term;
                View {
                    title: self.cname(*company),
                    lines: vec![
                        clean(&t.get(291, "Underwriters offer to issue the following bond:")),
                        String::new(),
                        format!(
                            "{}: {}",
                            t.get(292, "Amount"),
                            money(f64::from(q.principal))
                        ),
                        format!(
                            "{}: {:.1}%",
                            t.get(60, "Interest Rate"),
                            f64::from(q.rate) * 100.0
                        ),
                        format!("{}: {year}", t.get(58, "Year Matures")),
                        String::new(),
                        clean(&t.get(121, "Go ahead and issue the bond?")),
                    ],
                    buttons: self.ok_cancel(&ok),
                }
            }
            FinDlg::Stock { company } => {
                let q = w.fin_issue_quote(*company)?;
                let net = (500 + (q.gross - q.fee)) / 1000 * 1000;
                View {
                    title: self.cname(*company),
                    lines: vec![
                        clean(&t.get(279, "Underwriters offer to issue stock as follows:")),
                        String::new(),
                        format!("{}: {}", t.get(280, "Shares"), int(i64::from(q.lot))),
                        format!("{}: {}", t.get(281, "Price"), money(f64::from(q.price))),
                        format!(
                            "{}: {:.1}%",
                            t.get(282, "Underwriting Fees"),
                            f64::from(q.fee_rate) * 100.0
                        ),
                        format!(
                            "{}: {}",
                            t.get(283, "Total Proceeds"),
                            money(f64::from(net))
                        ),
                        String::new(),
                        clean(&t.get(119, "Go ahead and issue the stock?")),
                    ],
                    buttons: self.ok_cancel(&ok),
                }
            }
            FinDlg::Buyback { company } => {
                let q = w.fin_buyback_quote(*company)?;
                View {
                    title: self.cname(*company),
                    lines: vec![
                        clean(&t.get(
                            285,
                            "Brokers are able to locate the following shares on the market:",
                        )),
                        String::new(),
                        format!("{}: {}", t.get(280, "Shares"), int(i64::from(q.lot))),
                        format!("{}: {}", t.get(281, "Price"), money(f64::from(q.price))),
                        format!(
                            "{}: {:.1}%",
                            t.get(286, "Brokerage Fees"),
                            f64::from(q.fee_rate) * 100.0
                        ),
                        format!(
                            "{}: {}",
                            t.get(287, "Total Cost"),
                            money(f64::from(q.gross) + f64::from(q.fee))
                        ),
                        String::new(),
                        clean(&t.get(120, "Go ahead and buy back the stock?")),
                    ],
                    buttons: self.ok_cancel(&ok),
                }
            }
            FinDlg::Dividend { company, value } => {
                let info = w.fin_dividend(*company)?;
                View {
                    title: clean(&t.get(296, "Change Dividend Rate")),
                    lines: vec![
                        self.cname(*company),
                        String::new(),
                        format!("{}: ${:.2}", t.get(294, "Dividend Rate"), value),
                        format!(
                            "{}: {}",
                            t.get(295, "Total Annual Dividends"),
                            money(f64::from(info.shares) * f64::from(*value))
                        ),
                        format!("({}: ${:.2})", t.a("Board's limit"), info.cap),
                    ],
                    buttons: vec![
                        ("-".into(), A_FIN_MINUS, true),
                        ("+".into(), A_FIN_PLUS, true),
                        (ok, A_FIN_OK, true),
                        (t.get(lng::CANCEL, "Cancel"), super::A_DLG_CLOSE, true),
                    ],
                }
            }
            FinDlg::Merger {
                company,
                target,
                offer,
            } => {
                let targets = self.merger_targets(*company);
                let k = targets.iter().position(|c| c == target).unwrap_or(0);
                let q = w.fin_trade_quote(person, *target)?;
                let total = f64::from(q.shares) * f64::from(*offer);
                let cash = self.crec_f64(*company, 0x4766);
                View {
                    title: clean(&t.get(321, "Attempt Merger")),
                    lines: vec![
                        format!(
                            "{}: {} ({})",
                            t.get(529, "Target Company"),
                            self.cname(*target),
                            t.af_pair("%1 of %2", k + 1, targets.len())
                        ),
                        format!(
                            "{}: {}",
                            t.get(530, "Offer Price"),
                            money(f64::from(*offer))
                        ),
                        String::new(),
                        format!(
                            "{}: {}",
                            t.get(159, "Price per share:").trim_end_matches(':'),
                            money(f64::from(*offer))
                        ),
                        format!(
                            "{}: {}",
                            t.get(160, "Shares Outstanding:").trim_end_matches(':'),
                            int(i64::from(q.shares))
                        ),
                        format!(
                            "{}: {}",
                            t.get(161, "Total Price:").trim_end_matches(':'),
                            money(total)
                        ),
                        subst(&t.get(163, "Funds Available %1"), &[money(cash)]),
                        format!("({}: {})", t.get(43, "Price"), money(f64::from(q.price))),
                    ],
                    buttons: vec![
                        ("<".into(), A_FIN_PREV, targets.len() > 1),
                        (">".into(), A_FIN_NEXT, targets.len() > 1),
                        ("-1".into(), A_FIN_MINUS, true),
                        ("+1".into(), A_FIN_PLUS, true),
                        ("-10".into(), A_FIN_MINUS10, true),
                        ("+10".into(), A_FIN_PLUS10, true),
                        (ok, A_FIN_OK, true),
                        (t.get(lng::CANCEL, "Cancel"), super::A_DLG_CLOSE, true),
                    ],
                }
            }
            FinDlg::Bankrupt { company } => View {
                title: self.cname(*company),
                lines: vec![clean(&t.get(
                    366,
                    "If you declare bankruptcy, half of all of your debts will be nullified.",
                ))],
                buttons: self.ok_cancel(&ok),
            },
            FinDlg::Repay { company, bond } => {
                let lines = w.fin_bond_lines(*company);
                let b = lines.get(*bond)?;
                View {
                    title: t.a("Repay Bond"),
                    lines: vec![
                        format!(
                            "{}  {} / {} / {:.1}%",
                            t.af_pair("%1 of %2", bond + 1, lines.len()),
                            money(f64::from(b.principal)),
                            b.maturity_year,
                            f64::from(b.rate) * 100.0
                        ),
                        String::new(),
                        subst(
                            &clean(&t.get(288, "You will have to pay a prepayment fee of %1, in addition to the principal amount of %2.  Go ahead and repay this bond?")),
                            &[money(f64::from(b.fee)), money(f64::from(b.principal))],
                        ),
                    ],
                    buttons: vec![
                        ("<".into(), A_FIN_PREV, lines.len() > 1),
                        (">".into(), A_FIN_NEXT, lines.len() > 1),
                        (ok, A_FIN_OK, true),
                        (t.get(lng::CANCEL, "Cancel"), super::A_DLG_CLOSE, true),
                    ],
                }
            }
            FinDlg::Resign { company } => View {
                title: self.cname(*company),
                lines: vec![clean(&t.get(
                    362,
                    "Are you sure you want to resign the chairman's position?",
                ))],
                buttons: self.ok_cancel(&ok),
            },
            FinDlg::Assume { company } => View {
                title: self.cname(*company),
                lines: vec![clean(&t.get(
                    363,
                    "Are you sure you want to assume the chairman's position?",
                ))],
                buttons: self.ok_cancel(&ok),
            },
            FinDlg::Election { company } => View {
                title: self.cname(*company),
                lines: vec![clean(&t.get(
                    365,
                    "Are you sure you want to run for the chairman's position?",
                ))],
                buttons: self.ok_cancel(&ok),
            },
            FinDlg::Manager { company, pick } => {
                let cur = self.crec_i32(*company, 0x13c);
                let cands = [
                    self.crec_i32(*company, 0x140),
                    self.crec_i32(*company, 0x144),
                ];
                let idx = cands.get(*pick).copied().unwrap_or(-1);
                let m = w.fin_manager(*company, idx);
                let mut lines = Vec::new();
                if let Some(c) = w.fin_manager(*company, cur) {
                    lines.push(format!(
                        "{}: {} - {}",
                        t.get(116, "Manager"),
                        clean(&t.get(1456 + 2 * cur as u32, "")),
                        subst(&t.get(113, "Salary %1/year"), &[money(f64::from(c.salary))])
                    ));
                    lines.push(String::new());
                }
                if let Some(m) = &m {
                    lines.push(format!(
                        "{} ({})",
                        clean(&t.get(1456 + 2 * idx as u32, "")),
                        t.af_pair("%1 of %2", pick + 1, 2)
                    ));
                    lines.push(clean(&t.get(1457 + 2 * idx as u32, "")));
                    lines.push(String::new());
                    lines.push(subst(
                        &t.get(113, "Salary %1/year"),
                        &[money(f64::from(m.salary))],
                    ));
                    lines.push(subst(
                        &t.get(224, "Signing Bonus %1"),
                        &[money(f64::from(m.fee))],
                    ));
                } else {
                    lines.push(t.a("That object no longer exists."));
                }
                View {
                    title: t.get(496, "Managers"),
                    lines,
                    buttons: vec![
                        ("<".into(), A_FIN_PREV, true),
                        (">".into(), A_FIN_NEXT, true),
                        (
                            t.get(458, "Hire"),
                            A_FIN_OK,
                            m.as_ref().is_some_and(|m| !m.employed),
                        ),
                        (t.get(lng::CANCEL, "Cancel"), super::A_DLG_CLOSE, true),
                    ],
                }
            }
            FinDlg::Trade { company } => {
                let q = w.fin_trade_quote(person, *company)?;
                let margin = q.can_buy_margin && !q.can_buy;
                let short = q.can_short && !q.can_sell;
                View {
                    title: self.cname(*company),
                    lines: vec![
                        format!("{}: {}", t.get(43, "Price"), money(f64::from(q.price))),
                        format!("{}: {}", t.a("Shares held"), int(i64::from(q.held))),
                        format!("{}: {}", t.get(280, "Shares"), int(i64::from(q.shares))),
                        format!(
                            "{}: {}",
                            t.a("Shares for sale"),
                            int(i64::from(q.free_float))
                        ),
                        String::new(),
                        format!("{}: {}", t.get(31, "Cash"), money(q.cash)),
                        format!("{}: {}", t.get(2261, "Purchasing Power"), money(q.power)),
                        format!(
                            "{}: {}   {}: {}",
                            t.a("Lot to buy"),
                            int(i64::from(q.buy_lot)),
                            t.a("Lot to sell"),
                            int(i64::from(q.sell_lot))
                        ),
                    ],
                    buttons: vec![
                        (clean(&t.get(305, "Buy")), A_FIN_BUY, q.can_buy),
                        (t.a("Buy on Margin"), A_FIN_MARGIN, margin),
                        (clean(&t.get(307, "Sell")), A_FIN_SELL, q.can_sell),
                        (t.a("Short Sell"), A_FIN_SHORT, short),
                        (t.a("Close"), super::A_DLG_CLOSE, true),
                    ],
                }
            }
            FinDlg::TradeConfirm { company, short } => View {
                title: self.cname(*company),
                lines: vec![clean(&if *short {
                    t.get(2263, "Go ahead and short sell the stock?")
                } else {
                    t.get(2262, "Go ahead and buy the stock?")
                })],
                buttons: self.ok_cancel(&ok),
            },
            FinDlg::Vote(v) => {
                let pct = if v.merger {
                    self.crec_i32(v.other, 0x2c)
                } else {
                    self.crec_i32(v.company, 0x2c)
                };
                let pct = if pct > 0 {
                    format!("{:.1}%", f64::from(v.shares) / f64::from(pct) * 100.0)
                } else {
                    String::new()
                };
                let body = if v.merger {
                    subst(
                        &clean(&t.get(164, "Merger Attempt! %1 is attempting to buy %2 for %3 per share. You own %4 shares of %5 (%6 of the company). Do you vote in favor of the merger?")),
                        &[
                            self.cname(v.company),
                            self.cname(v.other),
                            money(f64::from(v.offer)),
                            int(i64::from(v.shares)),
                            self.cname(v.other),
                            pct,
                        ],
                    )
                } else {
                    let chair = self.crec_i32(v.company, 0x23);
                    let chair_name = if chair == 0 {
                        t.get(531, "the shareholder's committee")
                    } else {
                        self.pname(chair)
                    };
                    subst(
                        &clean(&t.get(165, "Battle for Control! %1 is running for chairman of %2, attempting to take control from %3. You own %4 shares of %5 (%6 of the company). Do you support %7's attempt?")),
                        &[
                            self.pname(v.other),
                            self.cname(v.company),
                            chair_name,
                            int(i64::from(v.shares)),
                            self.cname(v.company),
                            pct,
                            self.pname(v.other),
                        ],
                    )
                };
                View {
                    title: String::new(),
                    lines: vec![body],
                    buttons: vec![
                        (clean(&t.get(452, "Yes")), A_FIN_OK, true),
                        (clean(&t.get(453, "No")), A_FIN_NO, true),
                    ],
                }
            }
        })
    }

    // ---- building ---------------------------------------------------------------------------

    pub(in crate::state) fn build_fin(&self, ui: &mut Ui, d: &FinDlg) {
        let Some(v) = self.fin_view(d) else {
            let body = self.texts.a("That object no longer exists.");
            let ok = vec![(self.texts.get(lng::OK, "OK"), Act::Close)];
            self.build_message(ui, "", &body, &ok);
            return;
        };
        let (_, ch) = ui.logical_size();
        let sx0 = ((ch / 620.0).clamp(1.0, 2.0) * 0.95).min(1.6);
        let make_lines = |w: f32, size: f32| {
            let mut lines = Vec::new();
            if !v.title.is_empty() {
                lines.push(v.title.to_uppercase());
                lines.push(String::new());
            }
            for l in &v.lines {
                if l.is_empty() {
                    lines.push(String::new());
                } else {
                    for part in l.split('\n') {
                        if part.is_empty() {
                            lines.push(String::new());
                        } else {
                            lines.extend(Self::body_lines(part, w, size));
                        }
                    }
                }
            }
            lines
        };
        let cols = if v.buttons.len() > 4 { 3 } else { 2 };
        let brows = v.buttons.len().div_ceil(cols);
        let lines0 = make_lines(E_TEXT.2 * sx0, 15.0 * sx0);
        let strips = Self::strips_for(ui, lines0.len() + brows * 3 + 1, 15.0 * sx0 * 1.3, sx0);
        let fr = self.eagle(ui, strips);
        let size = 15.0 * fr.sx;
        let lines = make_lines(fr.text.2, size);
        let row_h = (size * 1.3).round();
        let text_h = lines.len() as f32 * row_h;
        self.text_in(ui, &fr, lines, size);
        let sx = fr.sx;
        let f = game_font(3).with_size((game_font(3).size * sx * 0.8).round().max(10.0));
        let gap = 8.0 * sx;
        let bw = ((fr.text.2 - gap * (cols as f32 - 1.0)) / cols as f32).min(150.0 * sx);
        let bh = 34.0 * sx;
        let x0 = fr.text.0 + (fr.text.2 - bw * cols as f32 - gap * (cols as f32 - 1.0)) / 2.0;
        let y0 = fr.text.1 + text_h + gap;
        for (i, (text, action, enabled)) in v.buttons.iter().enumerate() {
            let (c, r) = ((i % cols) as f32, (i / cols) as f32);
            // Small caps run about 0.66 em per letter: shrink the label to the button.
            let fit = (bw * 0.92 / (0.66 * f.size * text.chars().count().max(1) as f32)).min(1.0);
            let f = f.with_size((f.size * fit).round().max(9.0));
            ui.add(
                fr.root,
                Kind::Button {
                    text: text.clone(),
                    font: f,
                    action: *action,
                    enabled: *enabled,
                    hotkey: None,
                },
                at(x0 + c * (bw + gap), y0 + r * (bh + gap), bw, bh),
            );
        }
    }

    // ---- actions ----------------------------------------------------------------------------

    fn fin_top(&self) -> Option<FinDlg> {
        match self.game()?.dialogs.last()? {
            Dialog::Fin(d) => Some(d.clone()),
            _ => None,
        }
    }

    fn fin_set(&mut self, d: FinDlg) {
        if let Screen::Game(g) = &mut self.screen
            && let Some(top) = g.dialogs.last_mut()
        {
            *top = Dialog::Fin(d);
        }
        self.rebuild_ui();
    }

    /// Sends a command through `World::apply_fin`; the refusal opens a message.
    fn fin_send(&mut self, cmd: FinCmd) -> Option<FinOutcome> {
        let r = match &mut self.screen {
            Screen::Game(g) => g.world.apply_fin(cmd),
            Screen::Menu(_) => return None,
        };
        match r {
            Ok(o) => {
                self.sync_after_build();
                self.poll_votes();
                Some(o)
            }
            Err(e) => {
                let text = self.fin_error_text(e);
                self.fin_message(text);
                None
            }
        }
    }

    fn fin_done(&mut self, text: String) {
        self.close_dialog();
        self.add_news(text.clone());
        self.toast(text);
    }

    /// The buttons of the financial dialogs; true if `action` was one of them.
    pub(in crate::state) fn fin_activated(&mut self, action: u32) -> bool {
        if !(A_FIN_OK..=A_FIN_SHORT).contains(&action) {
            return false;
        }
        let Some(d) = self.fin_top() else {
            return false;
        };
        let Some(person) = self.human_person() else {
            return true;
        };
        match (action, d) {
            (A_FIN_OK, FinDlg::Bond { company }) => {
                let cmd = FinCmd::BondIssue {
                    company,
                    principal: 500_000,
                    term: 30,
                };
                if let Some(FinOutcome::Bond { rate }) = self.fin_send(cmd) {
                    let msg = self.texts.af(
                        "Bond issued at %1.",
                        &[format!("{:.1}%", f64::from(rate) * 100.0)],
                    );
                    self.fin_done(msg);
                }
            }
            (A_FIN_OK, FinDlg::Stock { company }) => {
                let lot = self
                    .fin_world()
                    .and_then(|w| w.fin_issue_quote(company))
                    .map_or(0, |q| q.lot);
                if let Some(FinOutcome::Stock { shares, price }) =
                    self.fin_send(FinCmd::StockIssue { company, n: lot })
                {
                    let msg = self.texts.af(
                        "Issued %1 shares at %2.",
                        &[int(i64::from(shares)), money(f64::from(price))],
                    );
                    self.fin_done(msg);
                }
            }
            (A_FIN_OK, FinDlg::Buyback { company }) => {
                let lot = self
                    .fin_world()
                    .and_then(|w| w.fin_buyback_quote(company))
                    .map_or(0, |q| q.lot);
                if let Some(FinOutcome::Stock { shares, price }) =
                    self.fin_send(FinCmd::StockIssue { company, n: -lot })
                {
                    let msg = self.texts.af(
                        "Bought back %1 shares at %2.",
                        &[int(i64::from(-shares)), money(f64::from(price))],
                    );
                    self.fin_done(msg);
                }
            }
            (A_FIN_MINUS | A_FIN_PLUS, FinDlg::Dividend { company, value }) => {
                let cap = self
                    .fin_world()
                    .and_then(|w| w.fin_dividend(company))
                    .map_or(0.0, |i| i.cap);
                match dividend_click(value, cap, action == A_FIN_PLUS) {
                    Some(v) => self.fin_set(FinDlg::Dividend { company, value: v }),
                    None => {
                        let text = self.fin_error_text(FinError::DividendRefused);
                        self.fin_message(text);
                    }
                }
            }
            (A_FIN_OK, FinDlg::Dividend { company, value }) => {
                if self
                    .fin_send(FinCmd::set_dividend(company, value))
                    .is_some()
                {
                    let msg = self
                        .texts
                        .af("Dividend set to %1 per share.", &[format!("${value:.2}")]);
                    self.fin_done(msg);
                }
            }
            (
                A_FIN_PREV | A_FIN_NEXT,
                FinDlg::Merger {
                    company,
                    target,
                    offer,
                },
            ) => {
                let targets = self.merger_targets(company);
                if targets.is_empty() {
                    return true;
                }
                let k = targets.iter().position(|&c| c == target).unwrap_or(0) as i32;
                let n = targets.len() as i32;
                let k = (k + if action == A_FIN_NEXT { 1 } else { -1 }).rem_euclid(n);
                let target = targets[k as usize];
                let base = self
                    .fin_world()
                    .and_then(|w| w.fin_trade_quote(person, target))
                    .map_or(offer, |q| q.price.max(1) + 1);
                self.fin_set(FinDlg::Merger {
                    company,
                    target,
                    offer: base,
                });
            }
            (
                A_FIN_MINUS | A_FIN_PLUS | A_FIN_MINUS10 | A_FIN_PLUS10,
                FinDlg::Merger {
                    company,
                    target,
                    offer,
                },
            ) => {
                let step = match action {
                    A_FIN_MINUS => -1,
                    A_FIN_PLUS => 1,
                    A_FIN_MINUS10 => -10,
                    _ => 10,
                };
                // TODO: the original's offer slider (`0x48ab20`, widget `0x118`) has a range that
                // is not decoded; the offer moves from US$ 1 to three times the quote.
                let base = self
                    .fin_world()
                    .and_then(|w| w.fin_trade_quote(person, target))
                    .map_or(1, |q| q.price.max(1));
                let offer = (offer + step).clamp(1, base * 3 + 10);
                self.fin_set(FinDlg::Merger {
                    company,
                    target,
                    offer,
                });
            }
            (
                A_FIN_OK,
                FinDlg::Merger {
                    company,
                    target,
                    offer,
                },
            ) => {
                let cmd = FinCmd::MergerPropose {
                    buyer: company,
                    target,
                    offer,
                };
                if self.fin_send(cmd).is_some() {
                    let msg = self.texts.af(
                        "%1 offers %2 per share for %3.",
                        &[
                            self.cname(company),
                            money(f64::from(offer)),
                            self.cname(target),
                        ],
                    );
                    self.fin_done(msg);
                }
            }
            (A_FIN_OK, FinDlg::Bankrupt { company }) => {
                if self.fin_send(FinCmd::Bankruptcy { company }).is_some() {
                    let msg = subst(
                        &self.texts.get(182, "%1 has declared bankruptcy!"),
                        &[self.cname(company)],
                    );
                    self.fin_done(msg);
                }
            }
            (A_FIN_PREV | A_FIN_NEXT, FinDlg::Repay { company, bond }) => {
                let n = self
                    .fin_world()
                    .map_or(0, |w| w.fin_bond_lines(company).len());
                if n > 0 {
                    let k = (bond + if action == A_FIN_NEXT { 1 } else { n - 1 }) % n;
                    self.fin_set(FinDlg::Repay { company, bond: k });
                }
            }
            (A_FIN_OK, FinDlg::Repay { company, bond }) => {
                let cmd = FinCmd::BondRepay {
                    company,
                    bond: bond as i32,
                };
                if self.fin_send(cmd).is_some() {
                    let msg = self.texts.a("Bond repaid.");
                    self.fin_done(msg);
                }
            }
            (A_FIN_OK, FinDlg::Resign { company }) => {
                if self
                    .fin_send(FinCmd::ResignChairman { person, company })
                    .is_some()
                {
                    let msg = self.texts.a("You resigned as chairman.");
                    self.fin_done(msg);
                }
            }
            (A_FIN_OK, FinDlg::Assume { company }) => {
                if self
                    .fin_send(FinCmd::ChairmanTakeover { person, company })
                    .is_some()
                {
                    self.close_dialog();
                }
            }
            (A_FIN_OK, FinDlg::Election { company }) => {
                let cmd = FinCmd::ElectionStart {
                    company,
                    challenger: person,
                };
                if self.fin_send(cmd).is_some() {
                    self.close_dialog();
                }
            }
            (A_FIN_PREV | A_FIN_NEXT, FinDlg::Manager { company, pick }) => {
                self.fin_set(FinDlg::Manager {
                    company,
                    pick: 1 - pick.min(1),
                });
            }
            (A_FIN_OK, FinDlg::Manager { company, pick }) => {
                let g = self.crec_i32(company, if pick == 0 { 0x140 } else { 0x144 });
                if self
                    .fin_send(FinCmd::SetManager {
                        company,
                        manager: g,
                    })
                    .is_some()
                {
                    let msg = subst(
                        &self.texts.get(2709, "Set manager to %1"),
                        &[clean(&self.texts.get(1456 + 2 * g.max(0) as u32, ""))],
                    );
                    self.fin_done(msg);
                }
            }
            (A_FIN_BUY | A_FIN_SELL, FinDlg::Trade { company }) => {
                self.fin_trade(company, action == A_FIN_BUY, false);
            }
            (A_FIN_MARGIN | A_FIN_SHORT, FinDlg::Trade { company }) => {
                self.fin_set(FinDlg::TradeConfirm {
                    company,
                    short: action == A_FIN_SHORT,
                });
            }
            (A_FIN_OK, FinDlg::TradeConfirm { company, short }) => {
                self.close_dialog();
                self.fin_trade(company, !short, true);
            }
            (A_FIN_OK | A_FIN_NO, FinDlg::Vote(v)) => {
                let yes = action == A_FIN_OK;
                let cmd = if v.merger {
                    FinCmd::MergerVote {
                        proposal: v.id,
                        person: v.person,
                        yes,
                    }
                } else {
                    FinCmd::ElectionVote {
                        election: v.id,
                        person: v.person,
                        yes,
                    }
                };
                self.close_dialog();
                let _ = self.fin_send(cmd);
            }
            _ => {}
        }
        true
    }

    /// One trade of the personal account: the lot of the original (`0x4a6e00`).
    fn fin_trade(&mut self, company: i32, buy: bool, flag: bool) {
        let Some(person) = self.human_person() else {
            return;
        };
        let cmd = FinCmd::StockTrade {
            person,
            company,
            buy,
            flag,
        };
        if let Some(FinOutcome::Trade {
            shares,
            price,
            cash,
        }) = self.fin_send(cmd)
        {
            let name = self.cname(company);
            let msg = if shares > 0 {
                self.texts.af(
                    "Bought %1 shares of %2 at %3 (%4).",
                    &[
                        int(i64::from(shares)),
                        name,
                        money(f64::from(price)),
                        money(-cash),
                    ],
                )
            } else {
                self.texts.af(
                    "Sold %1 shares of %2 at %3 (%4).",
                    &[
                        int(i64::from(-shares)),
                        name,
                        money(f64::from(price)),
                        money(cash),
                    ],
                )
            };
            self.add_news(msg.clone());
            self.toast(msg);
            // The trade moved the numbers: redraw the dialog.
            if self.fin_top().is_some() {
                self.fin_set(FinDlg::Trade { company });
            }
        }
    }

    /// The votes a shareholder has to cast: opens a dialog for the local human's pending
    /// merger or chairmanship vote (the original's `0x48b2f0` / `0x42eb40`).
    pub(in crate::state) fn poll_votes(&mut self) {
        let Some(person) = self.human_person() else {
            return;
        };
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let pending: Vec<PendingVote> = g
            .world
            .pending_votes()
            .into_iter()
            .filter(|v| v.person == person)
            .collect();
        for v in pending {
            let open = g
                .dialogs
                .iter()
                .any(|d| matches!(d, Dialog::Fin(FinDlg::Vote(o)) if o.merger == v.merger && o.id == v.id && o.person == v.person));
            if !open {
                self.push_dialog(Dialog::Fin(FinDlg::Vote(v)));
                return;
            }
        }
    }

    // ---- results ----------------------------------------------------------------------------

    fn tally_lines(&self, tally: &VoteTally) -> Vec<String> {
        let t = &self.texts;
        let side = |title: String, s: &VoteSide| {
            let mut out = vec![String::new(), clean(&title)];
            for &(p, n) in &s.voters {
                out.push(format!("{}: {}", self.pname(p), int(i64::from(n))));
            }
            out.push(format!(
                "{}: {}",
                clean(&t.get(19, "Other Investors")),
                int(i64::from(s.public))
            ));
            out.push(format!("= {}", int(i64::from(s.total))));
            out
        };
        let mut l = side(t.get(63, "Votes in favor"), &tally.yes);
        l.extend(side(t.get(64, "Votes against"), &tally.no));
        l
    }

    /// The dialog text of the vote results and margin messages (`None` for the ones that only
    /// go to the activity box).
    pub(in crate::state) fn fin_message_dialog(&self, m: &GameMessage) -> Option<Dialog> {
        let ok = self.texts.get(lng::OK, "OK");
        let body = match m {
            GameMessage::MergerResult { passed, tally, .. } => {
                let head = if *passed {
                    self.texts.get(24, "Merger Succeeds!!!")
                } else {
                    self.texts.get(25, "Merger Fails!!!")
                };
                let mut l = vec![clean(&head)];
                l.extend(self.tally_lines(tally));
                l.join("\n")
            }
            GameMessage::ChairmanVote { passed, tally, .. } => {
                let head = if *passed {
                    self.texts.get(26, "New Chairman Takes Over!!!")
                } else {
                    self.texts.get(28, "Chairmanship Takeover Fails!!!")
                };
                let mut l = vec![clean(&head)];
                l.extend(self.tally_lines(tally));
                l.join("\n")
            }
            GameMessage::MarginWarning { person, .. } | GameMessage::MarginTrade { person, .. }
                if Some(*person) != self.human_person() =>
            {
                return None;
            }
            GameMessage::MarginWarning { .. }
            | GameMessage::MarginTrade { .. }
            | GameMessage::DividendSuspended { .. }
            | GameMessage::DividendCut { .. } => self.fin_message_line(m),
            _ => return None,
        };
        Some(Dialog::Message(Message {
            title: String::new(),
            body,
            buttons: vec![(ok, Act::Close)],
        }))
    }

    /// The one-line text of the financial messages.
    pub(in crate::state) fn fin_message_line(&self, m: &GameMessage) -> String {
        let t = &self.texts;
        match m {
            GameMessage::MergerResult {
                buyer_name,
                target_name,
                passed,
                offer,
                ..
            } => format!(
                "{} {} -> {} ({})",
                clean(&if *passed {
                    t.get(24, "Merger Succeeds!!!")
                } else {
                    t.get(25, "Merger Fails!!!")
                }),
                buyer_name,
                target_name,
                money(f64::from(*offer))
            ),
            GameMessage::ChairmanVote {
                passed,
                company_name,
                challenger_name,
                ..
            } => format!(
                "{} {} / {}",
                clean(&if *passed {
                    t.get(26, "New Chairman Takes Over!!!")
                } else {
                    t.get(28, "Chairmanship Takeover Fails!!!")
                }),
                challenger_name,
                company_name
            ),
            GameMessage::ChairmanChanged {
                person_name,
                company_name,
                ..
            } => subst(
                &t.get(27, "%1 is new chairman of %2!"),
                &[person_name.clone(), company_name.clone()],
            ),
            GameMessage::MergerDone {
                buyer_name,
                target_name,
                ..
            } => subst(
                &t.get(168, "%1 takes over %2!"),
                &[buyer_name.clone(), target_name.clone()],
            ),
            GameMessage::MarginWarning { month, .. } => subst_s(
                &clean(&t.get(2270, "Your broker called the margin by the end of %s.")),
                &[t.month(*month % 12 + 1)],
            ),
            GameMessage::MarginTrade {
                company_name,
                shares,
                kind,
                ..
            } => match kind {
                MarginTradeKind::Sold => subst_s(
                    &clean(&t.get(2271, "Your broker has sold off %s shares of %s to meet a margin call.")),
                    &[int(i64::from(*shares)), company_name.clone()],
                ),
                MarginTradeKind::Covered => subst_s(
                    &clean(&t.get(2272, "Your broker has bought %s shares of %s to reduce your short position and meet a margin call.")),
                    &[int(i64::from(*shares)), company_name.clone()],
                ),
                MarginTradeKind::Squeeze => subst(
                    &clean(&t.get(2273, "You were caught in a short squeeze: %1, %2.")),
                    &[company_name.clone(), int(i64::from(*shares))],
                ),
            },
            GameMessage::DividendSuspended { .. } => clean(&t.get(2274, "Your board of directors has suspended the dividend.")),
            GameMessage::DividendCut { to_bits, .. } => subst(
                &clean(&t.get(2713, "The board has ordered the dividend rate reduced to %1.")),
                &[format!("${:.2}", f32::from_bits(*to_bits))],
            ),
            _ => String::new(),
        }
    }

    // ---- the stock page ---------------------------------------------------------------------

    /// The personal account line of the stock market screen (cash, stock, purchasing power,
    /// net worth) and the positions.
    pub(in crate::state) fn personal_lines(&self) -> Vec<String> {
        let (Some(w), Some(person)) = (self.fin_world(), self.human_person()) else {
            return Vec::new();
        };
        let Some(save) = w.save_ref() else {
            return Vec::new();
        };
        let t = &self.texts;
        let mut stock = 0.0f64;
        let mut cash = 0.0;
        let mut power = 0.0;
        let mut held = Vec::new();
        for r in save.companies.ordered() {
            let c = r.slot as i32;
            if r.data.get(0x16e..0x172).is_none_or(|b| b == [0, 0, 0, 0]) {
                continue;
            }
            if let Some(q) = w.fin_trade_quote(person, c) {
                cash = q.cash;
                power = q.power;
                if q.held != 0 {
                    let v = f64::from(q.held) * f64::from(q.price);
                    stock += v;
                    held.push(format!(
                        "{} {} ({})",
                        self.cname(c),
                        int(i64::from(q.held)),
                        money(v)
                    ));
                }
            }
        }
        let mut out = vec![t.af(
            "%1: %2   %3: %4   %5: %6   %7: %8",
            &[
                t.get(31, "Cash"),
                money(cash),
                t.get(41, "Stock"),
                money(stock),
                t.get(2261, "Purchasing Power"),
                money(power),
                t.get(181, "Net Worth"),
                money(cash + stock),
            ],
        )];
        if !held.is_empty() {
            out.push(format!(
                "{}: {}",
                t.get(40, "Stock Holdings"),
                held.join(";  ")
            ));
        }
        out
    }
}

/// Small helper to keep `"%1 of %2"` call sites short.
trait AfPair {
    fn af_pair(&self, english: &str, a: usize, b: usize) -> String;
}

impl AfPair for crate::text::Texts {
    fn af_pair(&self, english: &str, a: usize, b: usize) -> String {
        self.af(english, &[a.to_string(), b.to_string()])
    }
}
