//! Original window artwork for the Stock Market screen (`stockdtl.win`, `stockdtl.imb`).

use rt2_render::assets::{GameAssets, RgbaImage};
use rt2_ui::{SkinBuilder, SpriteId};

use super::detailwin::blend;
use super::{to_skin_image, ui_record_pub};

/// Art and exact `.win` control rectangles for `stockdtl.win` (1024x768).
#[derive(Clone, Debug, Default)]
pub struct StockArt {
    /// Composited 1024x768 desk/clipboard/glass-dome scene from `stockdtl.imb`.
    pub frame: Option<SpriteId>,
    /// `Image 1900`: main notepad/clipboard area (`409, 82, 577, 446`).
    pub r_pad: [f32; 4],
    /// `Bar 1908`: upper-left summary area on the notepad (`410, 81, 240, 104`).
    pub r_summary: [f32; 4],
    /// `Bar 1904`: Personal tab (`431, 538, 85, 27`).
    pub r_tab_personal: [f32; 4],
    /// `Bar 1905`: Corporate tab (`522, 538, 85, 27`).
    pub r_tab_corporate: [f32; 4],
    /// `Label 1971`: bottom status bar text (`100, 733, 824, 35`).
    pub r_status: [f32; 4],
    /// `Button 1978`: bottom-right OK button (`962, 734, 60, 32`).
    pub r_ok: [f32; 4],
}

pub(crate) fn load(assets: &GameAssets, b: &mut SkinBuilder) -> StockArt {
    use rt2_formats::ui::{ControlKind, Window};
    let Some(blob) = assets.blob("stockdtl.win") else {
        return StockArt::default();
    };
    let Ok(w) = Window::parse(blob) else {
        return StockArt::default();
    };
    let mut controls: Vec<_> = w.controls.iter().collect();
    controls.sort_by_key(|c| c.order);

    let mut canvas = RgbaImage {
        width: 1024,
        height: 768,
        rgba: vec![0; 1024 * 768 * 4],
        x: 0,
        y: 0,
    };
    let mut any = false;
    for c in &controls {
        if c.kind != ControlKind::Image || c.bank == "odbottom.imb" {
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
        return StockArt::default();
    }
    let frame = Some(b.add(to_skin_image(&canvas, 1)));
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
    StockArt {
        frame,
        r_pad: rect(1900, [409.0, 82.0, 577.0, 446.0]),
        r_summary: rect(1908, [410.0, 81.0, 240.0, 104.0]),
        r_tab_personal: rect(1904, [431.0, 538.0, 85.0, 27.0]),
        r_tab_corporate: rect(1905, [522.0, 538.0, 85.0, 27.0]),
        r_status: rect(1971, [100.0, 733.0, 824.0, 35.0]),
        r_ok: rect(1978, [962.0, 734.0, 60.0, 32.0]),
    }
}
