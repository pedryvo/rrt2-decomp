//! Loading the original game's graphics into memory.
//!
//! The PK2 archives are read from the install directory (`--game-dir` / `RT2_DIR`) and the sprite
//! banks (LAD, `docs/formats/lad.md`) are decoded to RGBA on demand. Nothing is ever written back
//! to the game folder, and nothing decoded is stored in the repository.

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

use rt2_formats::lad::{FLAG_PALETTE, Lad};
use rt2_formats::palette::{PaletteBank, palette_name, rgb555};
use rt2_formats::pk2::{Pk2, name_hash};

/// Archives in precedence order, first wins (`rt2-sprite` uses the same order: a patch archive
/// overrides the base game).
const ARCHIVES: [&str; 4] = ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"];

/// Palette (shade level, 0 dark ..= 31 light) that gives natural colours for ground, track and
/// trees: `docs/formats/sprite-catalog.md` section 2.2 ("rendering with palette 22").
pub const NATURAL_SHADE: usize = 22;

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a valid PK2: {source}")]
    Pk2 {
        path: String,
        #[source]
        source: rt2_formats::pk2::Pk2Error,
    },
}

/// A decoded sprite image: straight-alpha RGBA8 plus the box relative to the object's anchor.
/// Palette index 0 is the transparency key (`sprite-catalog.md` section 2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: usize,
    pub height: usize,
    /// Left edge relative to the anchor point, in pixels at native scale.
    pub x: i32,
    /// Top edge relative to the anchor point (negative = above).
    pub y: i32,
    pub rgba: Vec<u8>,
}

/// The opened PK2 archives of an installation.
pub struct GameAssets {
    archives: Vec<Vec<u8>>,
    /// Name hash -> (archive, byte range).
    index: HashMap<u32, (usize, Range<usize>)>,
}

impl GameAssets {
    /// Open the archives under `<dir>/data/`. Missing patch archives are fine, a missing `RT2.PK2`
    /// is an error.
    pub fn open(dir: &Path) -> Result<GameAssets, AssetError> {
        let mut archives = Vec::new();
        for name in ARCHIVES {
            let path = dir.join("data").join(name);
            match std::fs::read(&path) {
                Ok(raw) => archives.push(raw),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && name != "RT2.PK2" => {
                    archives.push(Vec::new());
                }
                Err(source) => {
                    return Err(AssetError::Io {
                        path: path.display().to_string(),
                        source,
                    });
                }
            }
        }
        Self::from_archives(archives)
    }

    /// Build from raw archive images in precedence order (first wins).
    pub fn from_archives(archives: Vec<Vec<u8>>) -> Result<GameAssets, AssetError> {
        let mut index = HashMap::new();
        // Insert the lowest precedence first so higher ones overwrite.
        for (i, raw) in archives.iter().enumerate().rev() {
            if raw.is_empty() {
                continue;
            }
            let pk2 = Pk2::parse(raw).map_err(|source| AssetError::Pk2 {
                path: ARCHIVES.get(i).unwrap_or(&"archive").to_string(),
                source,
            })?;
            for e in pk2.entries() {
                index.insert(e.hash, (i, pk2.range(e)));
            }
        }
        Ok(GameAssets { archives, index })
    }

    pub fn blob(&self, name: &str) -> Option<&[u8]> {
        let (i, r) = self.index.get(&name_hash(name))?;
        self.archives[*i].get(r.clone())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.index.contains_key(&name_hash(name))
    }

    pub fn lad(&self, name: &str) -> Option<Lad<'_>> {
        Lad::parse(self.blob(name)?).ok()
    }

    pub fn palette(&self, palette_id: u32) -> Option<PaletteBank> {
        PaletteBank::parse(self.blob(&palette_name(palette_id))?).ok()
    }
}

/// A parsed bank with its palettes, ready to hand out RGBA images.
pub struct LoadedBank<'a> {
    pub lad: Lad<'a>,
    palettes: HashMap<u32, PaletteBank>,
}

impl<'a> LoadedBank<'a> {
    pub fn open(assets: &'a GameAssets, name: &str) -> Option<LoadedBank<'a>> {
        let lad = assets.lad(name)?;
        let mut palettes = HashMap::new();
        for r in &lad.records {
            if r.uses_palette() && !palettes.contains_key(&r.palette_id()) {
                palettes.insert(r.palette_id(), assets.palette(r.palette_id())?);
            }
        }
        Some(LoadedBank { lad, palettes })
    }

    pub fn record_count(&self) -> usize {
        self.lad.records.len()
    }

    /// Image `image` of `group` of `record`, coloured with palette number `shade` of the record's
    /// bank (clamped to the bank size). Index 0 becomes transparent.
    pub fn image(
        &self,
        record: usize,
        group: usize,
        image: usize,
        shade: usize,
    ) -> Option<RgbaImage> {
        let rec = self.lad.records.get(record)?;
        let img = rec.groups.get(group)?.images.get(image)?;
        let px = img.decode().ok()?;
        if px.width == 0 || px.height == 0 {
            return None;
        }
        let palette: [u16; 256] = if img.flags & FLAG_PALETTE != 0 {
            let bank = self.palettes.get(&rec.palette_id())?;
            *bank
                .palettes
                .get(shade.min(bank.palettes.len().checked_sub(1)?))?
        } else {
            std::array::from_fn(|i| (i as u16 >> 3) * 0x421)
        };
        let mut rgba = Vec::with_capacity(px.indices.len() * 4);
        for (&i, &opaque) in px.indices.iter().zip(&px.opaque) {
            let [r, g, b] = rgb555(palette[i as usize]);
            rgba.extend([r, g, b, if opaque && i != 0 { 255 } else { 0 }]);
        }
        Some(RgbaImage {
            width: px.width,
            height: px.height,
            x: img.x,
            y: img.y,
            rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_archive_is_an_error_but_patches_are_optional() {
        let dir = std::env::temp_dir().join("rrt2-no-such-game-dir");
        assert!(matches!(GameAssets::open(&dir), Err(AssetError::Io { .. })));
    }

    #[test]
    fn empty_archive_set_has_no_entries() {
        let a = GameAssets::from_archives(vec![Vec::new(); 4]).unwrap();
        assert!(a.blob("ground.imb").is_none());
        assert!(!a.contains("ground.imb"));
    }
}
