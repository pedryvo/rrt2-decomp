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
        })
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
