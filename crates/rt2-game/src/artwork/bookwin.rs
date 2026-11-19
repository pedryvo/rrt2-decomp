//! Original window artwork for the book detail windows (`compndtl.win`, `playrdtl.win`,
//! and their `odbottom.imb` active-tab strips).

use rt2_formats::ui::{ControlKind, Window};
use rt2_render::assets::{GameAssets, RgbaImage};
use rt2_ui::{SkinBuilder, SpriteId};

use super::buywin::Placed;
use super::{to_skin_image, ui_record_pub};

/// Art and exact `.win` control rectangles for `compndtl.win` and `playrdtl.win` (1024x768).
#[derive(Clone, Debug, Default)]
pub struct BookArt {
    /// `compndtl.imb` rec 0+1+2: 1024x728 desk background.
    pub compndtl_bg: Option<SpriteId>,
    /// `compndtl.imb` rec 3..=7: 708x538 open book spreads at `(152, 83)` for tabs 0..5.
    pub compndtl_spreads: [Option<SpriteId>; 5],
    /// `playrdtl.imb` rec 0+1+2: 1024x728 desk + open book background.
    pub playrdtl_bg: Option<SpriteId>,
    /// `playrdtl.imb` rec 3: oval portrait frame + ribbon banner at `(-5, 0)`.
    pub playrdtl_frame: Option<Placed>,
    /// `odbottom.imb` rec 14: 4-tab strip with Companies active at `(0, 729)`.
    pub odbottom_companies: Option<Placed>,
    /// `odbottom.imb` rec 13: 4-tab strip with Players active at `(0, 729)`.
    pub odbottom_players: Option<Placed>,
    /// `compndtl.win` control 1601: left page rectangle.
    pub comp_left: [f32; 4],
    /// `compndtl.win` control 1602: right page rectangle.
    pub comp_right: [f32; 4],
    /// `compndtl.win` control 1619: left page top block.
    pub comp_left_top: [f32; 4],
    /// `compndtl.win` control 1620: left page bottom block.
    pub comp_left_bot: [f32; 4],
    /// `compndtl.win` 5 book tab hit rectangles on the right edge of the book.
    pub comp_tabs: [[f32; 4]; 5],
    /// `playrdtl.win` control 1001: left page rectangle.
    pub playr_left: [f32; 4],
    /// `playrdtl.win` control 1002: right page rectangle.
    pub playr_right: [f32; 4],
    /// `playrdtl.win` control 1005: left page body rectangle.
    pub playr_left_body: [f32; 4],
}

fn blend(dst: &mut RgbaImage, src: &RgbaImage, dx: i32, dy: i32) {
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

fn add1x(assets: &GameAssets, bank: &str, rec: usize, b: &mut SkinBuilder) -> Option<(SpriteId, RgbaImage)> {
    let img = ui_record_pub(assets, bank, rec)?;
    let id = b.add(to_skin_image(&img, 1));
    Some((id, img))
}

fn placed(assets: &GameAssets, bank: &str, rec: usize, ox: i32, oy: i32, b: &mut SkinBuilder) -> Option<Placed> {
    let (sprite, img) = add1x(assets, bank, rec, b)?;
    Some(Placed {
        sprite,
        x: ox + img.x,
        y: oy + img.y,
        w: img.width as f32,
        h: img.height as f32,
    })
}

fn ctrl_rect(w: Option<&Window>, id: u16, fallback: [f32; 4]) -> [f32; 4] {
    w.and_then(|win| win.controls.iter().find(|c| c.id == id))
        .map_or(fallback, |c| {
            [
                f32::from(c.rect.x),
                f32::from(c.rect.y),
                f32::from(c.rect.w),
                f32::from(c.rect.h),
            ]
        })
}

fn compose_bg(assets: &GameAssets, bank: &str, recs: &[usize], b: &mut SkinBuilder) -> Option<SpriteId> {
    let mut canvas = RgbaImage {
        width: 1024,
        height: 728,
        rgba: vec![0; 1024 * 728 * 4],
        x: 0,
        y: 0,
    };
    let mut any = false;
    for &rec in recs {
        if let Some(img) = ui_record_pub(assets, bank, rec) {
            blend(&mut canvas, &img, 0, 0);
            any = true;
        }
    }
    any.then(|| b.add(to_skin_image(&canvas, 1)))
}

fn load_spreads(assets: &GameAssets, w: Option<&Window>, b: &mut SkinBuilder) -> [Option<SpriteId>; 5] {
    let has_bank = w
        .map(|win| {
            win.controls
                .iter()
                .any(|c| c.kind == ControlKind::Image && c.bank == "compndtl.imb")
        })
        .unwrap_or(true);
    if !has_bank {
        return [None; 5];
    }
    std::array::from_fn(|i| add1x(assets, "compndtl.imb", 3 + i, b).map(|(id, _)| id))
}

pub(crate) fn load(assets: &GameAssets, b: &mut SkinBuilder) -> BookArt {
    let comp_win = assets.blob("compndtl.win").and_then(|d| Window::parse(d).ok());
    let playr_win = assets.blob("playrdtl.win").and_then(|d| Window::parse(d).ok());

    let compndtl_bg = comp_win
        .as_ref()
        .and_then(|_| compose_bg(assets, "compndtl.imb", &[0, 1, 2], b));
    let compndtl_spreads = if compndtl_bg.is_some() {
        load_spreads(assets, comp_win.as_ref(), b)
    } else {
        [None; 5]
    };

    let playrdtl_bg = playr_win
        .as_ref()
        .and_then(|_| compose_bg(assets, "playrdtl.imb", &[0, 1, 2], b));
    let playrdtl_frame = if playrdtl_bg.is_some() {
        placed(assets, "playrdtl.imb", 3, 0, 0, b)
    } else {
        None
    };

    BookArt {
        compndtl_bg,
        compndtl_spreads,
        playrdtl_bg,
        playrdtl_frame,
        odbottom_companies: placed(assets, "odbottom.imb", 14, 0, 729, b),
        odbottom_players: placed(assets, "odbottom.imb", 13, 0, 729, b),
        comp_left: ctrl_rect(comp_win.as_ref(), 1601, [216.0, 110.0, 265.0, 454.0]),
        comp_right: ctrl_rect(comp_win.as_ref(), 1602, [539.0, 110.0, 265.0, 454.0]),
        comp_left_top: ctrl_rect(comp_win.as_ref(), 1619, [215.0, 152.0, 275.0, 150.0]),
        comp_left_bot: ctrl_rect(comp_win.as_ref(), 1620, [215.0, 320.0, 275.0, 170.0]),
        comp_tabs: std::array::from_fn(|i| [828.0, 110.0 + i as f32 * 92.0, 28.0, 85.0]),
        playr_left: ctrl_rect(playr_win.as_ref(), 1001, [201.0, 114.0, 293.0, 396.0]),
        playr_right: ctrl_rect(playr_win.as_ref(), 1002, [544.0, 114.0, 293.0, 396.0]),
        playr_left_body: ctrl_rect(playr_win.as_ref(), 1005, [215.0, 146.0, 285.0, 344.0]),
    }
}
