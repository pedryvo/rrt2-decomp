//! Track geometry inside a tile: segment lengths and the position of a point along a segment
//! (trains.md 10.2).

use super::mem::Ts;
use crate::data::DataImage;
use crate::fp::ftol;

/// `dir_dx` / `dir_dy` (`0x537b48` / `0x537b68`).
pub const DX: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
pub const DY: [i32; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

impl Ts<'_> {
    pub fn seg_len(&self, din: u32, dout: u32, kind: u32) -> i32 {
        segment_length(self.img, din, dout, kind)
    }

    pub fn track_point(&self, din: u32, dout: u32, pos: i32, kind: u32) -> (i32, i32, u32) {
        track_point(self.img, din, dout, pos, kind)
    }
}

/// `curve_side_table[d_out + d_in*8]` (`0x55c3d8`, i32[64]).
fn curve_side(img: &DataImage, din: u32, dout: u32) -> i32 {
    img.i32(0x0055_c3d8 + 4 * ((dout + din * 8) & 63))
}

mod free {
    use super::*;

    /// `track_segment_length` (`0x4ca4e0`): length of the segment from edge `din` to edge `dout`
    /// with curve kind `kind` (the `+0x25` of a car / bits 1-2 of a route step): straight 64
    /// (orthogonal) or 90 (diagonal), curves 80 (kind 0), 69 or 91.
    pub fn segment_length(img: &DataImage, din: u32, dout: u32, kind: u32) -> i32 {
        if din.wrapping_sub(dout) & 7 == 4 {
            return if din & 1 != 0 { 0x5a } else { 0x40 };
        }
        match kind {
            0 => 0x50,
            1 => {
                if curve_side(img, din, dout) != 0 {
                    0x5b
                } else {
                    0x45
                }
            }
            _ => {
                if curve_side(img, din, dout) != 1 {
                    0x5b
                } else {
                    0x45
                }
            }
        }
    }

    /// `track_point` (`0x4ca550`): the sub-tile position `(x, y)` (0..63 inside the tile, may
    /// overflow into the neighbours) and the facing direction of the point at distance `pos`
    /// along the segment `din -> dout` of curve kind `kind`.
    pub fn track_point(
        img: &DataImage,
        din: u32,
        dout: u32,
        pos: i32,
        kind: u32,
    ) -> (i32, i32, u32) {
        let fx = |d: u32| f64::from(img.f32(0x0053_6ac8 + 4 * (d & 7)));
        let fy = |d: u32| f64::from(img.f32(0x0053_6ae8 + 4 * (d & 7)));
        if din.wrapping_sub(dout) & 7 == 4 {
            let k = din * 3 + kind;
            let x = ftol(f64::from(pos) * fx(din) + f64::from(img.i32(0x0053_6a08 + 4 * k)));
            let y = ftol(f64::from(pos) * fy(din) + f64::from(img.i32(0x0053_6a68 + 4 * k)));
            return (x, y, dout);
        }
        let (len, m, h, side) = if kind == 0 {
            (0x50, 0x40, 0x1e, 0)
        } else {
            let c = curve_side(img, din, dout);
            let side = if kind == 1 { c } else { 1 - c };
            if side == 0 {
                (0x45, 0x38, 0x19, side)
            } else {
                (0x5b, 0x49, 0x23, side)
            }
        };
        let (mut ox, mut oy) = (0, 0);
        let mut c = pos;
        if pos > len - 1 {
            let over = f64::from(pos - (len - 1));
            let d = dout.wrapping_sub(4) & 7;
            ox = ftol(over * fx(d));
            oy = ftol(over * fy(d));
            c = len - 1;
        } else if pos < 0 {
            ox = ftol(f64::from(pos) * fx(din));
            oy = ftol(f64::from(pos) * fy(din));
            c = 0;
        }
        let u = ftol(f64::from(m * c / len) + 0.5);
        let facing = if din & 1 == 0 {
            if u <= h {
                din.wrapping_sub(4) & 7
            } else {
                dout
            }
        } else if h < m - u {
            din.wrapping_sub(4) & 7
        } else {
            dout
        };
        let (b2, b3) = if kind == 0 {
            (
                img.u8(0x0053_6b8c + u as u32),
                img.u8((0x0053_6bcb - u) as u32),
            )
        } else if side == 0 {
            (
                img.u8(0x0053_6b08 + u as u32),
                img.u8((0x0053_6b3f - u) as u32),
            )
        } else {
            (
                img.u8(0x0053_6b40 + u as u32),
                img.u8((0x0053_6b88 - u) as u32),
            )
        };
        let (v7, v5) = (i32::from(b2), i32::from(b3));
        let i1 = m - 1;
        let (x, y) = match din {
            0 => (if dout == 3 { v7 } else { 0x3f - v7 }, u),
            1 => {
                if dout == 6 {
                    (i1 - u, 0x3f - v5)
                } else {
                    (v5, u - i1 + 0x3f)
                }
            }
            2 => (0x3f - u, if dout == 5 { v7 } else { 0x3f - v7 }),
            3 => {
                if dout == 6 {
                    (i1 - u, v5)
                } else {
                    (v5, i1 - u)
                }
            }
            4 => (if dout == 1 { v7 } else { 0x3f - v7 }, 0x3f - u),
            5 => {
                if dout == 2 {
                    (u - i1 + 0x3f, v5)
                } else {
                    (0x3f - v5, i1 - u)
                }
            }
            6 => (u, if dout == 3 { v7 } else { 0x3f - v7 }),
            7 => {
                if dout == 2 {
                    (u - i1 + 0x3f, 0x3f - v5)
                } else {
                    (0x3f - v5, u - i1 + 0x3f)
                }
            }
            // The original leaves the outputs untouched (garbage) for d_in > 7.
            _ => (0, 0),
        };
        (x + ox, y + oy, facing)
    }
}

pub use free::{segment_length, track_point};
