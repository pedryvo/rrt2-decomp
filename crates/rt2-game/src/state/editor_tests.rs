//! The map editor driven through the application state, without a window (needs the install in
//! `RT2_DIR`; skipped otherwise).

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::UiScale;

use super::editor_modals::*;
use super::editor_ui::{A_MODAL, Group, Modal};
use super::*;

fn home(p: &str) -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(p))
}

fn install() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| home("Games/railroad-tycoon-2"))
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())
}

fn state(tag: &str) -> Option<(AppState, PathBuf)> {
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
        scale: UiScale::default(),
        scale_pinned: true,
        viewport: (1600, 900),
    };
    let mut s = AppState::new(init);
    let dir = std::env::temp_dir().join(format!("rrt2-editor-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    s.set_saves_dir(Some(dir.clone()));
    s.open_editor(&install.join("maps/Local Industry.MP2"))
        .ok()?;
    Some((s, dir))
}

fn save_of(s: &AppState) -> &rt2_formats::map::Save {
    s.editor_world().unwrap().editor_save().unwrap()
}

#[test]
fn a_click_paints_and_undo_restores() {
    let Some((mut s, dir)) = state("paint") else {
        return;
    };
    s.editor_show_group("paint", 0, None);
    let before = save_of(&s).clone();
    // Class slot 7 is "snow" (class 8) in the solid palette.
    assert!(s.editor_activated(super::editor_ui::A_ITEM + 8));
    s.editor_apply(40, 30);
    assert_eq!(rt2_sim::editor::class_at(save_of(&s), 40, 30), 8);
    assert!(s.editor_dirty());
    assert!(s.editor_activated(super::editor_ui::A_UNDO));
    assert_eq!(save_of(&s).map.tiles, before.map.tiles);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn the_mouse_acts_on_the_tile_under_it() {
    let Some((mut s, dir)) = state("mouse") else {
        return;
    };
    s.editor_show_group("paint", 0, None);
    s.editor_activated(super::editor_ui::A_BRUSH);
    s.editor_activated(super::editor_ui::A_ITEM + 8);
    s.set_view(50.0, 30.0, 1.0, 0);
    s.update(0.0);
    let (cam, vp) = {
        let g = s.game().unwrap();
        (g.camera, s.viewport)
    };
    let map = save_of(&s).map.clone();
    let (px, py) = crate::pick::tile_to_screen(&cam, vp, &map, 50.5, 30.5);
    s.cursor_moved(px, py);
    s.mouse_button(true, true);
    s.mouse_button(true, false);
    assert_eq!(rt2_sim::editor::class_at(save_of(&s), 50, 30), 8);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_new_place_is_kept_by_ok_and_removed_by_cancel() {
    let Some((mut s, dir)) = state("place") else {
        return;
    };
    s.editor_show_group("add", 0, None);
    let n = s.editor_world().unwrap().editor_places().len();
    s.editor_apply(20, 20);
    assert!(matches!(s.ed().unwrap().modal, Some(Modal::Place(_))));
    // Cancel removes it again.
    s.editor_activated(super::editor_ui::A_MODAL + 1);
    assert_eq!(s.editor_world().unwrap().editor_places().len(), n);
    s.editor_apply(20, 20);
    s.editor_type(" Town");
    s.editor_activated(super::editor_ui::A_MODAL);
    let places = s.editor_world().unwrap().editor_places();
    assert_eq!(places.len(), n + 1);
    assert!(places.last().unwrap().name.ends_with(" Town"));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_saved_map_opens_again_and_the_install_is_never_written() {
    let Some((mut s, dir)) = state("save") else {
        return;
    };
    s.editor_show_group("trees", 0, None);
    s.editor_apply(30, 30);
    let path = dir.join("maps").join("test.MP2");
    s.editor_write(&path).unwrap();
    assert!(!s.editor_dirty());
    let mut s2 = state("save2").unwrap().0;
    s2.open_editor(&path).unwrap();
    assert_eq!(save_of(&s2).map.tiles, save_of(&s).map.tiles);
    // The saved file is valid and carries the overview.
    let raw = std::fs::read(&path).unwrap();
    let parsed = rt2_formats::map::Save::parse(&raw).unwrap();
    assert_eq!(
        rt2_sim::editor::preview(&parsed)[..],
        parsed.header[rt2_sim::editor::header::PREVIEW..][..170 * 170]
    );
    let inside = install().unwrap().join("maps").join("never.MP2");
    assert!(s.editor_write(&inside).is_err());
    assert!(!inside.exists());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn the_rules_dialog_validates_live_and_cancel_restores() {
    let Some((mut s, dir)) = state("rules") else {
        return;
    };
    let n = save_of(&s).rules.len();
    s.editor_show_dialog("rules");
    // Add a rule: its condition field has the focus.
    s.editor_activated(super::editor_ui::A_MODAL + 210);
    assert_eq!(save_of(&s).rules.len(), n + 1);
    s.editor_type("GameYear >= 1850 and");
    // Incomplete: the rule keeps the last valid (empty) condition.
    assert_eq!(save_of(&s).rules[n].condition, "");
    s.editor_type(" CompanyCash > 5");
    assert_eq!(
        save_of(&s).rules[n].condition,
        "GameYear >= 1850 and CompanyCash > 5"
    );
    assert_eq!(save_of(&s).rules[n].tokens_a.len(), 7);
    // Cancel gives the old rule list back.
    s.editor_activated(super::editor_ui::A_MODAL + 1);
    assert_eq!(save_of(&s).rules.len(), n);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn bulldozing_and_building_through_the_add_list() {
    let Some((mut s, dir)) = state("build") else {
        return;
    };
    let before = save_of(&s).objects.count;
    s.editor_show_group("add", 6, None);
    let mut done = false;
    'o: for row in 6..104usize {
        s.editor_select_row_for_test(row);
        for (x, y) in [(70, 40), (74, 44), (78, 36), (82, 48)] {
            s.editor_apply(x, y);
            if save_of(&s).objects.count > before {
                done = true;
                break 'o;
            }
        }
    }
    assert!(done, "some building fits");
    s.editor_show_group("bulldoze", 0, None);
    assert_eq!(s.ed().unwrap().group, Group::Bulldoze);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn leaving_with_unsaved_changes_asks_first() {
    let Some((mut s, dir)) = state("exit") else {
        return;
    };
    s.editor_show_group("fill", 0, None);
    s.editor_apply(10, 10);
    assert!(s.editor_dirty());
    s.editor_exit_request();
    assert!(matches!(s.ed().unwrap().modal, Some(Modal::Confirm(_))));
    s.editor_activated(super::editor_ui::A_MODAL + 1);
    assert!(s.editor_active());
    s.editor_exit_request();
    s.editor_activated(super::editor_ui::A_MODAL);
    assert!(!s.editor_active());
    let _: Option<NewMap> = None;
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn the_menu_entry_makes_a_blank_map_through_the_new_map_dialog() {
    let Some((mut s, dir)) = state("menu") else {
        return;
    };
    s.go_to_menu();
    s.activated(super::menus::A_EDITOR);
    s.activated(super::menus::A_ED_NEW);
    assert!(s.editor_active());
    assert!(matches!(s.ed().unwrap().modal, Some(Modal::NewMap(_))));
    // Wider by 10 and OK: the map is blank and of the chosen size.
    s.editor_activated(super::editor_ui::A_MODAL + 41);
    s.editor_activated(super::editor_ui::A_MODAL);
    let m = &save_of(&s).map;
    assert_eq!((m.width, m.height), (130, 120));
    assert!(save_of(&s).rules.is_empty());
    assert_eq!(rt2_sim::editor::class_at(save_of(&s), 5, 5), 5);
    std::fs::remove_dir_all(dir).ok();
}

fn act(s: &mut AppState, a: u32) {
    s.editor_activated(a);
}

#[test]
fn the_players_page_adds_edits_and_removes_players() {
    use super::editor_options_ui::*;
    let Some((mut s, dir)) = state("players") else {
        return;
    };
    assert!(s.editor_show_dialog("players"));
    let n0 = rt2_sim::editor_opts::players(save_of(&s)).len();
    act(&mut s, A_PLR_ADD);
    let list = rt2_sim::editor_opts::players(save_of(&s));
    assert_eq!(list.len(), n0 + 1);
    // The header summary follows the page (`0x46b890`).
    let hdr =
        |i: usize| u32::from_le_bytes(save_of(&s).header[0x150 + 4 * i..][..4].try_into().unwrap());
    assert_eq!(hdr(1) as usize, n0 + 1);
    // Select the new row, make it optional, computer only, then step its persona.
    s.editor_modal_row(A_PLR_LIST, n0);
    act(&mut s, A_PLR_OPT);
    act(&mut s, A_PLR_COMP);
    act(&mut s, A_PLR_NEXT);
    let me = rt2_sim::editor_opts::players(save_of(&s)).pop().unwrap();
    assert!(!me.mandatory);
    assert_eq!(me.controller, rt2_sim::editor_opts::Controller::Computer);
    assert_ne!(me.persona, 0);
    // The first player stays mandatory and human: the page refuses and says why.
    s.editor_modal_row(A_PLR_LIST, 0);
    act(&mut s, A_PLR_OPT);
    assert!(rt2_sim::editor_opts::players(save_of(&s))[0].mandatory);
    let Some(Modal::Options(o)) = s.ed().unwrap().modal.clone() else {
        panic!("options open");
    };
    assert!(!o.note.is_empty());
    // Cash through the field.
    s.editor_modal_row(A_PLR_LIST, n0);
    act(&mut s, A_MODAL + 620 + (F_CASH - 30));
    for _ in 0..8 {
        s.editor_modal_key(winit::keyboard::KeyCode::Backspace, None);
    }
    s.editor_type("123456");
    assert_eq!(
        rt2_sim::editor_opts::players(save_of(&s))
            .pop()
            .unwrap()
            .cash,
        123_456.0
    );
    s.editor_modal_row(A_PLR_LIST, n0);
    act(&mut s, A_PLR_DEL);
    assert_eq!(rt2_sim::editor_opts::players(save_of(&s)).len(), n0);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn engines_managers_ports_and_robbers_pages_edit_the_map() {
    use super::editor_options_ui::*;
    let Some((mut s, dir)) = state("pages") else {
        return;
    };
    assert!(s.editor_show_dialog("engines"));
    let was = rt2_sim::editor_opts::engine_allowed(save_of(&s), 5);
    act(&mut s, A_ENG_ITEM + 5);
    assert_eq!(rt2_sim::editor_opts::engine_allowed(save_of(&s), 5), !was);
    act(&mut s, A_ENG_PRESET + 1);
    assert!(rt2_sim::editor_opts::engine_count(save_of(&s)) > 10);
    assert!(s.editor_show_dialog("managers"));
    act(&mut s, A_MGR_ITEM + 2);
    assert!(!rt2_sim::editor_opts::manager_allowed(save_of(&s), 2));
    assert!(s.editor_show_dialog("pool"));
    act(&mut s, A_POOL_PRESET + 2);
    assert!(rt2_sim::editor_opts::pool_count(save_of(&s)) > 0);
    assert!(s.editor_show_dialog("ports"));
    act(&mut s, A_PORT_ANNUAL);
    act(&mut s, A_PORT_SUP + 1);
    let p = rt2_sim::editor_opts::port(save_of(&s), 0).unwrap();
    assert!(p.annual && p.demanded == rt2_sim::editor_opts::NO_CARGO);
    act(&mut s, A_MODAL + 620 + (F_PORT_Q - 30));
    s.editor_type("5");
    assert!(rt2_sim::editor_opts::port(save_of(&s), 0).unwrap().quantity >= 1.0);
    assert!(s.editor_show_dialog("robbers"));
    act(&mut s, A_MODAL + 620 + (F_ROB_NAME - 30));
    s.editor_type("Bart");
    act(&mut s, A_MODAL + 620 + (F_ROB_YEAR - 30));
    s.editor_type("1885");
    assert_eq!(
        rt2_sim::editor_opts::robber(save_of(&s), 0),
        Some(("Bart".to_string(), 1885))
    );
    act(&mut s, A_ROB_LEVEL + 1);
    assert_eq!(rt2_sim::editor_opts::robbery_level(save_of(&s)), 10);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn from_image_builds_a_map_with_the_heights_of_the_pcx() {
    let Some((mut s, dir)) = state("image") else {
        return;
    };
    // An 8-bit PCX 32x32, black at the left edge to white at the right, grey palette.
    let mut f = vec![0u8; 128];
    f[0] = 10;
    f[1] = 5;
    f[2] = 1;
    f[3] = 8;
    f[8] = 31;
    f[10] = 31;
    f[0x41] = 1;
    for _y in 0..32 {
        for x in 0..32u8 {
            let v = x * 8;
            if v >= 0xc0 {
                f.push(0xc1);
            }
            f.push(v);
        }
    }
    f.push(0x0c);
    for i in 0..=255u8 {
        f.extend([i; 3]);
    }
    let file = dir.join("hm.pcx");
    std::fs::write(&file, f).unwrap();
    s.editor_new_dialog();
    if let Some(Modal::NewMap(n)) = s.ed_mut().and_then(|e| e.modal.as_mut()) {
        n.image = file.to_string_lossy().into_owned();
        n.w = 40;
        n.h = 30;
    }
    act(&mut s, A_MODAL + 66);
    let m = &save_of(&s).map;
    assert_eq!((m.width, m.height), (40, 30));
    let w = m.width + 1;
    assert!(m.elevation[0] <= 1);
    assert!(m.elevation[w - 1] > 150);
    assert!(s.editor_dirty());
    // A missing file keeps the dialog and says so.
    s.editor_new_dialog();
    if let Some(Modal::NewMap(n)) = s.ed_mut().and_then(|e| e.modal.as_mut()) {
        n.image = "no-such-file.pcx".into();
    }
    act(&mut s, A_MODAL + 66);
    assert!(matches!(s.ed().unwrap().modal, Some(Modal::NewMap(_))));
    assert!(s.ed().unwrap().msg.is_some());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_territory_can_be_removed_from_its_dialog() {
    let Some((mut s, dir)) = state("terr") else {
        return;
    };
    s.editor_territory_new();
    act(&mut s, A_MODAL); // OK: a new territory 2
    let n = save_of(&s).territories.count;
    assert!(n >= 2);
    s.editor_show_group("territory", 0, Some(n - 1));
    s.editor_territory_edit();
    act(&mut s, A_MODAL + 2); // Remove Territory
    assert_eq!(save_of(&s).territories.count, n - 1);
    std::fs::remove_dir_all(dir).ok();
}
