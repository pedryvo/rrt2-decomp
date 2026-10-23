//! The play flow through the application state, without a window: New Scenario starts a game
//! with `World::new_game`, the briefing and Start New Company open, the company is founded by
//! typing a name and pressing OK, refusals show in the dialog, and the simulation's messages
//! become dialogs (needs the install in `RT2_DIR`; skipped otherwise).

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_sim::GameMessage;
use rt2_ui::UiScale;
use winit::keyboard::KeyCode;

use super::dialogs::{A_FOUND_OK, Dialog};
use super::*;

fn install() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Games/railroad-tycoon-2"))
        })
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())
}

fn state() -> Option<AppState> {
    let install = install()?;
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
    Some(AppState::new(init))
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

fn type_text(s: &mut AppState, text: &str) {
    for c in text.chars() {
        let t = c.to_string();
        s.key(KeyCode::KeyA, true, false, Some(&t));
    }
}

#[test]
fn start_a_scenario_found_a_company_and_see_the_refusal() {
    let Some(mut s) = state() else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    let map = install().unwrap().join("maps/Africa After the Flood.MP2");
    s.start(&map).expect("a scenario map starts a new game");
    s.queue_opening_dialogs();
    // The briefing (the start sequence's trigger 6 message) is on top of Start New Company.
    assert!(matches!(top(&s), Some(Dialog::Message(_))), "{:?}", top(&s));
    s.dialog_activated(dialogs::A_DLG_BTN);
    let Some(Dialog::Founding { name, .. }) = top(&s) else {
        panic!("Start New Company expected, got {:?}", top(&s));
    };
    assert!(
        !name.is_empty(),
        "the default name comes from the LNG table"
    );
    // The HUD has no cash before the company exists.
    assert_eq!(game(&s).cash, None);

    // Type a new name (Backspace erases the default) and press OK.
    for _ in 0..40 {
        s.key(KeyCode::Backspace, true, false, None);
    }
    type_text(&mut s, "Rift Valley Line");
    let Some(Dialog::Founding { name, .. }) = top(&s) else {
        panic!("still the founding dialog");
    };
    assert_eq!(name, "Rift Valley Line");
    s.dialog_activated(A_FOUND_OK);
    assert!(top(&s).is_none(), "the dialog closes: {:?}", top(&s));
    let save = game(&s).world.save_ref().unwrap();
    let co = crate::buildtools::human_company(save).expect("the player has a company");
    let named = s
        .with_info(|i| i.company_name(co as u32))
        .expect("an info view");
    assert_eq!(named, "Rift Valley Line");
    assert!(game(&s).news.iter().any(|n| n.text.contains("Rift Valley")));

    // A second founding is refused with the original's message and the dialog stays open.
    let again = s.founding_dialog_any().expect("a founder");
    s.push_dialog(again);
    s.dialog_activated(A_FOUND_OK);
    let Some(Dialog::Founding { error, .. }) = top(&s) else {
        panic!("the dialog stays open");
    };
    assert!(error.is_some(), "the refusal is shown");
}

#[test]
fn simulation_messages_become_dialogs_and_activity_lines() {
    let Some(mut s) = state() else {
        return;
    };
    let map = install().unwrap().join("maps/#01B&O~1.MAP");
    s.start(&map).unwrap();
    // The briefing of the start sequence and nothing else is pending.
    s.poll_world();
    assert!(!game(&s).dialogs.is_empty(), "the briefing opened");
    let n = game(&s).dialogs.len();
    s.poll_world();
    assert_eq!(game(&s).dialogs.len(), n, "the queue was drained");
    // Newspapers: the economy, a town bonus; notices without a dialog stay in the activity box.
    let eco = s.message_dialog(&GameMessage::EconomyChanged { from: 2, to: 1 });
    assert!(matches!(eco, Some(Dialog::Newspaper { .. })));
    let split = s.message_dialog(&GameMessage::StockSplit {
        company: 1,
        ratio: 2,
    });
    assert!(split.is_none());
    let line = s.message_line(&GameMessage::StockSplit {
        company: 1,
        ratio: 2,
    });
    assert!(line.contains('2'), "{line}");
}
