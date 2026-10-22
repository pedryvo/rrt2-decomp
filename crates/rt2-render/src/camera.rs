//! Classic RT2-style isometric camera: orthographic, 2:1 dimetric (30 degree elevation and a 45
//! degree yaw, so a square ground tile appears as a diamond twice as wide as it is tall), rotated
//! in 90 degree steps.

use glam::{Mat4, Vec2, Vec3};

/// Elevation of the view direction above the ground plane. sin(30 deg) = 1/2 gives the 2:1 diamond.
const PITCH: f32 = 30.0 * std::f32::consts::PI / 180.0;
/// Distance of the (orthographic) eye from the target; only matters for clipping.
const EYE_DISTANCE: f32 = 3000.0;
pub const MIN_ZOOM: f32 = 0.02;
pub const MAX_ZOOM: f32 = 200.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Ground point (x east, z south) at the centre of the screen.
    pub target: Vec2,
    /// Physical pixels per world unit (one tile edge) along the screen x axis.
    pub zoom: f32,
    /// Quarter turns, 0..4.
    pub rotation: u8,
}

impl Camera {
    /// Frame a `width` x `height` tile map in a viewport of physical pixels.
    pub fn fit(width: usize, height: usize, viewport: (u32, u32)) -> Camera {
        let (w, h) = (width as f32, height as f32);
        // On screen the map is a diamond of (w + h) / sqrt(2) by (w + h) / sqrt(2) * sin(PITCH).
        let diamond = (w + h) / std::f32::consts::SQRT_2;
        let zoom = (viewport.0 as f32 / diamond).min(viewport.1 as f32 / (diamond * PITCH.sin()));
        Camera {
            target: Vec2::new(w / 2.0, h / 2.0),
            zoom: (zoom * 0.95).clamp(MIN_ZOOM, MAX_ZOOM),
            rotation: 0,
        }
    }

    fn yaw(&self) -> f32 {
        std::f32::consts::FRAC_PI_4 + self.rotation as f32 * std::f32::consts::FRAC_PI_2
    }

    /// Unit ground direction that points up the screen.
    pub fn forward(&self) -> Vec2 {
        // The eye sits to the south-west of the target at yaw 45 degrees, looking north-east.
        let (s, c) = self.yaw().sin_cos();
        Vec2::new(s, -c)
    }

    /// Unit ground direction that points right on the screen.
    pub fn right(&self) -> Vec2 {
        let f = self.forward();
        Vec2::new(-f.y, f.x)
    }

    pub fn rotate(&mut self, quarter_turns: i32) {
        self.rotation = (self.rotation as i32 + quarter_turns).rem_euclid(4) as u8;
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// Move the view so the world follows a mouse drag of `(dx, dy)` physical pixels.
    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.target +=
            self.forward() * (dy / (self.zoom * PITCH.sin())) - self.right() * (dx / self.zoom);
    }

    /// Pan by screen pixels, `dx` right and `dy` up (used for keyboard movement).
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.drag(-dx, dy);
    }

    pub fn view_proj(&self, viewport: (u32, u32)) -> Mat4 {
        let (vw, vh) = (viewport.0.max(1) as f32, viewport.1.max(1) as f32);
        let f = self.forward();
        let towards_eye = Vec3::new(-f.x * PITCH.cos(), PITCH.sin(), -f.y * PITCH.cos());
        let target = Vec3::new(self.target.x, 0.0, self.target.y);
        let view = Mat4::look_at_rh(target + towards_eye * EYE_DISTANCE, target, Vec3::Y);
        let (hw, hh) = (vw / 2.0 / self.zoom, vh / 2.0 / self.zoom);
        let proj = Mat4::orthographic_rh(-hw, hw, -hh, hh, 1.0, EYE_DISTANCE * 2.0);
        proj * view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(cam: &Camera, p: Vec3, viewport: (u32, u32)) -> Vec2 {
        let c = cam.view_proj(viewport) * p.extend(1.0);
        Vec2::new(c.x * viewport.0 as f32 / 2.0, c.y * viewport.1 as f32 / 2.0)
    }

    #[test]
    fn tiles_are_two_to_one_diamonds() {
        let cam = Camera {
            target: Vec2::ZERO,
            zoom: 10.0,
            rotation: 0,
        };
        let vp = (800, 600);
        let o = screen(&cam, Vec3::ZERO, vp);
        let east = screen(&cam, Vec3::X, vp) - o;
        let south = screen(&cam, Vec3::Z, vp) - o;
        let diag = east + south; // across the diamond's width
        let other = east - south;
        assert!(
            (diag.length() / other.length() - 2.0).abs() < 1e-3
                || (other.length() / diag.length() - 2.0).abs() < 1e-3
        );
    }

    #[test]
    fn target_projects_to_screen_centre_and_height_goes_up() {
        let cam = Camera {
            target: Vec2::new(5.0, 7.0),
            zoom: 4.0,
            rotation: 0,
        };
        let vp = (640, 480);
        assert!(screen(&cam, Vec3::new(5.0, 0.0, 7.0), vp).length() < 1e-3);
        assert!(screen(&cam, Vec3::new(5.0, 3.0, 7.0), vp).y > 0.0);
    }

    #[test]
    fn rotation_wraps_and_drag_follows_the_cursor() {
        let mut cam = Camera {
            target: Vec2::ZERO,
            zoom: 2.0,
            rotation: 0,
        };
        cam.rotate(-1);
        assert_eq!(cam.rotation, 3);
        cam.rotate(2);
        assert_eq!(cam.rotation, 1);
        // Dragging right by 20 px moves the world point under the target 10 units right on screen.
        let vp = (800, 600);
        let p = Vec3::new(1.0, 0.0, 1.0);
        let before = screen(&cam, p, vp);
        cam.drag(20.0, -12.0);
        let after = screen(&cam, p, vp);
        assert!((after.x - before.x - 20.0).abs() < 1e-2);
        // Screen y is up in clip space, the mouse y is down.
        assert!((after.y - before.y - 12.0).abs() < 1e-2);
    }
}
