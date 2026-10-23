//! The editor's tool panels drawn with the original art (`editor_panel.rs`): at several window
//! sizes every control stays on the screen and none overlaps another (needs the install in
//! `RT2_DIR`; skipped otherwise).

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::{Kind, Rect, UiScale};

use super::editor_panel::art_frame;
use super::*;

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
        install: install.clone(),
        assets,
        backdrop: art.backdrop,
        icons: art.icons,
        dialogs: art.dialogs,
        config: Config::default(),
        scale: UiScale::default(),
        scale_pinned: true,
        viewport: (1280, 720),
    };
    let mut s = AppState::new(init);
    s.open_editor(&install.join("maps/Local Industry.MP2"))
        .ok()?;
    Some(s)
}

fn overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.right() - 0.5
        && b.x < a.right() - 0.5
        && a.y < b.bottom() - 0.5
        && b.y < a.bottom() - 0.5
}

#[test]
fn the_frame_keeps_the_proportions_and_stays_inside() {
    for (w, h) in [
        (700.0, 251.0),
        (300.0, 400.0),
        (1400.0, 372.0),
        (100.0, 100.0),
    ] {
        let area = Rect::new(10.0, 20.0, w, h);
        let (r, k) = art_frame(area);
        assert!(r.x >= area.x - 1.0 && r.right() <= area.right() + 1.0);
        assert!(r.y >= area.y - 1.0 && r.bottom() <= area.bottom() + 1.0);
        assert!((r.w / r.h - 458.0 / 216.0).abs() < 0.02);
        assert!((r.w - 458.0 * k).abs() < 0.01);
    }
}

#[test]
fn panels_fit_and_do_not_overlap_at_every_window_size() {
    let Some(mut s) = state() else { return };
    if s.dialog_art.editor.misc.is_none() {
        return;
    }
    for vp in [(800, 600), (1024, 768), (1280, 720), (1920, 1080)] {
        s.resize(vp);
        let mut art_panels = 0;
        for group in [
            "height",
            "paint",
            "fill",
            "trees",
            "territory",
            "add",
            "bulldoze",
            "overview",
        ] {
            for sub in 0..2 {
                if !s.editor_show_group(group, sub, None) {
                    continue;
                }
                let (cw, ch) = s.ui.logical_size();
                let screen = Rect::new(0.0, 0.0, cw, ch);
                let mut hot = Vec::new();
                for (_, id) in s.ui.draw_order() {
                    let r = s.ui.rect(id);
                    let interactive = match &s.ui.node(id).kind {
                        Kind::Hotspot { enabled, .. } => {
                            art_panels += 1;
                            *enabled
                        }
                        Kind::Toggle { .. } | Kind::Button { .. } | Kind::List { .. } => true,
                        _ => false,
                    };
                    if interactive {
                        assert!(
                            r.x >= -0.5
                                && r.y >= -0.5
                                && r.right() <= cw + 0.5
                                && r.bottom() <= ch + 0.5,
                            "{group}/{sub} at {vp:?}: {r:?} leaves the screen {screen:?}"
                        );
                        hot.push((id, r));
                    }
                }
                for (i, (a, ra)) in hot.iter().enumerate() {
                    for (b, rb) in &hot[i + 1..] {
                        // A dialog or modal root may sit over the panel; the editor starts without.
                        assert!(
                            !overlap(ra, rb),
                            "{group}/{sub} at {vp:?}: {a:?} {ra:?} overlaps {b:?} {rb:?}"
                        );
                    }
                }
            }
        }
        assert!(art_panels > 0, "the panel art was not used at {vp:?}");
    }
}

#[test]
fn hotspots_drive_the_same_actions_as_the_clone_widgets() {
    let Some(mut s) = state() else { return };
    if s.dialog_art.editor.misc.is_none() {
        return;
    }
    s.editor_show_group("paint", 0, None);
    // A click on the fourth brush hotspot activates the brush action.
    let id =
        s.ui.draw_order()
            .into_iter()
            .map(|(_, id)| id)
            .find(|&id| {
                matches!(
                    s.ui.node(id).kind,
                    Kind::Hotspot { action, .. } if action == super::editor_ui::A_BRUSH + 3
                )
            })
            .expect("brush hotspot");
    let r = s.ui.rect(id);
    let k = s.ui.scale().factor();
    s.ui.input(rt2_ui::Input::CursorMoved {
        x: (r.x + r.w / 2.0) * k,
        y: (r.y + r.h / 2.0) * k,
    });
    s.ui.input(rt2_ui::Input::MouseDown(rt2_ui::MouseButton::Left));
    let ev =
        s.ui.input(rt2_ui::Input::MouseUp(rt2_ui::MouseButton::Left));
    assert_eq!(
        ev,
        vec![rt2_ui::UiEvent::Activated(super::editor_ui::A_BRUSH + 3)]
    );
    assert!(s.editor_activated(super::editor_ui::A_BRUSH + 3));
    assert_eq!(s.ed().unwrap().brush, 3);
    // The palette scrolls by a row and clamps.
    assert!(s.editor_activated(super::editor_ui::A_PAL_DOWN));
    assert!(s.editor_activated(super::editor_ui::A_PAL_DOWN));
    assert_eq!(s.ed().unwrap().pal_row, 1);
    assert!(s.editor_activated(super::editor_ui::A_PAL_UP));
    assert_eq!(s.ed().unwrap().pal_row, 0);
}
