//! Map picking: the tile under a screen position.
//!
//! The camera is orthographic (`rt2_render::Camera`): a screen offset `(dx right, dy down)` from
//! the centre maps to a ground offset `right * dx / zoom - forward * dy / (zoom * sin 30)` on the
//! plane `y = 0`. The terrain is drawn at `y = corner elevation * height_scale`, so the point is
//! refined a few times against the elevation of the tile found (a point at height `h` appears
//! `h * cos 30 * zoom` pixels higher than its ground position).

use rt2_formats::map::Map;
use rt2_render::{Camera, height_scale};

const SIN_PITCH: f32 = 0.5;
const COS_PITCH: f32 = 0.866_025_4;

fn elevation(map: &Map, x: f32, y: f32) -> f32 {
    let (tx, ty) = (
        (x.floor().max(0.0) as usize).min(map.width.saturating_sub(1)),
        (y.floor().max(0.0) as usize).min(map.height.saturating_sub(1)),
    );
    let (fx, fy) = (
        (x - tx as f32).clamp(0.0, 1.0),
        (y - ty as f32).clamp(0.0, 1.0),
    );
    let c = |dx: usize, dy: usize| f32::from(map.corner(tx + dx, ty + dy));
    let top = c(0, 0) * (1.0 - fx) + c(1, 0) * fx;
    let bottom = c(0, 1) * (1.0 - fx) + c(1, 1) * fx;
    top * (1.0 - fy) + bottom * fy
}

/// The map tile under physical-pixel position `pos` in a `viewport`, or `None` outside the map.
pub fn screen_to_tile(
    cam: &Camera,
    viewport: (u32, u32),
    pos: (f32, f32),
    map: &Map,
) -> Option<(i32, i32)> {
    let dx = pos.0 - viewport.0 as f32 / 2.0;
    let dy0 = pos.1 - viewport.1 as f32 / 2.0;
    let (fwd, right) = (cam.forward(), cam.right());
    let at = |dy: f32| cam.target + right * (dx / cam.zoom) - fwd * (dy / (cam.zoom * SIN_PITCH));
    let mut g = at(dy0);
    for _ in 0..4 {
        let h = elevation(map, g.x, g.y) * height_scale();
        g = at(dy0 + h * COS_PITCH * cam.zoom);
    }
    let (tx, ty) = (g.x.floor() as i32, g.y.floor() as i32);
    (tx >= 0 && ty >= 0 && (tx as usize) < map.width && (ty as usize) < map.height)
        .then_some((tx, ty))
}

/// Physical-pixel position of the ground point `(fx, fy)` (tile units, the tile `(x, y)` is
/// `[x, x + 1)`), lifted by the terrain elevation there: the inverse of [`screen_to_tile`].
pub fn tile_to_screen(
    cam: &Camera,
    viewport: (u32, u32),
    map: &Map,
    fx: f32,
    fy: f32,
) -> (f32, f32) {
    let h = elevation(map, fx, fy) * height_scale();
    let m = cam.view_proj(viewport).to_cols_array();
    let (cx, cy) = (
        m[0] * fx + m[4] * h + m[8] * fy + m[12],
        m[1] * fx + m[5] * h + m[9] * fy + m[13],
    );
    (
        (cx + 1.0) / 2.0 * viewport.0 as f32,
        (1.0 - cy) / 2.0 * viewport.1 as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_map(w: usize, h: usize) -> Map {
        Map {
            version: 0x233C,
            name: String::new(),
            title: String::new(),
            width: w,
            height: h,
            tiles: Vec::new(),
            elevation: vec![0; (w + 1) * (h + 1)],
        }
    }

    /// Projects a ground point with the camera's matrix (column-major) to physical pixels.
    fn project(cam: &Camera, vp: (u32, u32), x: f32, y: f32) -> (f32, f32) {
        let m = cam.view_proj(vp).to_cols_array();
        let (cx, cy) = (m[0] * x + m[8] * y + m[12], m[1] * x + m[9] * y + m[13]);
        (
            (cx + 1.0) / 2.0 * vp.0 as f32,
            (1.0 - cy) / 2.0 * vp.1 as f32,
        )
    }

    #[test]
    fn picking_inverts_the_projection_on_flat_ground_for_every_rotation() {
        let map = flat_map(64, 64);
        let vp = (1280, 720);
        for rotation in 0..4u8 {
            let cam = Camera {
                target: rt2_render::Camera::fit(64, 64, vp).target,
                zoom: 30.0,
                rotation,
            };
            for &(tx, ty) in &[(10, 20), (32, 32), (50, 7)] {
                let p = project(&cam, vp, tx as f32 + 0.5, ty as f32 + 0.5);
                assert_eq!(
                    screen_to_tile(&cam, vp, p, &map),
                    Some((tx, ty)),
                    "rotation {rotation}"
                );
            }
        }
    }

    #[test]
    fn tile_to_screen_inverts_picking_with_relief() {
        let mut map = flat_map(16, 16);
        for (i, e) in map.elevation.iter_mut().enumerate() {
            *e = ((i % 17) * 3) as i16;
        }
        let vp = (1280, 720);
        let cam = Camera {
            target: rt2_render::camera::Vec2::new(8.0, 8.0),
            zoom: 40.0,
            rotation: 1,
        };
        for &(tx, ty) in &[(4, 5), (8, 8), (11, 3)] {
            let p = tile_to_screen(&cam, vp, &map, tx as f32 + 0.5, ty as f32 + 0.5);
            assert_eq!(screen_to_tile(&cam, vp, p, &map), Some((tx, ty)));
        }
    }

    #[test]
    fn outside_the_map_is_none() {
        let map = flat_map(8, 8);
        let cam = Camera::fit(8, 8, (800, 600));
        assert_eq!(screen_to_tile(&cam, (800, 600), (2.0, 2.0), &map), None);
    }
}
