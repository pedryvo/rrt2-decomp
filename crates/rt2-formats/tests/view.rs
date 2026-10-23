//! The camera/view block against the original maps and saves (docs/formats/map-gam.md 1.2).
//! Skipped unless `RT2_DIR` points at an original install.

use std::path::{Path, PathBuf};

use rt2_formats::map::Save;
use rt2_formats::view::{VIEW_LEN, View};

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["maps", "games"] {
        let Ok(rd) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "map" | "mp2" | "gam") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_shipped_view_block_decodes_and_the_center_is_on_the_map() {
    let Some(dir) = std::env::var_os("RT2_DIR").map(PathBuf::from) else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let list = files(&dir);
    assert!(list.len() > 100, "{} files", list.len());
    let mut near = 0;
    for p in &list {
        let raw = std::fs::read(p).unwrap();
        let save = Save::parse(&raw).unwrap();
        let v = save.view();
        let o = save.layout.view_offset();
        assert_eq!(&v.to_bytes()[..], &raw[o..o + VIEW_LEN], "{p:?}");
        assert!(v.zoom_valid(), "{p:?} zoom {}", v.zoom);
        assert!((0..4).contains(&v.rotation), "{p:?}");
        assert_eq!((v.width, v.height), (1024, 508), "{p:?}");
        assert_eq!(v.head, 0, "{p:?}");
        assert_eq!(v.tail, [0; 5], "{p:?}");
        let (w, h) = (save.map.width, save.map.height);
        assert_eq!(
            (v.map_width as usize, v.map_height as usize),
            (w, h),
            "{p:?}"
        );
        let (x, y) = v.center_tile(w, h);
        assert!(
            x > 0.0 && y > 0.0 && x < w as f32 && y < h as f32,
            "{p:?} centre ({x}, {y}) outside {w}x{h}"
        );
        // The author left the view on a city or industry area in most files.
        let d = save
            .place_list()
            .iter()
            .map(|c| ((c.x as f32 - x).powi(2) + (c.y as f32 - y).powi(2)).sqrt())
            .fold(f32::MAX, f32::min);
        if d < 40.0 {
            near += 1;
        }
    }
    assert!(
        near * 10 >= list.len() * 8,
        "{near} of {} near a place",
        list.len()
    );
}

#[test]
fn tutorial_view_is_on_a_city() {
    let Some(dir) = std::env::var_os("RT2_DIR").map(PathBuf::from) else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raw = std::fs::read(dir.join("games/Tutorial.GAM")).unwrap();
    let save = Save::parse(&raw).unwrap();
    let v = save.view();
    assert_eq!(
        (v.zoom, v.scroll_x, v.scroll_y, v.rotation, v.company),
        (5, 2231, 2137, 1, 0)
    );
    let (x, y) = v.center_tile(save.map.width, save.map.height);
    let near = save
        .place_list()
        .iter()
        .any(|c| ((c.x as f32 - x).powi(2) + (c.y as f32 - y).powi(2)).sqrt() < 8.0);
    assert!(near, "({x}, {y})");
}

#[test]
fn set_view_survives_a_save_round_trip() {
    let Some(dir) = std::env::var_os("RT2_DIR").map(PathBuf::from) else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raw = std::fs::read(dir.join("games/Tutorial.GAM")).unwrap();
    let mut save = Save::parse(&raw).unwrap();
    let mut v: View = save.view();
    v.set_center_tile((100.0, 50.0), 4, 2, save.map.width, save.map.height);
    save.set_view(&v);
    let again = Save::parse(&save.to_bytes()).unwrap();
    assert_eq!(again.view(), v);
}
