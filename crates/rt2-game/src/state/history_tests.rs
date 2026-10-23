//! The event history window and the per-category notices, driven through the real input path
//! (synthetic cursor moves and presses on the laid-out UI, key events). They need the install
//! (`RT2_DIR` or the default folder) and are skipped without it.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::{Kind, UiScale};
use winit::keyboard::KeyCode;

use super::dialogs::Dialog;
use super::history::*;
use super::mainwin::A_NEWS;
use super::options_ui::A_OPT_TOGGLE;
use super::*;
use crate::notices::Category;
use crate::options::{Setting, Tab};

fn install() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Games/railroad-tycoon-2"))
        })
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())
}

fn fresh() -> Option<AppState> {
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
        scale: UiScale::from_percent(100),
        scale_pinned: true,
        viewport: (1920, 1080),
    };
    let mut s = AppState::new(init);
    s.set_saves_dir(None);
    Some(s)
}

fn playing() -> Option<AppState> {
    let mut s = fresh()?;
    let file = install()?.join("maps/Africa After the Flood.MP2");
    s.start(&file).ok()?;
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

/// Physical middle of the first widget with `action`, if it is laid out.
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

fn press_at(s: &mut AppState, p: (f32, f32)) {
    s.cursor_moved(p.0, p.1);
    s.mouse_button(true, true);
    s.mouse_button(true, false);
    s.update(0.0);
}

fn click(s: &mut AppState, action: u32) {
    let p = center_of(s, action).unwrap_or_else(|| panic!("no control {action}"));
    press_at(s, p);
}

/// Physical middle of row `row` of the history list (rows start at its scroll offset).
fn row_at(s: &AppState, row: usize) -> (f32, f32) {
    let k = s.scale.factor();
    s.ui.draw_order()
        .into_iter()
        .find_map(|(_, id)| match &s.ui.node(id).kind {
            Kind::List {
                action: A_HIST_LIST,
                row_h,
                scroll,
                ..
            } => {
                let r = s.ui.rect(id);
                Some((
                    (r.x + 20.0) * k,
                    (r.y + row_h * ((row - scroll) as f32 + 0.5)) * k,
                ))
            }
            _ => None,
        })
        .expect("the history list")
}

fn click_row(s: &mut AppState, row: usize) {
    let p = row_at(s, row);
    press_at(s, p);
}

fn list_len(s: &AppState) -> usize {
    s.ui.draw_order()
        .into_iter()
        .find_map(|(_, id)| match &s.ui.node(id).kind {
            Kind::List {
                action: A_HIST_LIST,
                items,
                ..
            } => Some(items.len()),
            _ => None,
        })
        .unwrap_or(0)
}

fn target(s: &AppState) -> (f32, f32) {
    let t = game(s).camera.target;
    (t.x, t.y)
}

#[test]
fn the_history_lists_filters_and_centers_the_camera_on_a_click() {
    let Some(mut s) = playing() else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    s.add_event(
        Category::Towns,
        Some((10, 12)),
        "A town offered a bonus".into(),
        "A town offered a bonus to the first railroad.".into(),
    );
    s.add_event(
        Category::Competitors,
        Some((40, 50)),
        "A rival connected two towns".into(),
        "A rival connected two towns.".into(),
    );
    s.add_event(
        Category::Finance,
        None,
        "Bond issued".into(),
        "Bond issued".into(),
    );
    let total = game(&s).news.len();
    assert!(total >= 4, "the welcome line and the three events");

    // The activity box shows its history first; its History button opens the window.
    click(&mut s, A_NEWS);
    click(&mut s, A_HIST_OPEN);
    assert!(matches!(
        game(&s).dialogs.last(),
        Some(Dialog::History { filter: 0, .. })
    ));
    assert_eq!(list_len(&s), total);

    // Filter by category: only the towns' events remain.
    click(&mut s, A_HIST_FILTER + 1 + Category::Towns.index() as u32);
    assert_eq!(list_len(&s), 1);
    // A click on the row selects it and moves the camera to the event's tile.
    let before = target(&s);
    click_row(&mut s, 0);
    assert_ne!(target(&s), before);
    assert_eq!(target(&s), (10.5, 12.5));
    assert!(center_of(&s, A_HIST_GOTO).is_some());

    // Another category, another place.
    click(
        &mut s,
        A_HIST_FILTER + 1 + Category::Competitors.index() as u32,
    );
    assert_eq!(list_len(&s), 1);
    click_row(&mut s, 0);
    assert_eq!(target(&s), (40.5, 50.5));

    // An event without a place leaves the camera alone and its button disabled.
    click(&mut s, A_HIST_FILTER + 1 + Category::Finance.index() as u32);
    let before = target(&s);
    let finance = list_len(&s);
    assert!(finance >= 1);
    click_row(&mut s, 0);
    assert_eq!(target(&s), before);
    assert!(s.history_selected_has_no_pos_for_test());

    // "All" again, pick the rival's event and use Show on map: the window closes there.
    click(&mut s, A_HIST_FILTER);
    assert_eq!(list_len(&s), total);
    // Newest first: finance (0), competitors (1), towns (2).
    click_row(&mut s, 1);
    assert_eq!(target(&s), (40.5, 50.5));
    click(&mut s, A_HIST_GOTO);
    assert!(!s.has_dialog());
    assert_eq!(target(&s), (40.5, 50.5));

    // Escape closes the window too.
    click(&mut s, A_HIST_OPEN);
    assert!(s.has_dialog());
    s.key(KeyCode::Escape, true, false, None);
    assert!(!s.has_dialog());
}

#[test]
fn a_category_turned_off_in_the_options_shows_no_notice_but_stays_in_the_history() {
    let (Some(mut on), Some(mut off)) = (fresh(), fresh()) else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    // Turn the scenario notices off through the Notices tab of the Options screen.
    off.open_options(Tab::Notices);
    click(&mut off, A_OPT_TOGGLE + Setting::NoticeScenario.index());
    assert!(!off.config().notices.scenario);
    assert!(off.config().notices.finance, "the others stay on");
    assert!(
        off.take_effects()
            .iter()
            .any(|e| matches!(e, Effect::SaveConfig(c) if !c.notices.scenario)),
        "the choice is written to config.toml"
    );
    off.options_activated(super::options_ui::A_OPT_CLOSE);

    let map = install().unwrap().join("maps/#01B&O~1.MAP");
    for s in [&mut on, &mut off] {
        s.start(&map).unwrap();
        s.poll_world();
    }
    // With the notices on the briefing opens and shows in the activity box...
    assert!(!game(&on).dialogs.is_empty(), "the briefing opened");
    assert!(game(&on).news.iter().filter(|n| n.shown).count() >= 1);
    // ...with the category off neither the dialog nor the line appears,
    assert!(
        game(&off).dialogs.is_empty(),
        "no pop-up: {:?}",
        game(&off).dialogs.len()
    );
    let scenario_lines = game(&off)
        .news
        .iter()
        .filter(|n| n.category == Category::Scenario && n.shown)
        .count();
    assert_eq!(scenario_lines, 0);
    // but the event is in the history.
    let hidden = game(&off)
        .news
        .iter()
        .filter(|n| n.category == Category::Scenario && !n.shown)
        .count();
    assert!(hidden >= 1, "the briefing is recorded in the history");
}
