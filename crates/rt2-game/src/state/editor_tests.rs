//! The map editor driven through the application state, without a window (needs the install in
//! `RT2_DIR`; skipped otherwise).

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::UiScale;

use super::editor_modals::*;
use super::editor_ui::{Group, Modal};
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
