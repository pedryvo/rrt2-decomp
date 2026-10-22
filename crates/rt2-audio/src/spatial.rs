//! Positional volume and pan for sounds that happen somewhere on the map (train whistles,
//! brakes, crashes).
//!
//! The original scales the volume of train sounds by the distance between the train and the
//! view centre (`0x49b610`) and does not play them at all when the result is 5 or less
//! (`0x4d0400`, `0x4d29e0`); it has no panning. The distance formula itself is not decoded
//! (float arguments lost in `0x4f0ca0`), so the falloff below is the clone's own and the pan is a
//! modern extra. Both are on the audio side only.

/// What is on screen, in the same units as the sound position (pixels or tiles).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub center: (f32, f32),
    pub half_width: f32,
    pub half_height: f32,
}

/// Volume threshold under which the original skips the sound (`5 < volume`).
pub const MIN_VOLUME: u8 = 5;

/// Volume percent (0..=100) and pan (-1 left .. 1 right) for a sound at `pos`, or `None` when the
/// original's audibility threshold would drop it.
pub fn locate(view: &View, pos: (f32, f32)) -> Option<(u8, f32)> {
    let dx = (pos.0 - view.center.0) / view.half_width.max(f32::EPSILON);
    let dy = (pos.1 - view.center.1) / view.half_height.max(f32::EPSILON);
    // Full volume inside the screen, linear falloff to silence at twice the screen size.
    let dist = dx.hypot(dy);
    let volume = (100.0 * (1.0 - (dist - 1.0).max(0.0))).clamp(0.0, 100.0) as u8;
    (volume > MIN_VOLUME).then(|| (volume, (dx * 0.8).clamp(-0.8, 0.8)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: View = View {
        center: (100.0, 50.0),
        half_width: 40.0,
        half_height: 30.0,
    };

    #[test]
    fn centre_is_loud_and_centred() {
        assert_eq!(locate(&VIEW, (100.0, 50.0)), Some((100, 0.0)));
    }

    #[test]
    fn pan_follows_x() {
        let (_, left) = locate(&VIEW, (80.0, 50.0)).unwrap();
        let (_, right) = locate(&VIEW, (120.0, 50.0)).unwrap();
        assert!(left < 0.0 && right > 0.0);
        assert!((left + right).abs() < 1e-6);
    }

    #[test]
    fn far_sounds_are_dropped() {
        assert!(locate(&VIEW, (100.0 + 40.0 * 2.5, 50.0)).is_none());
        let (v, _) = locate(&VIEW, (100.0 + 40.0 * 1.5, 50.0)).unwrap();
        assert!((40..=60).contains(&v));
    }
}
