//! Loads the game data tables from the original executable at run time.
//!
//! These tables are copyrighted game data and are NOT stored in the repository: the caller
//! provides the bytes of `RT2_PLAT.EXE` ([`Exe::from_bytes`]) and this module reads them from the
//! `.data` section and applies the fix-ups the game performs at start-up (`0x471b10`):
//! cargo `price = ftol(price * 1.4)`, `wait_scale = days * 1440`, and engine `+0x2e` = loaded
//! tons of engine plus tender.

use crate::calendar::{SPEED_LEVELS, SpeedTable};
use crate::cargo::{
    CARGO_COUNT, CARGO_RECORD_SIZE, CargoType, INDUSTRY_RECORD_SIZE, INDUSTRY_TYPES, IndustryType,
    RevenueTables,
};
use crate::exe::{Exe, ExeError};
use crate::finance::FinanceTables;
use crate::track::TrackTables;
use crate::trains::{
    ENGINE_COUNT, ENGINE_RECORD_SIZE, Engine, VEHICLE_COUNT, VEHICLE_RECORD_SIZE, VehicleType,
};

pub const ADDR_SPEED_TABLE: u32 = 0x0053_6508;
pub const ADDR_CARGO_TABLE: u32 = 0x0054_04b1;
pub const ADDR_INDUSTRY_TABLE: u32 = 0x0054_1ae0;
pub const ADDR_ENGINE_TABLE: u32 = 0x0053_cec0;
pub const ADDR_VEHICLE_TABLE: u32 = 0x0053_ddb8;
pub const ADDR_RELIABILITY: u32 = 0x0055_c52c;
pub const ADDR_FUEL_LEVEL: u32 = 0x0055_2ae8;
pub const ADDR_MAINT_LEVEL: u32 = 0x0055_2af4;
const ADDR_DEMAND_LEVEL: u32 = 0x0055_0524;
const ADDR_UNDEMANDED: u32 = 0x0055_050c;
pub const ADDR_ECON_PRODUCTION: u32 = 0x0055_0494;
const ADDR_ECON_REVENUE: u32 = 0x0055_04bc;
const ADDR_SLIDER_HUMAN: u32 = 0x0055_0448;
const ADDR_SLIDER_AI: u32 = 0x0055_041c;
/// `bonus_table` (`0x53a4b0`): i32 per place size class (world-tick.md 3.6).
const ADDR_TOWN_BONUS: u32 = 0x0053_a4b0;
const TOWN_BONUS_ENTRIES: usize = 8;

/// Everything the simulation reads from the executable.
#[derive(Clone, Debug)]
pub struct GameData {
    pub speed_table: SpeedTable,
    pub cargo: Vec<CargoType>,
    /// `[era set 0..2][type 0..53]` (cargo.md 7.1), as stored in the file (indices not rebuilt;
    /// call [`IndustryType::rebuild_inputs`] for the game year).
    pub industries: Vec<Vec<IndustryType>>,
    pub engines: Vec<Engine>,
    pub vehicles: Vec<VehicleType>,
    /// `reliability_rate` (`0x55c52c`) f32[11].
    pub reliability_rate: [f32; 11],
    /// `0x552ae8` f32[3], `0x552af4` f32[3].
    pub fuel_levels: [f32; 3],
    pub maintenance_levels: [f32; 3],
    pub revenue: RevenueTables,
    /// `0x550494` f32[5]: production multiplier by economy state (cargo.md 7.3).
    pub production_econ: [f32; 5],
    /// Track cost and station tables (terrain-track.md 2.2, 3.3).
    pub track: TrackTables,
    /// Stock economy multiplier and company-age price weights (finance.md 3.1).
    pub finance: FinanceTables,
    /// Town bonus offer per place size class (`0x53a4b0`, world-tick.md 3.6).
    pub town_bonus: Vec<i32>,
    /// The `.rdata` and initialized `.data` of the executable, addressed by VA, with the start-up
    /// fix-ups of the engine and vehicle tables applied (`0x471b10`). The train step
    /// (`crate::trainstep`) reads its tables from here at the original addresses.
    pub image: DataImage,
    /// Frame count of each sequence of the map animation bank `SMOKE.imb` (`DAT_00567a1c`),
    /// `groups[header[3]].k` of each record (trains.md 10.6). Empty until
    /// [`GameData::load_sprites`] runs; without it the animation step cannot expire objects.
    pub anim_frames: Vec<i32>,
    /// The 350-row scenario effect table (`0x550550`, scenarios.md 4.1).
    pub scenario_effects: crate::scenario::EffectTable,
    /// Town growth and building tables (world-growth.md 2.3, 2.4, 2.5).
    pub growth: crate::growth::GrowthTables,
}

/// First and last+1 VA kept in [`DataImage`]: `.rdata` (0x535000) and the initialized part of
/// `.data` (0x539000..0x565000).
pub const IMAGE_START: u32 = 0x0053_5000;
pub const IMAGE_END: u32 = 0x0056_5000;

/// Read-only copy of the executable's data sections, addressed by virtual address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataImage {
    bytes: Vec<u8>,
}

impl DataImage {
    pub fn load(exe: &Exe) -> Result<DataImage, ExeError> {
        let mut bytes = exe.read(IMAGE_START, 0x4000)?.to_vec();
        bytes.extend_from_slice(exe.read(0x0053_9000, (IMAGE_END - 0x0053_9000) as usize)?);
        let mut img = DataImage { bytes };
        img.engine_fixups();
        img.load_engine_year();
        Ok(img)
    }

    fn at(&self, va: u32, n: usize) -> &[u8] {
        let o = va
            .checked_sub(IMAGE_START)
            .expect("VA below the data image") as usize;
        &self.bytes[o..o + n]
    }

    fn at_mut(&mut self, va: u32, n: usize) -> &mut [u8] {
        let o = (va - IMAGE_START) as usize;
        &mut self.bytes[o..o + n]
    }

    pub fn u8(&self, va: u32) -> u8 {
        self.at(va, 1)[0]
    }

    pub fn i8(&self, va: u32) -> i8 {
        self.u8(va) as i8
    }

    pub fn u16(&self, va: u32) -> u16 {
        u16::from_le_bytes(self.at(va, 2).try_into().expect("2 bytes"))
    }

    pub fn i16(&self, va: u32) -> i16 {
        self.u16(va) as i16
    }

    pub fn u32(&self, va: u32) -> u32 {
        u32::from_le_bytes(self.at(va, 4).try_into().expect("4 bytes"))
    }

    pub fn i32(&self, va: u32) -> i32 {
        self.u32(va) as i32
    }

    pub fn f32(&self, va: u32) -> f32 {
        f32::from_bits(self.u32(va))
    }

    pub fn f64(&self, va: u32) -> f64 {
        f64::from_le_bytes(self.at(va, 8).try_into().expect("8 bytes"))
    }

    /// Every `game_load_file` (`0x477a60`, near its end) rewrites the first year of engine 3
    /// (`0x53cf6f`) and of its loco vehicle (`0x53ddd4 + 0x39 * [0x53cf9a]`): 1955 (0x7a3),
    /// or 1973 (0x7b5, the table value) when a network peer is older than 0x2333. A local game
    /// (`net_min_peer_version` = 0x2334) always takes 1955 (trains.md 2.4).
    fn load_engine_year(&mut self) {
        let year: u16 = 0x7a3;
        let ev = ADDR_ENGINE_TABLE + 3 * ENGINE_RECORD_SIZE as u32;
        self.at_mut(ev + 1, 2).copy_from_slice(&year.to_le_bytes());
        let loco = u32::from(self.u8(ev + 0x2c));
        let v = ADDR_VEHICLE_TABLE + loco * VEHICLE_RECORD_SIZE as u32;
        self.at_mut(v + 0x1c, 2)
            .copy_from_slice(&year.to_le_bytes());
    }

    /// The part of `0x471b10` that rewrites the vehicle and engine tables: per vehicle type
    /// `t` (record `0x53ddb8 + t*0x39`), clear the bank pointers (`+0x08..+0x18`) and the bank
    /// reference count (`+0x22..+0x26`); an engine vehicle (`+0x2e == 0`) sets `engine.+0x2c =
    /// t`, a tender (`+0x2e == 1`) sets `engine.+0x2d = t` and `engine.+0x2b = 1`, and both copy
    /// the engine's years into `+0x1c`/`+0x1e`. Then `engine.+0x2e = loaded tons (+0x1a) of
    /// vehicle +0x2c, plus those of vehicle +0x2d when +0x2b != 0`.
    fn engine_fixups(&mut self) {
        let veh = |t: u32| ADDR_VEHICLE_TABLE + t * VEHICLE_RECORD_SIZE as u32;
        let eng = |e: i32| ADDR_ENGINE_TABLE.wrapping_add((e * ENGINE_RECORD_SIZE as i32) as u32);
        for t in 0..VEHICLE_COUNT as u32 {
            let v = veh(t);
            self.at_mut(v + 0x08, 0x10).fill(0);
            self.at_mut(v + 0x22, 4).fill(0);
            let species = self.u8(v + 0x2e);
            let e = i32::from(self.i8(v + 0x2f));
            if species > 1 {
                continue;
            }
            let ev = eng(e);
            if species == 0 {
                self.at_mut(ev + 0x2c, 1)[0] = t as u8;
            } else {
                self.at_mut(ev + 0x2d, 1)[0] = t as u8;
                self.at_mut(ev + 0x2b, 1)[0] = 1;
            }
            let first = self.u16(ev + 1);
            let last = self.u16(eng(i32::from(self.i8(v + 0x2f))) + 3);
            self.at_mut(v + 0x1c, 2)
                .copy_from_slice(&first.to_le_bytes());
            self.at_mut(v + 0x1e, 2)
                .copy_from_slice(&last.to_le_bytes());
        }
        for e in 0..ENGINE_COUNT as i32 {
            let ev = eng(e);
            let loco = u32::from(self.u8(ev + 0x2c));
            let mut tons = u32::from(self.u16(veh(loco) + 0x1a));
            if self.u8(ev + 0x2b) != 0 {
                let tender = u32::from(self.u8(ev + 0x2d));
                tons += u32::from(self.u16(veh(tender) + 0x1a));
            }
            self.at_mut(ev + 0x2e, 4)
                .copy_from_slice(&tons.to_le_bytes());
        }
    }
}

fn arr<const N: usize>(exe: &Exe, va: u32) -> Result<[f32; N], ExeError> {
    let v = exe.f32s(va, N)?;
    Ok(v.try_into().expect("length N"))
}

impl GameData {
    pub fn load(exe: &Exe) -> Result<GameData, ExeError> {
        let speed: [f32; SPEED_LEVELS] = arr(exe, ADDR_SPEED_TABLE)?;

        let cargo = (0..CARGO_COUNT)
            .map(|i| {
                let va = ADDR_CARGO_TABLE + (i * CARGO_RECORD_SIZE) as u32;
                Ok(CargoType::from_raw(exe.read(va, CARGO_RECORD_SIZE)?))
            })
            .collect::<Result<Vec<_>, ExeError>>()?;

        let industries = (0..2)
            .map(|set| {
                (0..INDUSTRY_TYPES)
                    .map(|t| {
                        let va = ADDR_INDUSTRY_TABLE
                            + ((set * INDUSTRY_TYPES + t) * INDUSTRY_RECORD_SIZE) as u32;
                        Ok(IndustryType::from_raw(exe.read(va, INDUSTRY_RECORD_SIZE)?))
                    })
                    .collect::<Result<Vec<_>, ExeError>>()
            })
            .collect::<Result<Vec<_>, ExeError>>()?;

        let vehicles = (0..VEHICLE_COUNT)
            .map(|i| {
                let va = ADDR_VEHICLE_TABLE + (i * VEHICLE_RECORD_SIZE) as u32;
                Ok(VehicleType::from_raw(exe.read(va, VEHICLE_RECORD_SIZE)?))
            })
            .collect::<Result<Vec<_>, ExeError>>()?;

        let mut engines = (0..ENGINE_COUNT)
            .map(|i| {
                let va = ADDR_ENGINE_TABLE + (i * ENGINE_RECORD_SIZE) as u32;
                Ok(Engine::from_raw(exe.read(va, ENGINE_RECORD_SIZE)?))
            })
            .collect::<Result<Vec<_>, ExeError>>()?;
        link_engine_weights(&mut engines, &vehicles);

        Ok(GameData {
            speed_table: SpeedTable(speed),
            cargo,
            industries,
            engines,
            vehicles,
            reliability_rate: arr(exe, ADDR_RELIABILITY)?,
            fuel_levels: arr(exe, ADDR_FUEL_LEVEL)?,
            maintenance_levels: arr(exe, ADDR_MAINT_LEVEL)?,
            revenue: RevenueTables {
                demand_level: arr(exe, ADDR_DEMAND_LEVEL)?,
                undemanded: arr(exe, ADDR_UNDEMANDED)?,
                econ_revenue: arr(exe, ADDR_ECON_REVENUE)?,
                slider_human: arr(exe, ADDR_SLIDER_HUMAN)?,
                slider_ai: arr(exe, ADDR_SLIDER_AI)?,
            },
            production_econ: arr(exe, ADDR_ECON_PRODUCTION)?,
            track: TrackTables::load(exe)?,
            finance: FinanceTables::load(exe)?,
            town_bonus: (0..TOWN_BONUS_ENTRIES)
                .map(|i| exe.u32_at(ADDR_TOWN_BONUS + 4 * i as u32).map(|v| v as i32))
                .collect::<Result<Vec<_>, ExeError>>()?,
            image: DataImage::load(exe)?,
            anim_frames: Vec::new(),
            scenario_effects: crate::scenario::EffectTable::load(exe)?,
            growth: crate::growth::GrowthTables::load(exe)?,
        })
    }

    /// Reads what the simulation needs from the PK2 archives of the install `game_dir`: the
    /// frame counts of the map animation bank `SMOKE.imb` ([`GameData::anim_frames`]). The
    /// archives are searched like the original's item lookup, patches first.
    pub fn load_sprites(&mut self, game_dir: &std::path::Path) -> Result<(), String> {
        const ARCHIVES: [&str; 4] = ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"];
        for name in ARCHIVES {
            let Ok(raw) = std::fs::read(game_dir.join("data").join(name)) else {
                continue;
            };
            let pk2 = rt2_formats::pk2::Pk2::parse(&raw).map_err(|e| e.to_string())?;
            let Some(entry) = pk2.find_name("SMOKE.imb") else {
                continue;
            };
            let lad =
                rt2_formats::lad::Lad::parse(pk2.contents(entry)).map_err(|e| e.to_string())?;
            self.anim_frames = lad
                .records
                .iter()
                .map(|r| {
                    let g = r.header[3] as usize;
                    r.groups.get(g).map_or(0, |g| g.map_len as i32)
                })
                .collect();
            return Ok(());
        }
        Err("SMOKE.imb not found in the PK2 archives".into())
    }
}

/// What `0x471b10` does at map start: engine `+0x2e` = loaded tons of the engine vehicle plus its
/// tender (trains.md 2.1). When several vehicles claim an engine the last one wins.
pub fn link_engine_weights(engines: &mut [Engine], vehicles: &[VehicleType]) {
    let mut links: Vec<(Option<usize>, Option<usize>)> = vec![(None, None); engines.len()];
    for (t, v) in vehicles.iter().enumerate() {
        let Ok(e) = usize::try_from(v.engine) else {
            continue;
        };
        let Some(slot) = links.get_mut(e) else {
            continue;
        };
        match v.species {
            0 => slot.0 = Some(t),
            1 => slot.1 = Some(t),
            _ => {}
        }
    }
    for (eng, (loco, tender)) in engines.iter_mut().zip(links) {
        let tons = |t: Option<usize>| t.map_or(0, |t| i32::from(vehicles[t].loaded_tons));
        // an engine without a vehicle record uses vehicle type 0, as the original does
        let loco_tons = loco.map_or_else(|| i32::from(vehicles[0].loaded_tons), |t| tons(Some(t)));
        eng.base_tons = loco_tons + tons(tender);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_weights_are_loco_plus_tender() {
        let mk = |species, engine, loaded| VehicleType {
            empty_tons: 0,
            loaded_tons: loaded,
            species,
            engine,
            cargo: 0x7f,
        };
        let vehicles = vec![mk(0, 0, 40), mk(1, 0, 20), mk(2, -1, 30), mk(0, 1, 90)];
        let mut engines = vec![Engine::from_raw(&[0u8; ENGINE_RECORD_SIZE]); 2];
        link_engine_weights(&mut engines, &vehicles);
        assert_eq!(engines[0].base_tons, 60);
        assert_eq!(engines[1].base_tons, 90);
    }

    /// Loads the real tables when the executable is available (RT2_EXE or re/bin/RT2_PLAT.EXE).
    #[test]
    fn loads_from_the_original_executable() {
        let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
        });
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipping: {path} not found");
            return;
        };
        let data = GameData::load(&Exe::from_bytes(bytes).unwrap()).unwrap();
        // time.md section 3
        assert_eq!(
            data.speed_table.0,
            [0.0, 0.3, 0.6, 1.0, 1.7, 2.1, 3.0, 5.0, 8.0, 12.0, 18.0]
        );
        // cargo.md section 3: goods 58799, paper 64399, uranium 60199, passenger 78400
        assert_eq!(data.cargo[7].price, 58799);
        assert_eq!(data.cargo[13].price, 64399);
        assert_eq!(data.cargo[24].price, 60199);
        assert_eq!(data.cargo[0].price, 78400);
        assert_eq!(data.cargo[0].wait_scale, 720_000);
        // trains.md 2.2: reliability classes 1/400 ... 1/24000
        assert_eq!(data.reliability_rate[0], 1.0 / 400.0);
        assert_eq!(data.reliability_rate[10], 1.0 / 24000.0);
        // trains.md 6.2
        assert_eq!(data.fuel_levels, [1.15, 1.0, 0.85]);
        assert_eq!(data.maintenance_levels, [1.05, 1.0, 0.95]);
        // cargo.md 4.4
        assert_eq!(data.revenue.econ_revenue, [0.8, 0.9, 1.0, 1.1, 1.2]);
    }
}
