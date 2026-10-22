//! 16-bit palette banks (`PALT%04d.PAL`), see `docs/formats/lad.md`.
//!
//! ```text
//! u32           n, number of palettes (loader 0x4f8ed0)
//! u8[35]        slot table: per slot the palette index, or 0xff for none
//! n × u16[256]  colours, XRGB1555 (R in bits 10-14, G in 5-9, B in 0-4)
//! ```
//!
//! The game converts the stored 1555 words to the display format (565, 555 or 8-bit LUT) at load
//! time; the files always hold 1555.

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PaletteError {
    #[error("palette bank is truncated: need {needed} bytes, have {len}")]
    Truncated { needed: usize, len: usize },
}

pub const SLOT_COUNT: usize = 35;
pub const COLOURS: usize = 256;
const HEADER: usize = 4 + SLOT_COUNT;

/// Name of the palette bank a sprite record with `palette_id` loads (format string at `0x55e5e0`).
pub fn palette_name(palette_id: u32) -> String {
    format!("PALT{palette_id:04}.PAL")
}

/// Expands an XRGB1555 word to 8-bit RGB.
pub fn rgb555(word: u16) -> [u8; 3] {
    let scale = |v: u16| ((v & 31) * 255 / 31) as u8;
    [scale(word >> 10), scale(word >> 5), scale(word)]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteBank {
    pub slots: [u8; SLOT_COUNT],
    pub palettes: Vec<[u16; COLOURS]>,
}

impl PaletteBank {
    pub fn parse(data: &[u8]) -> Result<Self, PaletteError> {
        let truncated = |needed| PaletteError::Truncated {
            needed,
            len: data.len(),
        };
        let head = data.get(..HEADER).ok_or_else(|| truncated(HEADER))?;
        let n = u32::from_le_bytes(head[..4].try_into().unwrap()) as usize;
        let needed = n
            .checked_mul(COLOURS * 2)
            .and_then(|b| b.checked_add(HEADER))
            .ok_or_else(|| truncated(usize::MAX))?;
        let body = data.get(HEADER..needed).ok_or_else(|| truncated(needed))?;
        let (banks, _) = body.as_chunks::<{ COLOURS * 2 }>();
        let palettes = banks
            .iter()
            .map(|chunk| {
                let (pairs, _) = chunk.as_chunks::<2>();
                std::array::from_fn(|i| u16::from_le_bytes(pairs[i]))
            })
            .collect();
        Ok(Self {
            slots: head[4..].try_into().unwrap(),
            palettes,
        })
    }

    /// RGB triples of palette `index` (None when out of range).
    pub fn rgb(&self, index: usize) -> Option<[[u8; 3]; COLOURS]> {
        let pal = self.palettes.get(index)?;
        Some(pal.map(rgb555))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(n: u32, first: u16) -> Vec<u8> {
        let mut out = n.to_le_bytes().to_vec();
        out.extend([0xff; SLOT_COUNT]);
        for p in 0..n {
            for c in 0..COLOURS as u16 {
                out.extend((if c == 1 { first } else { p as u16 }).to_le_bytes());
            }
        }
        out
    }

    #[test]
    fn parses_bank() {
        let bank = PaletteBank::parse(&build(2, 0x7c00)).unwrap();
        assert_eq!(bank.palettes.len(), 2);
        assert_eq!(bank.slots, [0xff; SLOT_COUNT]);
        assert_eq!(bank.rgb(0).unwrap()[1], [255, 0, 0]);
        assert!(bank.rgb(2).is_none());
    }

    #[test]
    fn expands_555() {
        assert_eq!(rgb555(0x001f), [0, 0, 255]);
        assert_eq!(rgb555(0x03e0), [0, 255, 0]);
        assert_eq!(rgb555(0), [0, 0, 0]);
    }

    #[test]
    fn rejects_truncated() {
        let mut raw = build(1, 0);
        raw.pop();
        assert!(matches!(
            PaletteBank::parse(&raw),
            Err(PaletteError::Truncated { .. })
        ));
        assert!(PaletteBank::parse(&[1, 0]).is_err());
    }

    #[test]
    fn names() {
        assert_eq!(palette_name(7), "PALT0007.PAL");
    }
}
