//! CPU-side texture stacks for the ground, ocean and track overlays: 64x64 layers with a full mip
//! chain, premultiplied alpha, gamma-encoded RGBA8 (the original blends in its encoded colour
//! space, so mips and blends are done in it as well and converted to linear only at the output).
//!
//! `docs/formats/sprite-catalog.md` section 2.2: `ground`, `tracks`, `trackd` and `groceana` are
//! atlases of 64x64 textures with their own mips; we regenerate the chain with a box filter so the
//! GPU can use ordinary mip sampling at every zoom.

use crate::assets::{GameAssets, LoadedBank, NATURAL_SHADE};

pub const TEXTURE_SIZE: u32 = 64;
pub const MIP_LEVELS: u32 = 7; // 64, 32, 16, 8, 4, 2, 1

/// Many equally sized layers, each with all its mip levels.
#[derive(Debug, Clone, Default)]
pub struct LayerStack {
    pub layers: Vec<[Vec<u8>; MIP_LEVELS as usize]>,
}

impl LayerStack {
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

/// Straight-alpha RGBA -> premultiplied.
pub fn premultiply(rgba: &mut [u8]) {
    for p in rgba.as_chunks_mut::<4>().0 {
        let a = p[3] as u32;
        for c in &mut p[..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
}

/// 2x2 box filter of a premultiplied RGBA image (odd sizes clamp at the edge).
pub fn downsample(src: &[u8], w: usize, h: usize) -> (Vec<u8>, usize, usize) {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        for x in 0..nw {
            for c in 0..4 {
                let at =
                    |xx: usize, yy: usize| src[(yy.min(h - 1) * w + xx.min(w - 1)) * 4 + c] as u32;
                let sum = at(2 * x, 2 * y)
                    + at(2 * x + 1, 2 * y)
                    + at(2 * x, 2 * y + 1)
                    + at(2 * x + 1, 2 * y + 1);
                out[(y * nw + x) * 4 + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
    (out, nw, nh)
}

/// Build the mip chain of a square premultiplied RGBA image of side [`TEXTURE_SIZE`].
pub fn mip_chain(base: Vec<u8>) -> [Vec<u8>; MIP_LEVELS as usize] {
    let mut levels: Vec<Vec<u8>> = vec![base];
    let mut side = TEXTURE_SIZE as usize;
    while levels.len() < MIP_LEVELS as usize {
        let (next, nw, _) = downsample(levels.last().unwrap(), side, side);
        side = nw;
        levels.push(next);
    }
    levels.try_into().expect("MIP_LEVELS levels")
}

/// Load every record of a 64x64 texture bank as one layer (first group = full-size image). Records
/// that are missing or not 64x64 become fully transparent.
pub fn load_stack(assets: &GameAssets, bank: &str) -> LayerStack {
    let Some(b) = LoadedBank::open(assets, bank) else {
        return LayerStack::default();
    };
    let side = TEXTURE_SIZE as usize;
    let layers = (0..b.record_count())
        .map(|r| {
            let mut rgba = match b.image(r, 0, 0, NATURAL_SHADE) {
                Some(img) if img.width == side && img.height == side => img.rgba,
                _ => vec![0; side * side * 4],
            };
            premultiply(&mut rgba);
            mip_chain(rgba)
        })
        .collect();
    LayerStack { layers }
}

/// The texture stacks the terrain pass samples.
#[derive(Debug, Clone, Default)]
pub struct TerrainTextures {
    /// `ground.imb`, 221 records.
    pub ground: LayerStack,
    /// `groceana.imb` (animated ocean, 16 frames x 4 turns = 64 records).
    pub ocean: LayerStack,
    /// `tracks.imb` (65 records) followed by `trackd.imb` (69 records).
    pub track: LayerStack,
    /// `riveran.imb` (90 records = 5 animation frames x 18 pieces) followed by `road.imb` (15).
    pub extra: LayerStack,
    /// `grbeacha.imb` (256 records = 32 animation frames x 8 shore pieces).
    pub beach: LayerStack,
}

/// Layer of `trackd` record 0 inside [`TerrainTextures::track`]: the stack is single then double.
pub const TRACK_DOUBLE_BASE: usize = 65;
/// Records of `riveran.imb`; `road.imb` follows them in [`TerrainTextures::extra`].
pub const RIVER_RECORDS: usize = 90;
pub const ROAD_RECORDS: usize = 15;
pub const BEACH_RECORDS: usize = 256;

/// Pad a stack to `n` layers so the layers of the bank that follows keep a fixed base.
fn pad_stack(stack: &mut LayerStack, n: usize) {
    let empty = [(); MIP_LEVELS as usize].map(|_| vec![0u8; 4]);
    while stack.layers.len() < n {
        stack.layers.push(empty.clone());
    }
}

impl TerrainTextures {
    pub fn load(assets: &GameAssets) -> TerrainTextures {
        let mut track = load_stack(assets, "tracks.imb");
        // Keep the single/double split stable even if a bank is short.
        pad_stack(&mut track, TRACK_DOUBLE_BASE);
        track.layers.extend(load_stack(assets, "trackd.imb").layers);
        let mut extra = load_stack(assets, "riveran.imb");
        pad_stack(&mut extra, RIVER_RECORDS);
        extra.layers.extend(load_stack(assets, "road.imb").layers);
        TerrainTextures {
            ground: load_stack(assets, "ground.imb"),
            ocean: load_stack(assets, "groceana.imb"),
            track,
            extra,
            beach: load_stack(assets, "grbeacha.imb"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiply_scales_colour_by_alpha() {
        let mut px = [200, 100, 50, 128, 10, 20, 30, 0];
        premultiply(&mut px);
        assert_eq!(px, [100, 50, 25, 128, 0, 0, 0, 0]);
    }

    #[test]
    fn downsample_averages_blocks() {
        let src: Vec<u8> = [
            0u8, 0, 0, 0, 100, 100, 100, 100, 0, 0, 0, 0, 100, 100, 100, 100,
        ]
        .to_vec();
        let (out, w, h) = downsample(&src, 2, 2);
        assert_eq!((w, h), (1, 1));
        assert_eq!(out, vec![50, 50, 50, 50]);
    }

    #[test]
    fn mip_chain_ends_at_one_texel() {
        let base = vec![255u8; (TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize];
        let chain = mip_chain(base);
        assert_eq!(chain[0].len(), 64 * 64 * 4);
        assert_eq!(chain[6].len(), 4);
        assert_eq!(chain[6], vec![255; 4]);
    }
}
