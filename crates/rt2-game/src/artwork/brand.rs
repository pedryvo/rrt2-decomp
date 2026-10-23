//! Company logos and chairman portraits (docs/formats/sprite-catalog.md section 9):
//! `brlogo00..40.imb` (the 41 railroad logos, 1 record each), `bigp00..40.imb` (large
//! portraits) and `brplyr00..40.imb` (156x179 oval portraits). Loaded into the skin next to the
//! other UI art; `None` entries when the install lacks the bank.

use rt2_render::assets::GameAssets;
use rt2_ui::{SkinBuilder, SpriteId};

use super::{record, to_skin_image};

/// Number of logos and portraits (`00..40`).
pub const BRAND_COUNT: usize = 41;

/// The sprites of one family, indexed by the company's logo byte (`+0x2b`).
#[derive(Clone, Debug, Default)]
pub struct BrandArt {
    pub logos: Vec<Option<SpriteId>>,
    pub portraits: Vec<Option<SpriteId>>,
}

fn family(assets: &GameAssets, b: &mut SkinBuilder, prefix: &str) -> Vec<Option<SpriteId>> {
    (0..BRAND_COUNT)
        .map(|i| {
            record(assets, &format!("{prefix}{i:02}.imb"), 0)
                .map(|img| b.add(to_skin_image(&img, 1)))
        })
        .collect()
}

pub(super) fn load(assets: &GameAssets, b: &mut SkinBuilder) -> BrandArt {
    BrandArt {
        logos: family(assets, b, "brlogo"),
        portraits: family(assets, b, "brplyr"),
    }
}
