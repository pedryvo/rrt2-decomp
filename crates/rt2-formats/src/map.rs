//! Scenario maps and saves (`maps/*.MAP`, `games/*.GAM`), see `docs/formats/map-gam.md`.
//!
//! Only the confirmed leading part of the file is parsed (everything after the elevation grid is
//! game state and is not decoded yet). Layout, all little-endian:
//!
//! ```text
//! 0x000  u32 0x233C            magic/version
//! 0x004  char[]                short name, NUL padded
//! 0x131  char[]                scenario title (medium confidence)
//! 0x231  u16 w, u16 h          map size in tiles
//! 0xB2D7 w*h * [u8; 10]        tile records, row by row (y*w + x), north at the top
//! next   (w+1)*(h+1) * i16     corner elevations, row by row
//! ```

/// Magic/version word at offset 0.
pub const MAGIC: u32 = 0x233C;
const NAME_OFFSET: usize = 0x004;
const TITLE_OFFSET: usize = 0x131;
const TITLE_END: usize = 0x150;
const SIZE_OFFSET: usize = 0x231;
/// Start of the tile records (fixed: the header has a fixed size).
pub const TILES_OFFSET: usize = 0xB2D7;
pub const TILE_SIZE: usize = 10;
/// Ground ids 0x61 and 0x62 are water (ocean, lakes, bays); high confidence.
pub const GROUND_WATER: [u8; 2] = [0x61, 0x62];
/// Sanity limit; the largest original map is 500x500.
const MAX_DIM: usize = 4096;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MapError {
    #[error("file is {len} bytes, too short for {what}")]
    Truncated { len: usize, what: &'static str },
    #[error("bad magic {0:#x}, expected 0x233c")]
    BadMagic(u32),
    #[error("implausible map size {width}x{height}")]
    BadSize { width: usize, height: usize },
}

/// One 10-byte tile record. Field names and confidence follow `docs/formats/map-gam.md` section 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    /// Byte 0: texture variant/rotation noise (0/0x40/0x80/0xC0 plus a low nibble). Low confidence.
    pub var: u8,
    /// Byte 1: ground type id (0x00..=0x62). 0x61/0x62 are water (high confidence); 0x00 outlines
    /// coasts; the rest is only a guess (vegetation/soil/rock).
    pub ground: u8,
    /// Byte 2: unknown flags (bit 6 on most land tiles). Low confidence.
    pub flags2: u8,
    /// Byte 3: tree density, 0..=28 (medium confidence).
    pub trees: u8,
    /// Byte 4: 0 or a growing id of a building (town/industry) in a table. Medium confidence.
    pub struct_id: u8,
    /// Byte 5: always 0 in all original files.
    pub reserved5: u8,
    /// Byte 6: unknown bitfield (bits 3 and 4 always set on water; coast/river?). Low confidence.
    pub flags6: u8,
    /// Byte 7: 0 or an id of pre-built track. Medium confidence.
    pub track_id: u8,
    /// Byte 8: always 0 in all original files.
    pub reserved8: u8,
    /// Byte 9: territory/region number, 1..=N (high confidence).
    pub region: u8,
}

impl Tile {
    fn from_bytes(b: &[u8; TILE_SIZE]) -> Self {
        Tile {
            var: b[0],
            ground: b[1],
            flags2: b[2],
            trees: b[3],
            struct_id: b[4],
            reserved5: b[5],
            flags6: b[6],
            track_id: b[7],
            reserved8: b[8],
            region: b[9],
        }
    }

    pub fn is_water(&self) -> bool {
        GROUND_WATER.contains(&self.ground)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    /// Short name at 0x004 (the 8.3 file name for the numbered scenarios).
    pub name: String,
    /// Scenario title at 0x131 (medium confidence; may be empty).
    pub title: String,
    pub width: usize,
    pub height: usize,
    /// `width * height` tiles, row-major.
    pub tiles: Vec<Tile>,
    /// `(width + 1) * (height + 1)` corner elevations, row-major. The sea floor can be negative.
    pub elevation: Vec<i16>,
}

fn c_string(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    // Latin-1 is a superset of the ASCII text actually seen.
    raw[..end].iter().map(|&b| b as char).collect()
}

impl Map {
    pub fn parse(raw: &[u8]) -> Result<Map, MapError> {
        if raw.len() < SIZE_OFFSET + 4 {
            return Err(MapError::Truncated {
                len: raw.len(),
                what: "the header",
            });
        }
        let magic = u32::from_le_bytes(raw[0..4].try_into().unwrap());
        if magic != MAGIC {
            return Err(MapError::BadMagic(magic));
        }
        let width = u16::from_le_bytes([raw[SIZE_OFFSET], raw[SIZE_OFFSET + 1]]) as usize;
        let height = u16::from_le_bytes([raw[SIZE_OFFSET + 2], raw[SIZE_OFFSET + 3]]) as usize;
        if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
            return Err(MapError::BadSize { width, height });
        }
        let tiles_end = TILES_OFFSET + TILE_SIZE * width * height;
        let corners = (width + 1) * (height + 1);
        let elev_end = tiles_end + 2 * corners;
        if raw.len() < elev_end {
            return Err(MapError::Truncated {
                len: raw.len(),
                what: "the tile and elevation grids",
            });
        }
        let tiles = raw[TILES_OFFSET..tiles_end]
            .as_chunks::<TILE_SIZE>()
            .0
            .iter()
            .map(Tile::from_bytes)
            .collect();
        let elevation = raw[tiles_end..elev_end]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect();
        Ok(Map {
            name: c_string(&raw[NAME_OFFSET..TITLE_OFFSET]),
            title: c_string(&raw[TITLE_OFFSET..TITLE_END]),
            width,
            height,
            tiles,
            elevation,
        })
    }

    pub fn tile(&self, x: usize, y: usize) -> &Tile {
        &self.tiles[y * self.width + x]
    }

    /// Elevation of the tile corner `(x, y)`, `x <= width`, `y <= height`.
    pub fn corner(&self, x: usize, y: usize) -> i16 {
        self.elevation[y * (self.width + 1) + x]
    }

    /// Fraction of tiles whose ground is water.
    pub fn water_fraction(&self) -> f64 {
        let water = self.tiles.iter().filter(|t| t.is_water()).count();
        water as f64 / self.tiles.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(w: usize, h: usize) -> Vec<u8> {
        let mut raw = vec![0u8; TILES_OFFSET];
        raw[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        raw[4..8].copy_from_slice(b"Test");
        raw[TITLE_OFFSET..TITLE_OFFSET + 5].copy_from_slice(b"Title");
        raw[SIZE_OFFSET..SIZE_OFFSET + 2].copy_from_slice(&(w as u16).to_le_bytes());
        raw[SIZE_OFFSET + 2..SIZE_OFFSET + 4].copy_from_slice(&(h as u16).to_le_bytes());
        for i in 0..w * h {
            let ground = if i == 1 { 0x61 } else { 0x2d };
            raw.extend_from_slice(&[0x40, ground, 0, 3, 0, 0, 0, 0, 0, 1 + i as u8]);
        }
        for i in 0..(w + 1) * (h + 1) {
            raw.extend_from_slice(&(i as i16 - 2).to_le_bytes());
        }
        raw.extend_from_slice(&[0xAA; 17]); // game-state tail is ignored
        raw
    }

    #[test]
    fn parses_synthetic_map() {
        let map = Map::parse(&synthetic(3, 2)).unwrap();
        assert_eq!((map.name.as_str(), map.title.as_str()), ("Test", "Title"));
        assert_eq!((map.width, map.height), (3, 2));
        assert_eq!(map.tiles.len(), 6);
        assert_eq!(map.tile(2, 1).region, 6);
        assert_eq!(map.tile(1, 0).ground, 0x61);
        assert!(map.tile(1, 0).is_water());
        assert_eq!(map.tile(0, 0).trees, 3);
        assert_eq!(map.elevation.len(), 12);
        assert_eq!(map.corner(0, 0), -2);
        assert_eq!(map.corner(3, 2), 9);
        assert!((map.water_fraction() - 1.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_bad_input() {
        let mut raw = synthetic(3, 2);
        assert!(matches!(
            Map::parse(&raw[..100]),
            Err(MapError::Truncated { .. })
        ));
        assert!(matches!(
            Map::parse(&raw[..TILES_OFFSET + 20]),
            Err(MapError::Truncated { .. })
        ));
        raw[SIZE_OFFSET] = 0;
        raw[SIZE_OFFSET + 1] = 0;
        assert!(matches!(Map::parse(&raw), Err(MapError::BadSize { .. })));
        raw[0] = 0;
        assert!(matches!(Map::parse(&raw), Err(MapError::BadMagic(_))));
    }
}
