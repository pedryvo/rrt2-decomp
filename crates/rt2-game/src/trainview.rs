//! The picture of the Purchase Train window (`trainbuy.win` control 1707, `0x4e3f90`): a 293x153
//! box with the terrain of `trainbuy.imb` record 6 and the locomotive and its tender standing on
//! the diagonal track, sprites `cr1<name>.imb` record 3 (docs/ui.md "Janelas de compra").
//!
//! Placement (`0x4e3f90`, first part): with `A` the length of the locomotive vehicle and `B` the
//! length of the tender (vehicle type `+6`), the train is centred on the track:
//! `off = max(0, ftol((265.64 - (A + B)) * 0.5))`; the locomotive's anchor is at distance
//! `A/2 + off` and the tender's at `A + B/2 + off` from the lower right corner anchor
//! `(230, 144)` of the box, along the diagonal `(-0.862, -0.431)` per length unit. The tender is
//! drawn first.

use rt2_render::assets::{GameAssets, LoadedBank, RgbaImage};

use crate::artwork::buywin::{PIC, blend};

/// Anchor of the track in the box (`0x4e3f90`: left + 0xe6, top + 0x90).
const ANCHOR: (f64, f64) = (230.0, 144.0);
/// `0x537878`: the track length the train is centred in; `0x537868`, `0x537858`: the step per
/// length unit along the diagonal.
const TRACK: f64 = 265.64;
const STEP: (f64, f64) = (0.862_068_965_517_241_4, 0.431_034_482_758_620_7);

/// Positions (box pixels) of the locomotive and the tender anchors.
pub fn anchors(len_engine: u32, len_tender: u32) -> ((i32, i32), (i32, i32)) {
    let (a, b) = (f64::from(len_engine), f64::from(len_tender));
    let off = (((TRACK - (a + b)) * 0.5).trunc()).max(0.0);
    let at = |d: f64| {
        (
            (ANCHOR.0 - d * STEP.0).trunc() as i32,
            (ANCHOR.1 - d * STEP.1).trunc() as i32,
        )
    };
    let d_engine = (len_engine / 2) as f64 + off;
    let d_tender = (len_tender / 2) as f64 + a + off;
    (at(d_engine), at(d_tender))
}

fn sprite(assets: &GameAssets, name: &str) -> Option<RgbaImage> {
    LoadedBank::open(assets, &format!("cr1{name}.imb"))?.image(3, 0, 0, 0)
}

/// The picture at the original size (`PIC.2` x `PIC.3`), `None` without the terrain piece.
pub fn compose(
    assets: &GameAssets,
    engine: &str,
    tender: Option<&str>,
    lens: (u32, u32),
) -> Option<RgbaImage> {
    let terrain = crate::artwork::ui_record_pub(assets, "trainbuy.imb", 6)?;
    let mut canvas = RgbaImage {
        width: PIC.2 as usize,
        height: PIC.3 as usize,
        x: 0,
        y: 0,
        rgba: vec![0; PIC.2 as usize * PIC.3 as usize * 4],
    };
    blend(&mut canvas, &terrain, 0, 0);
    let (pe, pt) = anchors(lens.0, lens.1);
    if let Some(t) = tender.and_then(|n| sprite(assets, n)) {
        blend(&mut canvas, &t, pt.0, pt.1);
    }
    if let Some(e) = sprite(assets, engine) {
        blend(&mut canvas, &e, pe.0, pe.1);
    }
    Some(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_train_is_centred_on_the_track() {
        // A long engine with its tender fills the track: no margin, engine in front (lower).
        let (e, t) = anchors(120, 100);
        assert!(e.1 > t.1 && e.0 > t.0);
        // A short engine alone sits nearer the lower right corner anchor.
        let (e2, _) = anchors(40, 0);
        assert!(e2.0 < 230 && e2.1 < 144);
    }

    #[test]
    fn engine_anchor_follows_the_formula() {
        // A = 100, B = 0: off = ftol(165.64 * 0.5) = 82, d = 50 + 82 = 132.
        let (e, _) = anchors(100, 0);
        assert_eq!(e, (116, 87));
    }
}
