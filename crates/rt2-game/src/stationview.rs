//! The picture of the station detail window (`statndtl.win`): a pre-rendered 1024x569 scene made
//! of a backdrop for the kind of terrain the station stands on and one overlay per building or
//! improvement the station has, drawn back to front at the positions stored in the art.
//!
//! The rules come from the original's `0x4c0c00` (the overlay loop), `0x4c0ed0` (backdrop),
//! `0x4c1da0` and `0x4c0a50` (the derived bits); see `docs/formats/sprite-catalog.md` 6.1.
//!
//! * Backdrop: `sdmbkg{N}.imb`, `N = station +0x4d + 1` (13 banks, one per site class).
//! * Overlays: `SD{L}{KEY}.imb` for each bit of the station's improvement word `+0x49` that is
//!   set, in the order of [`DRAW_ORDER`]. `L` is the style letter of the station building
//!   (`"VCMTBK"[object +0x52]`) for the three station sizes, `M` for the two restaurants of the
//!   Mission and Kyoto styles, and `V` for everything else.

use rt2_render::assets::{GameAssets, LoadedBank, RgbaImage};

/// Size of the composed picture in pixels (the backdrop's size).
pub const VIEW_W: u32 = 1024;
pub const VIEW_H: u32 = 569;

/// Style letters of the station buildings (`0x550474`), indexed by the object's `+0x52`.
pub const STYLES: &[u8; 6] = b"VCMTBK";

/// File key of each improvement row, 0..=28 (`0x55b238`; row 29 has none and is never set).
/// Rows 26 and 27 are the road and the hotel road; 23..=25 the catenary; 12..=14 the turntable
/// and the two tracks, all derived bits (see [`derive_mask`]).
const KEYS: [&str; 29] = [
    "STAT1", "STAT2", "STAT3", "PHONE", "GRAPH", "RESTS", "RESTB", "SALOO", "POSTO", "CUSTM",
    "SANDT", "WATER", "TURNT", "TRAK2", "TRAK1", "HOTLB", "HOTLS", "LIQID", "REFRG", "GRAIN",
    "WAREH", "ROUND", "ENGIN", "ELEC1", "ELEC2", "ELEC3", "ROAD_", "ROADB", "trees",
];

/// Rows in drawing order, back to front (`0x55b218`).
pub const DRAW_ORDER: [u8; 30] = [
    29, 26, 27, 22, 21, 20, 19, 18, 17, 16, 15, 25, 12, 0, 1, 28, 2, 11, 23, 14, 13, 24, 10, 9, 8,
    7, 5, 6, 4, 3,
];

/// The improvement word as the window draws it: the stored bits plus the ones the window derives
/// each time it opens (`0x4c1da0`, `0x4c0a50`), and the "bigger replaces smaller" cleanup.
/// `double` and `electrified` describe the track piece under the station's anchor tile
/// (`piece +0x23` bits 0 and 4).
pub fn derive_mask(stored: u32, double: bool, electrified: bool) -> u32 {
    let mut m = stored | 0x4000; // the main track line is always drawn
    m &= !(0x80_0000 | 0x100_0000);
    if electrified {
        m |= 0x80_0000;
    }
    if double {
        m |= 0x2000;
        if electrified {
            m |= 0x100_0000;
        }
    } else {
        m &= !0x2000;
    }
    // Road under the restaurants, saloon, post office and customs house; under the hotels.
    if m & 0x3e0 != 0 {
        m |= 0x400_0000;
    } else {
        m &= !0x400_0000;
    }
    m &= !0x200_0000;
    if m & 0x60_0000 != 0 {
        m |= 0x1000;
        if electrified {
            m |= 0x200_0000;
        }
    } else {
        m &= !0x1000;
    }
    if m & 0x1_8000 != 0 {
        m |= 0x800_0000;
    } else {
        m &= !0x800_0000;
    }
    // A bigger building hides the smaller one it replaces.
    if m & 4 != 0 {
        m &= !3;
    }
    if m & 2 != 0 {
        m &= !1;
    }
    if m & 0x40 != 0 {
        m &= !0x20;
    }
    if m & 0x8000 != 0 {
        m &= !0x1_0000;
    }
    if m & 8 != 0 {
        m &= !0x10;
    }
    m
}

/// The style letter of the overlay of `row` for a station of `style` (`0..=5`).
fn letter(row: u8, style: u8) -> u8 {
    match row {
        0..=2 => STYLES[usize::from(style).min(5)],
        5 | 6 if matches!(style, 2 | 5) => b'M',
        _ => b'V',
    }
}

/// File names of the overlays to draw, back to front, for the derived improvement word `mask`.
pub fn overlay_names(mask: u32, style: u8) -> Vec<String> {
    DRAW_ORDER
        .iter()
        .filter(|&&row| usize::from(row) < KEYS.len() && mask & (1 << row) != 0)
        .map(|&row| {
            format!(
                "SD{}{}.imb",
                letter(row, style) as char,
                KEYS[usize::from(row)]
            )
        })
        .collect()
}

/// File name of the backdrop for a station whose site class (`+0x4d`) is `site`.
pub fn backdrop_name(site: i32) -> String {
    format!("sdmbkg{}.imb", site.clamp(0, 12) + 1)
}

/// Palette used for every image: the art has three (record header), the first is the day look.
const SHADE: usize = 0;

fn first_image(assets: &GameAssets, bank: &str) -> Option<RgbaImage> {
    LoadedBank::open(assets, bank)?.image(0, 0, 0, SHADE)
}

/// Source-over blend of `img` at `(img.x, img.y)` on a `VIEW_W` x `VIEW_H` canvas.
fn blend(canvas: &mut [u8], img: &RgbaImage) {
    for iy in 0..img.height {
        let cy = img.y + iy as i32;
        if !(0..VIEW_H as i32).contains(&cy) {
            continue;
        }
        for ix in 0..img.width {
            let cx = img.x + ix as i32;
            if !(0..VIEW_W as i32).contains(&cx) {
                continue;
            }
            let s = (iy * img.width + ix) * 4;
            let a = u32::from(img.rgba[s + 3]);
            if a == 0 {
                continue;
            }
            let d = (cy as usize * VIEW_W as usize + cx as usize) * 4;
            for c in 0..3 {
                let (sc, dc) = (u32::from(img.rgba[s + c]), u32::from(canvas[d + c]));
                canvas[d + c] = ((sc * a + dc * (255 - a)) / 255) as u8;
            }
            let da = u32::from(canvas[d + 3]);
            canvas[d + 3] = (a + da * (255 - a) / 255) as u8;
        }
    }
}

/// Composes the picture: backdrop, then the overlays. `None` when the art is missing from the
/// install (no backdrop and no overlay found).
pub fn compose(assets: &GameAssets, site: i32, style: u8, derived_mask: u32) -> Option<Vec<u8>> {
    let mut canvas = vec![0u8; (VIEW_W * VIEW_H * 4) as usize];
    let mut any = false;
    if let Some(bg) = first_image(assets, &backdrop_name(site)) {
        blend(&mut canvas, &bg);
        any = true;
    }
    for name in overlay_names(derived_mask, style) {
        // A style without art (the "B" default style has none) falls back to Victorian.
        let img = first_image(assets, &name).or_else(|| {
            let mut alt = name.clone().into_bytes();
            alt[2] = b'V';
            first_image(assets, &String::from_utf8(alt).ok()?)
        });
        if let Some(img) = img {
            blend(&mut canvas, &img);
            any = true;
        }
    }
    any.then_some(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_small_station_draws_the_building_and_the_track() {
        let m = derive_mask(1, false, false);
        let names = overlay_names(m, 0);
        assert_eq!(names, ["SDVSTAT1.imb", "SDVTRAK1.imb"]);
    }

    #[test]
    fn style_letters_follow_the_building_style() {
        let names = overlay_names(derive_mask(0b10 | 1 << 5, false, false), 2);
        assert!(names.contains(&"SDMSTAT2.imb".to_string()));
        assert!(names.contains(&"SDMRESTS.imb".to_string()));
        // The restaurant of a Tudor station uses the Victorian art; its station building its own.
        let names = overlay_names(derive_mask(0b10 | 1 << 5, false, false), 3);
        assert!(names.contains(&"SDTSTAT2.imb".to_string()));
        assert!(names.contains(&"SDVRESTS.imb".to_string()));
    }

    #[test]
    fn derived_bits_add_roads_tracks_and_catenary() {
        // Medium station with a restaurant and a large hotel on electrified double track.
        let m = derive_mask(0b10 | 1 << 5 | 1 << 15, true, true);
        for row in [26, 27, 14, 13, 23, 24] {
            assert!(m & (1 << row) != 0, "row {row}");
        }
        // A roundhouse brings the turntable, and the catenary around it.
        let m = derive_mask(1 << 2 | 1 << 21, false, true);
        assert!(m & 0x1000 != 0 && m & 0x200_0000 != 0);
    }

    #[test]
    fn bigger_replaces_smaller() {
        let m = derive_mask(
            0b111 | 1 << 3 | 1 << 4 | 1 << 5 | 1 << 6 | 1 << 15 | 1 << 16,
            false,
            false,
        );
        assert_eq!(m & 0b11, 0);
        assert_eq!(m & (1 << 4 | 1 << 5 | 1 << 16), 0);
        assert_ne!(m & (1 << 2 | 1 << 3 | 1 << 6 | 1 << 15), 0);
    }

    #[test]
    fn back_to_front_order_puts_the_hotel_behind_the_station() {
        let names = overlay_names(derive_mask(1 << 2 | 1 << 15 | 1 << 11, false, false), 0);
        let at = |n: &str| names.iter().position(|x| x == n).unwrap();
        assert!(at("SDVHOTLB.imb") < at("SDVSTAT3.imb"));
        assert!(at("SDVSTAT3.imb") < at("SDVWATER.imb"));
    }

    #[test]
    fn backdrop_follows_the_site_class() {
        assert_eq!(backdrop_name(0), "sdmbkg1.imb");
        assert_eq!(backdrop_name(12), "sdmbkg13.imb");
        assert_eq!(backdrop_name(99), "sdmbkg13.imb");
    }
}
