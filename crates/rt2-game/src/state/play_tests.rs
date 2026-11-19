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

#[test]
fn campaign_opening_briefing_survives_buffered_clicks_and_order_notice_can_be_bypassed() {
    let Some(mut s) = state() else {
        return;
    };
    s.set_interactive_choices(true);
    s.open_page(menus::Page::NewCampaign);

    // A single mouse click on the already-selected row 0 should only arm the double-click timer,
    // not immediately launch the campaign while the player is moving to click <Play this Scenario>.
    s.via_mouse = true;
    s.menu_activated(menus::A_CAMP_LIST);
    s.via_mouse = false;
    assert!(
        matches!(s.screen, Screen::Menu(_)),
        "single click on pre-selected row 0 must not start the campaign immediately"
    );

    // Clicking <Play this Scenario> starts Campaign #1 (#01 B&O) with the Eagle Briefing Dialog open.
    s.menu_activated(menus::A_CAMP_PLAY);
    assert!(
        matches!(top(&s), Some(Dialog::Message(_))),
        "opening briefing dialog must be on screen: {:?}",
        top(&s)
    );

    // A buffered click at the exact coordinates of <Play this Scenario> / the Eagle Dialog OK button
    // (before 2 rendered frames + 350ms) must not dismiss the opening briefing dialog.
    s.cursor_moved(800.0, 620.0);
    s.mouse_button(true, true);
    s.mouse_button(true, false);
    assert!(
        matches!(top(&s), Some(Dialog::Message(_))),
        "buffered click must not dismiss the opening briefing dialog"
    );

    // After rendering frames past the 350ms opening guard, clicking OK closes the briefing dialog.
    for _ in 0..10 {
        s.update(50.0);
    }
    s.dialog_activated(dialogs::A_DLG_BTN);
    assert!(top(&s).is_none(), "briefing dialog closed after guard");

    // Campaign #3 (#03Bri~1.MAP) is beyond furthest_unlocked (0) and starts without a player company:
    // the first Play attempt shows the order notice; acknowledging it and pressing Play again bypasses
    // the notice and opens the Briefing Dialog on top of Start New Company (Dialog::Founding).
    s.open_page(menus::Page::NewCampaign);
    s.menu_event(&rt2_ui::UiEvent::Selected(menus::A_CAMP_LIST, 2));
    s.menu_activated(menus::A_CAMP_PLAY);
    assert!(
        matches!(&s.screen, Screen::Menu(m) if m.data.notice.is_some()),
        "first attempt on locked campaign #3 shows the order notice"
    );
    s.menu_activated(menus::A_NOTICE_OK);
    s.menu_activated(menus::A_CAMP_PLAY);
    assert!(
        matches!(top(&s), Some(Dialog::Message(_))),
        "second attempt bypasses the notice and opens the briefing dialog"
    );
    s.dialog_activated(dialogs::A_DLG_BTN);
    assert!(
        matches!(top(&s), Some(Dialog::Founding { .. })),
        "closing the briefing reveals Start New Company on Campaign #3: {:?}",
        top(&s)
    );
}
