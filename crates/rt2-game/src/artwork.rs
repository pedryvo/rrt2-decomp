//! The original UI art as a skin (docs/ui.md "Skin"): the title backdrop, the grey button plates
//! and the dark panel plate, decoded from the install and upscaled like the map sprites.
//! Without the install art everything falls back to the flat theme.

use rt2_render::assets::{GameAssets, LoadedBank, RgbaImage};
use rt2_render::atlas::scale2x;
use rt2_ui::{NineSlice, Skin, SkinBuilder, SkinImage, SpriteId, Theme};

/// `setup.imb` record 0: the 1024x768 title backdrop of `setup.win`.
const BACKDROP: (&str, usize) = ("setup.imb", 0);
/// `setupb.imb` records 0 and 1: the button plate, up and down (`setup.win` buttons 3135..3140).
const BUTTON: (&str, usize, usize) = ("setupb.imb", 0, 1);
/// `setupp.imb` record 2: a dark metal plate (the panels of `setup.win`), 490x417.
const PANEL: (&str, usize) = ("setupp.imb", 2);

/// 9-slice insets in original pixels (the plates' bevel and rivets).
const BUTTON_INSETS: [f32; 4] = [8.0, 8.0, 8.0, 8.0];
const PANEL_INSETS: [f32; 4] = [18.0, 18.0, 18.0, 18.0];

/// `mainbut.imb`: the main window's icon buttons, in pairs of records (up, down), 50x34 or
/// 44x34 pixels: 0 rotate right, 2 rotate left, 4 zoom out, 6 zoom in, 8 list toggle, 20 lay
/// track, 22 bulldoze, 24 build station, 32 purchase train, 34 stock market, 36 control panel,
/// 38 file options, 42 map overview (`mainwin.win` buttons 201..221, `docs/formats/ui.md`).
const ICON_BANK: &str = "mainbut.imb";
const ICON_RECORDS: [usize; 13] = [0, 2, 4, 6, 8, 20, 22, 24, 32, 34, 36, 38, 42];

/// Icon records of [`ICON_BANK`] loaded into the skin: first record of a pair -> (up, down).
#[derive(Clone, Debug, Default)]
pub struct Icons {
    map: std::collections::BTreeMap<usize, (SpriteId, SpriteId)>,
}

impl Icons {
    /// The (up, down) sprites of the icon whose first record is `record`.
    pub fn get(&self, record: usize) -> Option<(SpriteId, SpriteId)> {
        self.map.get(&record).copied()
    }
}

/// Dialog frames of the original windows (`eagled.win`, `paper.win`, `yearend.win`,
/// `winlose.win`; `docs/formats/ui.md`), decoded with the UI image decoder. Each is a plain
/// sprite the dialogs stretch to their size (never 9-sliced: the art has fixed borders).
#[derive(Clone, Copy, Debug, Default)]
pub struct DialogArt {
    /// `eagled.imb` 0, 1, 2: the eagle top (512x137), a repeatable 46 px strip and the bottom
    /// (513x256) of the generic dialog. Taller texts insert strips between top and bottom.
    pub eagle: Option<[SpriteId; 3]>,
    /// `eagled.imb` 3..=8: the left (137x60), right (137x60) and middle (110x54) dialog button
    /// plates, each as (up, down).
    pub eagle_button: Option<[SpriteId; 6]>,
    /// `paper.imb` 0: the newspaper page (771x505).
    pub paper: Option<SpriteId>,
    /// `yearend.imb` 0 (cover) and 1..=4 (the pages for the four tabs, 509x59x).
    pub pages: [Option<SpriteId>; 4],
    /// `winlose.imb` 0: the wooden plaque (583x685); 1 and 2: the gold and silver medals.
    pub plaque: Option<SpriteId>,
    pub medal_gold: Option<SpriteId>,
    pub medal_silver: Option<SpriteId>,
    /// `bl1stV{NN}.imb` record 0 (map sprite codec), the 24 station buildings in the
    /// Victorian style (shape `size * 8 + orientation`), for the station detail's picture.
    pub stations: [Option<SpriteId>; 24],
}

pub struct Artwork {
    pub skin: Skin,
    pub backdrop: Option<SpriteId>,
    pub icons: Icons,
    pub dialogs: DialogArt,
}

/// Image 0 of record `rec` of a UI bank (`*.imb` of the `.win` windows), which uses the text
/// drawer's token set rather than the map sprites' codec: indexed cells become opaque pixels,
/// empty cells transparent.
fn ui_record(assets: &GameAssets, bank: &str, rec: usize) -> Option<RgbaImage> {
    let lad = assets.lad(bank)?;
    let r = lad.records.get(rec)?;
    let img = r.groups.first()?.images.first()?;
    let layer = rt2_formats::font::decode_text_image(img)?;
    let pal = if img.flags & rt2_formats::lad::FLAG_PALETTE != 0 {
        *assets.palette(r.palette_id())?.palettes.first()?
    } else {
        std::array::from_fn(|i| (i as u16 >> 3) * 0x421)
    };
    let mut rgba = vec![0u8; layer.width * layer.height * 4];
    for (i, c) in layer.cells.iter().enumerate() {
        if let rt2_formats::font::Cell::Index(v) = c {
            let [r, g, b] = rt2_formats::palette::rgb555(pal[usize::from(*v)]);
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    Some(RgbaImage {
        width: layer.width,
        height: layer.height,
        x: layer.x,
        y: layer.y,
        rgba,
    })
}

fn to_skin_image(img: &RgbaImage, bake: u32) -> SkinImage {
    SkinImage {
        width: img.width as u32,
        height: img.height as u32,
        rgba: img.rgba.clone(),
        bake,
    }
}

fn record(assets: &GameAssets, bank: &str, rec: usize) -> Option<RgbaImage> {
    LoadedBank::open(assets, bank)?.image(rec, 0, 0, 0)
}

/// Opaque copy: transparent texels of the plates (palette index 0) become the edge colour so
/// stretched plates have no holes.
fn solidify(img: &mut RgbaImage) {
    let n = img.rgba.len() / 4;
    let mut sum = [0u64; 3];
    let mut count = 0u64;
    for i in 0..n {
        if img.rgba[i * 4 + 3] != 0 {
            for (c, acc) in sum.iter_mut().enumerate() {
                *acc += u64::from(img.rgba[i * 4 + c]);
            }
            count += 1;
        }
    }
    if count == 0 {
        return;
    }
    let avg = [
        (sum[0] / count) as u8,
        (sum[1] / count) as u8,
        (sum[2] / count) as u8,
    ];
    for i in 0..n {
        if img.rgba[i * 4 + 3] == 0 {
            img.rgba[i * 4..i * 4 + 4].copy_from_slice(&[avg[0], avg[1], avg[2], 255]);
        }
    }
}

pub fn load(assets: &GameAssets) -> Artwork {
    let mut b = SkinBuilder::new();
    // The backdrop is a photograph: no pixel-art upscale, bilinear filtering only.
    let backdrop = record(assets, BACKDROP.0, BACKDROP.1).map(|mut i| {
        solidify(&mut i);
        b.add(to_skin_image(&i, 1))
    });
    let buttons = record(assets, BUTTON.0, BUTTON.1)
        .zip(record(assets, BUTTON.0, BUTTON.2))
        .map(|(mut up, mut down)| {
            solidify(&mut up);
            solidify(&mut down);
            // Same size for both states so the 9-slice does not jump when pressed.
            let (w, h) = (up.width.min(down.width), up.height.min(down.height));
            let crop = |i: &RgbaImage| RgbaImage {
                width: w,
                height: h,
                x: 0,
                y: 0,
                rgba: (0..h)
                    .flat_map(|y| i.rgba[y * i.width * 4..y * i.width * 4 + w * 4].to_vec())
                    .collect(),
            };
            (
                b.add(to_skin_image(&scale2x(&crop(&up)), 2)),
                b.add(to_skin_image(&scale2x(&crop(&down)), 2)),
            )
        });
    let panel = record(assets, PANEL.0, PANEL.1).map(|mut i| {
        solidify(&mut i);
        b.add(to_skin_image(&scale2x(&i), 2))
    });
    let mut icons = Icons::default();
    for rec in ICON_RECORDS {
        if let (Some(up), Some(down)) = (
            record(assets, ICON_BANK, rec),
            record(assets, ICON_BANK, rec + 1),
        ) {
            icons.map.insert(
                rec,
                (
                    b.add(to_skin_image(&scale2x(&up), 2)),
                    b.add(to_skin_image(&scale2x(&down), 2)),
                ),
            );
        }
    }
    let mut add = |bank: &str, rec: usize| -> Option<SpriteId> {
        ui_record(assets, bank, rec).map(|i| b.add(to_skin_image(&i, 1)))
    };
    let all = |v: [Option<SpriteId>; 3]| Some([v[0]?, v[1]?, v[2]?]);
    let all6 = |v: [Option<SpriteId>; 6]| Some([v[0]?, v[1]?, v[2]?, v[3]?, v[4]?, v[5]?]);
    let dialogs = DialogArt {
        eagle: all([
            add("eagled.imb", 0),
            add("eagled.imb", 1),
            add("eagled.imb", 2),
        ]),
        eagle_button: all6([
            add("eagled.imb", 3),
            add("eagled.imb", 4),
            add("eagled.imb", 5),
            add("eagled.imb", 6),
            add("eagled.imb", 7),
            add("eagled.imb", 8),
        ]),
        paper: add("paper.imb", 0),
        pages: [
            add("yearend.imb", 1),
            add("yearend.imb", 2),
            add("yearend.imb", 3),
            add("yearend.imb", 4),
        ],
        plaque: add("winlose.imb", 0),
        medal_gold: add("winlose.imb", 1),
        medal_silver: add("winlose.imb", 2),
        stations: [None; 24],
    };
    let mut dialogs = dialogs;
    for (shape, slot) in dialogs.stations.iter_mut().enumerate() {
        let name = format!("bl1stV{}{}.imb", shape / 8 + 1, shape % 8);
        *slot = record(assets, &name, 0).map(|i| b.add(to_skin_image(&i, 1)));
    }
    let mut skin = b.build(Theme::dark());
    skin.button = buttons.map(|(up, down)| rt2_ui::ButtonSkin {
        up: NineSlice {
            sprite: up,
            insets: BUTTON_INSETS,
        },
        down: NineSlice {
            sprite: down,
            insets: BUTTON_INSETS,
        },
    });
    skin.panel = panel.map(|p| NineSlice {
        sprite: p,
        insets: PANEL_INSETS,
    });
    Artwork {
        skin,
        backdrop,
        icons,
        dialogs,
    }
}
