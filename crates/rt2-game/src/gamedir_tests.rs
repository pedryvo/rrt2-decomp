//! The game-folder screen driven through the real input path: synthetic cursor moves, presses and
//! key events on the laid-out UI, as the window loop sends them. The installs are fake folders in
//! a temporary directory (empty files with the required names; nothing of the original game).

use std::path::PathBuf;

use rt2_ui::Kind;
use winit::keyboard::KeyCode;

use super::*;
use crate::install_detect::{Tmp, fake_install};

struct Fixture {
    tmp: Tmp,
    good: PathBuf,
    partial: PathBuf,
    cfg: PathBuf,
}

fn fixture(tag: &str) -> Fixture {
    let tmp = Tmp::new(tag);
    let home = &tmp.0;
    let good = home.join(".wine/drive_c/Program Files/Railroad Tycoon II Platinum");
    fake_install(&good);
    let partial = home.join(".wine/drive_c/GOG Games/Railroad Tycoon 2 Demo");
    std::fs::create_dir_all(partial.join("data")).unwrap();
    std::fs::write(partial.join("RT2_PLAT.EXE"), b"x").unwrap();
    let cfg = home.join("config/rrt2/config.toml");
    Fixture {
        tmp,
        good,
        partial,
        cfg,
    }
}

fn screen_at(f: &Fixture, viewport: (u32, u32), dpi: f64, can_cancel: bool) -> GameDirScreen {
    GameDirScreen::new(
        Params {
            config: Config {
                language: Some("en".to_owned()),
                ..Config::default()
            },
            config_path: Some(f.cfg.clone()),
            can_cancel,
            roots: Some(Roots::for_home(&f.tmp.0)),
        },
        viewport,
        dpi,
    )
}

fn screen(f: &Fixture) -> GameDirScreen {
    screen_at(f, (1280, 720), 1.0, false)
}

/// Moves the cursor to the middle of a node and presses and releases the left button.
fn click(s: &mut GameDirScreen, node: NodeId) {
    let r = s.ui.rect(node);
    let k = s.scale.factor();
    s.cursor_moved((r.x + r.w / 2.0) * k, (r.y + r.h / 2.0) * k);
    s.mouse_button(true, true);
    s.mouse_button(true, false);
}

fn click_play(s: &mut GameDirScreen) {
    let n = s.nodes.play.unwrap();
    click(s, n);
}

fn click_browse(s: &mut GameDirScreen) {
    let n = s.nodes.browse.unwrap();
    click(s, n);
}

/// Clicks the `row`th row of the list.
fn click_row(s: &mut GameDirScreen, row: usize) {
    let list = s.nodes.list.unwrap();
    let r = s.ui.rect(list);
    let row_h = match &s.ui.node(list).kind {
        Kind::List { row_h, .. } => *row_h,
        _ => unreachable!(),
    };
    let k = s.scale.factor();
    s.cursor_moved(
        (r.x + r.w / 2.0) * k,
        (r.y + row_h * (row as f32 + 0.5)) * k,
    );
    s.mouse_button(true, true);
    s.mouse_button(true, false);
}

fn message(s: &GameDirScreen) -> String {
    match &s.ui.node(s.nodes.message.unwrap()).kind {
        Kind::Label { text, .. } => text.clone(),
        _ => unreachable!(),
    }
}

fn enabled(s: &GameDirScreen, id: NodeId) -> bool {
    matches!(s.ui.node(id).kind, Kind::Button { enabled: true, .. })
}

fn press(s: &mut GameDirScreen, code: KeyCode) {
    s.key(code, true, false, false);
    s.key(code, false, false, false);
}

fn rows(s: &GameDirScreen) -> Vec<String> {
    match &s.ui.node(s.nodes.list.unwrap()).kind {
        Kind::List { items, .. } => items.clone(),
        _ => unreachable!(),
    }
}

#[test]
fn lists_what_was_found_and_preselects_the_usable_install() {
    let f = fixture("list");
    let s = screen(&f);
    assert_eq!(s.found().len(), 2);
    assert_eq!(s.found()[0].path, f.good);
    assert_eq!(s.found()[1].path, f.partial);
    assert_eq!(s.selected(), Some(0));
    assert_eq!(s.playable(), Some(&f.good));
    assert!(enabled(&s, s.nodes.play.unwrap()));
    let items = rows(&s);
    assert!(items[0].contains("Railroad Tycoon II Platinum") && items[0].contains("Wine prefix"));
    assert!(
        items[1].contains("GOG") && items[1].contains("incomplete"),
        "{items:?}"
    );
    assert!(message(&s).contains("Looks good"));
    // Nothing was written yet.
    assert!(!f.cfg.exists());
}

#[test]
fn clicking_an_incomplete_folder_says_what_is_missing_and_blocks_play() {
    let f = fixture("missing");
    let mut s = screen(&f);
    click_row(&mut s, 1);
    assert_eq!(s.selected(), Some(1));
    assert!(s.playable().is_none());
    let m = message(&s);
    assert!(
        m.contains("Missing") && m.contains("data/RT2.PK2") && m.contains("default2.lng"),
        "{m}"
    );
    assert!(!m.contains("RT2_PLAT.EXE"), "the exe is there: {m}");
    let play = s.nodes.play.unwrap();
    assert!(!enabled(&s, play));
    click(&mut s, play);
    assert!(s.take_effects().is_empty(), "a disabled Play does nothing");
    assert!(!f.cfg.exists());
    // Back to the good one: Play works again.
    click_row(&mut s, 0);
    assert!(enabled(&s, play));
    assert!(message(&s).contains("Looks good"));
}

#[test]
fn play_saves_the_folder_in_the_config_the_set_path_command_uses() {
    let f = fixture("play");
    let mut s = screen(&f);
    click_play(&mut s);
    let fx = s.take_effects();
    assert_eq!(fx.len(), 1);
    match &fx[0] {
        Effect::Play { path, config } => {
            assert_eq!(path, &f.good);
            assert_eq!(config.game_dir.as_deref(), Some(f.good.as_path()));
        }
        e => panic!("{e:?}"),
    }
    let saved = Config::load(&f.cfg).unwrap();
    assert_eq!(saved.game_dir.as_deref(), Some(f.good.as_path()));
    // The same install passes the check `set-path` and the startup use.
    assert!(paths::validate_install(saved.game_dir.as_deref().unwrap()).is_ok());
    // Other settings of the config are kept.
    assert_eq!(saved.language.as_deref(), Some("en"));
}

#[test]
fn a_config_that_cannot_be_written_shows_the_error_and_does_not_start() {
    let f = fixture("saveerr");
    let blocker = f.tmp.0.join("blocker");
    std::fs::write(&blocker, b"file").unwrap();
    let mut s = screen(&f);
    s.config_path = Some(blocker.join("sub/config.toml"));
    click_play(&mut s);
    assert!(s.take_effects().is_empty());
    assert!(
        message(&s).starts_with("Cannot save the choice"),
        "{}",
        message(&s)
    );
}

#[test]
fn keyboard_moves_the_selection_and_enter_plays_escape_leaves() {
    let f = fixture("keys");
    let mut s = screen(&f);
    press(&mut s, KeyCode::ArrowDown);
    assert_eq!(s.selected(), Some(1));
    assert!(s.playable().is_none());
    press(&mut s, KeyCode::Enter);
    assert!(
        s.take_effects().is_empty(),
        "Enter on an incomplete folder does not play"
    );
    press(&mut s, KeyCode::ArrowUp);
    assert_eq!(s.selected(), Some(0));
    press(&mut s, KeyCode::Enter);
    assert!(matches!(s.take_effects().as_slice(), [Effect::Play { .. }]));
    press(&mut s, KeyCode::Escape);
    assert!(matches!(s.take_effects().as_slice(), [Effect::Leave]));
}

#[test]
fn tab_reaches_the_buttons_and_enter_presses_the_focused_one() {
    let f = fixture("tab");
    let mut s = screen(&f);
    // Focus order: list, Choose folder, Quit, Play.
    press(&mut s, KeyCode::Tab);
    press(&mut s, KeyCode::Enter);
    assert!(matches!(
        s.take_effects().as_slice(),
        [Effect::Browse { .. }]
    ));
}

#[test]
fn choose_folder_opens_the_chooser_and_the_pick_joins_the_list() {
    let f = fixture("browse");
    let mut s = screen(&f);
    let browse = s.nodes.browse.unwrap();
    click(&mut s, browse);
    let fx = s.take_effects();
    let [Effect::Browse { start, title }] = fx.as_slice() else {
        panic!("{fx:?}");
    };
    assert_eq!(start.as_deref(), Some(f.good.as_path()));
    assert_eq!(title, "Select the Railroad Tycoon II folder");
    assert!(s.is_browsing());
    assert!(!enabled(&s, browse) && !enabled(&s, s.nodes.play.unwrap()));
    assert_eq!(message(&s), "Choosing a folder...");

    // The player picks a usable folder that no detection knows.
    let mine = f.tmp.0.join("elsewhere/my rt2 copy");
    fake_install(&mine);
    s.picked(Some(mine.clone()));
    assert!(!s.is_browsing());
    assert_eq!(s.found().len(), 3);
    assert_eq!(s.found()[0].source, Source::Picked);
    assert_eq!(s.selected(), Some(0));
    assert_eq!(
        s.playable().map(|p| p.canonicalize().unwrap()),
        Some(mine.canonicalize().unwrap())
    );
    assert!(enabled(&s, s.nodes.play.unwrap()));
    assert!(rows(&s)[0].contains("Chosen by you"));

    // A folder that is not an install: listed, selected and explained.
    click(&mut s, browse);
    s.take_effects();
    let empty = f.tmp.0.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    s.picked(Some(empty));
    assert!(s.playable().is_none());
    assert!(
        message(&s).contains("Missing: data/RT2.PK2, RT2_PLAT.EXE, default2.lng"),
        "{}",
        message(&s)
    );

    // Cancelling the chooser keeps the selection and re-enables the buttons.
    click(&mut s, browse);
    s.take_effects();
    s.picked(None);
    assert!(!s.is_browsing() && enabled(&s, browse));
    assert!(s.playable().is_none());
}

#[test]
fn picking_a_folder_that_holds_the_install_goes_down_into_it() {
    let f = fixture("descend");
    let mut s = screen(&f);
    let parent = f.tmp.0.join("GOG Games");
    let inner = parent.join("Railroad Tycoon II");
    fake_install(&inner);
    click_browse(&mut s);
    s.take_effects();
    s.picked(Some(parent));
    let sel = s.selected().unwrap();
    assert_eq!(
        s.found()[sel].path.canonicalize().unwrap(),
        inner.canonicalize().unwrap()
    );
    assert!(s.playable().is_some());
}

#[test]
fn picking_an_already_listed_folder_selects_it() {
    let f = fixture("again");
    let mut s = screen(&f);
    click_row(&mut s, 1);
    click_browse(&mut s);
    s.take_effects();
    s.picked(Some(f.good.clone()));
    assert_eq!(s.found().len(), 2);
    assert_eq!(s.selected(), Some(0));
}

#[test]
fn no_install_found_says_so_and_offers_only_the_chooser() {
    let tmp = Tmp::new("none");
    let mut s = GameDirScreen::new(
        Params {
            config: Config {
                language: Some("en".to_owned()),
                ..Config::default()
            },
            config_path: Some(tmp.0.join("c.toml")),
            can_cancel: false,
            roots: Some(Roots::for_home(&tmp.0)),
        },
        (1280, 720),
        1.0,
    );
    assert!(s.found().is_empty() && s.selected().is_none());
    assert!(message(&s).starts_with("No installation found"));
    assert!(!enabled(&s, s.nodes.play.unwrap()));
    click_browse(&mut s);
    assert!(matches!(
        s.take_effects().as_slice(),
        [Effect::Browse { .. }]
    ));
}

#[test]
fn the_configured_folder_is_listed_even_when_it_moved() {
    let f = fixture("moved");
    let gone = f.tmp.0.join("old/place");
    let s = GameDirScreen::new(
        Params {
            config: Config {
                game_dir: Some(gone.clone()),
                language: Some("en".to_owned()),
                ..Config::default()
            },
            config_path: Some(f.cfg.clone()),
            can_cancel: false,
            roots: Some({
                let mut r = Roots::for_home(&f.tmp.0);
                r.configured = Some(gone.clone());
                r
            }),
        },
        (1280, 720),
        1.0,
    );
    assert_eq!(s.found()[0].path, gone);
    assert_eq!(s.found()[0].source, Source::Configured);
    // The first usable one is selected instead.
    assert_eq!(s.playable(), Some(&f.good));
}

#[test]
fn back_button_and_quit_button_both_leave() {
    let f = fixture("leave");
    for (can_cancel, label) in [(false, "Quit"), (true, "Back")] {
        let mut s = screen_at(&f, (1280, 720), 1.0, can_cancel);
        let found_label = s
            .ui
            .focus_order()
            .into_iter()
            .filter_map(|id| match &s.ui.node(id).kind {
                Kind::Button { text, action, .. } if *action == A_LEAVE => Some((id, text.clone())),
                _ => None,
            })
            .next()
            .unwrap();
        assert_eq!(found_label.1, label);
        click(&mut s, found_label.0);
        assert!(matches!(s.take_effects().as_slice(), [Effect::Leave]));
    }
}

#[test]
fn the_failure_to_load_the_game_is_shown() {
    let f = fixture("loadfail");
    let mut s = screen(&f);
    s.failed("bad archive");
    assert_eq!(
        message(&s),
        "Cannot load the game from this folder: bad archive"
    );
}

/// Everything fits the canvas on the resolutions and scales the game runs at, including 4K at
/// 100 % and a small window.
#[test]
fn layout_fits_from_a_small_window_to_4k() {
    let f = fixture("layout");
    for (vp, dpi) in [
        ((640, 480), 1.0),
        ((1280, 720), 1.0),
        ((1920, 1080), 1.0),
        ((1920, 1080), 1.5),
        ((3840, 2160), 1.0),
        ((3840, 2160), 2.0),
        ((3840, 2160), 3.0),
    ] {
        let s = screen_at(&f, vp, dpi, false);
        let (w, h) = s.scale.logical_size(vp);
        for (_, id) in s.ui.draw_order() {
            let r = s.ui.rect(id);
            assert!(
                r.x >= -0.5 && r.y >= -0.5 && r.right() <= w + 0.5 && r.bottom() <= h + 0.5,
                "{vp:?} @{dpi}: {r:?} outside {w}x{h}"
            );
        }
        let list = s.ui.rect(s.nodes.list.unwrap());
        assert!(
            list.h >= 40.0 && list.w > 100.0,
            "{vp:?} @{dpi}: list {list:?}"
        );
    }
}

#[test]
fn the_logo_is_rasterized_for_the_scale() {
    let f = fixture("art");
    let mut s = screen_at(&f, (3840, 2160), 2.0, false);
    // 200 % of a logo that is at most LOGO_MAX logical units.
    assert_eq!(s.logo_px, (LOGO_MAX * 2.0) as u32);
    assert!(s.take_art().is_some());
    assert!(s.take_art().is_none(), "the skin is handed over once");
    s.dpi_changed(1.0);
    assert!(s.take_art().is_some(), "a new scale makes a new skin");
    assert_eq!(s.logo_px, LOGO_MAX as u32);
}
