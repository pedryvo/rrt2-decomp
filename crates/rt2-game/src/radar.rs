//! The radar / overview minimap of the main window (manual chapter 5, "Radar Area"; LNG 1772
//! "Radar area - click to recenter main view"): the terrain grid as one coloured cell per block of
//! tiles, with the part of the map the camera shows outlined. A click or drag on it moves the
//! camera to that spot.
//!
//! The colours come from the terrain only (water by the tile's water class, land shaded by the
//! corner elevations); buildings, towns and track are not drawn. The map view looks at the map
//! through an isometric camera, so the outline is the ground quadrilateral of the screen corners,
//! not an axis-aligned box.

use rt2_formats::map::Map;
use rt2_render::Camera;
use rt2_ui::Color;

/// The most cells along the longer side of the minimap.
pub const MAX_CELLS: usize = 112;

const WATER_DEEP: Color = Color::rgb(0x1c, 0x3a, 0x5a);
const WATER_SHALLOW: Color = Color::rgb(0x2c, 0x56, 0x7c);
const FRAME: Color = Color::rgb(0xff, 0xf0, 0x90);

/// The minimap grid: base terrain colours and the map-to-cell mapping.
#[derive(Clone, Debug)]
pub struct Radar {
    pub cols: usize,
    pub rows: usize,
    /// Tiles per cell along each axis (>= 1, may be fractional for small maps: 1 tile spans
    /// several cells never, so cells == tiles up to [`MAX_CELLS`]).
    step: f32,
    map_w: usize,
    map_h: usize,
    base: Vec<Color>,
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color::rgb(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Land colour for a mean corner elevation: green lowlands, olive and tan hills, grey-white
/// peaks. The elevation unit is the original's (`height_scale` in `rt2-render`); typical land
/// is 0..~60, mountains above.
fn land_color(elev: f32) -> Color {
    const LOW: Color = Color::rgb(0x4c, 0x7a, 0x3c);
    const MID: Color = Color::rgb(0x8a, 0x8c, 0x4a);
    const HIGH: Color = Color::rgb(0xa8, 0x8c, 0x68);
    const PEAK: Color = Color::rgb(0xe8, 0xe4, 0xdc);
    let e = elev.max(0.0);
    if e < 60.0 {
        lerp(LOW, MID, e / 60.0)
    } else if e < 180.0 {
        lerp(MID, HIGH, (e - 60.0) / 120.0)
    } else {
        lerp(HIGH, PEAK, ((e - 180.0) / 240.0).min(1.0))
    }
}

impl Radar {
    /// Builds the grid of `map`: at most [`MAX_CELLS`] cells on the longer side, the aspect of the
    /// map kept.
    pub fn new(map: &Map) -> Radar {
        let (w, h) = (map.width.max(1), map.height.max(1));
        let step = (w.max(h) as f32 / MAX_CELLS as f32).max(1.0);
        let cols = ((w as f32 / step).ceil() as usize).max(1);
        let rows = ((h as f32 / step).ceil() as usize).max(1);
        let mut base = Vec::with_capacity(cols * rows);
        for j in 0..rows {
            for i in 0..cols {
                base.push(Self::cell_color(map, step, i, j));
            }
        }
        Radar {
            cols,
            rows,
            step,
            map_w: w,
            map_h: h,
            base,
        }
    }

    /// Mean of the tiles of one cell: the share of water decides between water and land.
    fn cell_color(map: &Map, step: f32, i: usize, j: usize) -> Color {
        let x0 = (i as f32 * step) as usize;
        let y0 = (j as f32 * step) as usize;
        let x1 = (((i + 1) as f32 * step).ceil() as usize)
            .min(map.width)
            .max(x0 + 1);
        let y1 = (((j + 1) as f32 * step).ceil() as usize)
            .min(map.height)
            .max(y0 + 1);
        let (mut water, mut n) = (0u32, 0u32);
        let mut elev = 0.0f32;
        for y in y0..y1.min(map.height) {
            for x in x0..x1.min(map.width) {
                n += 1;
                if map.tile(x, y).is_water() {
                    water += 1;
                }
                elev += f32::from(map.corner(x, y));
            }
        }
        if n == 0 {
            return WATER_DEEP;
        }
        let share = water as f32 / n as f32;
        let mean = elev / n as f32;
        if share > 0.5 {
            // Deeper sea floor reads darker.
            lerp(WATER_SHALLOW, WATER_DEEP, (-mean / 40.0).clamp(0.0, 1.0))
        } else {
            let land = land_color(mean);
            if share > 0.0 {
                lerp(land, WATER_SHALLOW, share)
            } else {
                land
            }
        }
    }

    /// The terrain colours, row-major (`cols * rows`).
    pub fn colors(&self) -> &[Color] {
        &self.base
    }

    /// The map position (tile units) at a fraction of the grid.
    pub fn to_tile(&self, fx: f32, fy: f32) -> (f32, f32) {
        (
            fx.clamp(0.0, 1.0) * self.map_w as f32,
            fy.clamp(0.0, 1.0) * self.map_h as f32,
        )
    }

    /// Cell coordinates (fractional) of a map position in tile units.
    fn to_cell(&self, x: f32, y: f32) -> (f32, f32) {
        (x / self.step, y / self.step)
    }

    /// The cells with the outline of what a camera shows: the ground points under the four
    /// corners of a `viewport` (physical pixels) drawn as a closed line.
    pub fn colors_with_view(&self, cam: &Camera, viewport: (u32, u32)) -> Vec<Color> {
        let mut out = self.base.clone();
        let (w, h) = (viewport.0 as f32, viewport.1 as f32);
        let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(px, py)| {
            let g = ground_point(cam, viewport, px, py);
            self.to_cell(g.0, g.1)
        });
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            self.line(&mut out, a, b);
        }
        out
    }

    fn line(&self, out: &mut [Color], a: (f32, f32), b: (f32, f32)) {
        let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.0)
            .ceil()
            .max(1.0) as usize;
        // Very long lines (a camera far outside the map) are cut to a sane count.
        let steps = steps.min(4 * (self.cols + self.rows));
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            if x >= 0.0 && y >= 0.0 {
                let (i, j) = (x as usize, y as usize);
                if i < self.cols && j < self.rows {
                    out[j * self.cols + i] = FRAME;
                }
            }
        }
    }
}

/// The ground point (tile units, elevation ignored) under a screen position of the camera
/// (`pick.rs` without the elevation refinement).
pub fn ground_point(cam: &Camera, viewport: (u32, u32), px: f32, py: f32) -> (f32, f32) {
    const SIN_PITCH: f32 = 0.5;
    let dx = px - viewport.0 as f32 / 2.0;
    let dy = py - viewport.1 as f32 / 2.0;
    let g =
        cam.target + cam.right() * (dx / cam.zoom) - cam.forward() * (dy / (cam.zoom * SIN_PITCH));
    (g.x, g.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(w: usize, h: usize) -> Map {
        Map {
            version: 0x233C,
            name: String::new(),
            title: String::new(),
            width: w,
            height: h,
            tiles: vec![
                rt2_formats::map::Tile {
                    var: 0,
                    ground: 0x10,
                    flags2: 0,
                    trees: 0,
                    struct_id: 0,
                    reserved5: 0,
                    flags6: 0,
                    track_id: 0,
                    reserved8: 0,
                    region: 1,
                };
                w * h
            ],
            elevation: vec![10; (w + 1) * (h + 1)],
        }
    }

    #[test]
    fn small_maps_get_one_cell_per_tile_and_big_ones_are_capped() {
        let r = Radar::new(&flat(40, 20));
        assert_eq!((r.cols, r.rows), (40, 20));
        let r = Radar::new(&flat(500, 250));
        assert!(r.cols <= MAX_CELLS + 1 && r.rows <= MAX_CELLS / 2 + 1);
        assert!(r.cols > r.rows, "the aspect ratio is kept");
    }

    #[test]
    fn a_click_maps_back_to_tile_units() {
        let r = Radar::new(&flat(300, 150));
        assert_eq!(r.to_tile(0.5, 0.5), (150.0, 75.0));
        assert_eq!(r.to_tile(2.0, -1.0), (300.0, 0.0), "clamped to the map");
    }

    #[test]
    fn the_view_outline_is_drawn_into_the_cells() {
        let map = flat(64, 64);
        let r = Radar::new(&map);
        let vp = (800, 600);
        let mut cam = Camera::fit(64, 64, vp);
        cam.zoom *= 4.0;
        let plain = r.base.clone();
        let with = r.colors_with_view(&cam, vp);
        assert_eq!(with.len(), plain.len());
        assert!(with.iter().zip(&plain).any(|(a, b)| a != b));
        assert!(with.contains(&FRAME));
    }

    #[test]
    fn the_ground_under_the_screen_centre_is_the_camera_target() {
        let vp = (800, 600);
        let cam = Camera::fit(64, 64, vp);
        let g = ground_point(&cam, vp, 400.0, 300.0);
        assert!((g.0 - cam.target.x).abs() < 1e-3 && (g.1 - cam.target.y).abs() < 1e-3);
    }

    #[test]
    fn elevation_changes_the_land_colour() {
        assert_ne!(land_color(0.0), land_color(50.0));
        assert_ne!(land_color(50.0), land_color(200.0));
    }
}
