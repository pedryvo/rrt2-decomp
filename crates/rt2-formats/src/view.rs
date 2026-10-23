//! The 0x40-byte camera/view block of `.MAP`/`.MP2`/`.GAM` files.
//!
//! The saver `0x477540` copies seven dwords from the main map window (`FUN_00502f60(0xfd)`,
//! fields at `window + 0x7b ..`) into the globals `0x577b98..0x577bd8`, adds the map size and two
//! UI globals and writes the 16 dwords after the state block. The loader `0x477a60` reads them
//! back and re-applies them to the window: `0x40ee50` (rotation), `0x40f160` (zoom), `0x40f3d0`
//! (scroll), then `0x467bd0` / `0x467fd0`. See `docs/formats/map-gam.md` section 1.2.

use crate::map::Save;

/// Size of the block in bytes.
pub const VIEW_LEN: usize = 0x40;
/// Smallest and largest zoom level (`0x40181b` accepts `1 < zoom < 8`).
pub const ZOOM_MIN: i32 = 2;
pub const ZOOM_MAX: i32 = 7;

/// The decoded block. Field names follow the window struct (`view = window + 0x7b`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    /// dword 0 (`0x577b98`, `view + 0`): copied back verbatim; 0 in every shipped file.
    pub head: i32,
    /// dword 1 (`view + 4`): zoom level 2..=7. One map tile is `2^zoom` screen pixels wide.
    pub zoom: i32,
    /// dwords 2, 3 (`view + 8`, `view + 0xc`): scroll of the top-left corner of the viewport in
    /// screen pixels at that zoom and rotation, origin at the rotated map's (0, 0) corner.
    pub scroll_x: i32,
    pub scroll_y: i32,
    /// dwords 4, 5 (`view + 0x10`, `+ 0x14`): viewport size in pixels (1024 x 508 in all files).
    /// The loader does not restore them: the window keeps its own size.
    pub width: i32,
    pub height: i32,
    /// dword 6 (`view + 0x18`): view rotation 0..=3 (quarter turns).
    pub rotation: i32,
    /// dwords 7, 8: map width and height in tiles (`0x55e7a4`, `0x55e7a0`). Fed to the map-size
    /// setup `0x472980`, which builds the per-zoom projection tables.
    pub map_width: i32,
    pub map_height: i32,
    /// dword 9 (`0x577bbc`): the local person (`0x566ca4`, a slot of the person pool), applied by
    /// `0x467bd0`; 1 (the first person) in all files but three that have 0.
    pub person: i32,
    /// dword 10 (`0x577bc0`): the local company (`0x574414`, 0 = none), applied by `0x467fd0`.
    pub company: i32,
    /// dwords 11..16: not used by the loader or saver (zero in every shipped file).
    pub tail: [i32; 5],
}

impl View {
    pub fn parse(b: &[u8]) -> Option<View> {
        if b.len() < VIEW_LEN {
            return None;
        }
        let d = |i: usize| i32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
        Some(View {
            head: d(0),
            zoom: d(1),
            scroll_x: d(2),
            scroll_y: d(3),
            width: d(4),
            height: d(5),
            rotation: d(6),
            map_width: d(7),
            map_height: d(8),
            person: d(9),
            company: d(10),
            tail: [d(11), d(12), d(13), d(14), d(15)],
        })
    }

    pub fn to_bytes(&self) -> [u8; VIEW_LEN] {
        let v = [
            self.head,
            self.zoom,
            self.scroll_x,
            self.scroll_y,
            self.width,
            self.height,
            self.rotation,
            self.map_width,
            self.map_height,
            self.person,
            self.company,
            self.tail[0],
            self.tail[1],
            self.tail[2],
            self.tail[3],
            self.tail[4],
        ];
        let mut out = [0u8; VIEW_LEN];
        for (i, x) in v.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&x.to_le_bytes());
        }
        out
    }
}

/// Viewport size the original saves (1024 x 508, the 1024x768 screen minus the bottom panel).
pub const SAVED_VIEWPORT: (i32, i32) = (1024, 508);

impl View {
    /// True when the loader would apply the zoom: `0x40181b` ignores levels outside `2..=7`.
    pub fn zoom_valid(&self) -> bool {
        (ZOOM_MIN..=ZOOM_MAX).contains(&self.zoom)
    }

    /// Screen scale of the zoom level relative to the native art (level 7: 128 px per tile).
    pub fn native_scale(zoom: i32) -> f32 {
        2f32.powi(zoom - ZOOM_MAX)
    }

    /// Tile (continuous corner coordinates, `0.0..=map size`) at the centre of the saved
    /// viewport. The screen of a rotated map is `sx = (x' - y') * 2^(zoom-1)`,
    /// `sy = (x' + y') * 2^(zoom-2)`, with `(x', y')` the tile coordinates in the rotated frame
    /// (`0x472980` builds the per-zoom/rotation extents from the same shifts). Rotation 1 has the
    /// map corner `(w, 0)` at the top: `x' = y`, `y' = w - x`. Verified against the places of the
    /// shipped files (the centre is within a few tiles of a city, `tests/view.rs`).
    pub fn center_tile(&self, map_w: usize, map_h: usize) -> (f32, f32) {
        let z = self.zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        let (w, h) = (map_w as f32, map_h as f32);
        let sx = (self.scroll_x + self.width / 2) as f32 / 2f32.powi(z - 1);
        let sy = (self.scroll_y + self.height / 2) as f32 / 2f32.powi(z - 2);
        let (xp, yp) = ((sy + sx) / 2.0, (sy - sx) / 2.0);
        match self.rotation.rem_euclid(4) {
            0 => (xp, yp),
            1 => (w - yp, xp),
            2 => (w - xp, h - yp),
            _ => (yp, h - xp),
        }
    }

    /// Writes the scroll so that tile `(x, y)` is at the centre of the saved viewport at the
    /// given zoom and rotation (the inverse of [`View::center_tile`]); `width`/`height` become
    /// [`SAVED_VIEWPORT`] and the map size is set.
    pub fn set_center_tile(
        &mut self,
        (x, y): (f32, f32),
        zoom: i32,
        rotation: i32,
        map_w: usize,
        map_h: usize,
    ) {
        let z = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        let (w, h) = (map_w as f32, map_h as f32);
        let rotation = rotation.rem_euclid(4);
        let (xp, yp) = match rotation {
            0 => (x, y),
            1 => (y, w - x),
            2 => (w - x, h - y),
            _ => (h - y, x),
        };
        let sx = (xp - yp) * 2f32.powi(z - 1);
        let sy = (xp + yp) * 2f32.powi(z - 2);
        self.zoom = z;
        self.rotation = rotation;
        self.width = SAVED_VIEWPORT.0;
        self.height = SAVED_VIEWPORT.1;
        self.scroll_x = sx.round() as i32 - self.width / 2;
        self.scroll_y = sy.round() as i32 - self.height / 2;
        self.map_width = map_w as i32;
        self.map_height = map_h as i32;
    }
}

impl Save {
    /// The camera block of the file.
    pub fn view(&self) -> View {
        let o = self.layout.view_offset();
        View::parse(&self.header[o..o + VIEW_LEN]).expect("header holds the view block")
    }

    /// Replaces the camera block (the saver writes the live window state here).
    pub fn set_view(&mut self, v: &View) {
        let o = self.layout.view_offset();
        self.header[o..o + VIEW_LEN].copy_from_slice(&v.to_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> View {
        // Tutorial.GAM: zoom 5, scroll (2231, 2137), rotation 1, 300 x 300 map.
        View {
            zoom: 5,
            scroll_x: 2231,
            scroll_y: 2137,
            width: 1024,
            height: 508,
            rotation: 1,
            map_width: 300,
            map_height: 300,
            person: 1,
            ..View::default()
        }
    }

    #[test]
    fn bytes_round_trip() {
        let v = sample();
        let b = v.to_bytes();
        assert_eq!(b.len(), VIEW_LEN);
        assert_eq!(&b[4..8], &5i32.to_le_bytes());
        assert_eq!(View::parse(&b), Some(v));
        assert_eq!(View::parse(&b[..0x3F]), None);
    }

    #[test]
    fn center_matches_the_tutorial_view() {
        let (x, y) = sample().center_tile(300, 300);
        assert!(
            (x - 236.28).abs() < 0.05 && (y - 235.16).abs() < 0.05,
            "{x} {y}"
        );
    }

    #[test]
    fn center_round_trips_for_every_rotation_on_a_rectangular_map() {
        for rot in 0..4 {
            for z in ZOOM_MIN..=ZOOM_MAX {
                let mut v = View::default();
                v.set_center_tile((120.5, 40.25), z, rot, 200, 90);
                let (x, y) = v.center_tile(200, 90);
                let tol = 1.0 / 2f32.powi(z - 2);
                assert!(
                    (x - 120.5).abs() < tol && (y - 40.25).abs() < tol,
                    "r{rot} z{z}"
                );
            }
        }
    }

    #[test]
    fn zeroed_block_has_no_valid_zoom() {
        assert!(!View::default().zoom_valid());
        assert!(sample().zoom_valid());
        assert_eq!(View::native_scale(7), 1.0);
        assert_eq!(View::native_scale(5), 0.25);
    }
}
