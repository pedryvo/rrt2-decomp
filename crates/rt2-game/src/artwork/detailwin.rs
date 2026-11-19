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
    /// `odbottom.imb` rec 12: 4-tab strip with Trains active at `(0, 729)`.
    pub odbottom_tabs_trains: Option<Placed>,
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
    /// `traindtl.win` art and exact control rectangles.
    pub traindtl: Option<TrainDtlArt>,
}

/// Art and exact `.win` control rectangles for `traindtl.win` (1024x768).
#[derive(Clone, Debug)]
pub struct TrainDtlArt {
    pub frame: SpriteId,
    pub btn_replace: Option<(SpriteId, SpriteId)>,
    pub btn_sell: Option<(SpriteId, SpriteId)>,
    pub btn_fleet: Option<(SpriteId, SpriteId)>,
    pub r_stops: [f32; 4],
    pub r_map: [f32; 4],
    pub r_info: [f32; 4],
    pub r_name: [f32; 4],
    pub r_stat1: [f32; 4],
    pub r_stat2: [f32; 4],
    pub r_bar: [f32; 4],
    pub r_btn_replace: [f32; 4],
    pub r_btn_sell: [f32; 4],
    pub r_btn_fleet: [f32; 4],
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

pub(super) fn placed(
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

pub(super) fn pair(
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

fn win_btn(
    assets: &GameAssets,
    win: &str,
    id: u16,
    b: &mut SkinBuilder,
) -> Option<(SpriteId, SpriteId)> {
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

pub(super) fn blend(dst: &mut RgbaImage, src: &RgbaImage, dx: i32, dy: i32) {
    for sy in 0..src.height {
        let ty = dy + src.y + sy as i32;
        if ty < 0 || ty >= dst.height as i32 {
            continue;
        }
        for sx in 0..src.width {
            let tx = dx + src.x + sx as i32;
            if tx < 0 || tx >= dst.width as i32 {
                continue;
            }
            let si = (sy * src.width + sx) * 4;
            if src.rgba[si + 3] == 0 {
                continue;
            }
            let di = (ty as usize * dst.width + tx as usize) * 4;
            dst.rgba[di..di + 4].copy_from_slice(&src.rgba[si..si + 4]);
        }
    }
}

fn load_traindtl(assets: &GameAssets, b: &mut SkinBuilder) -> Option<TrainDtlArt> {
    use rt2_formats::ui::{ControlKind, Window};
    let w = Window::parse(assets.blob("traindtl.win")?).ok()?;
    let mut canvas = RgbaImage {
        width: 1024,
        height: 768,
        rgba: vec![0; 1024 * 768 * 4],
        x: 0,
        y: 0,
    };
    let mut any = false;
    for c in &w.controls {
        if c.kind != ControlKind::Image
            || c.bank == "odbottom.imb"
            || matches!(c.id, 729 | 730 | 717)
        {
            continue;
        }
        let Some(rec) = c.image_record() else {
            continue;
        };
        let Some(img) = ui_record_pub(assets, &c.bank, usize::from(rec)) else {
            continue;
        };
        blend(&mut canvas, &img, i32::from(c.rect.x), i32::from(c.rect.y));
        any = true;
    }
    if !any {
        return None;
    }
    let frame = b.add(to_skin_image(&scale2x(&canvas), 2));
    let rect = |id: u16, fallback: [f32; 4]| -> [f32; 4] {
        w.controls
            .iter()
            .find(|c| c.id == id)
            .map_or(fallback, |c| {
                [
                    f32::from(c.rect.x),
                    f32::from(c.rect.y),
                    f32::from(c.rect.w),
                    f32::from(c.rect.h),
                ]
            })
    };
    Some(TrainDtlArt {
        frame,
        btn_replace: win_btn(assets, "traindtl.win", 714, b),
        btn_sell: win_btn(assets, "traindtl.win", 715, b),
        btn_fleet: win_btn(assets, "traindtl.win", 716, b),
        r_stops: rect(730, [24.0, 295.0, 422.0, 285.0]),
        r_map: rect(729, [494.0, 250.0, 498.0, 408.0]),
        r_info: rect(717, [24.0, 588.0, 974.0, 128.0]),
        r_name: rect(718, [564.0, 100.0, 100.0, 25.0]),
        r_stat1: rect(727, [690.0, 92.0, 160.0, 46.0]),
        r_stat2: rect(728, [854.0, 92.0, 160.0, 46.0]),
        r_bar: rect(735, [588.0, 214.0, 336.0, 36.0]),
        r_btn_replace: rect(714, [494.0, 658.0, 92.0, 54.0]),
        r_btn_sell: rect(715, [804.0, 658.0, 92.0, 54.0]),
        r_btn_fleet: rect(716, [904.0, 658.0, 92.0, 54.0]),
    })
}

pub(crate) fn load(assets: &GameAssets, b: &mut SkinBuilder) -> DetailArt {
    DetailArt {
        statndtl_frame: placed(assets, "statndtl.imb", 0, 0, 0, b),
        statndtl_buy: pair(assets, "statndtl.imb", 1, 2, None, None, b),
        odbottom_bg: placed(assets, "odbottom.imb", 0, 0, 728, b),
        odbottom_tabs: placed(assets, "odbottom.imb", 11, 0, 729, b),
        odbottom_tabs_trains: placed(assets, "odbottom.imb", 12, 0, 729, b),
        odbottom_prev: pair(assets, "odbottom.imb", 5, 6, None, None, b),
        odbottom_next: pair(assets, "odbottom.imb", 7, 8, None, None, b),
        odbottom_list: pair(assets, "odbottom.imb", 1, 2, Some(40), Some(31), b),
        odbottom_ok: pair(assets, "odbottom.imb", 9, 10, None, None, b),
        bpovervw_bg: win_img(assets, "bpovervw.win", 2000, b),
        bpovervw_all: win_btn(assets, "bpovervw.win", 3603, b),
        bpovervw_clear: win_btn(assets, "bpovervw.win", 3604, b),
        traindtl: load_traindtl(assets, b),
    }
}
