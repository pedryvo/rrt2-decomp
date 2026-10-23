//! The company book pages, the instrument panel, the car list, the name/logo dialog, the ticker
//! and the campaign medals, driven through the real input path (synthetic mouse moves, presses
//! and key events on the laid-out UI). Needs the install in `RT2_DIR` and the converted Example
//! save (`~/.cache/rrt2/buildwork/ex_dl0.GAM`, see `build_tests.rs`); skipped otherwise.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::{Kind, UiScale};

use super::companybook::*;
use super::dialogs::Dialog;
use super::dialogs::brand::*;
use super::info_ui::{A_ROW, InfoScreen};
use super::train_panel::*;
use super::*;
use crate::buildtools as bt;

fn home(p: &str) -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(p))
}

fn state() -> Option<AppState> {
    let install = std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| home("Games/railroad-tycoon-2"))
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())?;
    let save = std::env::var_os("RT2_SAVE_2348")
        .map(PathBuf::from)
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
        viewport: (1920, 1080),
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

/// Physical position of the middle of the first enabled control with `action`.
fn center_of(s: &AppState, action: u32) -> Option<(f32, f32)> {
    let k = s.scale.factor();
    s.ui.draw_order().into_iter().find_map(|(_, id)| {
        let hit = match &s.ui.node(id).kind {
            Kind::Button {
                action: a, enabled, ..
            } => *a == action && *enabled,
            Kind::Toggle {
                action: a, enabled, ..
            } => *a == action && *enabled,
            Kind::Icon { action: a, .. } | Kind::Cells { action: a, .. } => *a == action,
            _ => false,
        };
        hit.then(|| {
            let r = s.ui.rect(id);
            ((r.x + r.w / 2.0) * k, (r.y + r.h / 2.0) * k)
        })
    })
}

fn click_at(s: &mut AppState, p: (f32, f32)) {
    s.cursor_moved(p.0, p.1);
    s.mouse_button(true, true);
    s.mouse_button(true, false);
}

fn click(s: &mut AppState, action: u32) {
    let p = center_of(s, action).unwrap_or_else(|| panic!("no enabled control {action}"));
    click_at(s, p);
}

fn type_text(s: &mut AppState, text: &str) {
    for c in text.chars() {
        let t = c.to_string();
        s.key(KeyCode::KeyA, true, false, Some(&t));
        s.key(KeyCode::KeyA, false, false, None);
    }
}

#[test]
fn company_book_sections_territories_and_company_list_by_clicks() {
    let Some(mut s) = state() else {
        eprintln!("install or converted save missing; skipping");
        return;
    };
    s.open_info(InfoScreen::Finance(0));
    click(&mut s, A_BOOK);
    assert!(matches!(s.info_screen(), Some(InfoScreen::Overview(_))));
    // The Overview shows the chairman and a manager line.
    let page = s.with_info(|i| i.overview_page(i.player_company().unwrap()));
    let page = page.unwrap();
    assert!(!page.left.is_empty() && !page.right.is_empty());
    click(&mut s, A_BOOK + 2);
    assert!(matches!(
        s.info_screen(),
        Some(InfoScreen::Territories(_, 0))
    ));
    click(&mut s, A_TERR_MODE + 1);
    assert!(matches!(
        s.info_screen(),
        Some(InfoScreen::Territories(_, 1))
    ));
    click(&mut s, A_TERR_MODE + 2);
    assert!(matches!(
        s.info_screen(),
        Some(InfoScreen::Territories(_, 2))
    ));
    // A press on the territory map opens the rights window of the territory under it. The
    // centre of the Example map lies inside the first territory.
    click(&mut s, A_TERR_MAP);
    assert!(
        matches!(top(&s), Some(Dialog::Rights { .. })),
        "{:?}",
        top(&s)
    );
    s.close_dialog();
    // The companies list: a click on its first row opens that company's finances.
    click(&mut s, A_BOOK + 3);
    assert!(matches!(s.info_screen(), Some(InfoScreen::Companies)));
    let list = s
        .ui
        .draw_order()
        .into_iter()
        .find(
            |&(_, id)| matches!(&s.ui.node(id).kind, Kind::List { action, .. } if *action == A_ROW),
        )
        .map(|(_, id)| id)
        .expect("row list");
    let r = s.ui.rect(list);
    let slot = s.with_info(|i| i.companies_list().table.ids[0]).unwrap();
    click_at(&mut s, (r.x + r.w / 2.0, r.y + 10.0));
    assert_eq!(s.info_screen(), Some(InfoScreen::Finance(slot)));
}

#[test]
fn name_and_logo_dialog_takes_typing_and_arrows() {
    let Some(mut s) = state() else {
        return;
    };
    s.open_info(InfoScreen::Overview(0));
    click(&mut s, A_BOOK_BRAND);
    let Some(Dialog::Brand { name, logo, .. }) = top(&s).cloned() else {
        panic!("{:?}", top(&s));
    };
    type_text(&mut s, "Z");
    let Some(Dialog::Brand { name: n2, .. }) = top(&s).cloned() else {
        panic!()
    };
    assert_eq!(n2, format!("{name}Z"));
    s.key(KeyCode::Backspace, true, false, None);
    click(&mut s, A_BRAND_NEXT);
    let Some(Dialog::Brand { logo: l2, .. }) = top(&s).cloned() else {
        panic!()
    };
    assert_eq!(i32::from(l2), (i32::from(logo) + 1) % 41);
    // OK reports the missing simulation command instead of faking the change.
    click(&mut s, A_BRAND_OK);
    let Some(Dialog::Brand { error, .. }) = top(&s).cloned() else {
        panic!()
    };
    assert!(error.is_some_and(|e| e.contains("0x27")));
    s.key(KeyCode::Escape, true, false, None);
    assert!(top(&s).is_none());
}

#[test]
fn instrument_panel_and_car_list_send_the_stop_commands() {
    let Some(mut s) = state() else {
        return;
    };
    assert!(s.show_dialog("train-edit"));
    let Some(Dialog::TrainEdit { train, .. }) = top(&s).cloned() else {
        panic!()
    };
    let stop0 = |s: &AppState| {
        let g = game(s);
        bt::train_stops(g.world.save_ref().unwrap(), train)
            .first()
            .cloned()
            .unwrap()
    };
    let wait0 = stop0(&s).wait;
    click(&mut s, A_TP_WAIT);
    assert_eq!(stop0(&s).wait, (wait0 + 1) % 3);
    // The class cycles Normal -> Express.
    let class = |s: &AppState| {
        s.with_info(|i| i.train_panel(train as u32).unwrap().class)
            .unwrap()
    };
    let c0 = class(&s);
    click(&mut s, A_TP_CLASS);
    assert_eq!(class(&s), (c0 + 1) % 4);
    // Sell mode cycles.
    let m0 = s.with_info(|i| i.stop_sell_mode(train as u32, 0)).unwrap();
    click(&mut s, A_TP_SELL);
    assert_eq!(
        s.with_info(|i| i.stop_sell_mode(train as u32, 0)).unwrap(),
        (m0 + 1) & 3
    );
    // Car list: clear, add two cars, OK.
    click(&mut s, A_TP_CARS);
    assert!(matches!(top(&s), Some(Dialog::CarList { .. })));
    click(&mut s, A_CL_CLEAR);
    let cargos: Vec<u8> = s
        .with_info(|i| i.car_cargo_choices().into_iter().map(|c| c.0).collect())
        .unwrap();
    assert!(cargos.len() >= 2);
    click(&mut s, A_CL_ADD + u32::from(cargos[1]));
    click(&mut s, A_CL_ADD + u32::from(cargos[0]));
    click(&mut s, A_CL_OK);
    assert!(matches!(top(&s), Some(Dialog::TrainEdit { .. })));
    assert_eq!(stop0(&s).cargo, vec![cargos[1], cargos[0]]);
    // The next-station marker follows the Go to this stop button.
    click(&mut s, A_TP_GOTO);
    let cur = |s: &AppState| {
        bt::rec(&game(s).world.save_ref().unwrap().trains, train)
            .map(|r| r.u8(0x3c))
            .unwrap()
    };
    assert_eq!(cur(&s), 0);
}

#[test]
fn ticker_scrolls_and_shows_every_company() {
    let items = vec![("Alpha".to_owned(), 10, 1.5), ("Beta".to_owned(), 20, -2.0)];
    let a = ticker::ticker_window(&items, 0.0, 60);
    let b = ticker::ticker_window(&items, 3000.0, 60);
    assert_ne!(a, b);
    assert!(a.starts_with("ALPHA $10 +1.50"), "{a}");
    assert!(ticker::ticker_window(&items, 0.0, 200).contains("BETA $20 -2.00"));
    let Some(mut s) = state() else {
        return;
    };
    s.open_info(InfoScreen::Stock);
    let first = ticker_band(&s);
    assert!(!first.is_empty());
    // Wall time moves the band without touching the simulation.
    let day = game(&s).world.minutes();
    for _ in 0..40 {
        s.update(0.0);
        s.ticker_advance(200.0);
    }
    assert_eq!(game(&s).world.minutes(), day);
    assert_ne!(ticker_band(&s), first);
}

fn ticker_band(s: &AppState) -> String {
    s.ui.draw_order()
        .into_iter()
        .find_map(|(_, id)| match &s.ui.node(id).kind {
            Kind::Label { text, color, .. } if color.1 == 0xe0 && text.contains('$') => {
                Some(text.clone())
            }
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn campaign_medals_are_kept_and_shown_on_the_board() {
    let Some(mut s) = state() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("rrt2-campaign-{}", std::process::id()));
    s.saves_dir = Some(dir.clone());
    if let Screen::Game(g) = &mut s.screen {
        g.campaign = Some((0, 2));
    }
    s.record_campaign_medal(crate::campaign::SILVER);
    s.record_campaign_medal(crate::campaign::BRONZE);
    let p = s.campaign_progress();
    assert_eq!(p.medal(0, 2), Some(crate::campaign::SILVER));
    assert!(s.campaign_medal_text(&p, 0, 2).contains("Silver"));
    assert!(s.campaign_medal_text(&p, 0, 3).is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
