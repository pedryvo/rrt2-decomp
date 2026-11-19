//! Original window artwork for the 4 full-screen list windows (`statnlst.win`, `trainlst.win`,
//! `compnlst.win`, `playrlst.win`).

use rt2_formats::ui::{ControlKind, Window};
use rt2_render::assets::{GameAssets, RgbaImage};
use rt2_ui::{SkinBuilder, SpriteId};

use super::buywin::Placed;
use super::detailwin::{blend, pair, placed};
use super::{to_skin_image, ui_record_pub};

/// Art and exact `.win` control rectangles for the 4 list windows (1024x768).
#[derive(Clone, Debug, Default)]
pub struct ListArt {
    /// Composed 1024x768 frame (`trainlst.imb` 0..=5 + `odbottom.imb` 15).
    pub frame: Option<SpriteId>,
    /// `odbottom.imb` rec 11..=14: 4-tab strip at `(0, 729)` for Stations, Trains, Players, Companies.
    pub tabs: [Option<Placed>; 4],
    /// `odbottom.imb` rec 3, 4: bottom-right map/close button at `(916, 735, 40, 31)`.
    pub btn_map: Option<(SpriteId, SpriteId)>,
    /// `odbottom.imb` rec 9, 10: OK button at `(962, 734, 60, 32)`.
    pub btn_ok: Option<(SpriteId, SpriteId)>,
    /// List table rectangle `(26, 26, 974, 686)`.
    pub r_list: [f32; 4],
    /// Bottom status label rectangle `(185, 733, 653, 35)`.
    pub r_status: [f32; 4],
    /// Map/list toggle button rectangle `(916, 735, 40, 31)`.
    pub r_btn_map: [f32; 4],
    /// OK button rectangle `(962, 734, 60, 32)`.
    pub r_btn_ok: [f32; 4],
}

pub(crate) fn load(assets: &GameAssets, b: &mut SkinBuilder) -> ListArt {
    let Some(w) = assets
        .blob("statnlst.win")
        .and_then(|blob| Window::parse(blob).ok())
    else {
        return ListArt::default();
    };
    let mut canvas = RgbaImage {
        width: 1024,
        height: 768,
        rgba: vec![0; 1024 * 768 * 4],
        x: 0,
        y: 0,
    };
    let mut any = false;
    for c in &w.controls {
        if c.kind != ControlKind::Image {
            continue;
        }
        let Some(rec) = c.image_record() else {
            continue;
        };
        // Skip the active-tab overlay (drawn separately per active tab).
        if c.bank == "odbottom.imb" && matches!(rec, 11..=14) {
            continue;
        }
        let Some(img) = ui_record_pub(assets, &c.bank, usize::from(rec)) else {
            continue;
        };
        blend(&mut canvas, &img, i32::from(c.rect.x), i32::from(c.rect.y));
        any = true;
    }
    if !any {
        return ListArt::default();
    }
    let rect = |id: u16, fallback: [f32; 4]| -> [f32; 4] {
        w.controls.iter().find(|c| c.id == id).map_or(fallback, |c| {
            [
                f32::from(c.rect.x),
                f32::from(c.rect.y),
                f32::from(c.rect.w),
                f32::from(c.rect.h),
            ]
        })
    };
    ListArt {
        frame: Some(b.add(to_skin_image(&canvas, 1))),
        tabs: [
            None,
            None,
            placed(assets, "odbottom.imb", 13, 0, 729, b),
            placed(assets, "odbottom.imb", 14, 0, 729, b),
        ],
        btn_map: pair(assets, "odbottom.imb", 3, 4, None, None, b),
        btn_ok: None,
        r_list: rect(1201, [26.0, 26.0, 974.0, 686.0]),
        r_status: rect(1271, [185.0, 733.0, 653.0, 35.0]),
        r_btn_map: rect(1282, [916.0, 735.0, 40.0, 31.0]),
        r_btn_ok: rect(1278, [962.0, 734.0, 60.0, 32.0]),
    }
}
