//! The financial dialogs driven through the application state, without a window: a bond is issued
//! and repaid, stock issued, the dividend moved within the board's cap, shares traded, a merger
//! refused for lack of cash, a manager hired and a vote answered. Needs the install in `RT2_DIR`
//! and the Example save of the fidelity set (person 1 is the human chairman of company 4,
//! company 5 belongs to a computer); skipped otherwise.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_sim::fincmd::FinCmd;
use rt2_ui::UiScale;

use super::dialogs::Dialog;
use super::dialogs::fin::*;
use super::*;

const OWN: i32 = 4;
const OTHER: i32 = 5;

fn home(p: &str) -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(p))
}

fn state() -> Option<AppState> {
    let install = std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| home("Games/railroad-tycoon-2"))
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())?;
    let save = std::env::var_os("RT2_FIDELITY_DIR")
        .map(|d| PathBuf::from(d).join("Example__e4b2f915__s1234__d0.GAM"))
        .or_else(|| home(".cache/rrt2/buildwork/ex_dl0.GAM"))
        .filter(|p| p.is_file())?;
    let assets = Arc::new(GameAssets::open(&install).ok()?);
    let art = crate::artwork::load(&assets);
    let init = Init {
        install,
        assets,
        backdrop: art.backdrop,
        icons: art.icons,
        dialogs: art.dialogs,
        config: Config::default(),
        scale: UiScale::default(),
        scale_pinned: true,
        viewport: (1600, 900),
    };
    let mut s = AppState::new(init);
    s.start(&save).ok()?;
    Some(s)
}

fn game(s: &AppState) -> &GameState {
    match &s.screen {
        Screen::Game(g) => g,
        Screen::Menu(_) => panic!("no game"),
    }
}

fn top(s: &AppState) -> Option<&Dialog> {
    game(s).dialogs.last()
}

fn co_i32(s: &AppState, c: i32, o: usize) -> i32 {
    let d = &game(s)
        .world
        .save_ref()
        .unwrap()
        .companies
        .get(c as usize)
        .unwrap()
        .data;
    i32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

fn co_f64(s: &AppState, c: i32, o: usize) -> f64 {
    let d = &game(s)
        .world
        .save_ref()
        .unwrap()
        .companies
        .get(c as usize)
        .unwrap()
        .data;
    f64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}

fn holdings(s: &AppState, person: i32, idx: usize) -> i32 {
    let d = &game(s)
        .world
        .save_ref()
        .unwrap()
        .persons
        .get(person as usize)
        .unwrap()
        .data;
    let o = 0x1d3 + 4 * idx;
    i32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

#[test]
fn bonds_stock_and_dividend_through_the_dialogs() {
    let Some(mut s) = state() else {
        eprintln!("install or save missing; skipping");
        return;
    };
    // The company book offers the buttons of the player's own company.
    let labels: Vec<String> = s.fin_buttons(OWN as u32).into_iter().map(|b| b.0).collect();
    assert!(labels.iter().any(|l| l == "Issue Bonds"), "{labels:?}");
    assert!(
        !labels.iter().any(|l| l == "Repay Bond"),
        "no bonds yet: {labels:?}"
    );

    // Issue Bonds: the dialog shows the rate and OK sends 500 000 for 30 years.
    let cash0 = co_f64(&s, OWN, 0x4766);
    s.fin_act(FinAct::Bond, OWN);
    assert!(matches!(top(&s), Some(Dialog::Fin(FinDlg::Bond { .. }))));
    assert!(s.fin_activated(A_FIN_OK));
    assert_eq!(co_i32(&s, OWN, 0x40), 1);
    assert_eq!(co_f64(&s, OWN, 0x4766) - cash0, 490_000.0);
    assert!(top(&s).is_none(), "the dialog closed");
    assert!(game(&s).news.iter().any(|n| n.text.contains("Bond issued")));

    // Repay Bond appears; it costs the principal and the 2% fee.
    let labels: Vec<String> = s.fin_buttons(OWN as u32).into_iter().map(|b| b.0).collect();
    assert!(labels.iter().any(|l| l == "Repay Bond"));
    let cash1 = co_f64(&s, OWN, 0x4766);
    s.fin_act(FinAct::Repay, OWN);
    assert!(s.fin_activated(A_FIN_OK));
    assert_eq!(co_i32(&s, OWN, 0x40), 0);
    assert_eq!(co_f64(&s, OWN, 0x4766), cash1 - 510_000.0);

    // Issue Stock: shares grow by the lot; the second issue within a year is refused.
    let shares0 = co_i32(&s, OWN, 0x2c);
    s.fin_act(FinAct::Stock, OWN);
    assert!(s.fin_activated(A_FIN_OK));
    assert!(co_i32(&s, OWN, 0x2c) > shares0);
    s.fin_act(FinAct::Stock, OWN);
    assert!(
        matches!(top(&s), Some(Dialog::Message(_))),
        "refused: {:?}",
        top(&s)
    );
    s.close_dialog();

    // Buy Back Stock takes shares out again.
    let shares1 = co_i32(&s, OWN, 0x2c);
    s.fin_act(FinAct::Buyback, OWN);
    if matches!(top(&s), Some(Dialog::Fin(_))) {
        assert!(s.fin_activated(A_FIN_OK));
        assert!(co_i32(&s, OWN, 0x2c) < shares1);
    } else {
        s.close_dialog();
    }

    // Dividend: + moves by the original's step within the cap, - goes back, OK stores it.
    s.fin_act(FinAct::Dividend, OWN);
    assert!(s.fin_activated(A_FIN_PLUS));
    assert!(s.fin_activated(A_FIN_PLUS));
    let Some(Dialog::Fin(FinDlg::Dividend { value, .. })) = top(&s).cloned() else {
        panic!("dividend dialog");
    };
    assert!((value - 0.10).abs() < 1e-6, "{value}");
    assert!(s.fin_activated(A_FIN_MINUS));
    assert!(s.fin_activated(A_FIN_OK));
    let d = f32::from_bits(co_i32(&s, OWN, 0x30) as u32);
    assert!((d - 0.05).abs() < 1e-6, "{d}");
}

#[test]
fn trading_shares_merger_refusal_and_manager() {
    let Some(mut s) = state() else {
        return;
    };
    // The stock page shows the personal account.
    assert!(!s.personal_lines().is_empty());
    // Trade: Buy moves the lot into the portfolio, Sell takes it out (the lot is the original's).
    s.fin_act(FinAct::Trade, OTHER);
    assert!(matches!(top(&s), Some(Dialog::Fin(FinDlg::Trade { .. }))));
    assert!(s.fin_activated(A_FIN_BUY));
    let held = holdings(&s, 1, 1);
    assert!(held >= 1000, "bought {held}");
    assert!(s.fin_activated(A_FIN_SELL));
    assert!(holdings(&s, 1, 1) < held);
    s.close_dialog();

    // Merger: the offer moves, and OK without the cash is refused with the original's message.
    s.fin_act(FinAct::Merger, OWN);
    let Some(Dialog::Fin(FinDlg::Merger { offer, target, .. })) = top(&s).cloned() else {
        panic!("merger dialog, got {:?}", top(&s));
    };
    assert_eq!(target, OTHER);
    assert!(s.fin_activated(A_FIN_PLUS10));
    let Some(Dialog::Fin(FinDlg::Merger { offer: o2, .. })) = top(&s).cloned() else {
        panic!();
    };
    assert_eq!(o2, offer + 10);
    assert!(s.fin_activated(A_FIN_OK));
    assert!(matches!(top(&s), Some(Dialog::Message(_))), "{:?}", top(&s));
    s.close_dialog();
    s.close_dialog();

    // Manager: the first candidate is hired.
    let cand = co_i32(&s, OWN, 0x140);
    s.fin_act(FinAct::Manager, OWN);
    assert!(s.fin_activated(A_FIN_OK));
    assert_eq!(co_i32(&s, OWN, 0x13c), cand);
}

#[test]
fn a_merger_vote_reaches_the_human_shareholder() {
    let Some(mut s) = state() else {
        return;
    };
    // Company 5 proposes to buy company 4, where person 1 is chairman and holds 8000 shares.
    // The computer chairman votes yes, the human is asked.
    let r = match &mut s.screen {
        Screen::Game(g) => g.world.apply_fin_unchecked(FinCmd::MergerPropose {
            buyer: OTHER,
            target: OWN,
            offer: 40,
        }),
        Screen::Menu(_) => unreachable!(),
    };
    assert!(r.is_ok(), "{r:?}");
    s.poll_world();
    let Some(Dialog::Fin(FinDlg::Vote(v))) = top(&s).cloned() else {
        panic!("vote dialog, got {:?}", top(&s));
    };
    assert!(v.merger && v.person == 1);
    // No: the vote runs and the result comes back as a message.
    assert!(s.fin_activated(A_FIN_NO));
    s.poll_world();
    assert!(
        matches!(top(&s), Some(Dialog::Message(_))),
        "result dialog: {:?}",
        top(&s)
    );
    assert!(game(&s).world.pending_votes().is_empty());
}
