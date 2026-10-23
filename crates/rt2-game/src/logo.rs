//! The project logo (`docs/assets/logo.svg`), embedded in the binary and rasterized at the size it
//! is drawn, so it is sharp on any screen. The setup screen shows it before any original asset is
//! available.

use resvg::{tiny_skia, usvg};

static SVG: &str = include_str!("../../../docs/assets/logo.svg");
/// The face the logo's caption falls back to (the SVG names Verdana / DejaVu Sans): vendored, so
/// the look does not depend on the machine's fonts.
static CAPTION_FONT: &[u8] = include_bytes!("../../../assets/fonts/LibreFranklin.ttf");

/// A straight-alpha RGBA8 square image.
pub struct Raster {
    pub size: u32,
    pub rgba: Vec<u8>,
}

/// The logo rasterized into a `size` x `size` square (clamped to 16..=4096).
pub fn rasterize(size: u32) -> Option<Raster> {
    let size = size.clamp(16, 4096);
    let mut opt = usvg::Options::default();
    {
        let db = opt.fontdb_mut();
        db.load_font_data(CAPTION_FONT.to_vec());
        db.set_sans_serif_family("Libre Franklin");
    }
    opt.font_family = "Libre Franklin".to_owned();
    let tree = usvg::Tree::from_str(SVG, &opt).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let ts = tiny_skia::Transform::from_scale(
        size as f32 / tree.size().width(),
        size as f32 / tree.size().height(),
    );
    resvg::render(&tree, ts, &mut pixmap.as_mut());
    // tiny-skia is premultiplied; the UI atlas wants straight alpha.
    let mut rgba = pixmap.take();
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = u32::from(px[3]);
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Some(Raster { size, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_at_the_requested_size_with_a_round_badge() {
        let r = rasterize(256).unwrap();
        assert_eq!((r.size, r.rgba.len()), (256, 256 * 256 * 4));
        let alpha = |x: usize, y: usize| r.rgba[(y * 256 + x) * 4 + 3];
        assert_eq!(alpha(0, 0), 0, "the corner is outside the badge");
        assert_eq!(alpha(128, 128), 255, "the centre is opaque");
        assert!(rasterize(4096).is_some());
        assert_eq!(rasterize(1).unwrap().size, 16);
    }
}
