//! Art of the purchase windows (docs/ui.md "Janelas de compra"): `trainbuy.win` (Purchase Train,
//! docs/reference/screens.md E1) and `buildbuy.win` (Buy improvements, F2). Both are 774x578
//! windows built from `trainbuy.imb` pieces; the layout comes from the `.win` controls and the
//! pieces carry their own offsets, so the whole frame is composed once here into one sprite
//! (the clone scales it to any resolution).

use rt2_formats::ui::{ControlKind, Window};
use rt2_render::assets::{GameAssets, RgbaImage};
use rt2_render::atlas::scale2x;
use rt2_ui::{SkinBuilder, SpriteId};

use super::{to_skin_image, ui_record};

/// Design size of both windows (`trainbuy.win`, `buildbuy.win`).
pub const WIN_W: f32 = 774.0;
pub const WIN_H: f32 = 578.0;
/// The picture box of the right panel (`trainbuy.imb` 6: 293x153 at 357,76 in `trainbuy.win`).
pub const PIC: (f32, f32, f32, f32) = (357.0, 76.0, 293.0, 153.0);
/// Rows of the engine list: `trainbuy.win` list 1701 at (68,44), 229x450; rows 54 high and 197
/// wide (the 27 px left are the scroll bar of `trainblb.imb`).
pub const LIST: (f32, f32, f32, f32) = (68.0, 44.0, 197.0, 450.0);
pub const ROW_H: f32 = 54.0;
pub const SCROLL_X: f32 = 270.0;

/// A sprite with the offset of its image (original pixels).
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub sprite: SpriteId,
    pub x: i32,
    pub y: i32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, Default)]
pub struct BuyArt {
    /// The composed 774x578 frame of `trainbuy.win` (no list rows, no picture, no buttons).
    pub frame: Option<SpriteId>,
    /// The frame of `buildbuy.win`: the same pieces and the picture frame `trainbuy.imb` 19.
    pub frame_build: Option<SpriteId>,
    /// Purchase and Cancel plates (`trainbuy.imb` 2/3 and 4/5), (up, down) and their size.
    pub buttons: Option<[(SpriteId, SpriteId); 2]>,
    /// The arrows of the tons-per-car control and of the scroll bar: up, up pressed, down, down
    /// pressed (`trainbuy.imb` 25..=28).
    pub small_arrows: Option<[SpriteId; 4]>,
    /// The scroll bar of the list (`trainblb.imb`): up (0), down (2), track (4), thumb (5).
    pub scroll: Option<[Placed; 4]>,
    /// The side views of the engine list (`side11_8.imb`), by record.
    pub side: Vec<Option<Placed>>,
    /// Blank sprites the picture is composed into at run time (2x size).
    pub preview: Option<SpriteId>,
    /// `buildbuy.imb` 0 and 1: the empty and the checked box of a list row (15x15).
    pub checks: Option<(SpriteId, SpriteId)>,
    /// The picture of each improvement row (`bm<name>.imb` record 0, 218x134), by row of
    /// `improvement_table` (`0x407e10`); `None` for the rows without a name.
    pub pictures: Vec<Option<SpriteId>>,
    /// `bpaddstn.win` (D2): the 458x166 background (record 0, with the style and direction
    /// cells) and the strip of the three station sizes in each architecture (records 1..=6,
    /// 305x72 at (16,14); record 5 has no art), by record.
    pub station_bg: Option<SpriteId>,
    pub station_strips: Vec<Option<Placed>>,
}

/// The bank name stems of the improvement rows (`improvement_table` `0x5500c8`, first bytes of
/// each 0x1b byte row): the picture of row `i` is `bm<stem>.imb`.
pub const IMPROVEMENT_STEMS: [&str; 23] = [
    "statna", "statnb", "statnc", "telep", "telegr", "rests", "restl", "saloo", "post", "custom",
    "sand", "water", "", "", "", "hotell", "hotels", "liquid", "refrig", "grain", "wareh", "round",
    "engin",
];
/// Rows of the Buy list are 22 px high; the picture box sits at (393,97) in a 233x150 frame.
pub const BUY_ROW_H: f32 = 22.0;
pub const BUY_PIC: (f32, f32, f32, f32) = (393.0, 97.0, 218.0, 134.0);

/// Source-over blend of `img` at `(img.x + dx, img.y + dy)`.
pub fn blend(canvas: &mut RgbaImage, img: &RgbaImage, dx: i32, dy: i32) {
    for iy in 0..img.height {
        let cy = img.y + dy + iy as i32;
        if !(0..canvas.height as i32).contains(&cy) {
            continue;
        }
        for ix in 0..img.width {
            let cx = img.x + dx + ix as i32;
            if !(0..canvas.width as i32).contains(&cx) {
                continue;
            }
            let s = (iy * img.width + ix) * 4;
            let a = u32::from(img.rgba[s + 3]);
            if a == 0 {
                continue;
            }
            let d = (cy as usize * canvas.width + cx as usize) * 4;
            for c in 0..3 {
                let (sc, dc) = (u32::from(img.rgba[s + c]), u32::from(canvas.rgba[d + c]));
                canvas.rgba[d + c] = ((sc * a + dc * (255 - a)) / 255) as u8;
            }
            let da = u32::from(canvas.rgba[d + 3]);
            canvas.rgba[d + 3] = (a + da * (255 - a) / 255) as u8;
        }
    }
}

fn blank(w: usize, h: usize) -> RgbaImage {
    RgbaImage {
        width: w,
        height: h,
        x: 0,
        y: 0,
        rgba: vec![0; w * h * 4],
    }
}

/// Composes a window from the `Image` controls of its layout that draw `trainbuy.imb` pieces
/// (the controls with a size of 0 draw the record at its own offset), in file order.
fn compose_frame(assets: &GameAssets, win: &str, skip: &[u16]) -> Option<RgbaImage> {
    let w = Window::parse(assets.blob(win)?).ok()?;
    let mut canvas = blank(w.width as usize, w.height as usize);
    let mut any = false;
    let mut z1 = Vec::new();
    // The list box has no picture of its own: it shows the same olive leather as the right
    // panel (`trainbuy.imb` 8), taken from its middle.
    if let Some(leather) = ui_record(assets, "trainbuy.imb", 8) {
        let (lx, ly, lw, lh) = (LIST.0 as usize, LIST.1 as usize, 229usize, 450usize);
        let mut bg = blank(lw, lh);
        bg.x = lx as i32;
        bg.y = ly as i32;
        for yy in 0..lh {
            for xx in 0..lw {
                let m = yy % 720;
                let (sx, sy) = (80 + xx, 30 + if m < 360 { m } else { 719 - m });
                if sx < leather.width && sy < leather.height {
                    let d = (yy * lw + xx) * 4;
                    bg.rgba[d..d + 4]
                        .copy_from_slice(&leather.rgba[(sy * leather.width + sx) * 4..][..4]);
                }
            }
        }
        blend(&mut canvas, &bg, 0, 0);
    }
    for c in &w.controls {
        if c.kind != ControlKind::Image || c.bank != "trainbuy.imb" {
            continue;
        }
        let Some(rec) = c.image_record() else {
            continue;
        };
        if skip.contains(&rec) {
            continue;
        }
        let Some(img) = ui_record(assets, &c.bank, usize::from(rec)) else {
            continue;
        };
        if c.order > 0 {
            z1.push((img, c.rect));
        } else {
            blend(&mut canvas, &img, i32::from(c.rect.x), i32::from(c.rect.y));
            any = true;
        }
    }
    for (img, r) in z1 {
        blend(&mut canvas, &img, i32::from(r.x), i32::from(r.y));
    }
    any.then_some(canvas)
}

fn add2x(b: &mut SkinBuilder, img: &RgbaImage) -> SpriteId {
    b.add(to_skin_image(&scale2x(img), 2))
}

fn placed(b: &mut SkinBuilder, img: &RgbaImage) -> Placed {
    Placed {
        sprite: add2x(b, img),
        x: img.x,
        y: img.y,
        w: img.width as f32,
        h: img.height as f32,
    }
}

pub fn load(assets: &GameAssets, b: &mut SkinBuilder) -> BuyArt {
    let mut art = BuyArt::default();
    // The picture box is drawn at run time: leave out the terrain piece (record 6) and, in
    // `buildbuy.win`, the station picture bank.
    if let Some(img) = compose_frame(assets, "trainbuy.win", &[6]) {
        art.frame = Some(add2x(b, &img));
        art.frame_build = compose_frame(assets, "buildbuy.win", &[6]).map(|i| add2x(b, &i));
    }
    let rec = |r: usize| ui_record(assets, "trainbuy.imb", r);
    let pair =
        |b: &mut SkinBuilder, u: usize, d: usize| Some((add2x(b, &rec(u)?), add2x(b, &rec(d)?)));
    if let (Some(a), Some(c)) = (pair(b, 2, 3), pair(b, 4, 5)) {
        art.buttons = Some([a, c]);
    }
    let arrows: Vec<Option<SpriteId>> = (25..=28).map(|r| rec(r).map(|i| add2x(b, &i))).collect();
    if let [Some(a), Some(c), Some(d), Some(e)] = arrows[..] {
        art.small_arrows = Some([a, c, d, e]);
    }
    let sc: Vec<Option<Placed>> = [0usize, 2, 4, 5]
        .iter()
        .map(|&r| ui_record(assets, "trainblb.imb", r).map(|i| placed(b, &i)))
        .collect();
    if let [Some(a), Some(c), Some(d), Some(e)] = sc[..] {
        art.scroll = Some([a, c, d, e]);
    }
    for r in 0..139 {
        art.side
            .push(ui_record(assets, "side11_8.imb", r).map(|i| placed(b, &i)));
    }
    art.preview = Some(b.reserve(PIC.2 as u32 * 2, PIC.3 as u32 * 2));
    if let (Some(a), Some(c)) = (
        ui_record(assets, "buildbuy.imb", 0),
        ui_record(assets, "buildbuy.imb", 1),
    ) {
        art.checks = Some((add2x(b, &a), add2x(b, &c)));
    }
    art.station_bg = ui_record(assets, "bpaddstn.imb", 0).map(|i| add2x(b, &i));
    for r in 0..=6usize {
        let img = (r > 0 && r != 5)
            .then(|| ui_record(assets, "bpaddstn.imb", r))
            .flatten();
        art.station_strips.push(img.map(|i| placed(b, &i)));
    }
    for stem in IMPROVEMENT_STEMS {
        let img = (!stem.is_empty())
            .then(|| ui_record(assets, &format!("bm{stem}.imb"), 0))
            .flatten();
        art.pictures.push(img.map(|i| add2x(b, &i)));
    }
    art
}
