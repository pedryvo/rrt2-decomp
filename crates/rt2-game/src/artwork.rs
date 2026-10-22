//! The original UI art as a skin (docs/ui.md "Skin"): the title backdrop, the grey button plates
//! and the dark panel plate, decoded from the install and upscaled like the map sprites.
//! Without the install art everything falls back to the flat theme.

use rt2_render::assets::{GameAssets, LoadedBank, RgbaImage};
use rt2_render::atlas::scale2x;
use rt2_ui::{NineSlice, Skin, SkinBuilder, SkinImage, SpriteId, Theme};

/// `setup.imb` record 0: the 1024x768 title backdrop of `setup.win`.
const BACKDROP: (&str, usize) = ("setup.imb", 0);
/// `setupb.imb` records 0 and 1: the button plate, up and down (`setup.win` buttons 3135..3140).
const BUTTON: (&str, usize, usize) = ("setupb.imb", 0, 1);
/// `setupp.imb` record 2: a dark metal plate (the panels of `setup.win`), 490x417.
const PANEL: (&str, usize) = ("setupp.imb", 2);

/// 9-slice insets in original pixels (the plates' bevel and rivets).
const BUTTON_INSETS: [f32; 4] = [8.0, 8.0, 8.0, 8.0];
const PANEL_INSETS: [f32; 4] = [18.0, 18.0, 18.0, 18.0];

pub struct Artwork {
    pub skin: Skin,
    pub backdrop: Option<SpriteId>,
}

fn to_skin_image(img: &RgbaImage, bake: u32) -> SkinImage {
    SkinImage {
        width: img.width as u32,
        height: img.height as u32,
        rgba: img.rgba.clone(),
        bake,
    }
}

fn record(assets: &GameAssets, bank: &str, rec: usize) -> Option<RgbaImage> {
    LoadedBank::open(assets, bank)?.image(rec, 0, 0, 0)
}

/// Opaque copy: transparent texels of the plates (palette index 0) become the edge colour so
/// stretched plates have no holes.
fn solidify(img: &mut RgbaImage) {
    let n = img.rgba.len() / 4;
    let mut sum = [0u64; 3];
    let mut count = 0u64;
    for i in 0..n {
        if img.rgba[i * 4 + 3] != 0 {
            for (c, acc) in sum.iter_mut().enumerate() {
                *acc += u64::from(img.rgba[i * 4 + c]);
            }
            count += 1;
        }
    }
    if count == 0 {
        return;
    }
    let avg = [
        (sum[0] / count) as u8,
        (sum[1] / count) as u8,
        (sum[2] / count) as u8,
    ];
    for i in 0..n {
        if img.rgba[i * 4 + 3] == 0 {
            img.rgba[i * 4..i * 4 + 4].copy_from_slice(&[avg[0], avg[1], avg[2], 255]);
        }
    }
}

pub fn load(assets: &GameAssets) -> Artwork {
    let mut b = SkinBuilder::new();
    // The backdrop is a photograph: no pixel-art upscale, bilinear filtering only.
    let backdrop = record(assets, BACKDROP.0, BACKDROP.1).map(|mut i| {
        solidify(&mut i);
        b.add(to_skin_image(&i, 1))
    });
    let buttons = record(assets, BUTTON.0, BUTTON.1)
        .zip(record(assets, BUTTON.0, BUTTON.2))
        .map(|(mut up, mut down)| {
            solidify(&mut up);
            solidify(&mut down);
            // Same size for both states so the 9-slice does not jump when pressed.
            let (w, h) = (up.width.min(down.width), up.height.min(down.height));
            let crop = |i: &RgbaImage| RgbaImage {
                width: w,
                height: h,
                x: 0,
                y: 0,
                rgba: (0..h)
                    .flat_map(|y| i.rgba[y * i.width * 4..y * i.width * 4 + w * 4].to_vec())
                    .collect(),
            };
            (
                b.add(to_skin_image(&scale2x(&crop(&up)), 2)),
                b.add(to_skin_image(&scale2x(&crop(&down)), 2)),
            )
        });
    let panel = record(assets, PANEL.0, PANEL.1).map(|mut i| {
        solidify(&mut i);
        b.add(to_skin_image(&scale2x(&i), 2))
    });
    let mut skin = b.build(Theme::dark());
    skin.button = buttons.map(|(up, down)| rt2_ui::ButtonSkin {
        up: NineSlice {
            sprite: up,
            insets: BUTTON_INSETS,
        },
        down: NineSlice {
            sprite: down,
            insets: BUTTON_INSETS,
        },
    });
    skin.panel = panel.map(|p| NineSlice {
        sprite: p,
        insets: PANEL_INSETS,
    });
    Artwork { skin, backdrop }
}
