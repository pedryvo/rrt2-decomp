//! The main window's chrome (docs/ui.md "Janela principal"): the layout geometry for any canvas,
//! the tab strip, the rotate/zoom buttons, the compass and the message history, driven through
//! the real input path (synthetic cursor moves and presses on the laid-out UI). The behaviour
//! tests need the install in `RT2_DIR` (skipped otherwise); the geometry tests do not.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::{Kind, UiScale};

use super::mainwin::*;
use super::*;

fn install() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Games/railroad-tycoon-2"))
        })
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())
}

fn state(viewport: (u32, u32), scale: u32) -> Option<AppState> {
    let install = install()?;
    let assets = Arc::new(GameAssets::open(&install).ok()?);
    let art = crate::artwork::load(&assets);
    let init = Init {
        install: install.clone(),
        assets,
        backdrop: art.backdrop,
        icons: art.icons,
        dialogs: art.dialogs,
        config: Config::default(),
        scale: UiScale::from_percent(scale),
        scale_pinned: true,
        viewport,
    };
    let mut s = AppState::new(init);
    s.start(&install.join("maps/Africa After the Flood.MP2"))
        .ok()?;
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

/// Physical middle of the first widget with `action` (buttons, toggles, icons and hotspots).
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

/// The sprite of the compass picture, if the install has the art.
fn compass_sprite(s: &AppState) -> Option<rt2_ui::SpriteId> {
    let n = game(s).hud.compass?;
    match s.ui.node(n).kind {
        Kind::Image { sprite, .. } => Some(sprite),
        _ => None,
    }
}

#[test]
fn geometry_fits_every_canvas_and_pins_the_parts_to_their_edges() {
    // 640x360 is 1080p at 300 %, 960x540 is 4K at 400 % (never offered), the rest are logical
    // canvases of 1080p, 1440p ultrawide, 4K at 200 % and 4:3.
    for (cw, ch) in [
        (640.0, 360.0),
        (960.0, 540.0),
        (1920.0, 1080.0),
        (3440.0, 1440.0),
        (1920.0, 1080.0 / 2.0 * 2.0),
        (1024.0, 768.0),
        (5120.0, 1440.0),
    ] {
        let g = geometry(cw, ch);
        let inside = |r: [f32; 4], what: &str| {
            assert!(
                r[0] >= -0.5 && r[1] >= -0.5 && r[0] + r[2] <= cw + 0.5 && r[1] + r[3] <= ch + 0.5,
                "{what} {r:?} leaves {cw}x{ch}"
            );
        };
        inside(g.rl(0.0, 6.0, 85.0, 397.0), "left rail");
        inside(g.rr(954.0, 63.0, 70.0, 340.0), "right rail");
        inside(g.rr(766.0, 497.0, 237.0, 214.0), "activity box");
        inside(g.rr(929.0, 23.0, 80.0, 46.0), "compass");
        inside(g.center(), "center box");
        inside(g.span(0.0, 1024.0, 403.0, 365.0), "bottom frame");
        // The right parts touch the right edge, the left parts the left edge.
        assert!((g.xr(1024.0) - cw).abs() < 1.0, "{cw}x{ch}");
        assert_eq!(g.x(0.0), 0.0);
        // The extra width goes to the center box only.
        let narrow = geometry(1024.0 * g.dz, 768.0 * g.dz);
        assert!(g.center()[2] >= narrow.center()[2] - 1.0);
        assert!(g.ph < ch, "the frame must leave room for the map");
    }
}

#[test]
fn the_compass_picture_follows_the_view_rotation() {
    let all: Vec<usize> = (0..4).map(compass_index).collect();
    let mut sorted = all.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted,
        [0, 1, 2, 3],
        "four rotations, four pictures: {all:?}"
    );
    // Rotation 0 is the original's opening view: north towards the upper right.
    assert_eq!(compass_index(0), 0);
    assert_eq!(compass_index(4), compass_index(0));
}

#[test]
fn clicking_the_tabs_switches_the_list_and_the_hint_names_the_tab() {
    let Some(mut s) = state((1920, 1080), 100) else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    // The original opens on the Companies tab.
    assert_eq!(game(&s).center.tab, 3);
    for tab in [0usize, 1, 2, 3] {
        click(&mut s, A_CENTER_TAB + tab as u32);
        assert_eq!(game(&s).center.tab, tab, "tab {tab}");
    }
    // Hovering a tab writes its hint on the status line.
    let p = center_of(&s, A_CENTER_TAB).unwrap();
    s.cursor_moved(p.0, p.1);
    s.update(0.0);
    let status = game(&s).hud.status.unwrap();
    let Kind::Label { text, .. } = &s.ui.node(status).kind else {
        panic!("status is a label");
    };
    assert!(text.to_lowercase().contains("station"), "{text:?}");
    // And a floating tooltip with the tab's name appears over it, hiding when the mouse leaves.
    let (tt, label) = game(&s).hud.tooltip.unwrap();
    assert!(s.ui.is_shown(tt));
    let Kind::Label { text, .. } = &s.ui.node(label).kind else {
        panic!("tooltip label");
    };
    assert_eq!(text, &s.center_tab_names()[0]);
    s.cursor_moved(960.0, 300.0);
    s.update(0.0);
    assert!(!s.ui.is_shown(tt));
}

#[test]
fn the_rotate_and_zoom_buttons_turn_the_view_and_the_compass() {
    let Some(mut s) = state((1920, 1080), 100) else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    let (rot0, zoom0) = (game(&s).camera.rotation, game(&s).camera.zoom);
    let sprite0 = compass_sprite(&s);
    click(&mut s, A_ROTATE_RIGHT);
    assert_eq!(game(&s).camera.rotation, (rot0 + 1) % 4);
    if let Some(s0) = sprite0 {
        let s1 = compass_sprite(&s).unwrap();
        assert_ne!(s0, s1, "the compass turns with the view");
        click(&mut s, A_ROTATE_LEFT);
        assert_eq!(compass_sprite(&s), Some(s0), "and back");
    }
    click(&mut s, A_ZOOM_IN);
    assert!(game(&s).camera.zoom > zoom0);
    click(&mut s, A_ZOOM_OUT);
    assert!((game(&s).camera.zoom - zoom0).abs() < zoom0 * 0.01);
}

#[test]
fn the_activity_box_swaps_the_logo_for_the_message_history_and_the_briefing() {
    let Some(mut s) = state((1920, 1080), 100) else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    assert!(!game(&s).center.news_open);
    assert!(center_of(&s, A_BRIEFING).is_none(), "the logo shows first");
    click(&mut s, A_NEWS);
    assert!(game(&s).center.news_open);
    click(&mut s, A_BRIEFING);
    assert!(s.has_dialog(), "the Briefing button opens the briefing");
    while s.has_dialog() {
        s.close_dialog();
    }
    click(&mut s, A_NEWS);
    assert!(!game(&s).center.news_open);
}

#[test]
fn the_chrome_works_on_the_widest_and_the_smallest_canvases() {
    for (vp, scale) in [
        ((3440, 1440), 100),
        ((1920, 1080), 300),
        ((3840, 2160), 200),
    ] {
        let Some(mut s) = state(vp, scale) else {
            eprintln!("RT2_DIR is missing; skipping");
            return;
        };
        // Every tab and the navigation are reachable by a real click at this size.
        click(&mut s, A_CENTER_TAB + 1);
        assert_eq!(game(&s).center.tab, 1, "{vp:?} @{scale}");
        let rot = game(&s).camera.rotation;
        click(&mut s, A_ROTATE_LEFT);
        assert_eq!(game(&s).camera.rotation, (rot + 3) % 4, "{vp:?} @{scale}");
    }
}

#[test]
fn tool_icons_fill_plates_one_to_eight_and_stations_are_not_named_on_the_map() {
    // The first plate stays empty; the eight tools take the rest with no gap (the original).
    assert_eq!(TOOL_PLATE, [1, 2, 3, 4, 5, 6, 7, 8]);
    const { assert!(!maplabels::SHOW_STATION_NAMES) };
}
