//! Bitmap fonts, see `docs/formats/font.md`.
//!
//! There is no `font*.dat` file: `font%s.dat` (`0x55d56c`) is only a *name* whose hash selects one
//! of 10 built-in font definitions (constructor `0x4f24c0`). Each definition points at a LAD
//! sprite bank (`copp6.imb` ...) holding one record per character, record `i` = character
//! `0x20 + i`. Every record has two groups: group 0 is the glyph (an alpha mask drawn in the
//! text colour) and group 1 a one-pixel-offset shadow that darkens what is underneath.
//!
//! Glyph RLE rows share the LAD framing (`[len][tokens]`, see `lad.rs`) but the token set used by
//! the text drawer (`0x4fff00`) is richer:
//!
//! ```text
//! 0x00         end of row
//! 0x01..=0x7f  n palette indices follow (n = token)
//! 0x80..=0x9f  shade run: n = token & 7 (0: next byte), darkening table (token >> 3) & 3
//! 0xa0..=0xbf  alpha run: n = token & 0x1f (0: next byte); n alpha bytes follow,
//!              0 = fully solid, otherwise coverage a/256
//! 0xc0..=0xff  skip token & 0x3f pixels (0xc0 itself ends the image)
//! ```

use crate::lad::{ENC_RAW, ENC_RLE, Lad, LadImage, LadRecord};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FontError {
    #[error("font index {0} is not defined (valid: 1..=10)")]
    BadIndex(u8),
    #[error("sprite bank has {got} glyph records, need at least {need}")]
    TooFewGlyphs { got: usize, need: usize },
    #[error("glyph {glyph} has no image group")]
    MissingGroup { glyph: usize },
    #[error("glyph {glyph}: unsupported image encoding {encoding}")]
    BadEncoding { glyph: usize, encoding: u32 },
}

/// Static definition of one font (fields of the in-memory font object built by `0x4f24c0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontSpec {
    /// Font number N of the name `fontN.dat`.
    pub index: u8,
    /// Sprite bank (`.imb`) with the glyphs.
    pub bank: &'static str,
    /// `+0x08`: height; glyph baseline row = line top + `height - 1`.
    pub height: i32,
    /// `+0x0c`: extra pixels added to every advance.
    pub spacing: i32,
    /// `+0x10`: gap added to `height` for the line pitch.
    pub leading: i32,
    /// `+0x18`: underline offset below the baseline row (`+0x1c`: thickness).
    pub underline_offset: i32,
    pub underline_thickness: i32,
    /// `+0x30`: fold lower case and accented letters to plain capitals (display fonts).
    pub fold_to_capitals: bool,
    /// Font 7 only: advances come from [`SCRIPT_WIDTHS`] instead of the glyph boxes.
    pub width_table: bool,
}

const fn spec(
    index: u8,
    bank: &'static str,
    height: i32,
    spacing: i32,
    leading: i32,
    underline: (i32, i32),
) -> FontSpec {
    FontSpec {
        index,
        bank,
        height,
        spacing,
        leading,
        underline_offset: underline.0,
        underline_thickness: underline.1,
        fold_to_capitals: false,
        width_table: false,
    }
}

/// The ten fonts (`font1.dat` .. `font10.dat`), from the `switch` at `0x4f24c0`.
pub const FONTS: [FontSpec; 10] = [
    spec(1, "copp6.imb", 6, 1, 4, (1, 1)),
    spec(2, "copp8.imb", 8, 2, 5, (1, 1)),
    spec(3, "copp14.imb", 14, 3, 8, (2, 2)),
    spec(4, "copp24.imb", 24, 4, 11, (2, 3)),
    spec(5, "copp10.imb", 10, 2, 6, (1, 1)),
    spec(6, "copp12.imb", 12, 2, 7, (1, 1)),
    FontSpec {
        width_table: true,
        ..spec(7, "scri11.imb", 11, 1, 8, (1, 1))
    },
    spec(8, "fran13.imb", 13, 2, 8, (1, 1)),
    FontSpec {
        fold_to_capitals: true,
        ..spec(9, "haet46.imb", 46, 3, 15, (3, 4))
    },
    FontSpec {
        fold_to_capitals: true,
        ..spec(10, "htra46.imb", 46, 3, 15, (3, 4))
    },
];

/// Pseudo file name the game hashes to pick font `index`.
pub fn font_file_name(index: u8) -> String {
    format!("font{index}.dat")
}

/// Definition of font `index` (1..=10).
pub fn font_spec(index: u8) -> Result<&'static FontSpec, FontError> {
    FONTS
        .iter()
        .find(|f| f.index == index)
        .ok_or(FontError::BadIndex(index))
}

/// Advances of `scri11.imb` (font 7), indexed by `ch - 0x20` (table at `0x55d358`, signed bytes).
pub const SCRIPT_WIDTHS: [i8; 96] = [
    0, 3, 4, 9, 6, 10, 11, 2, 2, 7, 6, 8, 3, 5, 3, 7, 8, 4, 7, 6, 7, 7, 7, 5, 7, 7, 4, 4, 8, 9, 8,
    7, 12, 9, 11, 9, 13, 11, 9, 10, 11, 6, 8, 9, 10, 15, 12, 10, 11, 10, 10, 9, 9, 13, 11, 16, 8,
    9, 10, 4, 5, 6, 6, 10, 2, 8, 7, 7, 8, 6, 6, 8, 7, 4, 4, 8, 4, 10, 8, 7, 8, 7, 6, 7, 5, 8, 9,
    13, 9, 7, 8, 4, 2, 5, 9, 1,
];

/// Text colours selectable by the markup (table at `0x55d438`, XRGB1555).
pub const TEXT_COLOURS: [u16; 14] = [
    0x7fff, 0x7ff0, 0x6318, 0x0001, 0x7ff0, 0x6000, 0x7e0c, 0x39c4, 0x4a46, 0x03e0, 0x7fe0, 0x7c00,
    0x14a2, 0x2d84,
];

/// One decoded glyph pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Empty,
    /// Coverage 1..=255 of the text colour (255 stands for the stored 0 = fully solid).
    Alpha(u8),
    /// Darken what is below with table 0..=3.
    Shade(u8),
    /// Palette index (not used by the shipped fonts' alpha layers).
    Index(u8),
}

/// A decoded glyph layer: box relative to the pen on the baseline row, row-major cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
}

/// Decodes a glyph image with the text drawer's token set (module docs).
pub fn decode_text_image(image: &LadImage) -> Option<Layer> {
    let (w, h) = (
        usize::try_from(image.width).ok()?,
        usize::try_from(image.height).ok()?,
    );
    if w.checked_mul(h)? > 1 << 24 {
        return None;
    }
    let mut cells = vec![Cell::Empty; w * h];
    match image.encoding {
        ENC_RAW if image.data.len() == w * h => {
            for (c, &v) in cells.iter_mut().zip(image.data) {
                *c = Cell::Index(v);
            }
        }
        ENC_RLE => decode_rows(image.data, w, h, &mut cells),
        _ => return None,
    }
    Some(Layer {
        x: image.x,
        y: image.y,
        width: w,
        height: h,
        cells,
    })
}

fn decode_rows(data: &[u8], w: usize, h: usize, cells: &mut [Cell]) {
    let mut at = 0;
    for y in 0..h {
        let Some(&b0) = data.get(at) else { return };
        let (len, hdr) = if b0 & 0x80 != 0 {
            let Some(&lo) = data.get(at + 1) else { return };
            ((usize::from(b0 & 0x7f) << 8) | usize::from(lo), 2)
        } else {
            (usize::from(b0), 1)
        };
        let row = data.get((at + hdr).min(data.len())..(at + len.max(hdr)).min(data.len()));
        at += len.max(1);
        let Some(row) = row else { continue };
        let (mut j, mut x) = (0, 0usize);
        let mut put = |x: usize, c: Cell| {
            if x < w {
                cells[y * w + x] = c;
            }
        };
        'row: while j < row.len() {
            let t = row[j];
            j += 1;
            match t {
                0 | 0xc0 => break 'row,
                0xc1.. => x += usize::from(t & 0x3f),
                0x01..=0x7f => {
                    for _ in 0..t {
                        let Some(&v) = row.get(j) else { break 'row };
                        put(x, Cell::Index(v));
                        j += 1;
                        x += 1;
                    }
                }
                0x80..=0xbf => {
                    let mut n = usize::from(t & if t < 0xa0 { 7 } else { 0x1f });
                    if n == 0 {
                        let Some(&c) = row.get(j) else { break 'row };
                        n = usize::from(c);
                        j += 1;
                    }
                    for _ in 0..n {
                        if t < 0xa0 {
                            put(x, Cell::Shade((t >> 3) & 3));
                        } else {
                            let Some(&a) = row.get(j) else { break 'row };
                            put(x, Cell::Alpha(if a == 0 { 255 } else { a }));
                            j += 1;
                        }
                        x += 1;
                    }
                }
            }
        }
    }
}

/// One character: the glyph layer (group 0) and optional shadow layer (group 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub main: Layer,
    pub shadow: Option<Layer>,
    /// Pen advance in pixels, spacing included.
    pub advance: i32,
}

#[derive(Debug, Clone)]
pub struct Font {
    pub spec: FontSpec,
    /// Glyphs for characters `0x20 ..`.
    pub glyphs: Vec<Glyph>,
}

fn first_image<'a>(record: &'a LadRecord, group: usize) -> Option<&'a LadImage<'a>> {
    record.groups.get(group)?.images.first()
}

impl Font {
    /// Builds the font from its sprite bank (`spec.bank`).
    pub fn from_lad(spec: &FontSpec, lad: &Lad) -> Result<Self, FontError> {
        let mut glyphs = Vec::with_capacity(lad.records.len());
        for (i, record) in lad.records.iter().enumerate() {
            let img = first_image(record, 0).ok_or(FontError::MissingGroup { glyph: i })?;
            let main = decode_text_image(img).ok_or(FontError::BadEncoding {
                glyph: i,
                encoding: img.encoding,
            })?;
            let shadow = first_image(record, 1).and_then(decode_text_image);
            // 0x4f2970: box width plus the negative part of its x, plus the font spacing.
            let advance = main.width as i32 + main.x.min(0) + spec.spacing;
            glyphs.push(Glyph {
                main,
                shadow,
                advance,
            });
        }
        if glyphs.len() < 0x50 {
            return Err(FontError::TooFewGlyphs {
                got: glyphs.len(),
                need: 0x50,
            });
        }
        Ok(Self {
            spec: *spec,
            glyphs,
        })
    }

    /// Glyph index of byte `ch` (`0x4f1f60`): optional folding, then `ch - 0x20`, 0 if outside.
    pub fn glyph_index(&self, ch: u8) -> usize {
        let ch = if self.spec.fold_to_capitals {
            fold_to_capital(ch)
        } else {
            ch
        };
        let i = usize::from(ch).wrapping_sub(0x20);
        if i < self.glyphs.len() { i } else { 0 }
    }

    /// Pen advance of `ch` in pixels (`0x4f2970`). Markup characters and controls are 0 wide,
    /// a space is as wide as `1`.
    pub fn advance(&self, ch: u8) -> i32 {
        if matches!(ch, b'[' | b']' | b'{' | b'}' | b'|' | 0x1e) || ch < 0x20 && ch != b' ' {
            return 0;
        }
        let ch = if ch == b' ' { b'1' } else { ch };
        let i = self.glyph_index(ch);
        if self.spec.width_table {
            i32::from(SCRIPT_WIDTHS.get(i).copied().unwrap_or(0)) + self.spec.spacing
        } else {
            self.glyphs[i].advance
        }
    }

    /// Width in pixels of a single line of text.
    pub fn text_width(&self, text: &str) -> i32 {
        text.bytes().map(|c| self.advance(c)).sum()
    }

    /// Distance between two lines of text.
    pub fn line_pitch(&self) -> i32 {
        self.spec.height + self.spec.leading
    }
}

/// Folding applied by display fonts (`0x4f1f60`): accents removed, letters capitalised.
pub fn fold_to_capital(ch: u8) -> u8 {
    match ch {
        0xd9..=0xdc | 0xf9..=0xfc => b'U',
        0xd2..=0xd6 | 0xf2..=0xf6 => b'O',
        0xcc..=0xcf | 0xec..=0xef => b'I',
        0xc8..=0xcb | 0xe8..=0xeb => b'E',
        0xc0..=0xc5 | 0xe0..=0xe5 => b'A',
        0xdd | 0xfd | 0xff => b'Y',
        0xd1 | 0xf1 => b'N',
        0xc7 | 0xe7 => b'C',
        b'a'..=b'z' => ch - 0x20,
        _ => ch,
    }
}

/// An RGBA8 image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

/// Shade tables of `0x470b50` on a 5-bit channel: 0 and 1 darken (`max(c - 8, c / 2)`,
/// `max(c - 4, c / 2)`), 2 and 3 pull towards grey (`(0x78 + 3c) / 8`, `(0x5a + 3c) / 8`).
fn shade_channel(level: u8, c: u8) -> u8 {
    let c = i32::from(c);
    let v = match level & 3 {
        0 => (c - 8).max(c >> 1),
        1 => (c - 4).max(c >> 1),
        2 => (0x78 + c * 3) >> 3,
        _ => (0x5a + c * 3) >> 3,
    };
    v.clamp(0, 31) as u8
}

impl Font {
    /// Renders one line (`\n` starts a new one) on `background` (`None` = transparent, in which
    /// case the shadow layer is a translucent black). `shadow` enables the second glyph layer.
    pub fn render(
        &self,
        text: &str,
        colour: [u8; 3],
        background: Option<[u8; 3]>,
        shadow: bool,
    ) -> Rgba {
        let pad = 2i32;
        let lines: Vec<&str> = text.split('\n').collect();
        let width = lines.iter().map(|l| self.text_width(l)).max().unwrap_or(0) + 2 * pad;
        let height = (lines.len() as i32 - 1) * self.line_pitch() + self.spec.height + 2 * pad + 2;
        let (w, h) = (width.max(1) as usize, height.max(1) as usize);
        let mut px = vec![0u8; w * h * 4];
        if let Some([r, g, b]) = background {
            for p in px.as_chunks_mut::<4>().0 {
                p.copy_from_slice(&[r, g, b, 255]);
            }
        }
        for (n, line) in lines.iter().enumerate() {
            let base = pad + n as i32 * self.line_pitch() + self.spec.height - 1;
            let mut pen = pad;
            for ch in line.bytes() {
                if self.advance(ch) != 0 || ch == b' ' {
                    let g = &self.glyphs[self.glyph_index(if ch == b' ' { b' ' } else { ch })];
                    blit(&mut px, w, h, &g.main, pen, base, colour);
                    if let (true, Some(s)) = (shadow, &g.shadow) {
                        blit(&mut px, w, h, s, pen, base, colour);
                    }
                }
                pen += self.advance(ch);
            }
        }
        Rgba {
            width: w,
            height: h,
            pixels: px,
        }
    }
}

fn blit(px: &mut [u8], w: usize, h: usize, layer: &Layer, pen: i32, base: i32, colour: [u8; 3]) {
    for ly in 0..layer.height {
        for lx in 0..layer.width {
            let (x, y) = (pen + layer.x + lx as i32, base + layer.y + ly as i32);
            if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                continue;
            }
            let p = &mut px[(y as usize * w + x as usize) * 4..][..4];
            match layer.cells[ly * layer.width + lx] {
                Cell::Empty | Cell::Index(_) => {}
                Cell::Alpha(a) => {
                    // 0x4fff00: dst = (a * colour + (255 - a) * dst) >> 8
                    let a = u32::from(a);
                    for c in 0..3 {
                        p[c] =
                            ((a * u32::from(colour[c]) + (255 - a) * u32::from(p[c])) >> 8) as u8;
                    }
                    p[3] = (a + (255 - a) * u32::from(p[3]) / 255).min(255) as u8;
                }
                Cell::Shade(level) => {
                    if p[3] == 0 {
                        p.copy_from_slice(&[0, 0, 0, 128]);
                    } else {
                        for c in p.iter_mut().take(3) {
                            *c = shade_channel(level, *c >> 3) << 3;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: i32, h: i32, data: &[u8]) -> LadImage<'_> {
        LadImage {
            flags: 0x2a,
            encoding: ENC_RLE,
            x: 0,
            y: -(h - 1),
            width: w,
            height: h,
            data,
        }
    }

    #[test]
    fn decodes_alpha_shade_and_skip_tokens() {
        // Row 1: skip 2, 3 alpha pixels (0 = solid); row 2: shade 1 pixel (table 0), skip 0 ends.
        let data = [0x07, 0xc2, 0xa3, 0x00, 0x40, 0x03, 0x00, 0x03, 0x81, 0x00];
        let img = decode_text_image(&image(5, 2, &data)).unwrap();
        assert_eq!(img.cells[2], Cell::Alpha(255));
        assert_eq!(img.cells[3], Cell::Alpha(0x40));
        assert_eq!(img.cells[4], Cell::Alpha(3));
        assert_eq!(img.cells[5], Cell::Shade(0));
        assert_eq!(img.cells[0], Cell::Empty);
    }

    #[test]
    fn long_runs_use_the_following_count_byte() {
        let mut data = vec![0u8, 0xa0, 40];
        data.extend([0u8; 40]);
        data.push(0);
        data[0] = data.len() as u8;
        let img = decode_text_image(&image(40, 1, &data)).unwrap();
        assert!(img.cells.iter().all(|&c| c == Cell::Alpha(255)));
    }

    #[test]
    fn font_table_and_names() {
        assert_eq!(font_file_name(3), "font3.dat");
        assert_eq!(font_spec(7).unwrap().bank, "scri11.imb");
        assert_eq!(font_spec(0), Err(FontError::BadIndex(0)));
        assert_eq!(fold_to_capital(b'q'), b'Q');
        assert_eq!(fold_to_capital(0xe9), b'E');
    }
}
