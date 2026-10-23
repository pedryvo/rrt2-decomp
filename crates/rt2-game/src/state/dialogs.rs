//! Modal dialogs over the game: the original's eagle dialog (`eagled.win`) used for messages, the
//! briefing and goals, the Start New Company window, File Options, the placeholder for tools that
//! are not built yet, the year-end report (`yearend.win`, a book with four tabs), the newspaper
//! (`paper.win`) and the win / lose plaque (`winlose.win`).
//!
//! Dialogs queue up (`GameState::dialogs`, the last one is on top). The art is the original's
//! (`artwork::DialogArt`, loaded from the install); without it the frames fall back to the skin's
//! panel. Sizes derive from the canvas like the main window's.
//!
//! Data sources: the briefing and goals come from the scenario's rules (`scenario_info.rs`,
//! `docs/spec/scenarios.md` 2.2 and 5); the year-end report from the company ledger archive
//! (`finance.md` 1, 8); the founding numbers from `finance.md` 17 (`founding.rs`).
//!
//! Messages and the end of the game come from the simulation each frame
//! ([`AppState::poll_world`]): `World::take_messages()` fills the activity box and opens a
//! dialog or newspaper per message kind ([`AppState::message_dialog`]); `World::game_over()` opens
//! the win / lose plaque. Start New Company sends `Command` `0x26` through `World::apply_found`
//! and shows the refusals of the original's window (`finance.md` 17.1).

use rt2_ui::{Align, Anchor, Color, Fill, Fit, FontStyle, Kind, Layout, NodeId, Ui, game_font};

pub(super) mod fin;

use super::info_ui;
use super::{AppState, Screen};
use rt2_sim::build::cmd::CmdError;
use rt2_sim::found::FoundCompany;
use rt2_sim::message::{GameMessage, Outcome};
use rt2_sim::scenario::MessageKind;
use winit::keyboard::KeyCode;

use crate::founding::Founding;
use crate::info::{Block, Table};
use crate::scenario_info::{Medal, humanize_condition, wrap_text};
use crate::text::{id as lng, subst};

pub(super) const A_DLG_CLOSE: u32 = 700;
pub(super) const A_DLG_BTN: u32 = 710;
pub(super) const A_FOUND_PLAYER_MINUS: u32 = 720;
pub(super) const A_FOUND_PLAYER_PLUS: u32 = 721;
pub(super) const A_FOUND_OUTSIDE_MINUS: u32 = 722;
pub(super) const A_FOUND_OUTSIDE_PLUS: u32 = 723;
pub(super) const A_FOUND_PLAYER_SLIDER: u32 = 724;
pub(super) const A_FOUND_OUTSIDE_SLIDER: u32 = 725;
pub(super) const A_FOUND_OK: u32 = 726;
pub(super) const A_YEAR_TAB: u32 = 730;
pub(super) const A_FILE_LOAD: u32 = 740;
pub(super) const A_FILE_SAVE: u32 = 741;
pub(super) const A_FILE_NEW: u32 = 742;
pub(super) const A_FILE_MENU: u32 = 743;
pub(super) const A_FILE_QUIT: u32 = 744;

const DIM: Color = Color(0, 0, 0, 150);
const CREAM: Color = Color::rgb(0xf4, 0xea, 0xc8);
const BROWN: Color = Color::rgb(0x2a, 0x1c, 0x0c);
const INK: Color = Color::rgb(0x24, 0x1c, 0x12);
const GOLD: Color = Color::rgb(0xff, 0xd8, 0x50);

// Geometry of `eagled.win` (design pixels of the 513x405 window): the top with the eagle, a
// repeatable strip and the bottom with the buttons.
const E_W: f32 = 513.0;
const E_TOP: f32 = 137.0;
const E_STRIP: f32 = 46.0;
const E_BOTTOM: f32 = 256.0;
/// The text area of label 101 and the buttons' row, before any strips are inserted.
const E_TEXT: (f32, f32, f32, f32) = (79.0, 120.0, 379.0, 59.0);
const E_BUTTON_Y: f32 = 188.0;

/// What a message button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Act {
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Message {
    pub title: String,
    pub body: String,
    pub buttons: Vec<(String, Act)>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Dialog {
    Message(Message),
    /// Placeholder of an action icon whose tool is not built yet (index into the eight icons).
    Tool(usize),
    /// The detail of one of the player's stations with the picture and the Buy button.
    Station {
        station: i32,
    },
    /// The Buy dialog: `mask` = checked improvements, `last` = the row whose text is shown.
    StationBuy {
        station: i32,
        mask: u32,
        last: u32,
    },
    /// Purchase Train (or Replace engine of `replace`): selected engine and route to copy.
    TrainBuy {
        sel: usize,
        copy: usize,
        replace: Option<i32>,
    },
    /// The editable train detail.
    TrainEdit {
        train: i32,
        sel: usize,
        pick: usize,
        confirm: bool,
    },
    Industry {
        building: i32,
    },
    Rights {
        territory: i32,
    },
    FileOptions,
    /// The scenario briefing and its goals.
    Goals,
    /// Start New Company (`finance.md` 17).
    Founding {
        f: Founding,
        name: String,
        /// Why the last OK did nothing (`CmdError`), shown in red under the numbers.
        error: Option<String>,
    },
    /// The year-end report of `company` for `year`, tab 0..4.
    YearEnd {
        year: i32,
        tab: usize,
        company: u32,
    },
    Newspaper {
        headline: String,
        sub: String,
        body: String,
    },
    /// End of the game: medal (or loss) and the rule's text.
    GameOver {
        medal: Medal,
        text: String,
    },
    /// A financial action of the player (`fin.rs`).
    Fin(fin::FinDlg),
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

fn label(text: &str, f: FontStyle, color: Color, align: Align) -> Kind {
    Kind::Label {
        text: text.to_owned(),
        font: f,
        color,
        align,
        wrap: false,
    }
}

/// A frame placed on the canvas: the scale from design pixels, its root and the text rectangle.
struct Frame {
    root: NodeId,
    sx: f32,
    /// Text area in the frame's logical units.
    text: (f32, f32, f32, f32),
    /// Y of the button row in the frame's logical units.
    button_y: f32,
    /// Strips inserted.
    strips: u32,
}

impl AppState {
    // ---- queue ------------------------------------------------------------------------------

    pub(super) fn has_dialog(&self) -> bool {
        matches!(&self.screen, Screen::Game(g) if !g.dialogs.is_empty())
    }

    pub(super) fn push_dialog(&mut self, d: Dialog) {
        if let Screen::Game(g) = &mut self.screen {
            g.dialogs.push(d);
        }
        self.rebuild_ui();
    }

    /// Closes the topmost dialog (a game over goes back to the main menu).
    pub(super) fn close_dialog(&mut self) {
        let top = match &mut self.screen {
            Screen::Game(g) => g.dialogs.pop(),
            Screen::Menu(_) => None,
        };
        match top {
            Some(Dialog::GameOver { .. }) => self.go_to_menu(),
            Some(_) => self.rebuild_ui(),
            None => {}
        }
    }

    fn top_dialog(&self) -> Option<&Dialog> {
        match &self.screen {
            Screen::Game(g) => g.dialogs.last(),
            Screen::Menu(_) => None,
        }
    }

    /// Screenshots: opens a dialog by name (`dismiss[-N]` closes the top ones; `goals`, `founding`, `year-end[-1..-3]`, `news`,
    /// `dialog`, `win-gold`, `win-silver`, `win-bronze`, `lose`, `file`, `tool-0..7`, `fin-bond`, `fin-stock`, `fin-buyback`,
    /// `fin-dividend`, `fin-merger`, `fin-bankrupt`, `fin-repay`, `fin-manager`, `fin-resign`, `fin-trade`, `fin-assume`,
    /// `fin-election`, `fin-vote`); false for
    /// an unknown name.
    pub fn show_dialog(&mut self, name: &str) -> bool {
        let Screen::Game(g) = &self.screen else {
            return false;
        };
        let events = g
            .scenario
            .as_ref()
            .map(|s| s.events.clone())
            .unwrap_or_default();
        let year = i32::from(g.world.date().year);
        let d = match name {
            "goals" => Dialog::Goals,
            "founding" => {
                let Some(d) = self.founding_dialog_any() else {
                    return false;
                };
                d
            }
            n if n.starts_with("year-end") => {
                let tab = n
                    .strip_prefix("year-end-")
                    .and_then(|t| t.parse::<usize>().ok())
                    .unwrap_or(0)
                    .min(3);
                let company = self
                    .with_info(|i| i.player_company())
                    .flatten()
                    .unwrap_or(0);
                Dialog::YearEnd {
                    year: year - 1,
                    tab,
                    company,
                }
            }
            "news" => {
                let e = events
                    .iter()
                    .find(|e| e.kind == rt2_sim::scenario::MessageKind::Newspaper);
                Dialog::Newspaper {
                    headline: e.map_or_else(|| "Railroad reaches the coast!".to_owned(), |e| e.headline.clone()),
                    sub: e.map_or_else(
                        || "Crowds cheer as the first train arrives".to_owned(),
                        |e| e.text.clone(),
                    ),
                    body: "Citizens celebrate the connection of the two cities. Merchants expect trade to double within the year, and the company's shares rose on the news.".to_owned(),
                }
            }
            "dialog" => {
                let e = events
                    .iter()
                    .find(|e| e.kind != rt2_sim::scenario::MessageKind::Newspaper);
                Dialog::Message(Message {
                    title: e.map_or_else(|| "Encouragement".to_owned(), |e| e.headline.clone()),
                    body: e.map_or_else(
                        || "You've moved a good deal of cargo. Keep it up!".to_owned(),
                        |e| e.text.clone(),
                    ),
                    buttons: vec![(self.texts.get(lng::OK, "OK"), Act::Close)],
                })
            }
            "win-gold" | "win-silver" | "win-bronze" | "lose" => {
                let medal = match name {
                    "win-gold" => Medal::Gold,
                    "win-silver" => Medal::Silver,
                    "win-bronze" => Medal::Bronze,
                    _ => Medal::Loss,
                };
                let text = g
                    .scenario
                    .as_ref()
                    .and_then(|s| s.goals.iter().find(|g| g.medal == medal))
                    .map(|g| g.message.clone())
                    .unwrap_or_default();
                Dialog::GameOver { medal, text }
            }
            // Screenshots: closes the topmost `N` dialogs (the ones the simulation queued).
            n if n.starts_with("dismiss") => {
                let k = n["dismiss".len()..]
                    .trim_start_matches('-')
                    .parse::<usize>()
                    .unwrap_or(1);
                for _ in 0..k {
                    self.close_dialog();
                }
                return true;
            }
            "file" => Dialog::FileOptions,
            n if n.starts_with("tool-") => {
                let i = n["tool-".len()..].parse::<usize>().unwrap_or(0).min(7);
                Dialog::Tool(i)
            }
            n if n.starts_with("fin-") => return self.show_fin_dialog(n),
            n => return self.show_build_screen(n),
        };
        self.push_dialog(d);
        true
    }

    /// The year turned: the company's report for the year that ended (`yearend.win`).
    pub(super) fn offer_year_end(&mut self, year: i32) {
        let company = self
            .with_info(|i| i.player_company())
            .flatten()
            .unwrap_or(0);
        if company == 0 {
            return;
        }
        let msg = self
            .texts
            .af("The annual report for %1 is ready.", &[year.to_string()]);
        self.add_news(msg);
        self.push_dialog(Dialog::YearEnd {
            year,
            tab: 0,
            company,
        });
    }

    /// Screenshots and tests: founds a company named `name` with the dialog's starting slider
    /// positions, exactly as typing the name and pressing OK would.
    pub fn found_with_defaults(&mut self, name: &str) {
        let Some(Dialog::Founding { f, error, .. }) = self.founding_dialog() else {
            return;
        };
        self.push_dialog(Dialog::Founding {
            f,
            name: name.to_owned(),
            error,
        });
        self.found_company();
    }

    /// Start New Company for the human founder, when the player has no company yet.
    pub(super) fn founding_dialog(&self) -> Option<Dialog> {
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        let save = g.world.save_ref()?;
        if crate::buildtools::human_company(save).is_some() {
            return None;
        }
        self.founding_dialog_any()
    }

    /// The dialog for the first human person (else the first person), whatever they chair.
    pub(super) fn founding_dialog_any(&self) -> Option<Dialog> {
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        let save = g.world.save_ref()?;
        let f = crate::info::founder(save)?;
        // Confidence of the investors (`finance.md` 17.2): never a chairman here, the dialog is
        // only offered to a person without a company.
        let conf = crate::founding::investor_confidence(f.human, f.reputation, None);
        // `TEMP_COMPANY_NAMES[counter % 40]` (`finance.md` 17.4), the counter at `0x5707b9`.
        let n = crate::buildtools::st_i32(save, 0x0057_07b9).rem_euclid(40) as u32;
        let name = self
            .texts
            .table("TEMP_COMPANY_NAMES", n, "New Railroad Company");
        Some(Dialog::Founding {
            f: Founding::new(f.cash, conf),
            name,
            error: None,
        })
    }

    /// The dialog a simulation message opens, if any (`message.rs`); the activity-box line is
    /// [`AppState::message_line`].
    pub(super) fn message_dialog(&self, m: &GameMessage) -> Option<Dialog> {
        let ok = self.texts.get(lng::OK, "OK");
        let t = &self.texts;
        let info = |f: &dyn Fn(&crate::info::Info) -> String| self.with_info(|i| f(i));
        match m {
            GameMessage::Scenario(sm) => Some(match sm.kind {
                MessageKind::Newspaper => Dialog::Newspaper {
                    headline: sm.text.clone(),
                    sub: sm.label1.clone(),
                    body: String::new(),
                },
                // The engine takes the effects of button 0 (`message.rs`): the choice cannot be
                // answered yet, so only that button is offered.
                MessageKind::Choice => Dialog::Message(Message {
                    title: String::new(),
                    body: sm.text.clone(),
                    buttons: vec![(
                        if sm.label1.is_empty() {
                            ok
                        } else {
                            sm.label1.clone()
                        },
                        Act::Close,
                    )],
                }),
                MessageKind::Dialog => Dialog::Message(Message {
                    title: String::new(),
                    body: sm.text.clone(),
                    buttons: vec![(ok, Act::Close)],
                }),
            }),
            GameMessage::Historical { lng: id, .. } => Some(Dialog::Message(Message {
                title: String::new(),
                body: t.get(u32::from(*id), ""),
                buttons: vec![(ok, Act::Close)],
            })),
            // Short headline, the sentence as the story.
            GameMessage::EconomyChanged { to, .. } => Some(Dialog::Newspaper {
                headline: t.get(1215 + u32::from((*to).min(4)), ""),
                sub: String::new(),
                body: self.message_line(m),
            }),
            GameMessage::FirstOfType { building_type, .. } => Some(Dialog::Newspaper {
                headline: t.get(920 + u32::from(*building_type), ""),
                sub: String::new(),
                body: self.message_line(m),
            }),
            GameMessage::TownBonusOffer { place, .. } => Some(Dialog::Newspaper {
                headline: info(&|i| i.place_name(*place as u32)).unwrap_or_default(),
                sub: String::new(),
                body: self.message_line(m),
            }),
            GameMessage::CompanyBankrupt { .. } => Some(Dialog::Message(Message {
                title: String::new(),
                body: self.message_line(m),
                buttons: vec![(ok, Act::Close)],
            })),
            GameMessage::CompanyStarted { .. }
            | GameMessage::CompanyConnected { .. }
            | GameMessage::StockSplit { .. } => None,
            GameMessage::MergerResult { .. }
            | GameMessage::ChairmanVote { .. }
            | GameMessage::ChairmanChanged { .. }
            | GameMessage::MergerDone { .. }
            | GameMessage::MarginWarning { .. }
            | GameMessage::MarginTrade { .. }
            | GameMessage::DividendSuspended { .. }
            | GameMessage::DividendCut { .. } => self.fin_message_dialog(m),
        }
    }

    /// The one-line text of a message, for the activity box and headlines (LNG ids of
    /// `message.rs`).
    pub(super) fn message_line(&self, m: &GameMessage) -> String {
        let t = &self.texts;
        let co = |c: i32| {
            self.with_info(|i| i.company_name(c.max(0) as u32))
                .unwrap_or_default()
        };
        let place = |p: i32| {
            self.with_info(|i| i.place_name(p.max(0) as u32))
                .unwrap_or_default()
        };
        match m {
            GameMessage::Scenario(sm) => sm.text.clone(),
            GameMessage::Historical { lng: id, .. } => t.get(u32::from(*id), ""),
            GameMessage::EconomyChanged { to, .. } => t.af(
                "The economy is now: %1.",
                &[t.get(1215 + u32::from((*to).min(4)), "")],
            ),
            GameMessage::FirstOfType { place: p, .. } => {
                if *p > 0 {
                    subst(
                        &t.get(94, "A new type of building was built for the first time near %1:"),
                        &[place(*p)],
                    )
                } else {
                    t.get(95, "A new type of building was built for the first time")
                }
            }
            GameMessage::TownBonusOffer { place: p, amount } => subst(
                &t.get(226, "%1 has offered a bonus of %2 to the first railroad that connects it to another town!"),
                &[place(*p), crate::info::money(f64::from(*amount))],
            ),
            GameMessage::CompanyStarted { company } => subst(
                &t.get(166, "%1 has just started the %2."),
                &[
                    self.with_info(|i| i.chairman_name(company.max(&0).cast_unsigned()))
                        .unwrap_or_default(),
                    co(*company),
                ],
            ),
            GameMessage::CompanyConnected { company, from, to } => subst(
                &t.get(240, "%1 has connected %2 to %3."),
                &[co(*company), place(*from), place(*to)],
            ),
            GameMessage::CompanyBankrupt { company } => {
                subst(&t.get(182, "%1 has declared bankruptcy!"), &[co(*company)])
            }
            GameMessage::StockSplit { company, ratio } => subst(
                &t.get(169, "The stock of %1 has split %2 for 1!"),
                &[co(*company), ratio.to_string()],
            ),
            GameMessage::MergerResult { .. }
            | GameMessage::ChairmanVote { .. }
            | GameMessage::ChairmanChanged { .. }
            | GameMessage::MergerDone { .. }
            | GameMessage::MarginWarning { .. }
            | GameMessage::MarginTrade { .. }
            | GameMessage::DividendSuspended { .. }
            | GameMessage::DividendCut { .. } => self.fin_message_line(m),
        }
    }

    /// Drains what the simulation produced since the last frame: every message becomes a line
    /// of the activity box and, by kind, a dialog or newspaper (the oldest on top); then the end
    /// of the game, once, opens the win / lose plaque.
    ///
    /// Returns true when one of the messages was the scenario's briefing (trigger 6).
    pub(super) fn poll_world(&mut self) -> bool {
        let Screen::Game(g) = &mut self.screen else {
            return false;
        };
        let msgs = g.world.take_messages();
        self.poll_votes();
        let Screen::Game(g) = &mut self.screen else {
            return false;
        };
        let outcome = if g.over_shown {
            None
        } else {
            g.world.game_over()
        };
        let over_text = g.world.game_over_text().to_owned();
        let other_won = g.world.game_over_other_won();
        if msgs.is_empty() && outcome.is_none() {
            return false;
        }
        let briefing = msgs
            .iter()
            .any(|m| matches!(m, GameMessage::Scenario(sm) if sm.trigger == 6));
        let mut dialogs = Vec::new();
        for m in &msgs {
            let line = activity_line(&self.message_line(m));
            if !line.is_empty() {
                self.add_news(line);
            }
            if let Some(d) = self.message_dialog(m) {
                dialogs.push(d);
            }
        }
        if let Some(o) = outcome {
            let medal = match o {
                Outcome::Bronze => Medal::Bronze,
                Outcome::Silver => Medal::Silver,
                Outcome::Gold => Medal::Gold,
                Outcome::Lose => Medal::Loss,
            };
            let mut text = over_text;
            if text.is_empty() && other_won {
                text = self.texts.get(0xa83, "Another player has won the game.");
            }
            if let Screen::Game(g) = &mut self.screen {
                g.over_shown = true;
            }
            // The plaque goes on top of everything: the game is over.
            dialogs.insert(0, Dialog::GameOver { medal, text });
        }
        if let Screen::Game(g) = &mut self.screen {
            // Oldest message on top: push the newest first.
            g.dialogs.extend(dialogs.into_iter().rev());
        }
        self.rebuild_ui();
        briefing
    }

    // ---- actions ----------------------------------------------------------------------------

    /// Handles a dialog button; true if `action` belonged to the dialog layer.
    pub(super) fn dialog_activated(&mut self, action: u32) -> bool {
        match action {
            A_DLG_CLOSE => self.close_dialog(),
            a if (A_DLG_BTN..A_DLG_BTN + 4).contains(&a) => {
                let i = (a - A_DLG_BTN) as usize;
                let act = match self.top_dialog() {
                    Some(Dialog::Message(m)) => m.buttons.get(i).map(|b| b.1),
                    _ => None,
                };
                match act {
                    Some(Act::Close) => self.close_dialog(),
                    None => {}
                }
            }
            A_FOUND_PLAYER_MINUS => self.founding_step(|f| f.move_player(-1)),
            A_FOUND_PLAYER_PLUS => self.founding_step(|f| f.move_player(1)),
            A_FOUND_OUTSIDE_MINUS => self.founding_step(|f| f.move_outside(-1)),
            A_FOUND_OUTSIDE_PLUS => self.founding_step(|f| f.move_outside(1)),
            A_FOUND_OK => self.found_company(),
            a if (A_YEAR_TAB..A_YEAR_TAB + 4).contains(&a) => {
                if let Screen::Game(g) = &mut self.screen
                    && let Some(Dialog::YearEnd { tab, .. }) = g.dialogs.last_mut()
                {
                    *tab = (a - A_YEAR_TAB) as usize;
                }
                self.rebuild_ui();
            }
            a if (fin::A_FIN_OK..=fin::A_FIN_SHORT).contains(&a) => {
                self.fin_activated(a);
            }
            A_FILE_LOAD => self.file_load(),
            A_FILE_SAVE => {
                self.close_dialog();
                self.save_game();
            }
            A_FILE_NEW => self.file_new(),
            A_FILE_MENU => {
                self.close_dialog();
                self.go_to_menu();
            }
            A_FILE_QUIT => self.effects.push(super::Effect::Quit),
            _ => return false,
        }
        true
    }

    /// A press or drag on a founding slider.
    pub(super) fn dialog_pointer(&mut self, action: u32, fx: f32) {
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        let Some(Dialog::Founding { f, .. }) = g.dialogs.last_mut() else {
            return;
        };
        match action {
            A_FOUND_PLAYER_SLIDER => {
                let n = f.player().n;
                let target = (fx * (n - 1) as f32).round() as i32;
                let delta = target - f.player_pos;
                f.move_player(delta);
            }
            A_FOUND_OUTSIDE_SLIDER => {
                let n = f.outside().n;
                let target = (fx * (n - 1) as f32).round() as i32;
                let delta = target - f.outside_pos;
                f.move_outside(delta);
            }
            _ => return,
        }
        self.rebuild_ui();
    }

    fn founding_step(&mut self, f: impl FnOnce(&mut Founding)) {
        if let Screen::Game(g) = &mut self.screen
            && let Some(Dialog::Founding { f: fd, .. }) = g.dialogs.last_mut()
        {
            f(fd);
        }
        self.rebuild_ui();
    }

    /// OK in the founding dialog: command `0x26` (`finance.md` 17.3) with the numbers of
    /// `rt2_sim::found::founding_preview` and the typed name. A refusal keeps the dialog open
    /// with the original's message (`finance.md` 17.1); success closes it.
    fn found_company(&mut self) {
        let Some(Dialog::Founding { f, name, .. }) = self.top_dialog().cloned() else {
            return;
        };
        if !f.allowed() {
            let msg = subst(
                &self.texts.get(
                    145,
                    "You need at least %1 in personal cash to start a company.",
                ),
                &[crate::info::money(f64::from(rt2_sim::found::MIN_POWER))],
            );
            self.set_founding_error(Some(msg));
            return;
        }
        let Some(person) = self
            .game()
            .and_then(|g| g.world.save_ref())
            .and_then(crate::info::founder)
            .map(|p| p.person)
        else {
            return;
        };
        let p = rt2_sim::found::founding_preview(f.player_value(), f.outside_value());
        let name = name.trim().to_owned();
        let cmd = FoundCompany::new(person, &name, -1, p.capital, f.player_value());
        let result = match &mut self.screen {
            Screen::Game(g) => g.world.apply_found(cmd),
            Screen::Menu(_) => return,
        };
        match result {
            Ok(_) => {
                self.close_dialog();
                let shown = if name.is_empty() {
                    self.texts.a("a new company")
                } else {
                    name
                };
                let msg = self.texts.af("%1 was founded.", &[shown]);
                self.add_news(msg.clone());
                self.toast(msg);
                self.sync_after_build();
            }
            Err(e) => {
                let msg = self.founding_error_text(e);
                self.set_founding_error(Some(msg));
            }
        }
    }

    /// The text of a refusal (`CmdError`) of command `0x26`.
    fn founding_error_text(&self, e: CmdError) -> String {
        let t = &self.texts;
        match e {
            CmdError::AlreadyChairman => t.get(
                2863,
                "You would have to resign from your current company to start a new one.",
            ),
            CmdError::TooManyCompanies => subst(
                &t.get(
                    144,
                    "The maximum number of companies (%1) is already in the game.",
                ),
                &[rt2_sim::found::MAX_COMPANIES.to_string()],
            ),
            CmdError::FoundingCooldown => t.get(
                2708,
                "No government will grant you a charter so soon after founding your last company. Wait two years.",
            ),
            CmdError::InsufficientCash => subst(
                &t.get(145, "You need at least %1 in personal cash to start a company."),
                &[crate::info::money(f64::from(rt2_sim::found::MIN_POWER))],
            ),
            CmdError::InvalidInput => t.a("The investment values are not valid for this person."),
            CmdError::NoSuchObject | CmdError::Unknown => {
                t.a("There is no player who can start a company.")
            }
            _ => t.a("The company could not be started."),
        }
    }

    fn set_founding_error(&mut self, msg: Option<String>) {
        if let Screen::Game(g) = &mut self.screen
            && let Some(Dialog::Founding { error, .. }) = g.dialogs.last_mut()
        {
            *error = msg;
        }
        self.rebuild_ui();
    }

    /// Typing into the open Start New Company dialog: printable ASCII appends to the name (at
    /// most 30 bytes, the packet's field), Backspace deletes, Enter confirms. True when the key
    /// was consumed.
    pub(super) fn founding_key(&mut self, code: KeyCode, text: Option<&str>) -> bool {
        let Screen::Game(g) = &mut self.screen else {
            return false;
        };
        let Some(Dialog::Founding { name, error, .. }) = g.dialogs.last_mut() else {
            return false;
        };
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                self.found_company();
                return true;
            }
            KeyCode::Escape => {
                self.close_dialog();
                return true;
            }
            KeyCode::Backspace => {
                name.pop();
            }
            _ => {
                let Some(c) = text
                    .and_then(|t| t.chars().next())
                    .filter(|c| c.is_ascii() && !c.is_ascii_control())
                else {
                    return false;
                };
                if name.len() >= 30 {
                    return true;
                }
                name.push(c);
            }
        }
        *error = None;
        self.rebuild_ui();
        true
    }

    fn file_load(&mut self) {
        self.close_dialog();
        self.show_load_game();
    }

    fn file_new(&mut self) {
        self.close_dialog();
        self.show_new_game();
    }

    // ---- building ---------------------------------------------------------------------------

    /// Builds the dim layer and the topmost dialog.
    pub(super) fn build_dialogs(&mut self, ui: &mut Ui) {
        let Some(top) = self.top_dialog().cloned() else {
            return;
        };
        ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(DIM),
            },
            Layout::fill(),
        );
        match top {
            Dialog::Message(m) => self.build_message(ui, &m.title, &m.body, &m.buttons),
            Dialog::Tool(i) => {
                let (name, tip) = self.tool_name(i);
                let body = format!(
                    "{tip}\n\n{}",
                    self.texts.a(
                        "This tool is not available yet: construction is still being built into the simulation. (Placeholder panel.)"
                    )
                );
                let ok = vec![(self.texts.get(lng::OK, "OK"), Act::Close)];
                self.build_message(ui, &name, &body, &ok);
            }
            Dialog::FileOptions => self.build_file_options(ui),
            d @ (Dialog::Station { .. }
            | Dialog::StationBuy { .. }
            | Dialog::TrainBuy { .. }
            | Dialog::TrainEdit { .. }
            | Dialog::Industry { .. }
            | Dialog::Rights { .. }) => {
                self.build_build_dialog(ui, &d);
            }
            Dialog::Goals => self.build_goals(ui),
            Dialog::Founding { f, name, error } => {
                self.build_founding(ui, &f, &name, error.as_deref())
            }
            Dialog::YearEnd { year, tab, company } => self.build_year_end(ui, year, tab, company),
            Dialog::Newspaper {
                headline,
                sub,
                body,
            } => self.build_newspaper(ui, &headline, &sub, &body),
            Dialog::GameOver { medal, text } => self.build_game_over(ui, medal, &text),
            Dialog::Fin(d) => self.build_fin(ui, &d),
        }
    }

    /// The eagle frame with `strips` inserted, centred and scaled to fit.
    fn eagle(&self, ui: &mut Ui, strips: u32) -> Frame {
        let (cw, ch) = ui.logical_size();
        let base = (ch / 620.0).clamp(1.0, 2.0) * 0.95;
        let total = E_TOP + E_STRIP * strips as f32 + E_BOTTOM;
        let sx = base
            .min((cw - 16.0) / E_W)
            .min((ch - 16.0) / total)
            .max(0.3);
        let root = ui.add_root(
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Center, E_W * sx, total * sx),
        );
        match self.dialog_art.eagle {
            Some([top, strip, bottom]) => {
                ui.add(
                    root,
                    Kind::Image {
                        sprite: top,
                        fit: Fit::Stretch,
                    },
                    at(0.0, 0.0, E_W * sx, E_TOP * sx),
                );
                for i in 0..strips {
                    ui.add(
                        root,
                        Kind::Image {
                            sprite: strip,
                            fit: Fit::Stretch,
                        },
                        at(
                            0.0,
                            (E_TOP + E_STRIP * i as f32) * sx,
                            E_W * sx,
                            E_STRIP * sx,
                        ),
                    );
                }
                ui.add(
                    root,
                    Kind::Image {
                        sprite: bottom,
                        fit: Fit::Stretch,
                    },
                    at(
                        0.0,
                        (E_TOP + E_STRIP * strips as f32) * sx,
                        E_W * sx,
                        E_BOTTOM * sx,
                    ),
                );
            }
            None => {
                ui.add(
                    root,
                    Kind::Panel { fill: Fill::Panel },
                    at(0.0, 20.0 * sx, E_W * sx, (total - 60.0) * sx),
                );
            }
        }
        let extra = E_STRIP * strips as f32;
        Frame {
            root,
            sx,
            text: (
                E_TEXT.0 * sx,
                E_TEXT.1 * sx,
                E_TEXT.2 * sx,
                (E_TEXT.3 + extra) * sx,
            ),
            button_y: (E_BUTTON_Y + extra) * sx,
            strips,
        }
    }

    /// Buttons on the eagle frame's button row: one in the middle, two left and right, three
    /// left, middle and right.
    fn eagle_buttons(&self, ui: &mut Ui, fr: &Frame, buttons: &[(String, u32)]) {
        let sx = fr.sx;
        // (x, w, h, sprite indexes up/down) of the left, middle and right plates.
        let left = (80.0, 138.0, 60.0, 0usize);
        let mid = (216.0, 110.0, 54.0, 4);
        let right = (316.0, 138.0, 60.0, 2);
        let slots: Vec<(f32, f32, f32, usize)> = match buttons.len() {
            0 => vec![],
            1 => vec![mid],
            2 => vec![left, right],
            _ => vec![left, mid, right],
        };
        let f = game_font(3).with_size((game_font(3).size * sx * 0.85).round().max(10.0));
        for ((text, action), (x, w, h, si)) in buttons.iter().zip(slots) {
            let (up, down) = match self.dialog_art.eagle_button {
                Some(a) => (Some(a[si]), Some(a[si + 1])),
                None => (None, None),
            };
            let (bx, by, bw, bh) = (x * sx, fr.button_y, w * sx, h * sx);
            ui.add(
                fr.root,
                Kind::Icon {
                    up,
                    down,
                    fallback: text.clone(),
                    font: f,
                    action: *action,
                    on: false,
                },
                at(bx, by, bw, bh),
            );
            if up.is_some() {
                ui.add(
                    fr.root,
                    label(text, f, BROWN, Align::Center),
                    at(bx, by, bw, bh),
                );
            }
        }
    }

    /// Text lines of a dialog body for a text area of `w` logical units.
    fn body_lines(text: &str, w: f32, size: f32) -> Vec<String> {
        wrap_text(text, (w / (size * 0.56)).floor() as usize)
    }

    /// Strips needed so `lines` rows of `row_h` fit (capped so the frame fits the canvas).
    fn strips_for(ui: &Ui, lines: usize, row_h: f32, sx: f32) -> u32 {
        let (_, ch) = ui.logical_size();
        let need = lines as f32 * row_h / sx;
        let n = ((need - E_TEXT.3) / E_STRIP).ceil().max(0.0) as u32;
        let cap = (((ch - 16.0) / 0.75 - E_TOP - E_BOTTOM) / E_STRIP)
            .floor()
            .max(0.0) as u32;
        n.min(cap).min(6)
    }

    fn build_message(&self, ui: &mut Ui, title: &str, body: &str, buttons: &[(String, Act)]) {
        let (_, ch) = ui.logical_size();
        let sx0 = ((ch / 620.0).clamp(1.0, 2.0) * 0.95).min(1.6);
        let size = 15.0 * sx0;
        let mut lines = Self::body_lines(body, E_TEXT.2 * sx0, size);
        if !title.is_empty() {
            lines.insert(0, String::new());
            lines.insert(0, title.to_uppercase());
        }
        let strips = Self::strips_for(ui, lines.len(), size * 1.3, sx0);
        let fr = self.eagle(ui, strips);
        let size = 15.0 * fr.sx;
        let lines = {
            let mut l = Self::body_lines(body, fr.text.2, size);
            if !title.is_empty() {
                l.insert(0, String::new());
                l.insert(0, title.to_uppercase());
            }
            l
        };
        self.text_in(ui, &fr, lines, size);
        let ids: Vec<(String, u32)> = buttons
            .iter()
            .enumerate()
            .map(|(i, b)| (b.0.clone(), A_DLG_BTN + i as u32))
            .collect();
        self.eagle_buttons(ui, &fr, &ids);
    }

    /// The scrolling body text inside the frame's text area (title lines in gold).
    fn text_in(&self, ui: &mut Ui, fr: &Frame, lines: Vec<String>, size: f32) {
        let f = game_font(8).with_size(size.round().max(10.0));
        let (x, y, w, h) = fr.text;
        ui.add(
            fr.root,
            Kind::Text {
                lines,
                scroll: 0,
                row_h: (size * 1.3).round(),
                font: f,
                color: CREAM,
            },
            at(x, y, w, h),
        );
    }

    fn build_file_options(&self, ui: &mut Ui) {
        let t = &self.texts;
        let fr = self.eagle(ui, 3);
        let sx = fr.sx;
        let size = 15.0 * sx;
        self.text_in(
            ui,
            &fr,
            vec![
                t.get(1787, "File options (Load, Save, Quit, New Game)")
                    .to_uppercase(),
            ],
            size,
        );
        // Six buttons in two columns of three, under the title.
        let f = game_font(3).with_size((game_font(3).size * sx * 0.8).round().max(10.0));
        let items = [
            (t.get(lng::LOAD_GAME, "Load Game"), A_FILE_LOAD),
            (t.a("Save"), A_FILE_SAVE),
            (t.get(lng::NEW_GAME, "New Game"), A_FILE_NEW),
            (t.a("Main Menu"), A_FILE_MENU),
            (t.get(lng::QUIT, "Quit"), A_FILE_QUIT),
            (t.get(lng::CANCEL, "Cancel"), A_DLG_CLOSE),
        ];
        let (bw, bh) = (150.0 * sx, 40.0 * sx);
        let (x0, y0) = (
            fr.text.0 + (fr.text.2 - 2.0 * bw - 14.0 * sx) / 2.0,
            fr.text.1 + 36.0 * sx,
        );
        for (i, (text, action)) in items.into_iter().enumerate() {
            let (col, row) = ((i % 2) as f32, (i / 2) as f32);
            ui.add(
                fr.root,
                Kind::Button {
                    text,
                    font: f,
                    action,
                    enabled: true,
                    hotkey: None,
                },
                at(
                    x0 + col * (bw + 14.0 * sx),
                    y0 + row * (bh + 8.0 * sx),
                    bw,
                    bh,
                ),
            );
        }
        let _ = fr.strips;
    }

    fn build_goals(&self, ui: &mut Ui) {
        let (_, ch) = ui.logical_size();
        let sx0 = ((ch / 620.0).clamp(1.0, 2.0) * 0.95).min(1.6);
        let lines0 = self.goal_lines(E_TEXT.2 * sx0, 15.0 * sx0);
        let strips = Self::strips_for(ui, lines0.len(), 15.0 * sx0 * 1.3, sx0).max(3);
        let fr = self.eagle(ui, strips);
        let size = 15.0 * fr.sx;
        let lines = self.goal_lines(fr.text.2, size);
        self.text_in(ui, &fr, lines, size);
        let ok = vec![(self.texts.get(lng::OK, "OK"), A_DLG_CLOSE)];
        self.eagle_buttons(ui, &fr, &ok);
    }

    /// Briefing, then the medals' conditions as the rules state them.
    fn goal_lines(&self, w: f32, size: f32) -> Vec<String> {
        let Screen::Game(g) = &self.screen else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let title = g
            .scenario
            .as_ref()
            .map(|s| s.title.clone())
            .unwrap_or_else(|| g.name.clone());
        out.push(title.to_uppercase());
        out.push(String::new());
        let Some(s) = &g.scenario else {
            out.extend(Self::body_lines(
                &self.texts.a("No scenario rules in this game."),
                w,
                size,
            ));
            return out;
        };
        out.extend(Self::body_lines(&s.briefing_text(), w, size));
        out.push(String::new());
        out.push(self.texts.get(1309, "Victory Conditions").to_uppercase());
        let goals = s.goals_by_medal();
        if goals.is_empty() {
            out.extend(Self::body_lines(
                &self.texts.a("This scenario has no victory conditions."),
                w,
                size,
            ));
        }
        for goal in goals {
            let name = match goal.medal {
                Medal::Gold => self.texts.get(2759, "Gold"),
                Medal::Silver => self.texts.get(2760, "Silver"),
                Medal::Bronze => self.texts.get(2761, "Bronze"),
                Medal::Loss => self.texts.a("Defeat"),
            };
            let when = trigger_label(&self.texts, goal.trigger);
            let head = format!("{name} ({when}):");
            out.push(head);
            for l in Self::body_lines(&humanize_condition(&goal.condition), w - 14.0, size) {
                out.push(format!("   {l}"));
            }
        }
        out
    }

    fn build_founding(&self, ui: &mut Ui, f: &Founding, name: &str, error: Option<&str>) {
        let t = &self.texts;
        let fr = self.eagle(ui, if error.is_some() { 8 } else { 6 });
        let sx = fr.sx;
        let (x, y, w, _h) = fr.text;
        let size = 14.0 * sx;
        let fnt = game_font(8).with_size(size.round().max(10.0));
        let full = t.get(146, "Start New Company");
        let (head, rest) = full.split_once("\n\n").unwrap_or((full.as_str(), ""));
        let mut lines = vec![head.to_owned()];
        lines.extend(Self::body_lines(&rest.replace("\n\n", " "), w, size));
        let lh = (size * 1.3).round();
        let mut yy = y;
        for (i, l) in lines.iter().enumerate() {
            let color = if i == 0 { GOLD } else { CREAM };
            ui.add(fr.root, label(l, fnt, color, Align::Left), at(x, yy, w, lh));
            yy += lh;
        }
        yy += lh * 0.4;
        // The two sliders: label with value, then [-] bar [+].
        let rows = [
            (
                t.get(541, "Player Investment"),
                f.player_value(),
                f.player().n,
                f.player_pos,
                A_FOUND_PLAYER_MINUS,
                A_FOUND_PLAYER_PLUS,
                A_FOUND_PLAYER_SLIDER,
            ),
            (
                t.get(542, "Outside Investment"),
                f.outside_value(),
                f.outside().n,
                f.outside_pos,
                A_FOUND_OUTSIDE_MINUS,
                A_FOUND_OUTSIDE_PLUS,
                A_FOUND_OUTSIDE_SLIDER,
            ),
        ];
        let bh = (24.0 * sx).round();
        for (title, value, n, pos, minus, plus, slider) in rows {
            ui.add(
                fr.root,
                label(&title, fnt, CREAM, Align::Left),
                at(x, yy, w * 0.6, lh),
            );
            ui.add(
                fr.root,
                label(
                    &crate::info::money(f64::from(value)),
                    fnt,
                    GOLD,
                    Align::Right,
                ),
                at(x + w * 0.5, yy, w * 0.5, lh),
            );
            yy += lh;
            let arrow = game_font(3).with_size((game_font(3).size * sx * 0.7).round().max(10.0));
            ui.add(
                fr.root,
                Kind::Button {
                    text: "<".into(),
                    font: arrow,
                    action: minus,
                    enabled: f.allowed(),
                    hotkey: None,
                },
                at(x, yy, bh * 1.2, bh),
            );
            ui.add(
                fr.root,
                Kind::Button {
                    text: ">".into(),
                    font: arrow,
                    action: plus,
                    enabled: f.allowed(),
                    hotkey: None,
                },
                at(x + w - bh * 1.2, yy, bh * 1.2, bh),
            );
            let bar_w = w - 2.0 * bh * 1.2 - 8.0 * sx;
            let cells = n.clamp(2, 48) as usize;
            let filled =
                ((pos as f32 / (n - 1).max(1) as f32) * (cells - 1) as f32).round() as usize;
            let colors: Vec<Color> = (0..cells)
                .map(|i| {
                    if i <= filled && f.allowed() {
                        Color::rgb(0xd8, 0xb0, 0x40)
                    } else {
                        Color::rgb(0x30, 0x2c, 0x18)
                    }
                })
                .collect();
            ui.add(
                fr.root,
                Kind::Cells {
                    cols: cells,
                    rows: 1,
                    colors: std::sync::Arc::new(colors),
                    action: slider,
                },
                at(x + bh * 1.2 + 4.0 * sx, yy + 3.0 * sx, bar_w, bh - 6.0 * sx),
            );
            yy += bh + lh * 0.4;
        }
        // What the capital buys (`founding_preview`: finance.md 17.3 and 17.4; the founder's
        // shares are rounded to the nearest 1000, so "You buy" may cost more than P) and the
        // company name, edited by typing.
        let p = rt2_sim::found::founding_preview(f.player_value(), f.outside_value());
        let facts = [
            (
                t.a("Total capital"),
                crate::info::money(f64::from(p.capital)),
            ),
            (t.get(267, "Share Price"), format!("${:.2}", p.price)),
            (t.get(280, "Shares"), p.company_shares.to_string()),
            (
                t.a("You buy"),
                format!(
                    "{}  ({})",
                    p.founder_shares,
                    crate::info::money(f64::from(p.founder_cost))
                ),
            ),
            (t.get(543, "Company Name"), format!("{name}_")),
        ];
        for (k, v) in facts {
            if yy + lh > fr.text.1 + fr.text.3 + 4.0 * sx {
                break;
            }
            ui.add(
                fr.root,
                label(&k, fnt, CREAM, Align::Left),
                at(x, yy, w * 0.5, lh),
            );
            ui.add(
                fr.root,
                label(&v, fnt, GOLD, Align::Right),
                at(x + w * 0.4, yy, w * 0.6, lh),
            );
            yy += lh;
        }
        if let Some(e) = error {
            let lines = Self::body_lines(e, w, size);
            for l in lines.iter().take(3) {
                ui.add(
                    fr.root,
                    label(l, fnt, Color::rgb(0xf0, 0x8c, 0x7a), Align::Left),
                    at(x, yy, w, lh),
                );
                yy += lh;
            }
        }
        if !f.allowed() {
            ui.add(
                fr.root,
                label(
                    &t.af(
                        "You need at least %1 in personal cash and purchasing power to start a company.",
                        &["$100,000".to_owned()],
                    ),
                    fnt,
                    Color::rgb(0xf0, 0x8c, 0x7a),
                    Align::Left,
                ),
                at(x, yy, w, lh),
            );
        }
        let buttons = [
            (t.get(lng::OK, "OK"), A_FOUND_OK),
            (t.get(lng::CANCEL, "Cancel"), A_DLG_CLOSE),
        ];
        self.eagle_buttons(ui, &fr, &buttons);
    }

    fn build_year_end(&self, ui: &mut Ui, year: i32, tab: usize, company: u32) {
        let t = &self.texts;
        let (cw, ch) = ui.logical_size();
        // `yearend.win`: 521x604 window, pages 508x595 drawn at x 12, content at (87, 8).
        let (ww, wh) = (521.0, 604.0);
        let sx = ((ch / 620.0).clamp(1.0, 2.0) * 0.95)
            .min((cw - 16.0) / ww)
            .min((ch - 90.0) / wh);
        let root = ui.add_root(
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Center, ww * sx, wh * sx).offset(0.0, -14.0),
        );
        match self.dialog_art.pages.get(tab).copied().flatten() {
            Some(page) => {
                ui.add(
                    root,
                    Kind::Image {
                        sprite: page,
                        fit: Fit::Stretch,
                    },
                    at(12.0 * sx, 2.0 * sx, 508.0 * sx, 595.0 * sx),
                );
            }
            None => {
                ui.add(
                    root,
                    Kind::Panel { fill: Fill::Panel },
                    at(12.0 * sx, 0.0, 508.0 * sx, 597.0 * sx),
                );
            }
        }
        let (x, y, w, h) = (92.0 * sx, 20.0 * sx, 392.0 * sx, 480.0 * sx);
        let titles = [
            t.get(118, "Income Statement"),
            t.get(117, "Balance Sheet"),
            t.get(1005, "Share Price"),
            t.get(1309, "Victory Conditions"),
        ];
        ui.add(
            root,
            label(
                &format!("{} {year}", t.get(170, "Annual Report")).to_uppercase(),
                game_font(3).with_size((23.0 * sx).round()),
                INK,
                Align::Center,
            ),
            at(x, y, w, 30.0 * sx),
        );
        ui.add(
            root,
            label(
                &titles[tab.min(3)],
                game_font(6).with_size((20.0 * sx).round()),
                Color::rgb(0x5a, 0x3a, 0x14),
                Align::Center,
            ),
            at(x, y + 32.0 * sx, w, 26.0 * sx),
        );
        let blocks = self.year_end_blocks(company, tab);
        let rowh = (22.0 * sx).round();
        info_ui::draw_blocks(
            ui,
            root,
            &blocks,
            x,
            y + 66.0 * sx,
            w,
            y + h,
            rowh,
            (rowh * 0.64).max(10.0),
        );
        // The tab row under the book, and Exit.
        let names = [t.a("Income"), t.a("Balance"), t.a("Stock"), t.a("Goals")];
        let tab_w = ((ww * sx - 230.0 * sx) / 4.0).round();
        let by = wh * sx + 0.0;
        let bf = game_font(1).with_size((game_font(1).size * sx).round().max(9.0));
        for (i, n) in names.iter().enumerate() {
            ui.add(
                root,
                Kind::Toggle {
                    text: n.clone(),
                    font: bf,
                    action: A_YEAR_TAB + i as u32,
                    on: i == tab,
                    enabled: true,
                },
                at(
                    88.0 * sx + i as f32 * tab_w,
                    by - 6.0 * sx,
                    tab_w - 3.0,
                    26.0 * sx,
                ),
            );
        }
        ui.add(
            root,
            Kind::Button {
                text: t.get(1825, "Exit"),
                font: bf,
                action: A_DLG_CLOSE,
                enabled: true,
                hotkey: Some('e'),
            },
            at(ww * sx - 80.0 * sx, by - 6.0 * sx, 74.0 * sx, 26.0 * sx),
        );
    }

    /// The blocks of a year-end tab: the income statement (last year only), balance sheet, stock
    /// figures, then the scenario status.
    fn year_end_blocks(&self, company: u32, tab: usize) -> Vec<Block> {
        let page = self.with_info(|i| i.finance_page(company));
        let Some(page) = page else {
            return vec![Block::Note(self.texts.a("No game state to show."))];
        };
        match tab {
            0 => page
                .left
                .into_iter()
                .filter_map(|b| match b {
                    Block::Table { title, table } if table.cols.len() == 4 => {
                        // Keep the label and the "last year" column.
                        let keep = |v: Vec<String>| vec![v[0].clone(), v[2].clone()];
                        let t = Table {
                            cols: vec![table.cols[0].clone(), table.cols[2].clone()],
                            rows: table.rows.into_iter().map(keep).collect(),
                            ids: table.ids,
                        };
                        Some(Block::Table { title, table: t })
                    }
                    _ => None,
                })
                .collect(),
            1 => page.right.into_iter().take(1).collect(),
            2 => page
                .right
                .into_iter()
                .filter(|b| matches!(b, Block::Facts(_)))
                .take(1)
                .collect(),
            _ => {
                let Screen::Game(g) = &self.screen else {
                    return Vec::new();
                };
                let goals: Vec<(String, String)> = g
                    .scenario
                    .as_ref()
                    .map(|s| {
                        s.goals_by_medal()
                            .into_iter()
                            .map(|goal| {
                                let name = match goal.medal {
                                    Medal::Gold => self.texts.get(2759, "Gold"),
                                    Medal::Silver => self.texts.get(2760, "Silver"),
                                    Medal::Bronze => self.texts.get(2761, "Bronze"),
                                    Medal::Loss => self.texts.a("Defeat"),
                                };
                                (name, humanize_condition(&goal.condition))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if goals.is_empty() {
                    vec![Block::Note(
                        self.texts.a("This scenario has no victory conditions."),
                    )]
                } else {
                    goals
                        .into_iter()
                        .map(|(k, v)| Block::Note(format!("{k}: {v}")))
                        .collect()
                }
            }
        }
    }

    fn build_newspaper(&self, ui: &mut Ui, headline: &str, sub: &str, body: &str) {
        let t = &self.texts;
        let (cw, ch) = ui.logical_size();
        // `paper.win`: 771x505, headline over the top band, text area (60,119) 697x361.
        let (ww, wh) = (771.0, 505.0);
        let sx = ((ch / 620.0).clamp(1.0, 2.0) * 0.95)
            .min((cw - 16.0) / ww)
            .min((ch - 16.0) / wh);
        let root = ui.add_root(
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Center, ww * sx, wh * sx),
        );
        match self.dialog_art.paper {
            Some(p) => {
                ui.add(
                    root,
                    Kind::Image {
                        sprite: p,
                        fit: Fit::Stretch,
                    },
                    at(0.0, 0.0, ww * sx, wh * sx),
                );
            }
            None => {
                ui.add(
                    root,
                    Kind::Panel {
                        fill: Fill::Solid(Color::rgb(0xd8, 0xd2, 0xc0)),
                    },
                    at(0.0, 0.0, ww * sx, wh * sx),
                );
            }
        }
        let fit = 440.0 * sx / (headline.chars().count().max(1) as f32 * 0.42);
        let head_font = game_font(9).with_size((46.0 * sx * 0.8).min(fit).round().max(12.0));
        ui.add(
            root,
            label(headline, head_font, INK, Align::Center),
            at(210.0 * sx, 8.0 * sx, 440.0 * sx, 56.0 * sx),
        );
        let g = if let Screen::Game(g) = &self.screen {
            super::format_date(g.world.date(), t)
        } else {
            String::new()
        };
        let small = game_font(2).with_size((13.0 * sx).round().max(9.0));
        ui.add(
            root,
            label(&g, small, INK, Align::Left),
            at(62.0 * sx, 43.0 * sx, 140.0 * sx, 20.0 * sx),
        );
        let text_size = (17.0 * sx).round().max(10.0);
        let mut lines = Vec::new();
        if !sub.is_empty() {
            lines.extend(Self::body_lines(&sub.to_uppercase(), 697.0 * sx, text_size));
            lines.push(String::new());
        }
        lines.extend(Self::body_lines(body, 697.0 * sx, text_size));
        ui.add(
            root,
            Kind::Text {
                lines,
                scroll: 0,
                row_h: (text_size * 1.3).round(),
                font: game_font(8).with_size(text_size),
                color: INK,
            },
            at(64.0 * sx, 122.0 * sx, 690.0 * sx, 355.0 * sx),
        );
        ui.add(
            root,
            Kind::Button {
                text: t.get(1825, "Exit"),
                font: game_font(3).with_size((game_font(3).size * sx * 0.8).round().max(10.0)),
                action: A_DLG_CLOSE,
                enabled: true,
                hotkey: Some('e'),
            },
            at(668.0 * sx, 48.0 * sx, 78.0 * sx, 40.0 * sx),
        );
    }

    fn build_game_over(&self, ui: &mut Ui, medal: Medal, text: &str) {
        let t = &self.texts;
        let (cw, ch) = ui.logical_size();
        // `winlose.win`: 598x698, medal window (152,166) 320x240, text (78,418) 471x156, OK.
        let (ww, wh) = (598.0, 698.0);
        let sx = ((ch / 620.0).clamp(1.0, 2.0) * 0.95)
            .min((cw - 16.0) / ww)
            .min((ch - 16.0) / wh);
        let root = ui.add_root(
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Center, ww * sx, wh * sx),
        );
        match self.dialog_art.plaque {
            Some(p) => {
                ui.add(
                    root,
                    Kind::Image {
                        sprite: p,
                        fit: Fit::Stretch,
                    },
                    at(9.0 * sx, 7.0 * sx, 583.0 * sx, 685.0 * sx),
                );
            }
            None => {
                ui.add(
                    root,
                    Kind::Panel { fill: Fill::Panel },
                    at(9.0 * sx, 7.0 * sx, 583.0 * sx, 685.0 * sx),
                );
            }
        }
        // The medal: the original shows a video here; the coin art of the file stands in.
        let coin = match medal {
            Medal::Gold => self.dialog_art.medal_gold,
            Medal::Silver => self.dialog_art.medal_silver,
            _ => None,
        };
        let (mx, my, mw, mh) = (152.0 * sx, 166.0 * sx, 320.0 * sx, 240.0 * sx);
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(Color(0x10, 0x0c, 0x08, 0xd0)),
            },
            at(mx, my, mw, mh),
        );
        if let Some(c) = coin {
            ui.add(
                root,
                Kind::Image {
                    sprite: c,
                    fit: Fit::Contain,
                },
                at(
                    mx + 10.0 * sx,
                    my + 10.0 * sx,
                    mw - 20.0 * sx,
                    mh - 20.0 * sx,
                ),
            );
        } else {
            let name = match medal {
                Medal::Bronze => t.get(2761, "Bronze"),
                Medal::Loss => t.get(2385, "Game Over"),
                Medal::Silver => t.get(2760, "Silver"),
                Medal::Gold => t.get(2759, "Gold"),
            };
            ui.add(
                root,
                label(
                    &name.to_uppercase(),
                    game_font(9).with_size((46.0 * sx).round().max(14.0)),
                    GOLD,
                    Align::Center,
                ),
                at(mx, my, mw, mh),
            );
        }
        let (id, en) = match medal {
            Medal::Bronze => (
                2509,
                "Narrow Victory!\n\nYou've achieved your goals and pulled off a narrow victory.  Congratulations!",
            ),
            Medal::Silver => (
                2510,
                "Victory!\n\nYou've achieved your goals and pulled off a victory.  Congratulations!",
            ),
            Medal::Gold => (
                2511,
                "Decisive Victory!\n\nYou've achieved your goals and pulled off a decisive victory.  Congratulations!",
            ),
            Medal::Loss => (2512, "Loss!!!\n\nYou've lost the game."),
        };
        let headline = t.get(id, en);
        let size = (16.0 * sx).round().max(10.0);
        let mut lines = Self::body_lines(&headline, 471.0 * sx, size);
        if !text.trim().is_empty() {
            lines.push(String::new());
            lines.extend(Self::body_lines(text, 471.0 * sx, size));
        }
        ui.add(
            root,
            Kind::Text {
                lines,
                scroll: 0,
                row_h: (size * 1.3).round(),
                font: game_font(8).with_size(size),
                color: CREAM,
            },
            at(78.0 * sx, 418.0 * sx, 471.0 * sx, 150.0 * sx),
        );
        let bf = game_font(3).with_size((game_font(3).size * sx * 0.85).round().max(10.0));
        let (up, down) = match self.dialog_art.eagle_button {
            Some(a) => (Some(a[4]), Some(a[5])),
            None => (None, None),
        };
        let (bx, by, bw, bh) = (254.0 * sx, 579.0 * sx, 110.0 * sx, 54.0 * sx);
        let ok = t.get(lng::OK, "OK");
        ui.add(
            root,
            Kind::Icon {
                up,
                down,
                fallback: ok.clone(),
                font: bf,
                action: A_DLG_CLOSE,
                on: false,
            },
            at(bx, by, bw, bh),
        );
        if up.is_some() {
            ui.add(
                root,
                label(&ok, bf, BROWN, Align::Center),
                at(bx, by, bw, bh),
            );
        }
    }
}

/// "year end" style description of a rule trigger (`scenarios.md` 2.2).
fn trigger_label(texts: &crate::text::Texts, trigger: u8) -> String {
    match trigger {
        0 => texts.a("start of year"),
        1 => texts.a("end of year"),
        2 => texts.a("start of month"),
        3 => texts.a("end of month"),
        4 => texts.a("track or station change"),
        5 => texts.a("company founded"),
        _ => texts.a("checked during play"),
    }
}

/// The activity box shows a message's first paragraph, cut at 120 characters (briefings run to
/// pages; the dialog has the full text).
fn activity_line(text: &str) -> String {
    let first = text.split("\n\n").next().unwrap_or("").trim();
    let flat = first.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 120 {
        let cut: String = flat.chars().take(117).collect();
        format!("{}...", cut.trim_end())
    } else {
        flat
    }
}
