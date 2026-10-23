//! Skin: the sprite atlas and 9-slice definitions used to draw panels and buttons, plus the
//! colour theme. The application supplies the images (`rt2-game` decodes the original UI sprites
//! from the install and upscales them); with none supplied the skin is the flat fallback.

use crate::ui::Color;

/// A straight-alpha RGBA8 image to put in the atlas.
#[derive(Clone, Debug)]
pub struct SkinImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// How many times larger than the original art this image is (2 after the Scale2x bake), so
    /// the logical size stays that of the original.
    pub bake: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpriteId(pub(crate) u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Sprite {
    /// Interior texture rectangle (u0, v0, u1, v1).
    pub uv: [f32; 4],
    /// Size in original pixels.
    pub size: (f32, f32),
    pub bake: f32,
    pub valid: bool,
}

/// A sprite stretched by 9-slice. `insets` = left, top, right, bottom in original pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NineSlice {
    pub sprite: SpriteId,
    pub insets: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonSkin {
    pub up: NineSlice,
    pub down: NineSlice,
}

/// Colours of the flat fallback and of text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub text: Color,
    pub text_dim: Color,
    pub text_disabled: Color,
    /// Text drawn on the original button plates (light metal).
    pub text_on_button: Color,
    pub panel_fill: Color,
    pub panel_border: Color,
    pub button_fill: Color,
    pub button_border: Color,
    pub accent: Color,
    pub row_selected: Color,
    pub row_hover: Color,
    pub shadow: Color,
}

impl Theme {
    /// Dark industrial palette, close to the original's grimy greens and browns.
    pub const fn dark() -> Theme {
        Theme {
            text: Color::rgb(0xf2, 0xe9, 0xd0),
            text_dim: Color::rgb(0xb4, 0xab, 0x92),
            text_disabled: Color::rgb(0x7a, 0x75, 0x68),
            text_on_button: Color::rgb(0x18, 0x14, 0x0c),
            panel_fill: Color(0x1c, 0x20, 0x1a, 0xe6),
            panel_border: Color::rgb(0x8a, 0x7c, 0x55),
            button_fill: Color::rgb(0x3a, 0x3c, 0x30),
            button_border: Color::rgb(0xa8, 0x98, 0x68),
            accent: Color::rgb(0xff, 0xd8, 0x50),
            row_selected: Color(0xa8, 0x98, 0x68, 0x80),
            row_hover: Color(0xff, 0xff, 0xff, 0x24),
            shadow: Color(0, 0, 0, 0xc0),
        }
    }
}

pub struct Skin {
    pub theme: Theme,
    pub(crate) atlas_size: (u32, u32),
    pub(crate) atlas: Vec<u8>,
    pub(crate) sprites: Vec<Sprite>,
    /// Centre of an opaque white texel (solid fills draw from here).
    pub(crate) white: [f32; 2],
    pub panel: Option<NineSlice>,
    pub button: Option<ButtonSkin>,
    /// Logical units per original pixel (the art is drawn at this scale at 100 % UI scale).
    pub art_scale: f32,
}

impl Skin {
    /// The flat fallback (no images): solid colours only.
    pub fn flat() -> Skin {
        SkinBuilder::new().build(Theme::dark())
    }

    pub fn has_art(&self) -> bool {
        self.panel.is_some() || self.button.is_some()
    }

    pub fn atlas_size(&self) -> (u32, u32) {
        self.atlas_size
    }

    /// The pixel rectangle `(x, y, width, height)` of a sprite in the atlas, for sprites whose
    /// pixels the application replaces while running (see [`UiRenderer::upload_sprite`]).
    pub fn sprite_rect(&self, id: SpriteId) -> Option<(u32, u32, u32, u32)> {
        let s = self.sprite(id)?;
        let (aw, ah) = (self.atlas_size.0 as f32, self.atlas_size.1 as f32);
        let (x, y) = ((s.uv[0] * aw).round(), (s.uv[1] * ah).round());
        let (w, h) = (
            ((s.uv[2] - s.uv[0]) * aw).round(),
            ((s.uv[3] - s.uv[1]) * ah).round(),
        );
        Some((x as u32, y as u32, w as u32, h as u32))
    }

    pub(crate) fn sprite(&self, id: SpriteId) -> Option<&Sprite> {
        self.sprites.get(id.0 as usize).filter(|s| s.valid)
    }

    /// Size in original pixels of a sprite.
    pub fn sprite_size(&self, id: SpriteId) -> Option<(f32, f32)> {
        self.sprite(id).map(|s| s.size)
    }
}

pub const ATLAS_WIDTH: u32 = 4096;
pub const MAX_ATLAS_HEIGHT: u32 = 8192;
/// Border of extruded edge texels around each sprite (keeps bilinear filtering inside it).
const GUTTER: u32 = 2;

#[derive(Default)]
pub struct SkinBuilder {
    images: Vec<SkinImage>,
}

impl SkinBuilder {
    pub fn new() -> SkinBuilder {
        SkinBuilder::default()
    }

    pub fn add(&mut self, image: SkinImage) -> SpriteId {
        self.images.push(image);
        SpriteId(self.images.len() as u32 - 1)
    }

    /// Reserves a blank `width` x `height` sprite (original pixels, no bake) whose pixels the
    /// application fills later with [`UiRenderer::upload_sprite`]: a picture composed at run
    /// time, such as the station view.
    pub fn reserve(&mut self, width: u32, height: u32) -> SpriteId {
        self.add(SkinImage {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
            bake: 1,
        })
    }

    /// Packs the images (shelf packing, tallest first is not needed: images are few and big).
    pub fn build(self, theme: Theme) -> Skin {
        // Reserve a white block first, then the images in order.
        let mut placements: Vec<Option<(u32, u32)>> = Vec::new();
        let (mut x, mut y, mut shelf_h) = (0u32, 0u32, 0u32);
        let white_size = 4 + 2 * GUTTER;
        x += white_size;
        shelf_h = shelf_h.max(white_size);
        for img in &self.images {
            let (w, h) = (img.width + 2 * GUTTER, img.height + 2 * GUTTER);
            if w > ATLAS_WIDTH {
                placements.push(None);
                continue;
            }
            if x + w > ATLAS_WIDTH {
                y += shelf_h;
                x = 0;
                shelf_h = 0;
            }
            if y + h > MAX_ATLAS_HEIGHT {
                placements.push(None);
                continue;
            }
            placements.push(Some((x, y)));
            x += w;
            shelf_h = shelf_h.max(h);
        }
        let width = ATLAS_WIDTH;
        let height = (y + shelf_h).next_power_of_two().min(MAX_ATLAS_HEIGHT);
        let mut atlas = vec![0u8; (width * height * 4) as usize];
        // White block.
        fill_block(&mut atlas, width, 0, 0, white_size, white_size, [255; 4]);
        let white = [
            (GUTTER as f32 + 2.0) / width as f32,
            (GUTTER as f32 + 2.0) / height as f32,
        ];
        let mut sprites = Vec::new();
        for (img, place) in self.images.iter().zip(&placements) {
            let Some(&(px, py)) = place.as_ref() else {
                sprites.push(Sprite {
                    uv: [0.0; 4],
                    size: (0.0, 0.0),
                    bake: 1.0,
                    valid: false,
                });
                continue;
            };
            blit_extruded(&mut atlas, width, px, py, img);
            let (ix, iy) = ((px + GUTTER) as f32, (py + GUTTER) as f32);
            sprites.push(Sprite {
                uv: [
                    ix / width as f32,
                    iy / height as f32,
                    (ix + img.width as f32) / width as f32,
                    (iy + img.height as f32) / height as f32,
                ],
                size: (
                    img.width as f32 / img.bake as f32,
                    img.height as f32 / img.bake as f32,
                ),
                bake: img.bake as f32,
                valid: true,
            });
        }
        Skin {
            theme,
            atlas_size: (width, height),
            atlas,
            sprites,
            white,
            panel: None,
            button: None,
            art_scale: 1.0,
        }
    }
}

fn fill_block(atlas: &mut [u8], stride: u32, x: u32, y: u32, w: u32, h: u32, px: [u8; 4]) {
    for yy in y..y + h {
        for xx in x..x + w {
            let i = ((yy * stride + xx) * 4) as usize;
            atlas[i..i + 4].copy_from_slice(&px);
        }
    }
}

/// Copies an image into the atlas with a border of its own edge texels around it.
fn blit_extruded(atlas: &mut [u8], stride: u32, px: u32, py: u32, img: &SkinImage) {
    let (w, h) = (img.width as i64, img.height as i64);
    let g = GUTTER as i64;
    for ay in -g..h + g {
        for ax in -g..w + g {
            let sx = ax.clamp(0, w - 1) as usize;
            let sy = ay.clamp(0, h - 1) as usize;
            let s = (sy * img.width as usize + sx) * 4;
            let d = ((((py as i64 + g + ay) as u32) * stride + (px as i64 + g + ax) as u32) * 4)
                as usize;
            atlas[d..d + 4].copy_from_slice(&img.rgba[s..s + 4]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> SkinImage {
        SkinImage {
            width: w,
            height: h,
            rgba: c.repeat((w * h) as usize),
            bake: 2,
        }
    }

    #[test]
    fn flat_skin_has_only_the_white_block() {
        let s = Skin::flat();
        assert!(!s.has_art());
        let (w, h) = s.atlas_size();
        assert_eq!(s.atlas.len(), (w * h * 4) as usize);
        let i =
            ((((s.white[1] * h as f32) as u32) * w + (s.white[0] * w as f32) as u32) * 4) as usize;
        assert_eq!(&s.atlas[i..i + 4], &[255; 4]);
    }

    #[test]
    fn images_are_packed_without_overlap_and_report_original_size() {
        let mut b = SkinBuilder::new();
        let a = b.add(solid(300, 100, [255, 0, 0, 255]));
        let c = b.add(solid(2048, 1536, [0, 255, 0, 255]));
        let d = b.add(solid(1000, 20, [0, 0, 255, 255]));
        let s = b.build(Theme::dark());
        assert_eq!(s.sprite_size(a), Some((150.0, 50.0)));
        assert_eq!(s.sprite_size(c), Some((1024.0, 768.0)));
        let (w, h) = s.atlas_size();
        // the centre texel of every sprite has its own colour
        for (id, col) in [
            (a, [255, 0, 0, 255]),
            (c, [0, 255, 0, 255]),
            (d, [0, 0, 255, 255]),
        ] {
            let sp = s.sprite(id).unwrap();
            let cx = ((sp.uv[0] + sp.uv[2]) / 2.0 * w as f32) as u32;
            let cy = ((sp.uv[1] + sp.uv[3]) / 2.0 * h as f32) as u32;
            let i = ((cy * w + cx) * 4) as usize;
            assert_eq!(&s.atlas[i..i + 4], &col);
        }
    }

    #[test]
    fn oversized_images_are_dropped_not_fatal() {
        let mut b = SkinBuilder::new();
        let big = b.add(solid(5000, 10, [1, 2, 3, 255]));
        let s = b.build(Theme::dark());
        assert!(s.sprite(big).is_none());
    }
}
