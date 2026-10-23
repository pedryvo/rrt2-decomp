//! The Options screen, the Save / Load machinery and the interactive scenario choices driven
//! through the application state, without a window. They need the install (`RT2_DIR` or the
//! default folder) and are skipped without it; the logic they sit on (config, key conflicts,
//! autosave rotation, option rows) has install-free tests in `config.rs`, `keys.rs`, `saves.rs`
//! and `options.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_sim::scenario::{MessageKind, Rule, trigger};
use rt2_ui::UiScale;
use winit::keyboard::KeyCode;

use super::dialogs::{A_DLG_BTN, Act, Dialog};
use super::options_ui::*;
use super::*;
use crate::keys::KeyAction;
use crate::options::{Setting, Tab};

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
    let mut s = AppState::new(init);
    // Never the user's own folder.
    s.set_saves_dir(None);
    Some(s)
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rrt2-opt-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn game(s: &AppState) -> &GameState {
    match &s.screen {
        Screen::Game(g) => g,
        Screen::Menu(_) => panic!("no game"),
    }
}

fn press(s: &mut AppState, code: KeyCode) {
    s.key(code, true, false, None);
    s.key(code, false, false, None);
}

fn has_effect(s: &mut AppState, f: impl Fn(&Effect) -> bool) -> bool {
    s.take_effects().iter().any(f)
}

#[test]
fn the_options_screen_changes_settings_and_asks_for_the_side_effects() {
    let Some(mut s) = state() else { return };
    s.set_monitors(vec!["DP-1".into(), "HDMI-1".into()]);
    s.open_options(Tab::Audio);
    assert!(s.options_open());
    s.take_effects();
    s.take_cues();
    // The volume slider's minus button: one step of 5, written to the config, sent to audio.
    assert!(s.options_activated(A_OPT_DEC + Setting::Master.index()));
    assert_eq!(s.config().audio.master, 95);
    assert!(has_effect(
        &mut s,
        |e| matches!(e, Effect::SaveConfig(c) if c.audio.master == 95)
    ));
    assert!(
        s.take_cues()
            .iter()
            .any(|c| matches!(c, crate::sound::Cue::Settings(a) if a.master == 95))
    );
    // A press on the bar sets an absolute value.
    s.options_event(&rt2_ui::UiEvent::Pointer(
        A_OPT_BAR + Setting::Music.index(),
        0.5,
        0.5,
    ));
    assert_eq!(s.config().audio.music, 50);
    // Mute toggle.
    s.options_activated(A_OPT_TOGGLE + Setting::Mute.index());
    assert!(s.config().audio.mute);

    // Display: borderless asks the window loop; a second monitor can be picked.
    s.options_activated(A_OPT_TAB);
    s.take_effects();
    s.options_activated(A_OPT_PICK + Setting::WindowMode.index() * 16 + 1);
    assert!(has_effect(&mut s, |e| matches!(e, Effect::ApplyDisplay(d)
        if d.mode == crate::config::WindowMode::Borderless)));
    s.options_activated(A_OPT_PICK + Setting::Monitor.index() * 16 + 2);
    assert_eq!(s.config().display.monitor, Some(1));
    // The UI scale slider pins the scale and rebuilds the layout.
    s.options_activated(A_OPT_INC + Setting::UiScale.index());
    assert_eq!(s.scale.percent(), 125);
    assert_eq!(s.config().ui_scale, Some(125));
    // Automatic scale releases it (the DPI of the test window is 1.0).
    assert!(s.scale_pinned);
    s.options_activated(A_OPT_TOGGLE + Setting::UiScaleAuto.index());
    assert!(!s.scale_pinned);
    assert_eq!(s.config().ui_scale, None);
    assert_eq!(s.scale.percent(), 100);

    // Gameplay and the language tab.
    s.options_activated(A_OPT_TAB + 4);
    s.options_activated(A_OPT_TOGGLE + Setting::EdgeScroll.index());
    assert!(s.config().gameplay.edge_scroll);
    s.options_activated(A_OPT_TAB + 2);
    s.options_activated(A_OPT_LANG + 1);
    assert!(s.config().language.is_some());
    // Reset this tab: the language tab has nothing to reset; gameplay does.
    s.options_activated(A_OPT_TAB + 4);
    s.options_activated(A_OPT_RESET);
    assert!(!s.config().gameplay.edge_scroll);
    // Esc closes.
    press(&mut s, KeyCode::Escape);
    assert!(!s.options_open());
}

#[test]
fn rebinding_a_key_detects_conflicts_and_takes_effect() {
    let Some(mut s) = state() else { return };
    s.open_options(Tab::Keys);
    // Row 0 is "Pause / resume".
    s.options_event(&rt2_ui::UiEvent::Selected(A_OPT_KEYLIST, 0));
    s.options_activated(A_OPT_REBIND);
    // Q rotates the view: the first press only reports the conflict.
    press(&mut s, KeyCode::KeyQ);
    assert_eq!(s.config().keys.get(KeyAction::Pause), ["Pause", "Space"]);
    let Some(Overlay::Options(o)) = &s.overlay else {
        panic!("options closed");
    };
    let rb = o.rebinding.as_ref().expect("still waiting for a key");
    assert_eq!(
        rb.clash.as_ref().map(|c| c.1.clone()),
        Some(vec![KeyAction::RotateLeft])
    );
    // Pressing it again moves the key over.
    press(&mut s, KeyCode::KeyQ);
    assert_eq!(s.config().keys.get(KeyAction::Pause), ["KeyQ"]);
    assert!(s.config().keys.get(KeyAction::RotateLeft).is_empty());
    assert!(s.binds.pause.matches(KeyCode::KeyQ, false));
    assert!(!s.binds.rotate_left.matches(KeyCode::KeyQ, false));
    assert!(s.options_open(), "the screen stays open");
    // Rebinding with Esc cancels without changes; a key such as F11 is a plain key then.
    s.options_activated(A_OPT_REBIND);
    press(&mut s, KeyCode::Escape);
    assert!(s.options_open());
    assert_eq!(s.config().keys.get(KeyAction::Pause), ["KeyQ"]);
    // Add key (alternate) with a free key.
    s.options_activated(A_OPT_ADDKEY);
    press(&mut s, KeyCode::KeyP);
    assert_eq!(s.config().keys.get(KeyAction::Pause), ["KeyQ", "KeyP"]);
    // Reset this key brings the defaults back and gives Q back to its owner.
    s.options_activated(A_OPT_KEYRESET);
    assert_eq!(s.config().keys.get(KeyAction::Pause), ["Pause", "Space"]);
    s.options_activated(A_OPT_RESET);
    assert_eq!(s.config().keys, crate::config::Keys::default());
}

/// A game on Alaska with a one-shot month-start choice that sets game variable 0 to 1 or 2.
fn choice_game(s: &mut AppState) -> PathBuf {
    let install = install().unwrap();
    let raw = std::fs::read(install.join("maps/Alaska.MP2")).unwrap();
    let mut save = rt2_formats::map::Save::parse(&raw).unwrap();
    let mut r = Rule::from_condition("True", trigger::MONTH_START).unwrap();
    r.message_kind = MessageKind::Choice;
    r.message = "Pick one".into();
    r.label1 = "One".into();
    r.label2 = "Two".into();
    r.once = true;
    r.effects[0][241] = 1;
    r.effects[1][241] = 2;
    save.rules.push(r.to_file());
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = tmp(&format!(
        "choice{}",
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let path = dir.join("choice.GAM");
    std::fs::write(&path, save.to_bytes()).unwrap();
    s.start(&path).unwrap();
    path
}

fn var0(s: &AppState) -> i32 {
    // `0x573d8e`: game variable 0 (effects 241..244 set it).
    let st = game(s).world.to_save();
    i32::from_le_bytes(st.state[0x38A6..0x38A6 + 4].try_into().unwrap())
}

/// Runs frames at top speed until `f` holds (at most 200).
fn run_until(s: &mut AppState, f: impl Fn(&AppState) -> bool) -> bool {
    for _ in 0..200 {
        if f(s) {
            return true;
        }
        s.update(100.0);
    }
    f(s)
}

#[test]
fn an_interactive_choice_opens_a_dialog_and_the_button_answers_it() {
    let Some(mut s) = state() else { return };
    s.set_interactive_choices(true);
    let path = choice_game(&mut s);
    s.apply_command(Command::SetSpeed(SpeedChange::Absolute(10)));
    assert!(run_until(&mut s, |s| game(s)
        .world
        .pending_choice()
        .is_some()));
    // The dialog is on top with the rule's two labels; the world does not move meanwhile.
    s.update(0.0);
    let Some(Dialog::Message(m)) = game(&s).dialogs.last().cloned() else {
        panic!("no choice dialog: {:?}", game(&s).dialogs.last());
    };
    assert_eq!(m.body, "Pick one");
    let labels: Vec<_> = m.buttons.iter().map(|b| b.0.as_str()).collect();
    assert_eq!(labels, ["One", "Two"]);
    let at = game(&s).world.minutes();
    for _ in 0..5 {
        s.update(100.0);
    }
    assert_eq!(
        game(&s).world.minutes(),
        at,
        "the game waits for the player"
    );
    // Closing the dialog some other way brings it back (the world still waits).
    s.close_dialog();
    s.poll_world();
    assert!(matches!(game(&s).dialogs.last(), Some(Dialog::Message(m)) if m.body == "Pick one"));
    // The second button answers: the world takes the command, the step is replayed.
    assert!(matches!(m.buttons[1].1, Act::Choice { button: 1, .. }));
    assert!(s.dialog_activated(A_DLG_BTN + 1));
    assert!(game(&s).world.pending_choice().is_none());
    assert!(s.take_choice_answers().is_empty(), "applied, not queued");
    assert!(run_until(&mut s, |s| var0(s) != 0));
    assert_eq!(var0(&s), 2, "button 2 = effect set 1");
    assert!(
        game(&s).dialogs.is_empty(),
        "no second dialog for the replayed message"
    );
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn without_interactive_choices_button_zero_applies_at_once() {
    let Some(mut s) = state() else { return };
    let path = choice_game(&mut s);
    s.apply_command(Command::SetSpeed(SpeedChange::Absolute(10)));
    assert!(run_until(&mut s, |s| var0(s) != 0));
    assert_eq!(var0(&s), 1, "headless: button 1");
    assert!(game(&s).world.pending_choice().is_none());
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

fn example_game(s: &mut AppState) {
    let install = install().unwrap();
    s.start(&install.join("games/Example save game.GAM"))
        .expect("the example save loads");
}

#[test]
fn quick_save_and_quick_load_use_one_slot() {
    let Some(mut s) = state() else { return };
    let dir = tmp("quick");
    s.set_saves_dir(Some(dir.clone()));
    example_game(&mut s);
    // Nothing yet: F9 only says so.
    press(&mut s, KeyCode::F9);
    assert!(game(&s).toast.is_some());
    s.advance_days(5);
    let day = game(&s).world.date();
    press(&mut s, KeyCode::F5);
    assert!(dir.join("quicksave.GAM").is_file());
    let list = crate::saves::list(&dir);
    assert_eq!(list.len(), 1);
    let meta = list[0].meta.as_ref().expect("sidecar");
    assert_eq!(meta.kind, crate::saves::SaveKind::Quick);
    assert_eq!(
        (meta.year, meta.month, meta.day),
        (day.year, day.month, day.day)
    );
    assert!(!meta.scenario.is_empty());
    assert!(crate::saves::read_thumbnail(&list[0].path).is_some());
    // Play on, then F9 puts the game back.
    s.advance_days(40);
    assert_ne!(game(&s).world.date(), day);
    press(&mut s, KeyCode::F9);
    assert_eq!(game(&s).world.date(), day);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_save_dialog_writes_a_named_save_that_the_load_dialog_lists_and_opens() {
    let Some(mut s) = state() else { return };
    let dir = tmp("dialog");
    s.set_saves_dir(Some(dir.clone()));
    example_game(&mut s);
    s.open_saves(saves_ui::SavesMode::Save);
    // The suggested name is replaced by typing.
    for _ in 0..80 {
        s.key(KeyCode::Backspace, true, false, None);
    }
    for c in "My / test".chars() {
        let t = c.to_string();
        s.key(KeyCode::KeyA, true, false, Some(&t));
    }
    s.key(KeyCode::Enter, true, false, None);
    assert!(!s.saves_open(), "the dialog closes after saving");
    // `/` was made safe.
    assert!(dir.join("My _ test.GAM").is_file());
    s.advance_days(30);
    let later = game(&s).world.minutes();
    // The Load dialog lists our save first, then the original's games, and opens it.
    s.open_saves(saves_ui::SavesMode::Load);
    let Some(Overlay::Saves(st)) = &s.overlay else {
        panic!("no dialog");
    };
    assert_eq!(st.rows_len(), 1 + 2, "ours plus Example and Tutorial");
    assert_eq!(st.stem_at(0), "My _ test");
    assert!(s.saves_activated(saves_ui::A_SV_OK));
    assert!(!s.saves_open());
    assert!(game(&s).world.minutes() < later, "back at the saved date");
    // A second Save over the same name is an overwrite (the button says so, the file stays one).
    s.open_saves(saves_ui::SavesMode::Save);
    s.saves_event(&rt2_ui::UiEvent::Selected(saves_ui::A_SV_LIST, 0));
    s.saves_activated(saves_ui::A_SV_OK);
    assert_eq!(crate::saves::list(&dir).len(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn autosave_every_n_months_keeps_the_last_k() {
    let Some(mut s) = state() else { return };
    let dir = tmp("auto");
    s.set_saves_dir(Some(dir.clone()));
    s.config.gameplay.autosave_months = 1;
    s.config.gameplay.autosave_keep = 2;
    example_game(&mut s);
    // Off until the window loop enables persistence (tests and screenshots never write).
    s.advance_days(40);
    s.update(0.0);
    assert!(crate::saves::list(&dir).is_empty());
    s.set_persistence(true);
    for _ in 0..5 {
        s.advance_days(32);
        s.update(0.0);
    }
    let autos: Vec<_> = crate::saves::list(&dir)
        .into_iter()
        .filter(|e| crate::saves::autosave_seq(&e.stem).is_some())
        .collect();
    assert_eq!(
        autos.len(),
        2,
        "{:?}",
        autos.iter().map(|e| &e.stem).collect::<Vec<_>>()
    );
    assert!(
        autos
            .iter()
            .all(|e| e.kind() == crate::saves::SaveKind::Auto)
    );
    // Turning it off stops the writes.
    s.config.gameplay.autosave_months = 0;
    let before = crate::saves::next_autosave_seq(&dir);
    s.advance_days(100);
    s.update(0.0);
    assert_eq!(crate::saves::next_autosave_seq(&dir), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn edge_scrolling_and_camera_preferences_move_the_camera() {
    let Some(mut s) = state() else { return };
    example_game(&mut s);
    s.update(0.0);
    let start = game(&s).camera.target;
    // Off by default: the cursor at the edge does nothing.
    s.cursor_moved(1.0, 450.0);
    s.update(100.0);
    assert_eq!(game(&s).camera.target, start);
    s.config.gameplay.edge_scroll = true;
    s.update(100.0);
    let moved = game(&s).camera.target;
    assert_ne!(moved, start);
    // Zoom preferences: the inverted wheel zooms the other way.
    s.cursor_moved(800.0, 300.0);
    let z0 = game(&s).camera.zoom;
    s.wheel(1.0);
    let zoom_in = game(&s).camera.zoom;
    assert!(zoom_in > z0);
    s.config.gameplay.invert_zoom = true;
    s.wheel(1.0);
    assert!(game(&s).camera.zoom < zoom_in);
}

#[test]
fn the_default_speed_applies_to_a_new_game() {
    let Some(mut s) = state() else { return };
    s.config.gameplay.default_speed = 7;
    example_game(&mut s);
    assert_eq!(game(&s).world.speed(), 7);
}

#[test]
fn fullscreen_key_and_mute_key_go_through_the_config() {
    let Some(mut s) = state() else { return };
    s.take_effects();
    press(&mut s, KeyCode::F11);
    assert_eq!(
        s.config().display.mode,
        crate::config::WindowMode::Borderless
    );
    assert!(has_effect(&mut s, |e| matches!(e, Effect::ApplyDisplay(_))));
    s.ctrl = true;
    press(&mut s, KeyCode::KeyM);
    s.ctrl = false;
    assert!(s.config().audio.mute);
}

/// A canvas too small for every row of a tab (the 300 % scale on a short window): the tab shows
/// a window of rows, and the buttons, the mouse wheel and the tab change move it. Everything is
/// driven through mouse events on the laid-out screen.
#[test]
fn a_settings_tab_that_does_not_fit_scrolls() {
    use rt2_ui::Kind;
    let Some(mut s) = state() else { return };
    s.set_ui_scale(UiScale::from_percent(300), true);
    s.resize((1600, 560));
    s.open_options(Tab::Gameplay);
    let rows_total = Tab::Gameplay.settings().len();
    // Labels of the rows drawn now.
    let labels = |s: &AppState| -> Vec<String> {
        s.ui.draw_order()
            .into_iter()
            .filter_map(|(_, id)| match &s.ui.node(id).kind {
                Kind::Label { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    };
    let first_row = s.texts.a(Tab::Gameplay.settings()[0].english());
    let second_row = s.texts.a(Tab::Gameplay.settings()[1].english());
    let last_row = s
        .texts
        .a(Tab::Gameplay.settings()[rows_total - 1].english());
    assert!(labels(&s).contains(&first_row));
    assert!(
        !labels(&s).contains(&last_row),
        "the last row is below the fold"
    );
    let scroll_down = |s: &mut AppState| {
        let id = s
            .ui
            .draw_order()
            .into_iter()
            .map(|(_, id)| id)
            .find(|&id| matches!(&s.ui.node(id).kind, Kind::Button { action, .. } if *action == A_OPT_SCROLL + 1))
            .expect("a scroll-down button");
        let (r, k) = (s.ui.rect(id), s.scale.factor());
        s.cursor_moved((r.x + r.w / 2.0) * k, (r.y + r.h / 2.0) * k);
        s.mouse_button(true, true);
        s.mouse_button(true, false);
    };
    for _ in 0..rows_total {
        scroll_down(&mut s);
    }
    assert!(labels(&s).contains(&last_row), "scrolled to the end");
    assert!(!labels(&s).contains(&first_row));
    // The wheel scrolls back up, and switching tabs starts from the top again.
    s.cursor_moved(800.0, 300.0);
    for _ in 0..rows_total {
        s.wheel(1.0);
    }
    assert!(labels(&s).contains(&first_row));
    assert!(labels(&s).contains(&second_row) || rows_total < 2);
    s.options_activated(A_OPT_TAB);
    s.options_activated(A_OPT_TAB + 4);
    assert!(labels(&s).contains(&first_row));
    // A roomy canvas shows every row and no scroll buttons.
    s.set_ui_scale(UiScale::from_percent(100), true);
    s.resize((1600, 900));
    assert!(labels(&s).contains(&last_row));
    assert!(!s.ui.draw_order().into_iter().any(|(_, id)| {
        matches!(&s.ui.node(id).kind, Kind::Button { action, .. } if *action == A_OPT_SCROLL)
    }));
}
