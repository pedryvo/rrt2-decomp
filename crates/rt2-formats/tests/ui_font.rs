//! Fonts and UI windows against an original game install. Skipped unless `RT2_DIR` is set.

use std::path::PathBuf;

use rt2_formats::font::{FONTS, Font, font_file_name};
use rt2_formats::lad::Lad;
use rt2_formats::pk2::{Pk2, name_hash};
use rt2_formats::ui::{ControlKind, WINDOW_START, Window};

const ARCHIVES: [&str; 4] = ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"];

fn game_dir() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR").map(PathBuf::from)
}

fn raws(dir: &std::path::Path) -> Vec<Vec<u8>> {
    ARCHIVES
        .iter()
        .map(|n| std::fs::read(dir.join("data").join(n)).unwrap())
        .collect()
}

#[test]
fn all_fonts_load_and_render() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raws = raws(&dir);
    let pk2s: Vec<_> = raws.iter().map(|r| Pk2::parse(r).unwrap()).collect();
    for spec in &FONTS {
        // `fontN.dat` is only a name: no archive holds an entry with its hash.
        assert!(
            pk2s.iter()
                .all(|p| p.find(name_hash(&font_file_name(spec.index))).is_none())
        );
        let blob = pk2s
            .iter()
            .find_map(|p| p.find(name_hash(spec.bank)).map(|e| p.contents(e)))
            .unwrap_or_else(|| panic!("{} missing", spec.bank));
        let font = Font::from_lad(spec, &Lad::parse(blob).unwrap()).unwrap();
        assert!(font.glyphs.len() >= 96, "{}", spec.bank);
        assert!(font.advance(b'A') > 0 && font.advance(b' ') == font.advance(b'1'));
        let img = font.render("Railroad Tycoon", [255; 3], Some([0; 3]), true);
        assert!(img.pixels.as_chunks::<4>().0.iter().any(|p| p[0] > 100));
    }
}

#[test]
fn every_window_layout_parses() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raws = raws(&dir);
    let (mut windows, mut controls) = (0, 0);
    for raw in &raws {
        let pk2 = Pk2::parse(raw).unwrap();
        for e in pk2.entries() {
            let blob = pk2.contents(e);
            if blob.len() < 4 || u32::from_le_bytes(blob[..4].try_into().unwrap()) != WINDOW_START {
                continue;
            }
            let win = Window::parse(blob).unwrap_or_else(|err| panic!("{:08x}: {err}", e.hash));
            windows += 1;
            controls += win.controls.len();
        }
    }
    assert_eq!(windows, 46);
    assert!(controls > 1000);
}

#[test]
fn file_requester_geometry() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raws = raws(&dir);
    let blob = raws
        .iter()
        .find_map(|r| {
            let p = Pk2::parse(r).unwrap();
            p.find_name("filerq.win").map(|e| p.contents(e).to_vec())
        })
        .unwrap();
    let win = Window::parse(&blob).unwrap();
    assert_eq!((win.width, win.height, win.framed), (562, 572, 1));
    assert_eq!(win.controls.len(), 9);
    let list = win
        .controls
        .iter()
        .find(|c| c.kind == ControlKind::List)
        .unwrap();
    assert_eq!(
        (list.rect.x, list.rect.y, list.rect.w, list.rect.h),
        (95, 70, 431, 321)
    );
    assert_eq!(list.bank, "filerqlb.imb");
}
