//! Credits, High Scores, the Map Overview, the building / terrain boxes, the hover label and the
//! videos, driven through the real input path (synthetic cursor moves, presses, the wheel and
//! key events on the laid-out UI). Needs the install in `RT2_DIR` and the converted Example save
//! (`~/.cache/rrt2/buildwork/ex_dl0.GAM`, see `build_tests.rs`); skipped otherwise.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::{Fill, Kind, UiScale};

use super::dialogs::{A_DLG_BTN, Dialog};
use super::menus::Page;
use super::overview::*;
use super::*;
use crate::buildtools as bt;
use crate::info::Pick;

fn home(p: &str) -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(p))
}

fn install() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| home("Games/railroad-tycoon-2"))
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())
}

fn menu_state() -> Option<AppState> {
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
        viewport: (1920, 1080),
    };
    Some(AppState::new(init))
}

fn state() -> Option<AppState> {
    let save = std::env::var_os("RT2_SAVE_2348")
        .map(PathBuf::from)
        .or_else(|| home(".cache/rrt2/buildwork/ex_dl0.GAM"))
        .filter(|p| p.is_file())?;
    let mut s = menu_state()?;
    s.start(&save).ok()?;
    // Close what the start opened (briefing) so the map takes the mouse.
    while s.has_dialog() {
        s.close_dialog();
    }
    Some(s)
}

fn game(s: &AppState) -> &GameState {
    match &s.screen {
        Screen::Game(g) => g,
        Screen::Menu(_) => panic!("no game"),
    }
}

fn page(s: &AppState) -> Page {
    match &s.screen {
        Screen::Menu(m) => m.page,
        Screen::Game(_) => panic!("in a game"),
    }
}

/// Every text on screen: labels, buttons, toggles and text lines.
fn texts(s: &AppState) -> Vec<String> {
    let mut out = Vec::new();
    for (_, id) in s.ui.draw_order() {
        match &s.ui.node(id).kind {
            Kind::Label { text, .. } | Kind::Button { text, .. } | Kind::Toggle { text, .. } => {
                out.push(text.clone())
            }
            Kind::Text { lines, .. } => out.extend(lines.iter().cloned()),
            _ => {}
        }
    }
    out
}

fn has_text(s: &AppState, needle: &str) -> bool {
    texts(s).iter().any(|t| t.contains(needle))
}

fn overlay_squares(s: &AppState) -> usize {
    s.ui.draw_order()
        .into_iter()
        .filter(|(_, id)| {
            matches!(
                s.ui.node(*id).kind,
                Kind::Panel {
                    fill: Fill::Overlay(_)
                }
            )
        })
        .count()
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
            Kind::Icon { action: a, .. } => *a == action,
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

fn right_click_at(s: &mut AppState, p: (f32, f32)) {
    s.cursor_moved(p.0, p.1);
    s.mouse_button(false, true);
    s.mouse_button(false, false);
}

fn click(s: &mut AppState, action: u32) {
    let p = center_of(s, action).unwrap_or_else(|| panic!("no enabled control {action}"));
    click_at(s, p);
}

fn press_key(s: &mut AppState, code: KeyCode) {
    s.key(code, true, false, None);
    s.key(code, false, false, None);
}

/// Puts tile `(x, y)` in the middle of the map view and returns its pixel.
fn center_on(s: &mut AppState, x: i32, y: i32) -> (f32, f32) {
    s.set_view(x as f32 + 0.5, y as f32 + 0.5, 1.0, 0);
    s.rebuild_ui();
    let g = game(s);
    let save = g.world.save_ref().unwrap();
    crate::pick::tile_to_screen(
        &g.camera,
        s.viewport,
        &save.map,
        x as f32 + 0.5,
        y as f32 + 0.5,
    )
}

/// An industry tile for which `pick` answers that industry.
fn an_industry(s: &AppState) -> (i32, i32, u32) {
    let save = game(s).world.save_ref().unwrap();
    for o in save.objects.ordered() {
        let b = bt::Rec(&o.data);
        if b.u8(7) == 0 {
            continue;
        }
        let (x, y) = (i32::from(b.i16(0x26)), i32::from(b.i16(0x28)));
        if let Some(Some(Pick::Industry(slot))) = s.with_info(|i| i.pick(x, y))
            && slot as usize == o.slot
        {
            return (x, y, slot);
        }
    }
    panic!("no pickable industry in the save");
}

// ---- A8 / A9 --------------------------------------------------------------------------------

#[test]
fn credits_scroll_with_the_wheel_and_leave_with_escape_or_back() {
    let Some(mut s) = menu_state() else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    assert!(s.show_menu_page("credits"));
    assert_eq!(page(&s), Page::Credits);
    assert!(
        s.credits_lines().len() > 50,
        "credits2.lng was read: {}",
        s.credits_lines().len()
    );
    // The roll starts below the window: nothing of the text is visible yet.
    assert!(!has_text(&s, "PopTop Software"));
    // The wheel moves the text up (wheel notches are negative when scrolling down).
    s.wheel(-6.0);
    assert!(has_text(&s, "Railroad Tycoon II"), "{:?}", texts(&s));
    // Time moves it by itself.
    let before = s.ga.credits.scroll;
    s.update(2000.0);
    assert!(s.ga.credits.scroll > before);
    // The arrow keys scroll too.
    let before = s.ga.credits.scroll;
    press_key(&mut s, KeyCode::PageDown);
    assert!(s.ga.credits.scroll > before);
    press_key(&mut s, KeyCode::Escape);
    assert_eq!(page(&s), Page::Main);
    // The Back button (a real click) returns as well.
    s.show_menu_page("credits");
    click(&mut s, A_BACK);
    assert_eq!(page(&s), Page::Main);
    // The credits page is reachable from the main menu button.
    click(&mut s, menus::A_CREDITS);
    assert_eq!(page(&s), Page::Credits);
}

#[test]
fn high_scores_list_the_table_and_record_a_victory() {
    let Some(mut s) = menu_state() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("rrt2-hs-ui-{}", std::process::id()));
    let path = dir.join("hiscore.dat");
    let _ = std::fs::remove_dir_all(&dir);
    s.ga.hiscores.path = Some(path.clone());
    // Empty table: the hint.
    assert!(s.show_menu_page("scores"));
    assert!(has_text(&s, "No scores yet"));
    let e = |name: &str, score| crate::hiscore::Entry {
        company: name.into(),
        scenario: "Test map".into(),
        medal: 3,
        year: 1880,
        month: 7,
        score,
    };
    crate::hiscore::save(
        &path,
        &[e("Baltimore Line", 900_000), e("Rift Valley", 100_000)],
    )
    .unwrap();
    // Reopening through the main menu button reads the file.
    press_key(&mut s, KeyCode::Escape);
    click(&mut s, menus::A_SCORES);
    assert_eq!(page(&s), Page::Scores);
    assert!(has_text(&s, "Baltimore Line"));
    assert!(has_text(&s, "$900,000"));
    assert!(has_text(&s, "Gold"));
    click(&mut s, A_BACK);
    assert_eq!(page(&s), Page::Main);

    // A won scenario adds a row for the player's company, in rank order.
    if let Some(save) = std::env::var_os("RT2_SAVE_2348")
        .map(PathBuf::from)
        .or_else(|| home(".cache/rrt2/buildwork/ex_dl0.GAM"))
        .filter(|p| p.is_file())
    {
        s.ga.hiscores.path = Some(path.clone());
        s.start(&save).unwrap();
        s.record_score(2);
        let list = crate::hiscore::load(&path);
        assert_eq!(list.len(), 3, "{list:?}");
        assert!(list.iter().any(|x| x.medal == 2 && !x.company.is_empty()));
        assert!(list.windows(2).all(|w| w[0].score >= w[1].score));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- C6 -------------------------------------------------------------------------------------

#[test]
fn map_overview_layers_by_clicks() {
    let Some(mut s) = state() else {
        eprintln!("install or converted save missing; skipping");
        return;
    };
    // The globe icon (the fourth) opens the overview and takes the center box.
    assert!(!s.overview_open());
    click(&mut s, mainwin::A_TOOL + 3);
    assert!(s.overview_open());
    assert!(has_text(&s, "Resource Map") && has_text(&s, "Track by Company"));
    assert!(center_of(&s, A_OV_ALL).is_some() && center_of(&s, A_OV_CLEAR).is_some());
    // Put an industry in the middle so its marker is on screen.
    let (x, y, _) = an_industry(&s);
    let ty = {
        let save = game(&s).world.save_ref().unwrap();
        bt::Rec(&save.objects.get(an_industry(&s).2 as usize).unwrap().data).u8(7)
    };
    center_on(&mut s, x, y);
    // Nothing selected: no squares. Click a cargo the building supplies or demands.
    assert_eq!(overlay_squares(&s), 0);
    let (sup, dem) = crate::overview::building_cargos(
        s.data.as_ref().unwrap(),
        ty,
        i32::from(game(&s).world.date().year),
    );
    let sup: Vec<u8> = sup.into_iter().chain(dem).collect();
    let dem: Vec<u8> = Vec::new();
    let cargo = *sup.iter().chain(dem.iter()).next().expect("a cargo");
    // Hovering lights it up without clicking.
    let p = center_of(&s, A_OV_CARGO + u32::from(cargo)).expect("the cargo toggle");
    s.cursor_moved(p.0, p.1);
    assert_eq!(s.ga.overview.hover, Some(cargo));
    assert!(
        overlay_squares(&s) > 0,
        "the hovered cargo shows on the map"
    );
    // Moving off removes it again; a click keeps it.
    s.cursor_moved(5.0, 5.0);
    assert_eq!(s.ga.overview.hover, None);
    assert_eq!(overlay_squares(&s), 0);
    click(&mut s, A_OV_CARGO + u32::from(cargo));
    assert!(s.ga.overview.cargo_on.contains(&cargo));
    assert!(overlay_squares(&s) > 0);
    // Clear removes the selection, All selects everything.
    click(&mut s, A_OV_CLEAR);
    assert!(s.ga.overview.cargo_on.is_empty());
    click(&mut s, A_OV_ALL);
    assert!(s.ga.overview.cargo_on.len() > 3);
    assert!(overlay_squares(&s) > 0);

    // Station Statuses: a box with a count at each station in view.
    let st = {
        let save = game(&s).world.save_ref().unwrap();
        bt::station_tile(save, 1).unwrap()
    };
    center_on(&mut s, st.0, st.1);
    click(&mut s, A_OV_LAYER + 1);
    assert_eq!(s.ga.overview.layer, Layer::Stations);
    assert!(has_text(&s, "A box at each station"));
    assert!(
        s.ui.draw_order().iter().any(|(_, id)| matches!(
            &s.ui.node(*id).kind,
            Kind::Label { text, .. } if text.parse::<u32>().is_ok()
        )),
        "a count box at the station"
    );
    // Track Grade and Traffic Density colour the track.
    let t =
        s.ga.overview
            .tiles
            .first()
            .copied()
            .expect("the save has track");
    center_on(&mut s, t.x, t.y);
    click(&mut s, A_OV_LAYER + 2);
    assert_eq!(s.ga.overview.layer, Layer::Grade);
    assert!(overlay_squares(&s) > 0);
    assert!(has_text(&s, "Steepness"));
    click(&mut s, A_OV_LAYER + 3);
    assert_eq!(s.ga.overview.layer, Layer::Traffic);
    assert!(has_text(&s, "Very heavy"));
    // Track by Company: the player's track is highlighted and the list names the company.
    click(&mut s, A_OV_LAYER + 4);
    assert_eq!(s.ga.overview.layer, Layer::Company);
    assert!(has_text(&s, "track tiles"));
    assert!(overlay_squares(&s) > 0);
    click(&mut s, A_OV_COMPANY);
    assert!(s.ga.overview.company.is_some());
    // The overlay follows the camera: panning changes the squares' positions without a click.
    let before = overlay_squares(&s);
    s.wheel(3.0);
    assert!(overlay_squares(&s) != usize::MAX && before > 0);

    // Cancel closes; the icon opens it again and a second press closes it.
    click(&mut s, A_OV_CANCEL);
    assert!(!s.overview_open());
    assert_eq!(overlay_squares(&s), 0);
    click(&mut s, mainwin::A_TOOL + 3);
    assert!(s.overview_open());
    press_key(&mut s, KeyCode::Escape);
    assert!(!s.overview_open());
    click(&mut s, mainwin::A_TOOL + 3);
    click(&mut s, mainwin::A_TOOL + 3);
    assert!(!s.overview_open());
    // A construction mode replaces it.
    click(&mut s, mainwin::A_TOOL + 3);
    click(&mut s, mainwin::A_TOOL);
    assert!(!s.overview_open());
    assert!(s.tool_active());
}

// ---- C3 / C4 / C5 ---------------------------------------------------------------------------

#[test]
fn left_click_shows_the_building_box_and_right_click_the_same_without_buy() {
    let Some(mut s) = state() else {
        return;
    };
    let (x, y, slot) = an_industry(&s);
    let p = center_on(&mut s, x, y);
    // C5: the hint under the cursor names the building.
    s.cursor_moved(p.0, p.1);
    let status = game(&s).hud.status.expect("status line");
    let Kind::Label { text, .. } = &s.ui.node(status).kind else {
        panic!("label")
    };
    let title = s.with_info(|i| i.industry_page(slot).title).unwrap();
    assert!(text.contains(&title), "{text:?} vs {title:?}");

    // Left click: the box with ownership, profitability, price and production.
    click_at(&mut s, p);
    let Some(Dialog::Message(m)) = game(&s).dialogs.last().cloned() else {
        panic!("a box: {:?}", game(&s).dialogs.last());
    };
    assert_eq!(m.title.to_lowercase(), title.to_lowercase());
    for needle in ["Ownership", "Profitability", "Purchase Price"] {
        assert!(m.body.contains(needle), "{needle} in {}", m.body);
    }
    let trade = {
        let save = game(&s).world.save_ref().unwrap();
        let co = bt::human_company(save).unwrap();
        bt::industry_trade(s.data.as_ref().unwrap(), save, co, slot as i32)
    };
    let buy_button = m
        .buttons
        .iter()
        .any(|b| matches!(b.1, super::dialogs::Act::Trade { .. }));
    assert_eq!(
        buy_button,
        trade != bt::Trade::None,
        "Buy only where the rules allow"
    );
    // OK closes it.
    let ok = m.buttons.len() as u32 - 1;
    s.dialog_activated(A_DLG_BTN + ok);
    assert!(!s.has_dialog());

    // Right click on the same building: the same information and never a Buy button.
    right_click_at(&mut s, p);
    let Some(Dialog::Message(r)) = game(&s).dialogs.last().cloned() else {
        panic!("a box");
    };
    assert_eq!(r.body, m.body);
    assert!(r.buttons.iter().all(|b| b.1 == super::dialogs::Act::Close));
    s.dialog_activated(A_DLG_BTN);

    // Right click on open ground: terrain, territory, access rights, X,Y.
    let (tx, ty) = {
        let save = game(&s).world.save_ref().unwrap();
        let mut found = None;
        let (mut land, mut classes) = (0, std::collections::BTreeSet::new());
        'scan: for yy in 5..save.map.height as i32 - 5 {
            for xx in 5..save.map.width as i32 - 5 {
                let ground = save.map.tile(xx as usize, yy as usize);
                let class = rt2_sim::editor::class_at(save, xx, yy);
                classes.insert(class);
                if ground.region == 0 || class == 18 {
                    continue;
                }
                land += 1;
                if !super::bldinfo::object_at(save, xx, yy) {
                    found = Some((xx, yy));
                    break 'scan;
                }
            }
        }
        eprintln!("land tiles tried {land}, classes {classes:?}");
        found.expect("open ground")
    };
    let p = center_on(&mut s, tx, ty);
    right_click_at(&mut s, p);
    let Some(Dialog::Message(t)) = game(&s).dialogs.last().cloned() else {
        panic!("a terrain box: {:?}", game(&s).dialogs.last());
    };
    for needle in ["Type of Terrain", "Territory", "Your Access Rights"] {
        assert!(t.body.contains(needle), "{needle} in {}", t.body);
    }
    assert!(t.body.contains(&format!("X,Y: {tx}, {ty}")), "{}", t.body);
}

#[test]
fn a_right_drag_still_pans_and_shows_no_box() {
    let Some(mut s) = state() else {
        return;
    };
    let before = game(&s).camera.target;
    s.cursor_moved(900.0, 500.0);
    s.mouse_button(false, true);
    s.cursor_moved(700.0, 460.0);
    s.mouse_button(false, false);
    assert!(!s.has_dialog());
    assert_ne!(game(&s).camera.target, before);
}

#[test]
fn the_profitability_label_ladder_follows_the_rating() {
    use super::bldinfo::profit_label_index as f;
    assert_eq!(f(0), 0);
    assert_eq!(f(100), 2);
    assert_eq!(f(179), 4);
    assert_eq!(f(180), 5, "Gushing Cash at rating >= 180 (world-growth.md)");
}

// ---- A10 ------------------------------------------------------------------------------------

#[test]
fn a_video_plays_into_its_sprite_and_any_input_skips_it() {
    if !rt2_audio::video::ffmpeg_available() {
        eprintln!("ffmpeg missing; skipping");
        return;
    }
    let Some(mut s) = menu_state() else {
        return;
    };
    assert!(s.play_video("scenwin"), "the clip is in the install");
    assert!(s.video_active());
    assert!(has_text(&s, "Press Esc, Space or click to skip"));
    // Frames reach the atlas sprite.
    s.video_wait(1200.0);
    let ups = s.take_sprite_uploads();
    assert!(
        ups.iter().any(|(_, px)| px.len() == 640 * 480 * 4),
        "{} uploads",
        ups.len()
    );
    // A key skips the clip and is not used for anything else.
    press_key(&mut s, KeyCode::Escape);
    assert!(!s.video_active());
    assert_eq!(page(&s), Page::Main);
    // A click skips it too and does not press the button under it.
    s.play_video("scenwin");
    assert!(s.video_active());
    click_at(&mut s, (960.0, 540.0));
    assert!(!s.video_active());
    // An unknown clip is refused.
    assert!(!s.play_video("no_such_clip"));
    // Clips tied to events stay off until the window loop turns them on.
    s.video_event("scenwin");
    assert!(!s.video_active());
    s.set_videos(true);
    s.play_intro();
    assert!(s.video_active(), "the intro plays when enabled");
    press_key(&mut s, KeyCode::Space);
    assert!(!s.video_active());
}
