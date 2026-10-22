//! Everything the renderer needs from a map: the textured terrain mesh, the texture stacks and the
//! list of sprites to place (trees, houses, industries, stations). The scene is pure data; the
//! GPU upload lives in `renderer.rs`.

use std::sync::Arc;

use rt2_formats::map::{Map, Save};

use crate::assets::{GameAssets, LoadedBank, NATURAL_SHADE, RgbaImage};
use crate::atlas::{Atlas, bake_atlas};
use crate::ground_map::{ground_texture, texture_turns, tile_hash, track_record, water_variant};
use crate::mesh::{TerrainMesh, TileLook, height_scale, surface_elevation};
use crate::textures::{TRACK_DOUBLE_BASE, TerrainTextures};

/// Building names by `building_type_table` index (`0x5457e8`, `sprite-catalog.md` section 7). The
/// first 24 entries are the station designs `sta10..sta37`.
const BUILDING_NAMES: [&str; 122] = [
    "sta10", "sta11", "sta12", "sta13", "sta14", "sta15", "sta16", "sta17", "sta20", "sta21",
    "sta22", "sta23", "sta24", "sta25", "sta26", "sta27", "sta30", "sta31", "sta32", "sta33",
    "sta34", "sta35", "sta36", "sta37", "lumbr", "housa", "housb", "housc", "housd", "house",
    "housf", "housg", "housh", "grain", "grain", "grain", "grain", "auto_", "textl", "sheep",
    "paper", "oilrf", "nuker", "fertl", "dproc", "dfarm", "meatp", "gravl", "cottn", "cottn",
    "cottn", "cottn", "cmine", "chem_", "cemnt", "caner", "baker", "steel", "tiref", "toold",
    "alum_", "cattl", "coffe", "coffe", "coffe", "coffe", "elect", "landf", "loggi", "oilwl",
    "prodc", "prodc", "prodc", "prodc", "rubbr", "rubbr", "rubbr", "rubbr", "uran_", "sugar",
    "sugar", "sugar", "sugar", "baux_", "port_", "port_", "port_", "port_", "imine", "geoc_",
    "th1ab", "th1ac", "th2ca", "th2cb", "th3ab", "th3ac", "th4ba", "th4bc", "com1a", "com1b",
    "com2b", "com2c", "com3a", "com3b", "com4a", "com4c", "ret1a", "ret1b", "ret2b", "ret2c",
    "ret3a", "ret3b", "ret4a", "ret4c", "distl", "barrk", "depot", "munit", "weapf", "airpt",
    "airpn", "airps",
];

/// Tree banks by vegetation class slot, in `0x536450` order (`sprite-catalog.md` section 1).
const TREE_BANKS: [&str; 12] = [
    "treoak_", "trepine", "scrub__", "trepalm", "trecact", "trebrus", "tremesq", "trerfor",
    "treswam", "treprod", "trerubb", "trecoff",
];

/// Station art style letter. PROVISIONAL: the original picks one of `V C M T K` per station
/// (`sprite-catalog.md` section 1, `"VCMTBK"` at `0x550474`); the selector is not decoded.
const STATION_STYLE: char = 'V';

/// Tree species for a tile's vegetation class (`tile.flags2 & 0xf`).
///
/// PROVISIONAL: the species -> vegetation class association was not read from the executable
/// (`sprite-catalog.md` section 5, open); this follows density and climate seen in the maps.
// TODO(sprite-catalog.md section 5): replace once the class -> species table is decoded.
fn tree_bank_for(class: u8, hash: u32) -> Option<&'static str> {
    Some(match class {
        1 => TREE_BANKS[0],
        2 => TREE_BANKS[1],
        3 => {
            if hash & 4 == 0 {
                TREE_BANKS[0]
            } else {
                TREE_BANKS[1]
            }
        }
        4 => TREE_BANKS[3],
        5 => TREE_BANKS[4],
        6 => TREE_BANKS[2],
        7 => TREE_BANKS[6],
        8 => TREE_BANKS[5],
        9 => TREE_BANKS[7],
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankKind {
    /// `bn{rot}{name}.imb`, falling back to the animated `bl{rot}{name}.imb`.
    Building,
    /// `bl{rot}st{style}{NN}.imb`.
    Station,
    /// `{name}{rot}.imb`.
    Tree,
}

/// One sprite bank family; the file depends on the view rotation (`rot + 1` digit, section 2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankSpec {
    pub kind: BankKind,
    pub stem: String,
    /// Palette (shade level) of the bank used to colour the sprite.
    pub shade: usize,
}

impl BankSpec {
    /// Candidate file names for a view rotation, best first.
    pub fn file_names(&self, rotation: u8) -> Vec<String> {
        let d = rotation % 4 + 1;
        match self.kind {
            BankKind::Building => vec![
                format!("bn{d}{}.imb", self.stem),
                format!("bl{d}{}.imb", self.stem),
            ],
            BankKind::Station => vec![format!("bl{d}{}.imb", self.stem)],
            BankKind::Tree => vec![format!("{}{d}.imb", self.stem)],
        }
    }
}

/// A sprite anchored on the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub bank: u32,
    /// Variant selector: the record used is `variant % record_count`.
    pub variant: u32,
    /// World position of the anchor: x east, height (world units), z south.
    pub pos: [f32; 3],
}

#[derive(Debug, Clone, Default)]
pub struct SpritePlan {
    pub banks: Vec<BankSpec>,
    pub placements: Vec<Placement>,
}

/// One sprite instance as the GPU reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub pos: [f32; 3],
    pub shade: f32,
    /// `[x, y, w, h]` at native scale relative to the anchor (y down).
    pub rect: [f32; 4],
    /// `[u0, v0, u1, v1]` in the atlas.
    pub uv: [f32; 4],
}

/// Sprites of one view rotation: baked atlas plus instances (unsorted).
pub struct RotationSprites {
    pub atlas: Atlas,
    pub instances: Vec<Instance>,
}

impl SpritePlan {
    fn bank(&mut self, spec: BankSpec) -> u32 {
        if let Some(i) = self.banks.iter().position(|b| *b == spec) {
            return i as u32;
        }
        self.banks.push(spec);
        self.banks.len() as u32 - 1
    }

    /// Plan trees and, with a save, the buildings of the object pool.
    pub fn build(map: &Map, save: Option<&Save>) -> SpritePlan {
        let mut plan = SpritePlan::default();
        // Trees: one to eight sprites per tile by the density byte (`tile[3]`), scattered with a
        // position hash (the original's own scatter uses `rng_hash_range`, not decoded).
        for ty in 0..map.height {
            for tx in 0..map.width {
                let t = map.tile(tx, ty);
                if t.is_water() || t.trees == 0 {
                    continue;
                }
                let h0 = tile_hash(tx, ty, 0x7EE5);
                let Some(stem) = tree_bank_for(t.flags2 & 0xf, h0) else {
                    continue;
                };
                let bank = plan.bank(BankSpec {
                    kind: BankKind::Tree,
                    stem: stem.to_string(),
                    shade: NATURAL_SHADE,
                });
                let n = (t.trees as u32 & 0x1f).div_ceil(3).clamp(1, 8);
                for i in 0..n {
                    let h = tile_hash(tx, ty, 0x7EE5 + 1 + i);
                    let fx = 0.12 + 0.76 * ((h & 0xffff) as f32 / 65535.0);
                    let fy = 0.12 + 0.76 * ((h >> 16) as f32 / 65535.0);
                    let elev = surface_elevation(map, tx, ty, fx, fy);
                    plan.placements.push(Placement {
                        bank,
                        variant: h0.wrapping_add(i * 7),
                        pos: [tx as f32 + fx, elev, ty as f32 + fy],
                    });
                }
            }
        }
        if let Some(save) = save {
            // Map objects (`0x574c74`): byte 6 = building type, i16 x/y at +0x22/+0x24
            // (`map-gam.md` 5.3, decoded by comparing the pool with the stations and houses).
            for r in &save.objects.records {
                let d = &r.data;
                if d.len() < 0x26 {
                    continue;
                }
                let kind_idx = d[6] as usize;
                let x = i16::from_le_bytes([d[0x22], d[0x23]]);
                let y = i16::from_le_bytes([d[0x24], d[0x25]]);
                if x < 0 || y < 0 || x as usize >= map.width || y as usize >= map.height {
                    continue;
                }
                let Some(name) = BUILDING_NAMES.get(kind_idx) else {
                    continue;
                };
                let (kind, stem) = if kind_idx < 24 {
                    let mut s: Vec<char> = name.chars().collect();
                    s[2] = STATION_STYLE;
                    (BankKind::Station, s.into_iter().collect::<String>())
                } else {
                    (BankKind::Building, name.to_string())
                };
                let bank = plan.bank(BankSpec {
                    kind,
                    stem,
                    shade: 0,
                });
                let (tx, ty) = (x as usize, y as usize);
                plan.placements.push(Placement {
                    bank,
                    variant: 0,
                    pos: [
                        tx as f32 + 0.5,
                        surface_elevation(map, tx, ty, 0.5, 0.5),
                        ty as f32 + 0.5,
                    ],
                });
            }
        }
        plan
    }

    /// Decode, bake and pack the sprites for a view rotation. Banks that do not exist for this
    /// rotation are skipped silently.
    pub fn build_rotation(
        &self,
        assets: &GameAssets,
        rotation: u8,
        height_scale: f32,
    ) -> RotationSprites {
        // (bank, record) -> image index.
        let mut images: Vec<RgbaImage> = Vec::new();
        let mut lookup: Vec<Vec<Option<usize>>> = Vec::with_capacity(self.banks.len());
        for spec in &self.banks {
            let loaded = spec
                .file_names(rotation)
                .iter()
                .find_map(|n| LoadedBank::open(assets, n));
            let mut per_record = Vec::new();
            if let Some(b) = &loaded {
                for r in 0..b.record_count() {
                    per_record.push(b.image(r, 0, 0, spec.shade).map(|img| {
                        images.push(img);
                        images.len() - 1
                    }));
                }
            }
            lookup.push(per_record);
        }
        let atlas = bake_atlas(&images);
        let mut instances = Vec::with_capacity(self.placements.len());
        for p in &self.placements {
            let records = &lookup[p.bank as usize];
            if records.is_empty() {
                continue;
            }
            let record = if self.banks[p.bank as usize].kind == BankKind::Tree {
                p.variant as usize % records.len()
            } else {
                0
            };
            let Some(entry) = records[record].and_then(|i| atlas.entries[i]) else {
                continue;
            };
            instances.push(Instance {
                pos: [p.pos[0], p.pos[1] * height_scale, p.pos[2]],
                shade: 1.0,
                rect: entry.rect,
                uv: entry.uv,
            });
        }
        RotationSprites { atlas, instances }
    }
}

/// Everything needed to draw one map.
pub struct Scene {
    pub assets: Arc<GameAssets>,
    pub mesh: TerrainMesh,
    pub textures: TerrainTextures,
    pub plan: SpritePlan,
    /// Map size in tiles.
    pub size: (usize, usize),
    /// World height per elevation unit.
    pub height_scale: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneOptions {
    pub sprites: bool,
}

impl Default for SceneOptions {
    fn default() -> Self {
        SceneOptions { sprites: true }
    }
}

impl Scene {
    pub fn build(
        assets: Arc<GameAssets>,
        map: &Map,
        save: Option<&Save>,
        options: SceneOptions,
    ) -> Scene {
        let scale = height_scale();
        let textures = TerrainTextures::load(&assets);
        let ground_layers = textures.ground.len() as u16;
        let mut looks = Vec::with_capacity(map.tiles.len());
        for y in 0..map.height {
            for x in 0..map.width {
                let t = map.tile(x, y);
                let track = save.and_then(|s| track_layer(s, t.track_id));
                let look = if t.is_water() {
                    TileLook {
                        ground: None,
                        water: Some(water_variant(t)),
                        track,
                        turns: 0,
                    }
                } else {
                    TileLook {
                        ground: Some(ground_texture(t).min(ground_layers.saturating_sub(1))),
                        water: None,
                        track,
                        turns: texture_turns(t),
                    }
                };
                looks.push(look);
            }
        }
        let mesh = TerrainMesh::build(map, scale, &looks);
        let plan = if options.sprites {
            SpritePlan::build(map, save)
        } else {
            SpritePlan::default()
        };
        Scene {
            assets,
            mesh,
            textures,
            plan,
            size: (map.width, map.height),
            height_scale: scale,
        }
    }
}

/// Track-stack layer for a tile's `track_id`: the piece's edge mask (`+0`) picks the record, bit 0
/// of `+0x23` selects double track (`terrain-track.md` 1.3, `sprite-catalog.md` 4.2).
fn track_layer(save: &Save, track_id: u8) -> Option<u16> {
    if track_id == 0 {
        return None;
    }
    let piece = save.track_pieces.get(track_id as usize)?;
    let mask = *piece.data.first()?;
    let record = track_record(mask)? as usize;
    let double = piece.data.get(0x23).is_some_and(|b| b & 1 != 0);
    Some((record + if double { TRACK_DOUBLE_BASE } else { 0 }) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_table_matches_the_catalog() {
        assert_eq!(BUILDING_NAMES.len(), 122);
        assert_eq!(BUILDING_NAMES[8], "sta20");
        assert_eq!(BUILDING_NAMES[20], "sta34");
        assert_eq!(BUILDING_NAMES[25], "housa");
        assert_eq!(BUILDING_NAMES[38], "textl");
        assert_eq!(BUILDING_NAMES[57], "steel");
        assert_eq!(BUILDING_NAMES[84], "port_");
        assert_eq!(BUILDING_NAMES[121], "airps");
    }

    #[test]
    fn bank_names_follow_the_rotation_digit() {
        let b = BankSpec {
            kind: BankKind::Building,
            stem: "steel".into(),
            shade: 0,
        };
        assert_eq!(b.file_names(0), ["bn1steel.imb", "bl1steel.imb"]);
        assert_eq!(b.file_names(3)[0], "bn4steel.imb");
        let s = BankSpec {
            kind: BankKind::Station,
            stem: "stV20".into(),
            shade: 0,
        };
        assert_eq!(s.file_names(1), ["bl2stV20.imb"]);
        let t = BankSpec {
            kind: BankKind::Tree,
            stem: "treoak_".into(),
            shade: 22,
        };
        assert_eq!(t.file_names(0), ["treoak_1.imb"]);
    }
}
