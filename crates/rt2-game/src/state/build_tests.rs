//! The construction tools and windows driven through the application state, without a window:
//! a track drag, a station, a train bought and edited, an improvement bought (needs the install
//! in `RT2_DIR` and the Example save converted to the `0x2348` layout, see
//! `tests/e2e_build.rs`; skipped otherwise).

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::UiScale;

use super::build_dialogs::*;
use super::dialogs::Dialog;
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

fn cash(s: &AppState) -> f64 {
    let g = game(s);
    let save = g.world.save_ref().unwrap();
    bt::cash(save, bt::human_company(save).unwrap())
}

#[test]
fn track_station_train_and_improvement_through_the_ui() {
    let Some(mut s) = state() else {
        eprintln!("install or converted save missing; skipping");
        return;
    };
    let (x1, y1) = {
        let save = game(&s).world.save_ref().unwrap();
        bt::station_tile(save, 1).unwrap()
    };
    // Track: the drag from a station, then the same command the release sends.
    let cash0 = cash(&s);
    assert!(s.show_tool("track", Some((x1, y1)), Some((x1 - 6, y1))));
    let plan = game(&s).build.plan.clone().expect("a plan for the drag");
    assert!(plan.error.is_none(), "{:?}", plan.error);
    assert!(plan.cost > 0.0 && !plan.tiles.is_empty());
    s.apply_tool();
    assert!(
        (cash0 - cash(&s) - plan.cost).abs() < 1.0,
        "the preview is the price"
    );

    // Station: next to the new track; the first orientation the simulation accepts.
    assert!(s.show_tool("station", None, None));
    let mut built = false;
    'f: for dx in -3..=3 {
        for dy in -3..=3 {
            if let Screen::Game(g) = &mut s.screen {
                g.build.hover = Some((x1 - 6 + dx, y1 + dy));
            }
            s.recompute_plan();
            if game(&s)
                .build
                .plan
                .as_ref()
                .is_some_and(|p| p.error.is_none())
            {
                let n0 = game(&s)
                    .world
                    .save_ref()
                    .unwrap()
                    .stations
                    .ordered()
                    .count();
                s.tool_commit();
                let n1 = game(&s)
                    .world
                    .save_ref()
                    .unwrap()
                    .stations
                    .ordered()
                    .count();
                assert_eq!(n1, n0 + 1);
                built = true;
                break 'f;
            }
        }
    }
    assert!(built, "a station site near the new track");
    // The station window opened on the new station; its Buy dialog buys an improvement.
    let station = match game(&s).dialogs.last() {
        Some(Dialog::Station { station }) => *station,
        d => panic!("expected the station window, got {d:?}"),
    };
    assert!(s.build_dialog_activated(A_SD_BUY));
    let (year, month) = s.date_ym();
    let items = {
        let save = game(&s).world.save_ref().unwrap();
        bt::improvements(s.data.as_ref().unwrap(), save, station, year, month)
    };
    let pick = items
        .iter()
        .find(|i| i.available && !i.owned && !i.excluded)
        .unwrap();
    let before = cash(&s);
    assert!(s.build_dialog_activated(A_SB_ITEM + pick.row));
    assert!(s.build_dialog_activated(A_SB_OK));
    let mask = bt::rec(&game(&s).world.save_ref().unwrap().stations, station)
        .unwrap()
        .u32(0x49);
    assert!(mask & (1 << pick.row) != 0, "the improvement is bought");
    assert!(cash(&s) < before);

    // Train: Purchase Train, default route, then a third stop in the editable detail.
    assert!(game(&s).dialogs.is_empty());
    s.open_train_buy();
    assert!(matches!(
        game(&s).dialogs.last(),
        Some(Dialog::TrainBuy { .. })
    ));
    let trains0 = game(&s).world.save_ref().unwrap().trains.ordered().count();
    assert!(s.build_dialog_activated(A_TB_OK));
    assert_eq!(
        game(&s).world.save_ref().unwrap().trains.ordered().count(),
        trains0 + 1
    );
    let train = match game(&s).dialogs.last() {
        Some(Dialog::TrainEdit { train, .. }) => *train,
        d => panic!("expected the train window, got {d:?}"),
    };
    let stops = |s: &AppState| bt::train_stops(game(s).world.save_ref().unwrap(), train).len();
    assert_eq!(stops(&s), 2);
    assert!(s.build_dialog_activated(A_TE_ADD));
    assert_eq!(stops(&s), 3);
    assert!(s.build_dialog_activated(A_TE_DELETE));
    assert_eq!(stops(&s), 2);
    // Selling takes two clicks (confirmation).
    assert!(s.build_dialog_activated(A_TE_SELL));
    assert!(
        game(&s)
            .world
            .save_ref()
            .unwrap()
            .trains
            .get(train as usize)
            .is_some()
    );
    assert!(s.build_dialog_activated(A_TE_SELL));
    assert!(
        game(&s)
            .world
            .save_ref()
            .unwrap()
            .trains
            .get(train as usize)
            .is_none()
    );
}

#[test]
fn refusals_are_shown_not_sent() {
    let Some(mut s) = state() else {
        eprintln!("install or converted save missing; skipping");
        return;
    };
    // A station in the middle of nowhere is refused with the original's message (LNG 535).
    assert!(s.show_tool("station", None, Some((5, 5))));
    let plan = game(&s).build.plan.clone().unwrap();
    assert!(plan.error.is_some());
    let n0 = game(&s)
        .world
        .save_ref()
        .unwrap()
        .stations
        .ordered()
        .count();
    s.apply_tool();
    assert_eq!(
        game(&s)
            .world
            .save_ref()
            .unwrap()
            .stations
            .ordered()
            .count(),
        n0
    );
    assert!(game(&s).build.message.as_ref().is_some_and(|m| !m.1));
}

/// Tiles of the player's stations (owner = the human company).
fn player_stations(s: &AppState) -> Vec<(i32, i32)> {
    let save = game(s).world.save_ref().unwrap();
    let co = bt::human_company(save).unwrap();
    bt::company_stations(save, co)
        .into_iter()
        .filter_map(|id| bt::station_tile(save, id))
        .collect()
}

/// Screen position of a tile's centre with the current camera.
fn tile_px(s: &AppState, t: (i32, i32)) -> (f32, f32) {
    let g = game(s);
    let save = g.world.save_ref().unwrap();
    crate::pick::tile_to_screen(
        &g.camera,
        s.viewport,
        &save.map,
        t.0 as f32 + 0.5,
        t.1 as f32 + 0.5,
    )
}

#[test]
fn dragging_track_between_two_stations_with_the_mouse() {
    let Some(mut s) = state() else {
        eprintln!("install or converted save missing; skipping");
        return;
    };
    let st = player_stations(&s);
    assert!(st.len() >= 2, "the save has two stations of the player");
    // Two stations not already linked, the closest pair.
    let mut best = None;
    for (i, a) in st.iter().enumerate() {
        for b in &st[i + 1..] {
            let d = (a.0 - b.0).abs() + (a.1 - b.1).abs();
            if d > 2 && best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, *a, *b));
            }
        }
    }
    let (_, a, b) = best.unwrap();
    // Frame both stations.
    if let Screen::Game(g) = &mut s.screen {
        g.camera.target = rt2_render::camera::Vec2::new(
            (a.0 + b.0) as f32 / 2.0 + 0.5,
            (a.1 + b.1) as f32 / 2.0 + 0.5,
        );
        g.camera.zoom = 24.0;
    }
    s.select_tool(super::build_ui::ToolKind::Track);
    assert!(s.tool_active());
    let (pa, pb) = (tile_px(&s, a), tile_px(&s, b));
    let target0 = game(&s).camera.target;
    let cash0 = cash(&s);
    // The press lands on the building the player sees, a tile beside the station's own.
    let near = tile_px(&s, (a.0, a.1 + 1));
    s.cursor_moved(near.0, near.1);
    s.mouse_button(true, true);
    assert_eq!(
        game(&s).build.from,
        Some(a),
        "the press on the station starts the drag"
    );
    // Move in steps; the preview follows and the camera stays.
    for k in 1..=8 {
        let f = k as f32 / 8.0;
        s.cursor_moved(pa.0 + (pb.0 - pa.0) * f, pa.1 + (pb.1 - pa.1) * f);
    }
    assert_eq!(
        game(&s).camera.target,
        target0,
        "the left drag does not pan"
    );
    let plan = game(&s).build.plan.clone().expect("a live preview");
    assert!(plan.error.is_none(), "{:?}", plan.error);
    assert!(!plan.tiles.is_empty());
    s.mouse_button(true, false);
    assert!(cash(&s) < cash0, "released: the track is laid and paid");
    assert!(game(&s).build.from.is_none());
    // Both stations' tiles are connected by track now: the path exists.
    let save = game(&s).world.save_ref().unwrap();
    let built = |t: (i32, i32)| save.map.tile(t.0 as usize, t.1 as usize).track_id != 0;
    assert!(built(a) && built(b));
}

#[test]
fn escape_cancels_the_drag_then_the_tool_and_the_right_button_pans() {
    let Some(mut s) = state() else {
        eprintln!("install or converted save missing; skipping");
        return;
    };
    let a = player_stations(&s)[0];
    s.select_tool(super::build_ui::ToolKind::Track);
    let p = tile_px(&s, a);
    s.cursor_moved(p.0, p.1);
    s.mouse_button(true, true);
    assert!(game(&s).build.from.is_some());
    s.key(KeyCode::Escape, true, false, None);
    assert!(game(&s).build.from.is_none() && game(&s).build.tool.is_some());
    s.mouse_button(true, false);
    // The right button drags the camera while the tool stays selected.
    let t0 = game(&s).camera.target;
    s.cursor_moved(300.0, 300.0);
    s.mouse_button(false, true);
    s.cursor_moved(380.0, 340.0);
    s.mouse_button(false, false);
    assert_ne!(game(&s).camera.target, t0);
    assert!(game(&s).build.tool.is_some());
    s.key(KeyCode::Escape, true, false, None);
    assert!(game(&s).build.tool.is_none());
}
