//! Art of the main window's chrome (docs/ui.md "Janela principal"; `mainwin.win`,
//! docs/formats/ui.md): the metal frame, rails, tab strip, compass and logo.
//!
//! The original frame is one 1024x768 design. `mwfrme.imb` holds it as four pieces: records 0
//! and 1 are the left rail (85x397 at 0,6) and the right rail (70x340 at 954,63), records 2 and
//! 3 are the bottom frame in two rows (1024x105 at y 403 and 1024x260 at y 508). To fit any
//! screen width the two rows are cut in three pieces each (left, a seamless filler repeated as
//! often as needed, right); the filler windows sit on plain pipe (row 0: x 402..515, between
//! two posts a period of 113 px apart) and on the empty tab area (row 1: x 520..633).

use rt2_render::assets::{GameAssets, RgbaImage};
use rt2_render::atlas::scale2x;
use rt2_ui::{SkinBuilder, SpriteId};

use super::{to_skin_image, ui_record};

/// Cut positions of the two bottom rows: (end of the left piece, end of the filler), in design
/// pixels of the 1024 wide rows.
pub const ROW0_CUT: (usize, usize) = (402, 515);
pub const ROW1_CUT: (usize, usize) = (520, 633);

/// Sprites of the main window chrome (each `None` without the install art).
#[derive(Clone, Copy, Debug, Default)]
pub struct MainArt {
    /// `mwfrme.imb` 0, 1: the left and right rails.
    pub rail_left: Option<SpriteId>,
    pub rail_right: Option<SpriteId>,
    /// `mwfrme.imb` 2, 3 cut in [left, filler, right].
    pub row0: Option<[SpriteId; 3]>,
    pub row1: Option<[SpriteId; 3]>,
    /// `mwfrma.imb` 0, 1, 2: the inner shadows of the radar, center and activity boxes.
    pub shade: Option<[SpriteId; 3]>,
    /// The mustard texture of the center box (a crop of the ground strip of row 1).
    pub olive: Option<SpriteId>,
    /// `maintab.imb` 0..3: the tab strip with the tab `i` lit.
    pub tabs: Option<[SpriteId; 4]>,
    /// `compass.imb`: the compass for the four view rotations (arrow plus the four plates).
    pub compass: Option<[SpriteId; 4]>,
    /// `brlogo.imb` 0: the Railroad Tycoon II logo of the activity box.
    pub title: Option<SpriteId>,
    /// `mainmisc.imb` 1, 2: the small message button (up, down).
    pub misc: Option<(SpriteId, SpriteId)>,
}

fn crop(img: &RgbaImage, x: usize, y: usize, w: usize, h: usize) -> RgbaImage {
    let mut rgba = Vec::with_capacity(w * h * 4);
    for yy in y..y + h {
        rgba.extend_from_slice(&img.rgba[(yy * img.width + x) * 4..][..w * 4]);
    }
    RgbaImage {
        width: w,
        height: h,
        x: 0,
        y: 0,
        rgba,
    }
}

/// A full-row record scaled 2x, then cut at the design columns `cuts` (so the Scale2x pass sees
/// the neighbours of every cut and the seams stay clean).
fn row_pieces(
    b: &mut SkinBuilder,
    img: &RgbaImage,
    (a, f): (usize, usize),
) -> Option<[SpriteId; 3]> {
    if img.width != 1024 {
        return None;
    }
    let big = scale2x(img);
    let h = big.height;
    let mut add =
        |x0: usize, x1: usize| b.add(to_skin_image(&crop(&big, x0 * 2, 0, (x1 - x0) * 2, h), 2));
    Some([add(0, a), add(a, f), add(f, 1024)])
}

/// The compass for rotation `k`: the arrow (record `5k`) and the four plates (`5k + 1..5k + 4`),
/// each drawn at its own offset inside a 80x46 area.
fn compass_image(assets: &GameAssets, k: usize) -> Option<RgbaImage> {
    let (w, h) = (80usize, 46usize);
    let mut out = vec![0u8; w * h * 4];
    for part in 0..5 {
        let img = ui_record(assets, "compass.imb", k * 5 + part)?;
        for y in 0..img.height {
            for x in 0..img.width {
                let (dx, dy) = (x as i32 + img.x, y as i32 + img.y);
                if dx < 0 || dy < 0 || dx as usize >= w || dy as usize >= h {
                    continue;
                }
                let s = (y * img.width + x) * 4;
                if img.rgba[s + 3] != 0 {
                    let d = (dy as usize * w + dx as usize) * 4;
                    out[d..d + 4].copy_from_slice(&img.rgba[s..s + 4]);
                }
            }
        }
    }
    Some(RgbaImage {
        width: w,
        height: h,
        x: 0,
        y: 0,
        rgba: out,
    })
}

pub(super) fn load(assets: &GameAssets, b: &mut SkinBuilder) -> MainArt {
    let mut art = MainArt::default();
    if let Some(rail) = ui_record(assets, "mwfrme.imb", 0) {
        art.rail_left = Some(b.add(to_skin_image(&scale2x(&rail), 2)));
    }
    if let Some(rail) = ui_record(assets, "mwfrme.imb", 1) {
        art.rail_right = Some(b.add(to_skin_image(&scale2x(&rail), 2)));
    }
    if let Some(r) = ui_record(assets, "mwfrme.imb", 2) {
        art.row0 = row_pieces(b, &r, ROW0_CUT);
    }
    if let Some(r) = ui_record(assets, "mwfrme.imb", 3) {
        art.row1 = row_pieces(b, &r, ROW1_CUT);
        // The olive ground strip below the boxes: a 64x16 crop mirrored on both axes, so the
        // tile repeats without a visible seam (the strip has a large-scale gradient).
        if r.height >= 258 && r.width >= 400 {
            let big = scale2x(&r);
            let q = crop(&big, 300, 456, 256, 44);
            let (w, h) = (q.width, q.height);
            let mut rgba = vec![0u8; w * 2 * h * 2 * 4];
            for y in 0..h * 2 {
                for x in 0..w * 2 {
                    let (sx, sy) = (
                        if x < w { x } else { 2 * w - 1 - x },
                        if y < h { y } else { 2 * h - 1 - y },
                    );
                    let d = (y * w * 2 + x) * 4;
                    rgba[d..d + 4].copy_from_slice(&q.rgba[(sy * w + sx) * 4..][..4]);
                }
            }
            let tile = RgbaImage {
                width: w * 2,
                height: h * 2,
                x: 0,
                y: 0,
                rgba,
            };
            art.olive = Some(b.add(to_skin_image(&tile, 2)));
        }
    }
    let shade: Vec<Option<SpriteId>> = (0..3)
        .map(|i| {
            ui_record(assets, "mwfrma.imb", i).map(|img| b.add(to_skin_image(&scale2x(&img), 2)))
        })
        .collect();
    if let [Some(a), Some(c), Some(d)] = shade[..] {
        art.shade = Some([a, c, d]);
    }
    let tabs: Vec<Option<SpriteId>> = (0..4)
        .map(|i| {
            ui_record(assets, "maintab.imb", i).map(|img| b.add(to_skin_image(&scale2x(&img), 2)))
        })
        .collect();
    if let [Some(a), Some(c), Some(d), Some(e)] = tabs[..] {
        art.tabs = Some([a, c, d, e]);
    }
    let comp: Vec<Option<SpriteId>> = (0..4)
        .map(|k| compass_image(assets, k).map(|img| b.add(to_skin_image(&scale2x(&img), 2))))
        .collect();
    if let [Some(a), Some(c), Some(d), Some(e)] = comp[..] {
        art.compass = Some([a, c, d, e]);
    }
    art.title =
        ui_record(assets, "brlogo.imb", 0).map(|img| b.add(to_skin_image(&scale2x(&img), 2)));
    if let (Some(up), Some(down)) = (
        ui_record(assets, "mainmisc.imb", 1),
        ui_record(assets, "mainmisc.imb", 2),
    ) {
        art.misc = Some((
            b.add(to_skin_image(&scale2x(&up), 2)),
            b.add(to_skin_image(&scale2x(&down), 2)),
        ));
    }
    art
}
