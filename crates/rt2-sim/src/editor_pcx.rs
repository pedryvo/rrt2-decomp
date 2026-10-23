//! New Map > From Image: the height map of a PCX (docs/formats/map-gam.md section 10.4).
//!
//! The dialog `0x4694f0` asks for the file (default `EXAMPLE.PCX`) and a "height scale" (1..2000,
//! default 100); the reader is `0x469bc0` ([`Pcx::parse`]) and the application of the image
//! to the new map is `0x469670` ([`import`]): every corner height is the bilinear sample of the
//! image's lightness, the whole map is then painted with terrain class 6 (`0x42a3e0`).

use rt2_formats::map::Save;

use crate::editor::{paint_rect, redraw};
use crate::rng::GameRng;

/// Terrain class the import paints over the whole map (`0x469b46`: `0x42a3e0(0, 0, w, h, 6, ..)`).
pub const IMPORT_CLASS: usize = 6;
/// Height scale limits and default of the dialog (`0x4694f0`).
pub const SCALE_MIN: i32 = 1;
pub const SCALE_MAX: i32 = 2000;
pub const SCALE_DEFAULT: i32 = 100;

#[derive(Debug, PartialEq, Eq)]
pub enum PcxError {
    /// Shorter than the 128-byte header.
    Truncated,
    /// A one-plane image without the `0x0C` palette marker ("Error reading PCX file").
    NoPalette,
    /// More than the planes the reader handles or an empty image.
    Unsupported,
}

/// An image as `0x469bc0` keeps it: one buffer per plane of `stride * rows` bytes.
#[derive(Debug, Clone)]
pub struct Pcx {
    /// Bytes per scanline, `(xmax + 2) & ~1` (computed from the header's `xmax`, not from its
    /// BytesPerLine field).
    pub stride: usize,
    pub rows: usize,
    pub planes: Vec<Vec<u8>>,
    /// 768 bytes (RGB) for a one-plane image.
    pub palette: Option<Vec<u8>>,
}

impl Pcx {
    /// `0x469bc0`: RLE decode of every scanline (all planes of a row follow each other), then,
    /// for one plane, the `0x0C` marker and the 256-entry palette.
    pub fn parse(raw: &[u8]) -> Result<Pcx, PcxError> {
        if raw.len() < 0x80 {
            return Err(PcxError::Truncated);
        }
        let xmax = usize::from(u16::from_le_bytes([raw[8], raw[9]]));
        let ymax = usize::from(u16::from_le_bytes([raw[10], raw[11]]));
        let nplanes = usize::from(raw[0x41]);
        if nplanes == 0 || nplanes > 4 {
            return Err(PcxError::Unsupported);
        }
        let stride = (xmax + 2) & 0xfffe;
        let rows = ymax + 1;
        let line = nplanes * stride;
        let mut planes = vec![vec![0u8; stride * rows]; nplanes];
        let mut pos = 0x80;
        let at = |p: usize| raw.get(p).copied().unwrap_or(0);
        for row in 0..rows {
            let mut buf: Vec<u8> = Vec::with_capacity(line + 64);
            while buf.len() < line {
                let b = at(pos);
                if b & 0xc0 == 0xc0 {
                    let n = usize::from(b & 0x3f);
                    let v = at(pos + 1);
                    buf.extend(std::iter::repeat_n(v, n));
                    pos += 2;
                } else {
                    buf.push(b);
                    pos += 1;
                }
                if pos > raw.len() + 2 {
                    break;
                }
            }
            buf.resize(line, 0);
            for (k, plane) in planes.iter_mut().enumerate() {
                plane[row * stride..(row + 1) * stride]
                    .copy_from_slice(&buf[k * stride..(k + 1) * stride]);
            }
        }
        let palette = if nplanes == 1 {
            if at(pos) != 0x0c || raw.len() < pos + 1 + 0x300 {
                return Err(PcxError::NoPalette);
            }
            Some(raw[pos + 1..pos + 1 + 0x300].to_vec())
        } else {
            None
        };
        Ok(Pcx {
            stride,
            rows,
            planes,
            palette,
        })
    }

    /// Lightness of the one-plane pixel at `(x, y)`: `(R + G + B) / 3` of its palette entry.
    fn gray(&self, x: usize, y: usize) -> f64 {
        let idx = usize::from(self.planes[0][y * self.stride + x]);
        let p = self
            .palette
            .as_ref()
            .expect("one-plane images have a palette");
        let sum = u32::from(p[idx * 3]) + u32::from(p[idx * 3 + 1]) + u32::from(p[idx * 3 + 2]);
        f64::from(sum) * (1.0 / 3.0)
    }

    /// Byte of plane `k` at `(x, y)`.
    fn byte(&self, k: usize, x: usize, y: usize) -> f64 {
        f64::from(self.planes[k][y * self.stride + x])
    }

    fn sample(&self, k: usize, x: usize, y: usize) -> f64 {
        if self.planes.len() == 1 {
            self.gray(x, y)
        } else {
            self.byte(k, x, y)
        }
    }
}

/// `(float)n * (float)0.01` of the dialog: the height scale of the import.
pub fn scale_factor(n: i32) -> f32 {
    (f64::from(n.clamp(SCALE_MIN, SCALE_MAX)) * f64::from(0.01f32)) as f32
}

/// The height of every corner `(w + 1) x (h + 1)` of a `w x h` map (`0x469670`, row by row).
///
/// A corner `(x, y)` samples the image at `(x * stride / (w + 1), y * rows / (h + 1))` (clamped to
/// the last column and row) by bilinear interpolation (a weight above 0.99 counts as 1); the
/// planes are summed, then `h = (sum - 1) * scale / planes` is truncated and clamped to
/// `-255..=510` (`0x4260c0`).
pub fn heights(img: &Pcx, w: usize, h: usize, scale_percent: i32) -> Vec<i16> {
    let scale = f64::from(scale_factor(scale_percent));
    let planes = img.planes.len();
    let (cw, ch) = (w + 1, h + 1);
    let f1 = (img.stride as f64 / cw as f64) as f32;
    let f2 = (img.rows as f64 / ch as f64) as f32;
    let xlim = (img.stride - 1) as f32;
    let ylim = (img.rows - 1) as f32;
    let mut out = Vec::with_capacity(cw * ch);
    for y in 0..ch {
        let ypos = (y as f64 * f64::from(f2)) as f32;
        for x in 0..cw {
            let mut xpos = x as f64 * f64::from(f1);
            if xpos >= f64::from(xlim) {
                xpos = f64::from(xlim);
            }
            let ypos = if ypos >= ylim { ylim } else { ypos };
            let (ix, iy) = (xpos as usize, ypos as usize);
            let mut a = (1.0 - (f64::from(ypos) - iy as f64)) as f32;
            let mut b = (1.0 - (xpos - ix as f64)) as f32;
            if f64::from(a) > 0.99 {
                a = 1.0;
            }
            if f64::from(b) > 0.99 {
                b = 1.0;
            }
            let (a, b) = (f64::from(a), f64::from(b));
            let mut acc = 0.0f64;
            for k in 0..planes {
                acc += img.sample(k, ix, iy) * (b * a);
                if a < 1.0 {
                    acc += img.sample(k, ix, iy + 1) * (1.0 - a) * b;
                }
                if b < 1.0 {
                    acc += img.sample(k, ix + 1, iy) * (1.0 - b) * a;
                }
                if a < 1.0 && b < 1.0 {
                    acc += img.sample(k, ix + 1, iy + 1) * (1.0 - b) * (1.0 - a);
                }
            }
            let v = (acc - 1.0) * scale / planes as f64;
            out.push((v.trunc() as i32).clamp(-255, 510) as i16);
        }
    }
    out
}

/// `0x469670` on a new map: sets every corner height from the image, then paints the whole map
/// with terrain class 6 (ids already in the class are kept). Returns the redraw rectangle.
pub fn import(s: &mut Save, rng: &mut GameRng, img: &Pcx, scale_percent: i32) -> [i32; 4] {
    let (w, h) = (s.map.width, s.map.height);
    s.map.elevation = heights(img, w, h, scale_percent);
    let r = [0, 0, w as i32 - 1, h as i32 - 1];
    redraw(s, r);
    paint_rect(s, rng, 0, 0, w as i32, h as i32, IMPORT_CLASS);
    r
}

impl crate::world::World {
    /// From Image on the world's map (`0x469670`): the corner heights from the image, the map
    /// painted with terrain class 6 and the ground blends baked again.
    pub fn editor_import_image(&mut self, img: &Pcx, scale_percent: i32) {
        let Some(sim) = self.sim.as_mut() else {
            return;
        };
        let r = import(&mut sim.save, &mut self.rng, img, scale_percent);
        sim.dirty.add(r[0], r[1], r[2], r[3]);
        let (w, h) = (r[2] + 1, r[3] + 1);
        self.editor_blend([-1, -1, w + 2, h + 2]);
    }
}
