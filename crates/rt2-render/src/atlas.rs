//! Sprite atlas with a one-off upscaling bake.
//!
//! The original sprites are 8-bit palette images drawn at a single scale (`sprite-catalog.md`
//! section 2.3). To look good at 2-4x on high-DPI screens every sprite is baked once, at load:
//!
//! 1. recoloured to RGBA with a binary alpha (palette index 0 is the key),
//! 2. enlarged 2x with Scale2x/EPX ([`scale2x`]), which rounds the staircase of the silhouette and
//!    of flat colour areas without touching dithered shading,
//! 3. packed into one atlas with a transparent gutter, premultiplied, with a few mip levels.
//!
//! The GPU then samples it with a "sharp bilinear" filter (see `sprite.wgsl`): texels stay crisp
//! but transitions between them are exactly one screen pixel wide at any scale.

use crate::assets::RgbaImage;
use crate::textures::{downsample, premultiply};

/// Upscale factor of the bake.
pub const BAKE_SCALE: usize = 2;
/// Transparent border around each baked sprite, in baked texels (covers 2 mip levels).
pub const GUTTER: usize = 4;
pub const ATLAS_WIDTH: usize = 4096;
pub const MAX_ATLAS_HEIGHT: usize = 8192;
pub const ATLAS_MIPS: usize = 4;

/// Scale2x (AdvMAME2x / EPX) on an RGBA image. All fully transparent pixels count as equal.
pub fn scale2x(src: &RgbaImage) -> RgbaImage {
    let (w, h) = (src.width, src.height);
    let px = |x: isize, y: isize| -> [u8; 4] {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            return [0; 4];
        }
        let i = (y as usize * w + x as usize) * 4;
        let p = [
            src.rgba[i],
            src.rgba[i + 1],
            src.rgba[i + 2],
            src.rgba[i + 3],
        ];
        if p[3] == 0 { [0; 4] } else { p }
    };
    let mut out = vec![0u8; w * h * 4 * 4];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let p = px(x, y);
            let (a, b, c, d) = (px(x, y - 1), px(x + 1, y), px(x - 1, y), px(x, y + 1));
            let e0 = if c == a && c != d && a != b { a } else { p };
            let e1 = if a == b && a != c && b != d { b } else { p };
            let e2 = if d == c && d != b && c != a { c } else { p };
            let e3 = if b == d && b != a && d != c { d } else { p };
            let ow = w * 2;
            let (ox, oy) = (x as usize * 2, y as usize * 2);
            for (dx, dy, e) in [(0, 0, e0), (1, 0, e1), (0, 1, e2), (1, 1, e3)] {
                let o = ((oy + dy) * ow + ox + dx) * 4;
                out[o..o + 4].copy_from_slice(&e);
            }
        }
    }
    RgbaImage {
        width: w * 2,
        height: h * 2,
        x: src.x,
        y: src.y,
        rgba: out,
    }
}

/// Where a sprite sits in the atlas and how it is placed relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasEntry {
    /// `[u0, v0, u1, v1]` of the baked image (gutter excluded).
    pub uv: [f32; 4],
    /// Box relative to the anchor at native scale: `[x, y, w, h]` (y down, negative = above).
    pub rect: [f32; 4],
}

/// The packed atlas: premultiplied RGBA8, level 0 first.
pub struct Atlas {
    pub width: usize,
    pub height: usize,
    pub mips: Vec<Vec<u8>>,
    pub entries: Vec<Option<AtlasEntry>>,
}

/// Bake and pack `images` (index-stable: `entries[i]` is for `images[i]`; images that do not fit
/// or are empty get `None`).
pub fn bake_atlas(images: &[RgbaImage]) -> Atlas {
    let baked: Vec<RgbaImage> = images.iter().map(scale2x).collect();

    // Shelf packing, tallest first.
    let mut order: Vec<usize> = (0..baked.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(baked[i].height), i));
    let mut placed: Vec<Option<(usize, usize)>> = vec![None; baked.len()];
    let (mut cx, mut cy, mut shelf_h) = (0usize, 0usize, 0usize);
    for &i in &order {
        let (w, h) = (baked[i].width + 2 * GUTTER, baked[i].height + 2 * GUTTER);
        if w > ATLAS_WIDTH {
            continue;
        }
        if cx + w > ATLAS_WIDTH {
            cx = 0;
            cy += shelf_h;
            shelf_h = 0;
        }
        if cy + h > MAX_ATLAS_HEIGHT {
            continue;
        }
        placed[i] = Some((cx + GUTTER, cy + GUTTER));
        cx += w;
        shelf_h = shelf_h.max(h);
    }
    let used_h = cy + shelf_h;
    let height = used_h
        .next_multiple_of(1 << ATLAS_MIPS)
        .max(1 << ATLAS_MIPS);
    let width = ATLAS_WIDTH;

    let mut base = vec![0u8; width * height * 4];
    let mut entries = vec![None; baked.len()];
    for (i, img) in baked.iter().enumerate() {
        let Some((px, py)) = placed[i] else { continue };
        if img.width == 0 {
            continue;
        }
        let mut rgba = img.rgba.clone();
        premultiply(&mut rgba);
        for row in 0..img.height {
            let dst = ((py + row) * width + px) * 4;
            base[dst..dst + img.width * 4]
                .copy_from_slice(&rgba[row * img.width * 4..(row + 1) * img.width * 4]);
        }
        let (fw, fh) = (width as f32, height as f32);
        entries[i] = Some(AtlasEntry {
            uv: [
                px as f32 / fw,
                py as f32 / fh,
                (px + img.width) as f32 / fw,
                (py + img.height) as f32 / fh,
            ],
            rect: [
                images[i].x as f32,
                images[i].y as f32,
                images[i].width as f32,
                images[i].height as f32,
            ],
        });
    }

    let mut mips = vec![base];
    let (mut mw, mut mh) = (width, height);
    for _ in 1..ATLAS_MIPS {
        let (next, nw, nh) = downsample(mips.last().unwrap(), mw, mh);
        mips.push(next);
        (mw, mh) = (nw, nh);
    }
    Atlas {
        width,
        height,
        mips,
        entries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: usize, h: usize, f: impl Fn(usize, usize) -> [u8; 4]) -> RgbaImage {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend(f(x, y));
            }
        }
        RgbaImage {
            width: w,
            height: h,
            x: -1,
            y: -2,
            rgba,
        }
    }

    const R: [u8; 4] = [255, 0, 0, 255];
    const N: [u8; 4] = [0; 4];

    #[test]
    fn scale2x_doubles_size_and_keeps_flat_areas() {
        let flat = img(3, 3, |_, _| R);
        let big = scale2x(&flat);
        assert_eq!((big.width, big.height), (6, 6));
        // The interior stays solid, only the outer transparent neighbours could round corners.
        let at = |x: usize, y: usize| &big.rgba[(y * 6 + x) * 4..][..4];
        assert_eq!(at(2, 2), R);
        assert_eq!(at(3, 3), R);
        assert_eq!((big.x, big.y), (-1, -2));
    }

    #[test]
    fn scale2x_rounds_a_diagonal_staircase() {
        // A solid lower-left triangle: the diagonal staircase gets half-steps.
        let tri = img(8, 4, |x, y| if x <= 2 * y + 1 { R } else { N });
        let big = scale2x(&tri);
        let solid = big
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] == 255)
            .count();
        let nearest: usize = tri
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] == 255)
            .count()
            * 4;
        assert!(solid != nearest, "solid = {solid}");
    }

    #[test]
    fn scale2x_leaves_isolated_pixels_alone() {
        // Checkerboard: the centre pixel's four neighbours are all equal, so nothing changes.
        let dots = img(3, 3, |x, y| {
            if (x + y) % 2 == 0 {
                R
            } else {
                [0, 255, 0, 255]
            }
        });
        let big = scale2x(&dots);
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            assert_eq!(&big.rgba[(y * 6 + x) * 4..][..4], R);
        }
    }

    #[test]
    fn atlas_packs_without_overlap_and_maps_uvs_inside() {
        let imgs: Vec<RgbaImage> = (0..40)
            .map(|i| img(20 + i % 7, 30 + i % 5, |_, _| R))
            .collect();
        let atlas = bake_atlas(&imgs);
        assert_eq!(atlas.mips.len(), ATLAS_MIPS);
        assert_eq!(atlas.mips[0].len(), atlas.width * atlas.height * 4);
        let mut rects = Vec::new();
        for e in &atlas.entries {
            let e = e.expect("everything fits");
            assert!(e.uv[0] >= 0.0 && e.uv[2] <= 1.0 && e.uv[1] >= 0.0 && e.uv[3] <= 1.0);
            let (w, h) = (atlas.width as f32, atlas.height as f32);
            rects.push((e.uv[0] * w, e.uv[1] * h, e.uv[2] * w, e.uv[3] * h));
        }
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                let apart = a.2 + GUTTER as f32 <= b.0
                    || b.2 + GUTTER as f32 <= a.0
                    || a.3 + GUTTER as f32 <= b.1
                    || b.3 + GUTTER as f32 <= a.1;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn oversized_sprites_are_dropped_not_fatal() {
        let huge = img(ATLAS_WIDTH, 2, |_, _| R); // baked width 2 * 4096 > atlas
        let atlas = bake_atlas(&[huge]);
        assert!(atlas.entries[0].is_none());
    }
}
