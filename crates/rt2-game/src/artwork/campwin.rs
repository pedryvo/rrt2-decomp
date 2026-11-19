//! Original window artwork for Campaign (`campaign.win`, `campaign.imb`, `campmisc.imb`),
//! File Options (`fileopt.win`, `fileopt.imb`), and Help Dialog (`helpd.win`, `helpd.imb`).

use rt2_formats::ui::{ControlKind, Window};
use rt2_render::assets::{GameAssets, RgbaImage};
use rt2_render::atlas::scale2x;
use rt2_ui::{SkinBuilder, SpriteId};

use super::{to_skin_image, ui_record_pub};

/// Art for Campaign, File Options, and Help dialogs.
#[derive(Clone, Debug, Default)]
pub struct CampModalArt {
    /// `campaign.imb` records 0..40 assembled into the 1024x768 rural station backdrop.
    pub campaign_bg: Option<SpriteId>,
    /// `campmisc.imb` records 0, 1: radio button (off, on).
    pub radio: Option<(SpriteId, SpriteId)>,
    /// `campmisc.imb` records 2, 3, 4: bronze, silver, gold medals.
    pub medals: [Option<SpriteId>; 3],
    /// `campmisc.imb` record 5: unavailable / X mark.
    pub lock_x: Option<SpriteId>,
    /// `fileopt.win` frame and button plates.
    pub fileopt: Option<FileOptArt>,
    /// `helpd.win` frame, text rect, and button plates.
    pub helpd: Option<HelpdArt>,
}

/// Composed `fileopt.win` frame (`475x592`) and its 7 button controls.
#[derive(Clone, Debug)]
pub struct FileOptArt {
    pub w: f32,
    pub h: f32,
    pub frame: SpriteId,
    /// `(id, rect [x, y, w, h], (up, down))` for buttons `501..=507`.
    pub buttons: Vec<(u16, [f32; 4], (SpriteId, SpriteId))>,
}

/// `(id, rect [x, y, w, h], optional (up, down))` for a `helpd.win` button control.
pub type HelpdButton = (u16, [f32; 4], Option<(SpriteId, SpriteId)>);

/// Composed `helpd.win` frame (`470x351`), label rect, and its 3 button controls.
#[derive(Clone, Debug)]
pub struct HelpdArt {
    pub w: f32,
    pub h: f32,
    pub frame: SpriteId,
    pub r_text: [f32; 4],
    /// Buttons `102` (Prev), `104` (Next), `103` (Close).
    pub buttons: [HelpdButton; 3],
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

fn add1x(assets: &GameAssets, bank: &str, rec: usize, b: &mut SkinBuilder) -> Option<SpriteId> {
    let img = ui_record_pub(assets, bank, rec)?;
    Some(b.add(to_skin_image(&img, 1)))
}

fn pair1x(
    assets: &GameAssets,
    bank: &str,
    up: usize,
    down: usize,
    b: &mut SkinBuilder,
) -> Option<(SpriteId, SpriteId)> {
    Some((add1x(assets, bank, up, b)?, add1x(assets, bank, down, b)?))
}

fn add2x(assets: &GameAssets, bank: &str, rec: usize, b: &mut SkinBuilder) -> Option<SpriteId> {
    let img = ui_record_pub(assets, bank, rec)?;
    Some(b.add(to_skin_image(&scale2x(&img), 2)))
}

fn pair2x(
    assets: &GameAssets,
    bank: &str,
    up: usize,
    down: usize,
    b: &mut SkinBuilder,
) -> Option<(SpriteId, SpriteId)> {
    Some((add2x(assets, bank, up, b)?, add2x(assets, bank, down, b)?))
}

fn load_campaign_bg(assets: &GameAssets, b: &mut SkinBuilder) -> Option<SpriteId> {
    // `campaign.imb` stores the 1024x768 rural station backdrop as an 8x5 grid of tiles.
    let mut tiles = Vec::with_capacity(40);
    for rec in 0..40 {
        tiles.push(ui_record_pub(assets, "campaign.imb", rec)?);
    }
    let mut canvas = RgbaImage {
        width: 1024,
        height: 768,
        rgba: vec![0; 1024 * 768 * 4],
        x: 0,
        y: 0,
    };
    let mut y = 0i32;
    for row in 0..5 {
        let mut x = 0i32;
        let row_h = tiles[row * 8].height as i32;
        for col in 0..8 {
            let tile = &tiles[row * 8 + col];
            blend(&mut canvas, tile, x, y);
            x += tile.width as i32;
        }
        y += row_h;
    }
    Some(b.add(to_skin_image(&canvas, 1)))
}

fn compose_window_images(
    assets: &GameAssets,
    w: &Window,
    width: usize,
    height: usize,
    b: &mut SkinBuilder,
) -> Option<SpriteId> {
    let mut canvas = RgbaImage {
        width,
        height,
        rgba: vec![0; width * height * 4],
        x: 0,
        y: 0,
    };
    let mut any = false;
    for c in &w.controls {
        if c.kind != ControlKind::Image || c.bank.is_empty() {
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
    Some(b.add(to_skin_image(&canvas, 1)))
}

fn load_fileopt(assets: &GameAssets, b: &mut SkinBuilder) -> Option<FileOptArt> {
    let w = Window::parse(assets.blob("fileopt.win")?).ok()?;
    let width = (w.width as usize).max(475);
    let height = (w.height as usize).max(592);
    let frame = compose_window_images(assets, &w, width, height, b)?;
    let mut buttons = Vec::new();
    for c in &w.controls {
        if c.kind != ControlKind::Button || c.bank.is_empty() {
            continue;
        }
        let Some((up, down)) = c.button_frames() else {
            continue;
        };
        if let Some(sprites) = pair1x(assets, &c.bank, usize::from(up), usize::from(down), b) {
            buttons.push((
                c.id,
                [
                    f32::from(c.rect.x),
                    f32::from(c.rect.y),
                    f32::from(c.rect.w),
                    f32::from(c.rect.h),
                ],
                sprites,
            ));
        }
    }
    Some(FileOptArt {
        w: width as f32,
        h: height as f32,
        frame,
        buttons,
    })
}

fn load_helpd(assets: &GameAssets, b: &mut SkinBuilder) -> Option<HelpdArt> {
    let w = Window::parse(assets.blob("helpd.win")?).ok()?;
    // Button 103 extends to x = 327 + 138 = 465; pad canvas width so nothing is clipped.
    let width = (w.width as usize).max(470);
    let height = (w.height as usize).max(351);
    let frame = compose_window_images(assets, &w, width, height, b)?;
    let r_text = w
        .controls
        .iter()
        .find(|c| c.id == 101)
        .map_or([85.0, 101.0, 246.0, 123.0], |c| {
            [
                f32::from(c.rect.x),
                f32::from(c.rect.y),
                f32::from(c.rect.w),
                f32::from(c.rect.h),
            ]
        });
    let btn = |id: u16,
               fallback: [f32; 4],
               b: &mut SkinBuilder|
     -> (u16, [f32; 4], Option<(SpriteId, SpriteId)>) {
        let Some(c) = w.controls.iter().find(|c| c.id == id) else {
            return (id, fallback, None);
        };
        let rect = [
            f32::from(c.rect.x),
            f32::from(c.rect.y),
            f32::from(c.rect.w),
            f32::from(c.rect.h),
        ];
        let sprites = c
            .button_frames()
            .and_then(|(up, down)| pair1x(assets, &c.bank, usize::from(up), usize::from(down), b));
        (id, rect, sprites)
    };
    let buttons = [
        btn(102, [72.0, 246.0, 138.0, 59.0], b),
        btn(104, [216.0, 246.0, 112.0, 59.0], b),
        btn(103, [327.0, 246.0, 138.0, 59.0], b),
    ];
    Some(HelpdArt {
        w: width as f32,
        h: height as f32,
        frame,
        r_text,
        buttons,
    })
}

pub(crate) fn load(assets: &GameAssets, b: &mut SkinBuilder) -> CampModalArt {
    CampModalArt {
        campaign_bg: load_campaign_bg(assets, b),
        radio: pair2x(assets, "campmisc.imb", 0, 1, b),
        medals: [
            add2x(assets, "campmisc.imb", 2, b),
            add2x(assets, "campmisc.imb", 3, b),
            add2x(assets, "campmisc.imb", 4, b),
        ],
        lock_x: add2x(assets, "campmisc.imb", 5, b),
        fileopt: load_fileopt(assets, b),
        helpd: load_helpd(assets, b),
    }
}
