//! LAD sprite banks (the `.imb` image banks), see `docs/formats/lad.md`.
//!
//! Loader: `0x4f8b80` (type 0 item of `pk2_get_item`). Layout, all little-endian:
//!
//! ```text
//! u32                    record count N (the loader keeps the low 16 bits)
//! N × record:
//!   u32[16]              header: [0] kind, [1] palette id, [2..] mostly 0/1 or 0/-1 pairs
//!   u32                  group count G
//!   G × group:
//!     u32 k, u16 cnt     k = length of the frame map
//!     u8[k]              frame map, only present when k >= 2 (frame f shows image map[f % k])
//!     cnt × image:
//!       u8 flags         bit 1: the image is drawn with the record's palette
//!       u32 0            (unused)
//!       u32 encoding     3 = per-row RLE, 2 = raw w*h bytes
//!       i32 x, y, w, h   bounding box relative to the hotspot
//!       u32 tag          runtime pointer garbage
//!       u32 size, u8[size]
//! ```

use crate::palette::rgb555;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LadError {
    #[error("sprite bank is truncated at offset {at}: need {needed} more bytes")]
    Truncated { at: usize, needed: usize },
    #[error("image has invalid size {width}x{height}")]
    BadSize { width: i32, height: i32 },
    #[error("unknown image encoding {0}")]
    BadEncoding(u32),
    #[error("raw image has {got} bytes, expected {expected}")]
    RawLength { got: usize, expected: usize },
}

pub const ENC_RAW: u32 = 2;
pub const ENC_RLE: u32 = 3;
/// Image flag bit: pixels are palette indices (otherwise unknown/unused).
pub const FLAG_PALETTE: u8 = 2;
const HEADER_WORDS: usize = 16;
/// Largest plausible sprite (the biggest real one is far smaller).
const MAX_PIXELS: usize = 1 << 26;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadImage<'a> {
    pub flags: u8,
    pub encoding: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub data: &'a [u8],
}

/// Decoded 8-bit indices; `opaque[i]` is false for transparent pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedImage {
    pub width: usize,
    pub height: usize,
    pub indices: Vec<u8>,
    pub opaque: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadGroup<'a> {
    /// `k`: length of the frame map as stored (the map itself is only present when `k >= 2`).
    /// For the animation banks this is the number of frames of the sequence (`0x4283e0`).
    pub map_len: u32,
    pub frame_map: Vec<u8>,
    pub images: Vec<LadImage<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadRecord<'a> {
    pub header: [u32; HEADER_WORDS],
    pub groups: Vec<LadGroup<'a>>,
}

impl LadRecord<'_> {
    pub fn kind(&self) -> u32 {
        self.header[0]
    }

    /// Palette bank number: the bank is the PK2 entry `PALT%04d.PAL` (`0x4f8d1a`).
    pub fn palette_id(&self) -> u32 {
        self.header[1]
    }

    /// Whether the loader fetches a palette for this record (flag bit 1 of its first image).
    pub fn uses_palette(&self) -> bool {
        self.groups
            .first()
            .and_then(|g| g.images.first())
            .is_some_and(|i| i.flags & FLAG_PALETTE != 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lad<'a> {
    pub records: Vec<LadRecord<'a>>,
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], LadError> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.data.len());
        let end = end.ok_or(LadError::Truncated {
            at: self.at,
            needed: n.saturating_sub(self.data.len() - self.at.min(self.data.len())),
        })?;
        let out = &self.data[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u32(&mut self) -> Result<u32, LadError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, LadError> {
        Ok(self.u32()? as i32)
    }
    fn u16(&mut self) -> Result<u16, LadError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
}

impl<'a> Lad<'a> {
    /// Parses a whole blob. Trailing bytes after the last record are an error-free no-op
    /// (none occur in the original data); use [`Lad::parse_with_len`] to learn the consumed size.
    pub fn parse(data: &'a [u8]) -> Result<Self, LadError> {
        Self::parse_with_len(data).map(|(lad, _)| lad)
    }

    pub fn parse_with_len(data: &'a [u8]) -> Result<(Self, usize), LadError> {
        let mut c = Cursor { data, at: 0 };
        let count = (c.u32()? & 0xffff) as usize;
        let mut records = Vec::new();
        for _ in 0..count {
            let mut header = [0u32; HEADER_WORDS];
            for w in &mut header {
                *w = c.u32()?;
            }
            let group_count = c.u32()? as usize;
            let mut groups = Vec::new();
            for _ in 0..group_count {
                let k = c.u32()? as usize;
                let cnt = c.u16()? as usize;
                let frame_map = if k >= 2 {
                    c.take(k)?.to_vec()
                } else {
                    Vec::new()
                };
                let mut images = Vec::new();
                for _ in 0..cnt {
                    let flags = c.take(1)?[0];
                    let _zero = c.u32()?;
                    let encoding = c.u32()?;
                    let (x, y, width, height) = (c.i32()?, c.i32()?, c.i32()?, c.i32()?);
                    let _tag = c.u32()?;
                    let size = c.u32()? as usize;
                    images.push(LadImage {
                        flags,
                        encoding,
                        x,
                        y,
                        width,
                        height,
                        data: c.take(size)?,
                    });
                }
                groups.push(LadGroup {
                    map_len: k as u32,
                    frame_map,
                    images,
                });
            }
            records.push(LadRecord { header, groups });
        }
        Ok((Self { records }, c.at))
    }
}

impl LadImage<'_> {
    /// Decodes to palette indices. RLE rows are `[len][tokens]`: `len` counts itself and the
    /// terminating 0 (two bytes, big-endian 15 bits, when bit 7 is set); token `>= 0xc0` skips
    /// `token - 0xc0` pixels, token `1..0xc0` is followed by that many literal indices, `0` ends
    /// the row. Missing or short rows decode as transparent.
    pub fn decode(&self) -> Result<IndexedImage, LadError> {
        let bad = || LadError::BadSize {
            width: self.width,
            height: self.height,
        };
        let (w, h) = (
            usize::try_from(self.width).map_err(|_| bad())?,
            usize::try_from(self.height).map_err(|_| bad())?,
        );
        let n = w
            .checked_mul(h)
            .filter(|&n| n <= MAX_PIXELS)
            .ok_or_else(bad)?;
        match self.encoding {
            ENC_RAW => {
                if self.data.len() != n {
                    return Err(LadError::RawLength {
                        got: self.data.len(),
                        expected: n,
                    });
                }
                Ok(IndexedImage {
                    width: w,
                    height: h,
                    indices: self.data.to_vec(),
                    opaque: vec![true; n],
                })
            }
            ENC_RLE => Ok(decode_rle(self.data, w, h)),
            other => Err(LadError::BadEncoding(other)),
        }
    }
}

fn decode_rle(data: &[u8], w: usize, h: usize) -> IndexedImage {
    let mut indices = vec![0u8; w * h];
    let mut opaque = vec![false; w * h];
    let (mut i, mut y) = (0, 0);
    while i < data.len() && y < h {
        let mut len = data[i] as usize;
        let mut hdr = 1;
        if len & 0x80 != 0 {
            let Some(&lo) = data.get(i + 1) else { break };
            len = ((len & 0x7f) << 8) | lo as usize;
            hdr = 2;
        }
        if len < hdr + 1 {
            // Degenerate row (`01`): empty, consumes just the length byte.
            i += 1;
            y += 1;
            continue;
        }
        let row = &data[(i + hdr).min(data.len())..(i + len).min(data.len())];
        i += len;
        let (mut j, mut x) = (0, 0);
        while j < row.len() {
            let t = row[j];
            j += 1;
            match t {
                0 => break,
                0xc0.. => x += (t - 0xc0) as usize,
                n => {
                    for _ in 0..n {
                        let Some(&v) = row.get(j) else { break };
                        if x < w {
                            indices[y * w + x] = v;
                            opaque[y * w + x] = true;
                        }
                        j += 1;
                        x += 1;
                    }
                }
            }
        }
        y += 1;
    }
    IndexedImage {
        width: w,
        height: h,
        indices,
        opaque,
    }
}

impl IndexedImage {
    /// RGBA8 (row-major) using an XRGB1555 palette; transparent pixels get alpha 0.
    pub fn to_rgba(&self, palette: &[u16; 256]) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.indices.len() * 4);
        for (&i, &op) in self.indices.iter().zip(&self.opaque) {
            let [r, g, b] = rgb555(palette[i as usize]);
            out.extend([r, g, b, if op { 255 } else { 0 }]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(enc: u32, w: i32, h: i32, data: &[u8]) -> Vec<u8> {
        let mut o = vec![0x2a];
        o.extend(0u32.to_le_bytes());
        o.extend(enc.to_le_bytes());
        for v in [0, 0, w, h] {
            o.extend(v.to_le_bytes());
        }
        o.extend(0u32.to_le_bytes());
        o.extend((data.len() as u32).to_le_bytes());
        o.extend(data);
        o
    }

    fn blob(palette: u32, groups: &[(Vec<u8>, Vec<Vec<u8>>)]) -> Vec<u8> {
        let mut o = 1u32.to_le_bytes().to_vec();
        let mut header = [0u32; 16];
        header[0] = 3;
        header[1] = palette;
        for w in header {
            o.extend(w.to_le_bytes());
        }
        o.extend((groups.len() as u32).to_le_bytes());
        for (map, imgs) in groups {
            let k = if map.is_empty() { 1 } else { map.len() as u32 };
            o.extend(k.to_le_bytes());
            o.extend((imgs.len() as u16).to_le_bytes());
            o.extend(map);
            for i in imgs {
                o.extend(i);
            }
        }
        o
    }

    #[test]
    fn parses_and_decodes_rle() {
        // row 0: skip 1, literals 5 6; row 1: empty (02 00); width 4.
        let rle = [5, 0xc1, 2, 5, 6, 0, 0x02, 0];
        let raw = blob(700, &[(vec![], vec![image(3, 4, 2, &rle)])]);
        let (lad, used) = Lad::parse_with_len(&raw).unwrap();
        assert_eq!(used, raw.len());
        let rec = &lad.records[0];
        assert_eq!((rec.kind(), rec.palette_id()), (3, 700));
        assert!(rec.uses_palette());
        let img = rec.groups[0].images[0].decode().unwrap();
        assert_eq!(img.indices[..4], [0, 5, 6, 0]);
        assert_eq!(
            img.opaque,
            [false, true, true, false, false, false, false, false]
        );
        let mut pal = [0u16; 256];
        pal[5] = 0x7c00;
        let rgba = img.to_rgba(&pal);
        assert_eq!(rgba[4..8], [255, 0, 0, 255]);
        assert_eq!(rgba[0..4], [0, 0, 0, 0]);
    }

    #[test]
    fn frame_map_and_raw() {
        let raw = blob(
            1,
            &[(
                vec![1, 0],
                vec![image(2, 2, 1, &[7, 8]), image(2, 1, 1, &[9])],
            )],
        );
        let lad = Lad::parse(&raw).unwrap();
        let g = &lad.records[0].groups[0];
        assert_eq!(g.frame_map, [1, 0]);
        assert_eq!(g.images.len(), 2);
        assert_eq!(g.images[0].decode().unwrap().indices, [7, 8]);
    }

    #[test]
    fn two_byte_row_length() {
        let mut row = vec![0x80, 0, 0, 0];
        // 300-wide row: skip 63*4+... keep simple: one literal at end.
        let mut tokens = vec![0xff, 0xff, 0xff, 0xff, 0xc0 + 39, 1, 42, 0]; // x = 252+39 = 291
        let total = 2 + tokens.len();
        row[0] = 0x80 | (total >> 8) as u8;
        row[1] = total as u8;
        row.truncate(2);
        row.append(&mut tokens);
        let img = LadImage {
            flags: 0x2a,
            encoding: 3,
            x: 0,
            y: 0,
            width: 300,
            height: 1,
            data: &row,
        }
        .decode()
        .unwrap();
        assert_eq!(img.indices[291], 42);
        assert!(img.opaque[291] && !img.opaque[290]);
    }

    #[test]
    fn degenerate_row_is_empty() {
        let img = LadImage {
            flags: 0,
            encoding: 3,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            data: &[1, 0xc0],
        }
        .decode()
        .unwrap();
        assert_eq!(img.opaque, [false]);
    }

    #[test]
    fn errors() {
        let raw = blob(0, &[(vec![], vec![image(2, 2, 2, &[1, 2, 3, 4])])]);
        assert!(matches!(
            Lad::parse(&raw[..raw.len() - 1]),
            Err(LadError::Truncated { .. })
        ));
        let bad = |enc, w, h, d: &'static [u8]| {
            LadImage {
                flags: 0,
                encoding: enc,
                x: 0,
                y: 0,
                width: w,
                height: h,
                data: d,
            }
            .decode()
        };
        assert!(matches!(bad(2, -1, 1, &[]), Err(LadError::BadSize { .. })));
        assert!(matches!(
            bad(2, 2, 2, &[1]),
            Err(LadError::RawLength { .. })
        ));
        assert!(matches!(bad(9, 1, 1, &[1]), Err(LadError::BadEncoding(9))));
    }
}
