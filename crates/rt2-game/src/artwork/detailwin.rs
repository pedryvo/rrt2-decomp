//! Original window artwork for detail dialogs (`statndtl.win`, `odbottom.imb`, etc.).

use rt2_render::assets::{GameAssets, RgbaImage};
use rt2_render::atlas::scale2x;
use rt2_ui::{SkinBuilder, SpriteId};

use super::buywin::Placed;
use super::{to_skin_image, ui_record_pub};

/// Art for the station detail window (`statndtl.win`) and its shared `odbottom.imb` bar.
#[derive(Clone, Debug, Default)]
pub struct DetailArt {
    /// `statndtl.imb` rec 0: lower console frame (`0, 0`).
    pub statndtl_frame: Option<Placed>,
    /// `statndtl.imb` rec 1, 2: Buy buildings button (up, down) at `(929, 509, 82, 46)`.
    pub statndtl_buy: Option<(SpriteId, SpriteId)>,
    /// `odbottom.imb` rec 0: bottom toolbar background at `(0, 728)`.
    pub odbottom_bg: Option<Placed>,
    /// `odbottom.imb` rec 11: 4-tab strip with Stations active at `(0, 729)`.
    pub odbottom_tabs: Option<Placed>,
    /// `odbottom.imb` rec 5, 6: previous item button `<` at `(845, 735, 32, 29)`.
    pub odbottom_prev: Option<(SpriteId, SpriteId)>,
    /// `odbottom.imb` rec 7, 8: next item button `>` at `(880, 735, 32, 29)`.
    pub odbottom_next: Option<(SpriteId, SpriteId)>,
    /// `odbottom.imb` rec 1, 2 (cropped to `40x31`): list button at `(916, 735, 40, 31)`.
    pub odbottom_list: Option<(SpriteId, SpriteId)>,
    /// `odbottom.imb` rec 9, 10: OK button at `(962, 734, 60, 32)`.
    pub odbottom_ok: Option<(SpriteId, SpriteId)>,
    /// `bpovervw.win` control 2000: game-mode Map Overview 458x166 background.
    pub bpovervw_bg: Option<SpriteId>,
    /// `bpovervw.win` control 3603: All button (up, down) at `(256, 138, 60, 21)`.
    pub bpovervw_all: Option<(SpriteId, SpriteId)>,
    /// `bpovervw.win` control 3604: Clear button (up, down) at `(323, 138, 60, 21)`.
    pub bpovervw_clear: Option<(SpriteId, SpriteId)>,
}

fn crop_if_needed(img: RgbaImage, max_w: Option<usize>, max_h: Option<usize>) -> RgbaImage {
    let w = max_w.map_or(img.width, |m| img.width.min(m));
    let h = max_h.map_or(img.height, |m| img.height.min(m));
    if w == img.width && h == img.height {
        return img;
    }
    let mut rgba = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        let start = row * img.width * 4;
        let end = start + w * 4;
        rgba.extend_from_slice(&img.rgba[start..end]);
    }
    RgbaImage {
        width: w,
        height: h,
        rgba,
        x: img.x,
        y: img.y,
    }
}

fn add2x(
    assets: &GameAssets,
    bank: &str,
    rec: usize,
    max_w: Option<usize>,
    max_h: Option<usize>,
    b: &mut SkinBuilder,
) -> Option<(SpriteId, RgbaImage)> {
    let img = crop_if_needed(ui_record_pub(assets, bank, rec)?, max_w, max_h);
    let id = b.add(to_skin_image(&scale2x(&img), 2));
    Some((id, img))
}

fn placed(
    assets: &GameAssets,
    bank: &str,
    rec: usize,
    ox: i32,
    oy: i32,
    b: &mut SkinBuilder,
) -> Option<Placed> {
    let (sprite, img) = add2x(assets, bank, rec, None, None, b)?;
    Some(Placed {
        sprite,
        x: ox + img.x,
        y: oy + img.y,
        w: img.width as f32,
        h: img.height as f32,
    })
}

fn pair(
    assets: &GameAssets,
    bank: &str,
    up: usize,
    down: usize,
    max_w: Option<usize>,
    max_h: Option<usize>,
    b: &mut SkinBuilder,
) -> Option<(SpriteId, SpriteId)> {
    let (u, _) = add2x(assets, bank, up, max_w, max_h, b)?;
    let (d, _) = add2x(assets, bank, down, max_w, max_h, b)?;
    Some((u, d))
}

fn win_btn(assets: &GameAssets, win: &str, id: u16, b: &mut SkinBuilder) -> Option<(SpriteId, SpriteId)> {
    let w = rt2_formats::ui::Window::parse(assets.blob(win)?).ok()?;
    let c = w.controls.iter().find(|c| c.id == id)?;
    let up = usize::from(c.tail_u16(0)?);
    let down = usize::from(c.tail_u16(2).unwrap_or(up as u16));
    pair(assets, &c.bank, up, down, None, None, b)
}

fn win_img(assets: &GameAssets, win: &str, id: u16, b: &mut SkinBuilder) -> Option<SpriteId> {
    let w = rt2_formats::ui::Window::parse(assets.blob(win)?).ok()?;
    let c = w.controls.iter().find(|c| c.id == id)?;
    let rec = usize::from(c.image_record()?);
    add2x(assets, &c.bank, rec, None, None, b).map(|(id, _)| id)
}

pub(crate) fn load(assets: &GameAssets, b: &mut SkinBuilder) -> DetailArt {
    DetailArt {
        statndtl_frame: placed(assets, "statndtl.imb", 0, 0, 0, b),
        statndtl_buy: pair(assets, "statndtl.imb", 1, 2, None, None, b),
        odbottom_bg: placed(assets, "odbottom.imb", 0, 0, 728, b),
        odbottom_tabs: placed(assets, "odbottom.imb", 11, 0, 729, b),
        odbottom_prev: pair(assets, "odbottom.imb", 5, 6, None, None, b),
        odbottom_next: pair(assets, "odbottom.imb", 7, 8, None, None, b),
        odbottom_list: pair(assets, "odbottom.imb", 1, 2, Some(40), Some(31), b),
        odbottom_ok: pair(assets, "odbottom.imb", 9, 10, None, None, b),
        bpovervw_bg: win_img(assets, "bpovervw.win", 2000, b),
        bpovervw_all: win_btn(assets, "bpovervw.win", 3603, b),
        bpovervw_clear: win_btn(assets, "bpovervw.win", 3604, b),
    }
}
