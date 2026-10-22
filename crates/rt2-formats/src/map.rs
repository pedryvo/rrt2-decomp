//! Scenario maps and saves (`maps/*.MAP`, `games/*.GAM`), see `docs/formats/map-gam.md`.
//!
//! `Map::parse` reads the terrain; `Save::parse` (version 0x233C files) also reads the date, the
//! seed and the entity pools after the elevation grid. Layout, all little-endian:
//!
//! ```text
//! 0x000  u32 0x233C            magic/version
//! 0x004  char[]                short name, NUL padded
//! 0x131  char[]                scenario title (medium confidence)
//! 0x231  u16 w, u16 h          map size in tiles
//! 0xB2D7 w*h * [u8; 10]        tile records, row by row (y*w + x), north at the top
//! next   (w+1)*(h+1) * i16     corner elevations, row by row
//! ```

/// Magic/version word at offset 0.
pub const MAGIC: u32 = 0x233C;
const NAME_OFFSET: usize = 0x004;
const TITLE_OFFSET: usize = 0x131;
const TITLE_END: usize = 0x150;
const SIZE_OFFSET: usize = 0x231;
/// Start of the tile records (fixed: the header has a fixed size).
pub const TILES_OFFSET: usize = 0xB2D7;
pub const TILE_SIZE: usize = 10;
/// Ground ids 0x61 and 0x62 are water (ocean, lakes, bays); high confidence.
pub const GROUND_WATER: [u8; 2] = [0x61, 0x62];
/// Sanity limit; the largest original map is 500x500.
const MAX_DIM: usize = 4096;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MapError {
    #[error("file is {len} bytes, too short for {what}")]
    Truncated { len: usize, what: &'static str },
    #[error("bad magic {0:#x}, expected 0x233c")]
    BadMagic(u32),
    #[error("implausible map size {width}x{height}")]
    BadSize { width: usize, height: usize },
}

/// One 10-byte tile record. Field names and confidence follow `docs/formats/map-gam.md` section 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    /// Byte 0: texture variant/rotation noise (0/0x40/0x80/0xC0 plus a low nibble). Low confidence.
    pub var: u8,
    /// Byte 1: ground type id (0x00..=0x62). 0x61/0x62 are water (high confidence); 0x00 outlines
    /// coasts; the rest is only a guess (vegetation/soil/rock).
    pub ground: u8,
    /// Byte 2: unknown flags (bit 6 on most land tiles). Low confidence.
    pub flags2: u8,
    /// Byte 3: tree density, 0..=28 (medium confidence).
    pub trees: u8,
    /// Byte 4: 0 or a growing id of a building (town/industry) in a table. Medium confidence.
    pub struct_id: u8,
    /// Byte 5: always 0 in all original files.
    pub reserved5: u8,
    /// Byte 6: unknown bitfield (bits 3 and 4 always set on water; coast/river?). Low confidence.
    pub flags6: u8,
    /// Byte 7: 0 or an id of pre-built track. Medium confidence.
    pub track_id: u8,
    /// Byte 8: always 0 in all original files.
    pub reserved8: u8,
    /// Byte 9: territory/region number, 1..=N (high confidence).
    pub region: u8,
}

impl Tile {
    fn from_bytes(b: &[u8; TILE_SIZE]) -> Self {
        Tile {
            var: b[0],
            ground: b[1],
            flags2: b[2],
            trees: b[3],
            struct_id: b[4],
            reserved5: b[5],
            flags6: b[6],
            track_id: b[7],
            reserved8: b[8],
            region: b[9],
        }
    }

    pub fn is_water(&self) -> bool {
        GROUND_WATER.contains(&self.ground)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    /// Short name at 0x004 (the 8.3 file name for the numbered scenarios).
    pub name: String,
    /// Scenario title at 0x131 (medium confidence; may be empty).
    pub title: String,
    pub width: usize,
    pub height: usize,
    /// `width * height` tiles, row-major.
    pub tiles: Vec<Tile>,
    /// `(width + 1) * (height + 1)` corner elevations, row-major. The sea floor can be negative.
    pub elevation: Vec<i16>,
}

fn c_string(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    // Latin-1 is a superset of the ASCII text actually seen.
    raw[..end].iter().map(|&b| b as char).collect()
}

impl Map {
    pub fn parse(raw: &[u8]) -> Result<Map, MapError> {
        if raw.len() < SIZE_OFFSET + 4 {
            return Err(MapError::Truncated {
                len: raw.len(),
                what: "the header",
            });
        }
        let magic = u32::from_le_bytes(raw[0..4].try_into().unwrap());
        if magic != MAGIC {
            return Err(MapError::BadMagic(magic));
        }
        let width = u16::from_le_bytes([raw[SIZE_OFFSET], raw[SIZE_OFFSET + 1]]) as usize;
        let height = u16::from_le_bytes([raw[SIZE_OFFSET + 2], raw[SIZE_OFFSET + 3]]) as usize;
        if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
            return Err(MapError::BadSize { width, height });
        }
        let tiles_end = TILES_OFFSET + TILE_SIZE * width * height;
        let corners = (width + 1) * (height + 1);
        let elev_end = tiles_end + 2 * corners;
        if raw.len() < elev_end {
            return Err(MapError::Truncated {
                len: raw.len(),
                what: "the tile and elevation grids",
            });
        }
        let tiles = raw[TILES_OFFSET..tiles_end]
            .as_chunks::<TILE_SIZE>()
            .0
            .iter()
            .map(Tile::from_bytes)
            .collect();
        let elevation = raw[tiles_end..elev_end]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect();
        Ok(Map {
            name: c_string(&raw[NAME_OFFSET..TITLE_OFFSET]),
            title: c_string(&raw[TITLE_OFFSET..TITLE_END]),
            width,
            height,
            tiles,
            elevation,
        })
    }

    pub fn tile(&self, x: usize, y: usize) -> &Tile {
        &self.tiles[y * self.width + x]
    }

    /// Elevation of the tile corner `(x, y)`, `x <= width`, `y <= height`.
    pub fn corner(&self, x: usize, y: usize) -> i16 {
        self.elevation[y * (self.width + 1) + x]
    }

    /// Fraction of tiles whose ground is water.
    pub fn water_fraction(&self) -> f64 {
        let water = self.tiles.iter().filter(|t| t.is_water()).count();
        water as f64 / self.tiles.len() as f64
    }
}

// ---------------------------------------------------------------------------------------------
// Game state after the elevation grid, and the blocks before the tiles (version 0x233C only).
//
// Source: the game's own loader `0x477a60` (reads) and saver `0x477540` (writes version 0x2348);
// see `docs/formats/map-gam.md` sections 2 and 5. The shipped `.MAP`/`.GAM` files all use the old
// version 0x233C, whose block sizes are the ones below. Newer files (`.MP2`, magic 0x2348) differ
// and are rejected by `Save::parse`.
// ---------------------------------------------------------------------------------------------

/// Size of the header block read into the game's globals at `0x567b38` (version word included).
pub const HEADER_LEN: usize = 0x8793;
/// File offset of the 0x40-byte camera/view block (globals `0x577b98..0x577bd8`).
pub const VIEW_OFFSET: usize = 0x8819;
/// File offset of the old (0x233C) global game state block, `0x29C8` bytes.
pub const STATE_OFFSET: usize = 0x8859;
pub const STATE_LEN: usize = 0x29C8;
/// File offset of the 0xB6-byte block just before the tiles (globals `0x574840..0x5748f6`).
pub const TRAILER_OFFSET: usize = 0xB221;
/// `game_seed` (`0x57486e`) is 0x2E bytes into the trailer block.
pub const GAME_SEED_OFFSET: usize = TRAILER_OFFSET + 0x2E;
/// Offsets inside the state block (they equal the offsets from `game_running` `0x5704e8`).
const STATE_MINUTES: usize = 0x0C;
const STATE_DATE: usize = 0x10;
const STATE_TIME: usize = 0x14;
const STATE_SPEED: usize = 0x26;

/// Calendar date (`game_date` `0x5704f8`): year u16, month u8 1..=12, day u8 1..=28.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

/// One live record of a slot pool (`FUN_004f1bc0` reader / `FUN_004f1a40` writer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolRecord {
    /// Slot index (>= 1; slot 0 is never used). Ids stored in other records refer to this.
    pub slot: usize,
    pub data: Vec<u8>,
}

/// A slot pool as serialized by the game: header, free bitmap, live records only, and for
/// linked pools the `next`/`prev` u16 arrays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool {
    /// Number of slots (including unused ones).
    pub slots: usize,
    /// Record size recorded in the file header.
    pub record_size: usize,
    pub linked: bool,
    /// Header fields `+0x00`, `+0x04`, `+0x0c`, `+0x14` (live count), `+0x18` (pool type id).
    pub field_00: i32,
    pub field_04: i32,
    pub field_0c: i32,
    pub count: usize,
    pub pool_type: i32,
    /// First and last slot of the linked list (0 when not linked or empty).
    pub head: u16,
    pub tail: u16,
    pub records: Vec<PoolRecord>,
    /// Live slots in list order (head to tail) for linked pools, slot order otherwise.
    pub order: Vec<usize>,
}

impl Pool {
    pub fn get(&self, slot: usize) -> Option<&PoolRecord> {
        self.records.iter().find(|r| r.slot == slot)
    }

    /// Records in list order (the game iterates linked pools this way).
    pub fn ordered(&self) -> impl Iterator<Item = &PoolRecord> {
        self.order.iter().filter_map(|&s| self.get(s))
    }
}

/// Place record (`DAT_00574c5c`, 0x96 bytes in the old format): cities and named regions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// Own slot id (u16 at +0).
    pub id: u16,
    /// Tile coordinates of the centre (+4, +6).
    pub x: u16,
    pub y: u16,
    /// Bounding box in tiles (+8 x0, +10 y0, +12 x1, +14 y1).
    pub bbox: [u16; 4],
    /// NUL-terminated name at +22 (32 bytes).
    pub name: String,
    /// The whole 0x96-byte record; fields beyond the above are not decoded.
    pub raw: Vec<u8>,
}

/// Company record (`DAT_00574c6c`, 0x196C bytes in the old format).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Company {
    pub slot: usize,
    /// u32 at +0 (equals the slot in all samples).
    pub id: u32,
    /// Name at +4 (up to 32 bytes).
    pub name: String,
    /// Founding year candidate: u32 at +0x148 (1870 in the Britain-based saves). Low confidence.
    pub year_148: u32,
    pub raw: Vec<u8>,
}

/// Person/player persona (`DAT_00574c68`, 0x4DC bytes): name and biography.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub slot: usize,
    pub name: String,
    /// Biography text at +0x27 (may be empty).
    pub bio: String,
    pub raw: Vec<u8>,
}

/// Train extras that follow the pools, in linked-list order (consist and route arrays).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainExtra {
    pub slot: usize,
    /// `count_3b` entries of 10 bytes (the old file format; memory uses 14).
    pub cars_raw: Vec<u8>,
    /// Route arrays (`+0x59` entries each: u16, u16, u8, u8, u8), empty when absent.
    pub route_raw: Vec<u8>,
}

/// Scenario/victory rule (`DAT_00574c9c`): condition source text, compiled tokens and messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Fixed 0x2C8-byte part (mostly pointers and flags, not decoded).
    pub fixed: Vec<u8>,
    /// Condition expression text, e.g. `GameYear=1855 and PlayerNetWorth(IsHighestInClass)`.
    pub condition: String,
    /// Token arrays, `fixed[4]` entries each.
    pub tokens_a: Vec<i32>,
    pub tokens_b: Vec<i32>,
    /// Message shown when the rule fires.
    pub message: String,
    pub text2: String,
    pub text3: String,
    /// Short title (e.g. `Gold`).
    pub title: String,
}

/// Everything in a version-0x233C file beyond the terrain grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Save {
    pub map: Map,
    /// Raw 0x29C8-byte global state block.
    pub state: Vec<u8>,
    /// `game_time_minutes` (`0x5704f4`): `((((year*12+month)*28+day-29)*24+hour)*60)+minute`.
    pub game_time_minutes: i32,
    pub date: GameDate,
    pub hour: u8,
    pub minute: u8,
    /// `game_speed` (`0x57050e`), 0..=10.
    pub game_speed: u8,
    /// `game_seed` (`0x57486e`): input of the position/seed hash used for terrain and decoration.
    pub game_seed: u32,
    /// Pools in file order: `0x574c54` structures, `c60` stations, `c68` persons, `c6c` companies,
    /// `c70`, `c74` map objects, `c78`, `c58`, `c7c`, `c80`, `c84` track pieces, `c64` trains,
    /// `c5c` places, `ca8` territories, `c88`.
    pub structures: Pool,
    pub stations: Pool,
    pub persons: Pool,
    pub companies: Pool,
    pub pool_70: Pool,
    pub objects: Pool,
    pub pool_78: Pool,
    pub pool_58: Pool,
    pub pool_7c: Pool,
    pub pool_80: Pool,
    pub track_pieces: Pool,
    pub trains: Pool,
    pub places: Pool,
    pub territories: Pool,
    pub pool_88: Pool,
    pub train_extras: Vec<TrainExtra>,
    pub rules: Vec<Rule>,
}

/// Record sizes passed to the pool reader by the loader for version 0x233C (0 = use the file's).
const POOL_SIZES: [usize; 15] = [
    0, 0x39B, 0x4DC, 0x196C, 0, 0, 0, 0, 0x11, 0, 0, 0x143, 0x96, 0x27A, 0,
];
const RULE_FIXED_LEN: usize = 0x2C8;

struct Reader<'a> {
    raw: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize, what: &'static str) -> Result<&'a [u8], MapError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.raw.len());
        let Some(end) = end else {
            return Err(MapError::Truncated {
                len: self.raw.len(),
                what,
            });
        };
        let out = &self.raw[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn i32(&mut self, what: &'static str) -> Result<i32, MapError> {
        Ok(i32::from_le_bytes(self.take(4, what)?.try_into().unwrap()))
    }

    fn len(&mut self, what: &'static str) -> Result<usize, MapError> {
        let v = self.i32(what)?;
        usize::try_from(v).map_err(|_| MapError::Truncated {
            len: self.raw.len(),
            what,
        })
    }

    fn pool(&mut self, record_size: usize) -> Result<Pool, MapError> {
        const WHAT: &str = "a slot pool";
        let slots = self.len(WHAT)?;
        let file_size = self.len(WHAT)?;
        let linked = self.i32(WHAT)? != 0;
        let field_00 = self.i32(WHAT)?;
        let field_04 = self.i32(WHAT)?;
        let field_0c = self.i32(WHAT)?;
        let count = self.len(WHAT)?;
        let pool_type = self.i32(WHAT)?;
        let head = u16::from_le_bytes(self.take(2, WHAT)?.try_into().unwrap());
        let tail = u16::from_le_bytes(self.take(2, WHAT)?.try_into().unwrap());
        if slots > 1 << 20 || file_size > 1 << 20 {
            return Err(MapError::Truncated {
                len: self.raw.len(),
                what: "a plausible slot pool",
            });
        }
        let bitmap = self.take((slots + 8) >> 3, WHAT)?;
        let size = if record_size != 0 {
            record_size
        } else {
            file_size
        };
        let mut records = Vec::with_capacity(count);
        for slot in 0..slots {
            // A set bit marks a free slot.
            if bitmap[slot >> 3] & (1 << (slot & 7)) == 0 {
                records.push(PoolRecord {
                    slot,
                    data: self.take(size, WHAT)?.to_vec(),
                });
            }
        }
        let mut order: Vec<usize> = records.iter().map(|r| r.slot).collect();
        if linked {
            let next = self.take(slots * 2, WHAT)?;
            self.take(slots * 2, WHAT)?; // prev array, derivable from next
            let mut walk = Vec::with_capacity(records.len());
            let mut cur = head as usize;
            while cur != 0 && cur < slots && walk.len() <= records.len() {
                walk.push(cur);
                cur = u16::from_le_bytes([next[cur * 2], next[cur * 2 + 1]]) as usize;
            }
            if walk.len() == records.len() {
                order = walk;
            }
        }
        Ok(Pool {
            slots,
            record_size: file_size,
            linked,
            field_00,
            field_04,
            field_0c,
            count,
            pool_type,
            head,
            tail,
            records,
            order,
        })
    }

    fn blob(&mut self, what: &'static str) -> Result<&'a [u8], MapError> {
        let n = self.len(what)?;
        self.take(n, what)
    }
}

fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}

impl Save {
    /// Parses a complete version-0x233C `.MAP`/`.GAM` file. The pools and rules must end exactly
    /// at the end of the file; trailing bytes are reported as an error.
    pub fn parse(raw: &[u8]) -> Result<Save, MapError> {
        let map = Map::parse(raw)?;
        let grids_end = TILES_OFFSET
            + TILE_SIZE * map.width * map.height
            + 2 * (map.width + 1) * (map.height + 1);
        if raw.len() < TRAILER_OFFSET + 0xB6 {
            return Err(MapError::Truncated {
                len: raw.len(),
                what: "the state blocks",
            });
        }
        let state = raw[STATE_OFFSET..STATE_OFFSET + STATE_LEN].to_vec();
        let game_time_minutes =
            i32::from_le_bytes(state[STATE_MINUTES..STATE_MINUTES + 4].try_into().unwrap());
        let date = GameDate {
            year: u16_at(&state, STATE_DATE),
            month: state[STATE_DATE + 2],
            day: state[STATE_DATE + 3],
        };
        let game_seed = u32::from_le_bytes(
            raw[GAME_SEED_OFFSET..GAME_SEED_OFFSET + 4]
                .try_into()
                .unwrap(),
        );

        let mut r = Reader {
            raw,
            pos: grids_end,
        };
        let mut pools = Vec::with_capacity(POOL_SIZES.len());
        for &size in &POOL_SIZES {
            pools.push(r.pool(size)?);
        }
        let mut it = pools.into_iter();
        let mut next = || it.next().expect("15 pools");
        let structures = next();
        let stations = next();
        let persons = next();
        let companies = next();
        let pool_70 = next();
        let objects = next();
        let pool_78 = next();
        let pool_58 = next();
        let pool_7c = next();
        let pool_80 = next();
        let track_pieces = next();
        let trains = next();
        let places = next();
        let territories = next();
        let pool_88 = next();

        let mut train_extras = Vec::new();
        for rec in trains.ordered() {
            let cars = rec.data[0x3B] as usize;
            let cars_raw = r.take(cars * 10, "train cars")?.to_vec();
            let n = i32::from_le_bytes(rec.data[0x59..0x5D].try_into().unwrap());
            let flag = i32::from_le_bytes(rec.data[0x65..0x69].try_into().unwrap());
            let route_raw = if n > 0 && flag != 0 {
                r.take(n as usize * 7, "train route")?.to_vec()
            } else {
                Vec::new()
            };
            train_extras.push(TrainExtra {
                slot: rec.slot,
                cars_raw,
                route_raw,
            });
        }

        let rule_count = r.len("the rule count")?;
        let mut rules = Vec::with_capacity(rule_count.min(4096));
        for _ in 0..rule_count {
            let fixed = r.take(RULE_FIXED_LEN, "a rule")?.to_vec();
            let tokens = i32::from_le_bytes(fixed[4..8].try_into().unwrap());
            let tokens = usize::try_from(tokens).map_err(|_| MapError::Truncated {
                len: raw.len(),
                what: "rule tokens",
            })?;
            let condition = c_string(r.blob("a rule condition")?);
            let ints = |r: &mut Reader| -> Result<Vec<i32>, MapError> {
                Ok(r.take(tokens * 4, "rule tokens")?
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| i32::from_le_bytes(*c))
                    .collect())
            };
            let tokens_a = ints(&mut r)?;
            let tokens_b = ints(&mut r)?;
            let message = c_string(r.blob("a rule message")?);
            let text2 = c_string(r.blob("a rule text")?);
            let text3 = c_string(r.blob("a rule text")?);
            let title = c_string(r.blob("a rule title")?);
            rules.push(Rule {
                fixed,
                condition,
                tokens_a,
                tokens_b,
                message,
                text2,
                text3,
                title,
            });
        }
        if r.pos != raw.len() {
            return Err(MapError::Truncated {
                len: raw.len(),
                what: "an exact end of file (trailing bytes after the rules)",
            });
        }

        Ok(Save {
            hour: state[STATE_TIME],
            minute: state[STATE_TIME + 1],
            game_speed: state[STATE_SPEED],
            map,
            state,
            game_time_minutes,
            date,
            game_seed,
            structures,
            stations,
            persons,
            companies,
            pool_70,
            objects,
            pool_78,
            pool_58,
            pool_7c,
            pool_80,
            track_pieces,
            trains,
            places,
            territories,
            pool_88,
            train_extras,
            rules,
        })
    }

    /// `date_to_minutes` (`0x4a4990`) of the stored calendar fields; equals `game_time_minutes`.
    pub fn minutes_from_date(&self) -> i64 {
        let d = &self.date;
        (((i64::from(d.year) * 12 + i64::from(d.month)) * 28 + i64::from(d.day) - 29) * 24
            + i64::from(self.hour))
            * 60
            + i64::from(self.minute)
    }

    /// Places (cities, regions) with names and coordinates, in list order.
    pub fn place_list(&self) -> Vec<Place> {
        self.places
            .ordered()
            .map(|r| {
                let d = &r.data;
                Place {
                    id: u16_at(d, 0),
                    x: u16_at(d, 4),
                    y: u16_at(d, 6),
                    bbox: [u16_at(d, 8), u16_at(d, 10), u16_at(d, 12), u16_at(d, 14)],
                    name: c_string(&d[22..54]),
                    raw: d.clone(),
                }
            })
            .collect()
    }

    pub fn company_list(&self) -> Vec<Company> {
        self.companies
            .ordered()
            .map(|r| {
                let d = &r.data;
                Company {
                    slot: r.slot,
                    id: u32::from_le_bytes(d[0..4].try_into().unwrap()),
                    name: c_string(&d[4..36]),
                    year_148: u32::from_le_bytes(d[0x148..0x14C].try_into().unwrap()),
                    raw: d.clone(),
                }
            })
            .collect()
    }

    pub fn person_list(&self) -> Vec<Person> {
        self.persons
            .ordered()
            .map(|r| {
                let d = &r.data;
                Person {
                    slot: r.slot,
                    name: c_string(&d[8..0x27]),
                    bio: c_string(&d[0x27..0x120]),
                    raw: d.clone(),
                }
            })
            .collect()
    }

    /// Station names (name at +26 of the 0x39B-byte record), in list order.
    pub fn station_names(&self) -> Vec<String> {
        self.stations
            .ordered()
            .map(|r| c_string(&r.data[26..58]))
            .collect()
    }

    /// Train names (name at +12 of the 0x143-byte record), in list order.
    pub fn train_names(&self) -> Vec<String> {
        self.trains
            .ordered()
            .map(|r| c_string(&r.data[12..44]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(w: usize, h: usize) -> Vec<u8> {
        let mut raw = vec![0u8; TILES_OFFSET];
        raw[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        raw[4..8].copy_from_slice(b"Test");
        raw[TITLE_OFFSET..TITLE_OFFSET + 5].copy_from_slice(b"Title");
        raw[SIZE_OFFSET..SIZE_OFFSET + 2].copy_from_slice(&(w as u16).to_le_bytes());
        raw[SIZE_OFFSET + 2..SIZE_OFFSET + 4].copy_from_slice(&(h as u16).to_le_bytes());
        for i in 0..w * h {
            let ground = if i == 1 { 0x61 } else { 0x2d };
            raw.extend_from_slice(&[0x40, ground, 0, 3, 0, 0, 0, 0, 0, 1 + i as u8]);
        }
        for i in 0..(w + 1) * (h + 1) {
            raw.extend_from_slice(&(i as i16 - 2).to_le_bytes());
        }
        raw.extend_from_slice(&[0xAA; 17]); // game-state tail is ignored
        raw
    }

    #[test]
    fn parses_synthetic_map() {
        let map = Map::parse(&synthetic(3, 2)).unwrap();
        assert_eq!((map.name.as_str(), map.title.as_str()), ("Test", "Title"));
        assert_eq!((map.width, map.height), (3, 2));
        assert_eq!(map.tiles.len(), 6);
        assert_eq!(map.tile(2, 1).region, 6);
        assert_eq!(map.tile(1, 0).ground, 0x61);
        assert!(map.tile(1, 0).is_water());
        assert_eq!(map.tile(0, 0).trees, 3);
        assert_eq!(map.elevation.len(), 12);
        assert_eq!(map.corner(0, 0), -2);
        assert_eq!(map.corner(3, 2), 9);
        assert!((map.water_fraction() - 1.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_bad_input() {
        let mut raw = synthetic(3, 2);
        assert!(matches!(
            Map::parse(&raw[..100]),
            Err(MapError::Truncated { .. })
        ));
        assert!(matches!(
            Map::parse(&raw[..TILES_OFFSET + 20]),
            Err(MapError::Truncated { .. })
        ));
        raw[SIZE_OFFSET] = 0;
        raw[SIZE_OFFSET + 1] = 0;
        assert!(matches!(Map::parse(&raw), Err(MapError::BadSize { .. })));
        raw[0] = 0;
        assert!(matches!(Map::parse(&raw), Err(MapError::BadMagic(_))));
    }
}
