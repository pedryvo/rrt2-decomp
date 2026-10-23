//! The purchase windows (docs/ui.md "Janelas de compra"; E1 Purchase Train, F2 Buy improvements)
//! driven through the real input path: synthetic cursor moves and presses on the laid-out UI.
//! They need the install in `RT2_DIR` (skipped otherwise).

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::{Kind, UiScale};

use super::buy_ui::*;
use super::dialogs::Dialog;
use super::*;
use crate::buildtools as bt;

fn state(viewport: (u32, u32)) -> Option<AppState> {
    let install = std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Games/railroad-tycoon-2"))
        })
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())?;
    let assets = Arc::new(GameAssets::open(&install).ok()?);
    let art = crate::artwork::load(&assets);
    let init = Init {
        install: install.clone(),
        assets,
        backdrop: art.backdrop,
        icons: art.icons,
        dialogs: art.dialogs,
        config: Config::default(),
        scale: UiScale::from_percent(100),
        scale_pinned: true,
        viewport,
    };
    let mut s = AppState::new(init);
    s.start(&install.join("games/Example save game.GAM")).ok()?;
    s.update(0.0);
    while s.has_dialog() {
        s.close_dialog();
    }
    s.update(0.0);
    Some(s)
}

fn game(s: &AppState) -> &GameState {
    match &s.screen {
        Screen::Game(g) => g,
        Screen::Menu(_) => panic!("no game"),
    }
}

fn center_of(s: &AppState, action: u32) -> Option<(f32, f32)> {
    let k = s.scale.factor();
    s.ui.draw_order().into_iter().find_map(|(_, id)| {
        let hit = match &s.ui.node(id).kind {
            Kind::Button { action: a, .. }
            | Kind::Toggle { action: a, .. }
            | Kind::Icon { action: a, .. }
            | Kind::Hotspot { action: a, .. } => *a == action,
            _ => false,
        };
        hit.then(|| {
            let r = s.ui.rect(id);
            ((r.x + r.w / 2.0) * k, (r.y + r.h / 2.0) * k)
        })
    })
}

fn click(s: &mut AppState, action: u32) {
    let p = center_of(s, action).unwrap_or_else(|| panic!("no control {action}"));
    s.cursor_moved(p.0, p.1);
    s.mouse_button(true, true);
    s.mouse_button(true, false);
    s.update(0.0);
}

fn labels(s: &AppState) -> Vec<String> {
    s.ui.draw_order()
        .into_iter()
        .filter_map(|(_, id)| match &s.ui.node(id).kind {
            Kind::Label { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn train_buy(s: &AppState) -> (usize, usize, i32) {
    match game(s).dialogs.last() {
        Some(Dialog::TrainBuy {
            sel, scroll, tons, ..
        }) => (*sel, *scroll, *tons),
        d => panic!("expected Purchase Train, got {d:?}"),
    }
}

#[test]
fn the_matrix_follows_the_speed_curve_of_the_original() {
    let Some(s) = state((1920, 1080)) else {
        return;
    };
    let (year, month) = s.date_ym();
    let (speeds, top, first) = s
        .with_game(|save, data, co| {
            let rows = bt::engine_rows(data, save, co, year, month);
            let r = rows.iter().find(|r| r.first_year == 1868)?;
            bt::engine_detail(data, save, co, r.index, 30)
                .map(|d| (d.speeds, d.top_mph, r.first_year))
        })
        .flatten()
        .expect("the 8-Wheeler (1868) is on sale in December 1875");
    assert_eq!(first, 1868);
    // One car on the level goes at the table's top speed (the guide's table A-1: 48 mph).
    assert_eq!((top, speeds[0][0]), (48, 48));
    // More cars and steeper grades only slow it down (never below 1 mph, 0x4cfea0).
    for g in 0..4 {
        for c in 0..6 {
            assert!(speeds[g][c] >= 1);
            if c > 0 {
                assert!(speeds[g][c] <= speeds[g][c - 1]);
            }
            if g > 0 {
                assert!(speeds[g][c] <= speeds[g - 1][c]);
            }
        }
    }
    // Regression values at 30 tons per car (Manual.html C609, an older build, shows 47 42 37 33
    // 29 26 on the level: the weight per car is not given there).
    assert_eq!(speeds[0], [48, 44, 40, 36, 33, 30]);
    assert_eq!(speeds[3], [6, 3, 1, 1, 1, 1]);
}

#[test]
fn train_buy_with_real_clicks_selects_changes_the_matrix_and_buys() {
    let Some(mut s) = state((1920, 1080)) else {
        return;
    };
    s.open_train_buy();
    assert_eq!(train_buy(&s), (0, 0, 30));
    let before = labels(&s);
    // A click on the second row selects that engine: the name and the figures change.
    click(&mut s, A_TB_ROW + 1);
    assert_eq!(train_buy(&s).0, 1);
    let after = labels(&s);
    assert_ne!(before, after);
    assert!(after.iter().any(|t| t.contains("Iron Duke")), "{after:?}");
    // The arrows add and take 5 tons per car; the speeds fall with the weight.
    let numbers = |s: &AppState| -> Vec<String> {
        labels(s)
            .into_iter()
            .filter(|t| t.parse::<i32>().is_ok())
            .collect()
    };
    let at_30 = numbers(&s);
    click(&mut s, A_TB_TONS_UP);
    assert_eq!(train_buy(&s).2, 35);
    assert_ne!(
        numbers(&s),
        at_30,
        "the matrix changes with the weight per car"
    );
    click(&mut s, A_TB_TONS_DOWN);
    click(&mut s, A_TB_TONS_DOWN);
    assert_eq!(train_buy(&s).2, 25);
    // Purchase sends the command: a train more and the train window opens.
    let trains = |s: &AppState| game(s).world.save_ref().unwrap().trains.ordered().count();
    let n = trains(&s);
    click(&mut s, super::build_dialogs::A_TB_OK);
    assert_eq!(trains(&s), n + 1);
    assert!(matches!(
        game(&s).dialogs.last(),
        Some(Dialog::TrainEdit { .. })
    ));
}

#[test]
fn train_buy_cancel_and_list_scroll() {
    let Some(mut s) = state((1280, 720)) else {
        return;
    };
    s.open_train_buy();
    click(&mut s, super::build_dialogs::A_CLOSE);
    assert!(!s.has_dialog());
    // The visible rows keep the selection in view.
    assert_eq!(AppState::keep_in_view(0, 9), 2);
    assert_eq!(AppState::keep_in_view(5, 2), 2);
    assert_eq!(AppState::keep_in_view(1, 4), 1);
    assert_eq!(default_tons(1829), 10);
    assert_eq!(default_tons(1875), 30);
    assert_eq!(default_tons(1950), 50);
}

#[test]
fn the_windows_fit_every_canvas() {
    for vp in [(1024, 768), (1920, 1080), (3440, 1440), (800, 600)] {
        let Some(mut s) = state(vp) else {
            return;
        };
        s.open_train_buy();
        let (cw, ch) = s.ui.logical_size();
        for (_, id) in s.ui.draw_order() {
            let r = s.ui.rect(id);
            assert!(
                r.x >= -1.0 && r.y >= -1.0 && r.x + r.w <= cw + 1.0 && r.y + r.h <= ch + 1.0,
                "{vp:?}: {r:?} outside {cw}x{ch}"
            );
        }
    }
}

#[test]
fn buy_improvements_with_real_clicks() {
    let Some(mut s) = state((1920, 1080)) else {
        return;
    };
    assert!(s.show_dialog("station-buy"));
    let Some(Dialog::StationBuy { station, .. }) = game(&s).dialogs.last().cloned() else {
        panic!("expected the Buy dialog");
    };
    let (year, month) = s.date_ym();
    let rows: Vec<u32> = s
        .with_game(|save, data, _| bt::improvements(data, save, station, year, month))
        .unwrap()
        .iter()
        .filter(|i| i.available && !i.owned && !i.excluded)
        .map(|i| i.row)
        .collect();
    assert!(rows.len() >= 2, "{rows:?}");
    // Nothing is selected at first (`0x407e80`): no text, the default picture.
    assert!(matches!(
        game(&s).dialogs.last(),
        Some(Dialog::StationBuy { last: u32::MAX, .. })
    ));
    // A click on a name selects it and shows its text; a click on the box checks it.
    click(&mut s, A_SB_PICK + rows[0]);
    assert!(
        labels(&s)
            .iter()
            .any(|t| t.starts_with("Cost") || t.contains("$"))
    );
    click(&mut s, super::build_dialogs::A_SB_ITEM + rows[1]);
    let mask_of = |s: &AppState| match game(s).dialogs.last() {
        Some(Dialog::StationBuy { mask, .. }) => *mask,
        _ => 0,
    };
    assert_eq!(mask_of(&s), 1 << rows[1]);
    // Purchase buys the checked row and the selected one.
    let bought = |s: &AppState| {
        bt::rec(&game(s).world.save_ref().unwrap().stations, station)
            .unwrap()
            .u32(0x49)
    };
    let before = bought(&s);
    click(&mut s, super::build_dialogs::A_SB_OK);
    let after = bought(&s);
    assert_eq!(
        after & !before,
        (1 << rows[0]) | (1 << rows[1]),
        "{before:#x} -> {after:#x}"
    );
}
