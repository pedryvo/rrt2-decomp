//! TOML export and import for game data mods (docs/PLAN.md section 12).
//!
//! Allows exporting the core simulation tables (engines, vehicles, cargo,
//! buildings, effects, etc.) to a documented TOML format, and applying mod
//! packages (either standalone TOML strings or directories of `.toml` files in
//! alphabetical order) over the default tables loaded from the executable.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::data::GameData;

/// Root mod container representing game data in TOML format.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ModData {
    #[serde(default)]
    pub meta: ModMetadata,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engines: Vec<ModEngine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vehicles: Vec<ModVehicle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cargo: Vec<ModCargo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buildings: Vec<ModBuilding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<ModEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reliability_rate: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_levels: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance_levels: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ModMetadata {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ModEngine {
    pub id: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_year: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_year: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reliability_class: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_mph: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accel: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_tons: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_tons: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ModVehicle {
    pub id: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_tons: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_tons: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub species: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<i8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ModCargo {
    pub id: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_scale: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ship: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_year: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub towns_until: Option<i16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cities_until: Option<i16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demand_per_house: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_bit: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ModBuilding {
    pub id: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<[i8; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Vec<bool>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ModEffect {
    pub id: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_company: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_player: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_territory: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instant: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_lng: Option<u16>,
}

impl GameData {
    /// Exports game data to a formatted TOML string.
    pub fn to_toml(&self) -> String {
        let meta = ModMetadata {
            title: "Railroad Tycoon II - GameData Export".into(),
            version: "1.0".into(),
            description:
                "Exported game tables for engines, vehicles, cargo, buildings, and effects".into(),
            author: "rt2-sim".into(),
        };

        let engines = self
            .engines
            .iter()
            .enumerate()
            .map(|(id, eng)| ModEngine {
                id,
                name: None,
                enabled: Some(eng.enabled),
                first_year: Some(eng.first_year),
                last_year: Some(eng.last_year),
                reliability_class: Some(eng.reliability_class),
                price: Some(eng.price),
                maintenance: Some(eng.maintenance),
                top_mph: Some(eng.top_mph),
                power: Some(eng.power),
                accel: Some(eng.accel),
                free_tons: Some(eng.free_tons),
                fuel: Some(eng.fuel),
                kind: Some(eng.kind),
                base_tons: Some(eng.base_tons),
            })
            .collect();

        let vehicles = self
            .vehicles
            .iter()
            .enumerate()
            .map(|(id, veh)| ModVehicle {
                id,
                empty_tons: Some(veh.empty_tons),
                loaded_tons: Some(veh.loaded_tons),
                species: Some(veh.species),
                engine: Some(veh.engine),
                cargo: Some(veh.cargo),
            })
            .collect();

        let cargo = self
            .cargo
            .iter()
            .enumerate()
            .map(|(id, c)| ModCargo {
                id,
                price: Some(c.price),
                days: Some(c.days),
                wait_scale: Some(c.wait_scale),
                ship: Some(c.ship),
                first_year: Some(c.first_year),
                towns_until: Some(c.towns_until),
                cities_until: Some(c.cities_until),
                demand_per_house: Some(c.demand_per_house),
                storage_bit: Some(c.storage_bit),
            })
            .collect();

        let buildings = self
            .growth
            .types
            .iter()
            .enumerate()
            .map(|(id, b)| ModBuilding {
                id,
                weight: Some(b.weight),
                size: Some(b.size),
                first_year: Some(b.first_year),
                site: Some(b.site),
                allowed: Some(b.allowed.to_vec()),
            })
            .collect();

        let effects = self
            .scenario_effects
            .rows
            .iter()
            .enumerate()
            .map(|(id, eff)| ModEffect {
                id,
                ui_id: Some(eff.ui_id),
                visible: Some(eff.visible),
                needs_company: Some(eff.needs_company),
                needs_player: Some(eff.needs_player),
                needs_territory: Some(eff.needs_territory),
                instant: Some(eff.instant),
                min: Some(eff.min),
                max: Some(eff.max),
                name_lng: Some(eff.name_lng),
            })
            .collect();

        let mod_data = ModData {
            meta,
            engines,
            vehicles,
            cargo,
            buildings,
            effects,
            reliability_rate: Some(self.reliability_rate.to_vec()),
            fuel_levels: Some(self.fuel_levels),
            maintenance_levels: Some(self.maintenance_levels),
        };

        toml::to_string_pretty(&mod_data).expect("Serialization to TOML should succeed")
    }

    /// Applies a mod TOML string, overriding fields on the current [`GameData`].
    pub fn apply_mod_toml(&mut self, toml_str: &str) -> Result<(), String> {
        let mod_data: ModData = toml::from_str(toml_str).map_err(|e| e.to_string())?;

        let mut engines_or_vehicles_changed = false;

        for me in mod_data.engines {
            if me.id >= self.engines.len() {
                continue;
            }
            let eng = &mut self.engines[me.id];
            if let Some(v) = me.enabled {
                eng.enabled = v;
            }
            if let Some(v) = me.first_year {
                eng.first_year = v;
            }
            if let Some(v) = me.last_year {
                eng.last_year = v;
            }
            if let Some(v) = me.reliability_class {
                eng.reliability_class = v;
            }
            if let Some(v) = me.price {
                eng.price = v;
            }
            if let Some(v) = me.maintenance {
                eng.maintenance = v;
            }
            if let Some(v) = me.top_mph {
                eng.top_mph = v;
            }
            if let Some(v) = me.power {
                eng.power = v;
            }
            if let Some(v) = me.accel {
                eng.accel = v;
            }
            if let Some(v) = me.free_tons {
                eng.free_tons = v;
            }
            if let Some(v) = me.fuel {
                eng.fuel = v;
            }
            if let Some(v) = me.kind {
                eng.kind = v;
            }
            if let Some(v) = me.base_tons {
                eng.base_tons = v;
            }
            self.image.write_engine(me.id, eng);
            engines_or_vehicles_changed = true;
        }

        for mv in mod_data.vehicles {
            if mv.id >= self.vehicles.len() {
                continue;
            }
            let veh = &mut self.vehicles[mv.id];
            if let Some(v) = mv.empty_tons {
                veh.empty_tons = v;
            }
            if let Some(v) = mv.loaded_tons {
                veh.loaded_tons = v;
            }
            if let Some(v) = mv.species {
                veh.species = v;
            }
            if let Some(v) = mv.engine {
                veh.engine = v;
            }
            if let Some(v) = mv.cargo {
                veh.cargo = v;
            }
            self.image.write_vehicle(mv.id, veh);
            engines_or_vehicles_changed = true;
        }

        if engines_or_vehicles_changed {
            crate::data::link_engine_weights(&mut self.engines, &self.vehicles);
            for (id, eng) in self.engines.iter().enumerate() {
                self.image.write_engine(id, eng);
            }
        }

        for mc in mod_data.cargo {
            if mc.id >= self.cargo.len() {
                continue;
            }
            let c = &mut self.cargo[mc.id];
            if let Some(v) = mc.price {
                c.price = v;
            }
            if let Some(v) = mc.days {
                c.days = v;
                c.wait_scale = v.wrapping_mul(0x5a0);
            }
            if let Some(v) = mc.wait_scale {
                c.wait_scale = v;
            }
            if let Some(v) = mc.ship {
                c.ship = v;
            }
            if let Some(v) = mc.first_year {
                c.first_year = v;
            }
            if let Some(v) = mc.towns_until {
                c.towns_until = v;
            }
            if let Some(v) = mc.cities_until {
                c.cities_until = v;
            }
            if let Some(v) = mc.demand_per_house {
                c.demand_per_house = v;
            }
            if let Some(v) = mc.storage_bit {
                c.storage_bit = v;
            }
            self.image.write_cargo(mc.id, c);
        }

        for mb in mod_data.buildings {
            if mb.id >= self.growth.types.len() {
                continue;
            }
            let b = &mut self.growth.types[mb.id];
            if let Some(v) = mb.weight {
                b.weight = v;
            }
            if let Some(v) = mb.size {
                b.size = v;
            }
            if let Some(v) = mb.first_year {
                b.first_year = v;
            }
            if let Some(v) = mb.site {
                b.site = v;
            }
            if let Some(v) = mb.allowed {
                for (i, allowed) in v.into_iter().take(19).enumerate() {
                    b.allowed[i] = allowed;
                }
            }
            self.image.write_building_type(mb.id, b);
        }

        for meff in mod_data.effects {
            if meff.id >= self.scenario_effects.rows.len() {
                continue;
            }
            let eff = &mut self.scenario_effects.rows[meff.id];
            if let Some(v) = meff.ui_id {
                eff.ui_id = v;
            }
            if let Some(v) = meff.visible {
                eff.visible = v;
            }
            if let Some(v) = meff.needs_company {
                eff.needs_company = v;
            }
            if let Some(v) = meff.needs_player {
                eff.needs_player = v;
            }
            if let Some(v) = meff.needs_territory {
                eff.needs_territory = v;
            }
            if let Some(v) = meff.instant {
                eff.instant = v;
            }
            if let Some(v) = meff.min {
                eff.min = v;
            }
            if let Some(v) = meff.max {
                eff.max = v;
            }
            if let Some(v) = meff.name_lng {
                eff.name_lng = v;
            }
            self.image.write_effect_row(meff.id, eff);
        }

        if let Some(rates) = mod_data.reliability_rate {
            for (i, &r) in rates.iter().take(11).enumerate() {
                self.reliability_rate[i] = r;
                self.image
                    .write_f32(crate::data::ADDR_RELIABILITY + (i * 4) as u32, r);
            }
        }

        if let Some(fuels) = mod_data.fuel_levels {
            self.fuel_levels = fuels;
            for (i, &f) in fuels.iter().enumerate() {
                self.image
                    .write_f32(crate::data::ADDR_FUEL_LEVEL + (i * 4) as u32, f);
            }
        }

        if let Some(maints) = mod_data.maintenance_levels {
            self.maintenance_levels = maints;
            for (i, &m) in maints.iter().enumerate() {
                self.image
                    .write_f32(crate::data::ADDR_MAINT_LEVEL + (i * 4) as u32, m);
            }
        }

        Ok(())
    }

    /// Applies all `.toml` mod files found in `dir` in alphabetical order.
    pub fn apply_mod_dir(&mut self, dir: &Path) -> Result<usize, String> {
        if !dir.is_dir() {
            return Err(format!("'{}' is not a directory", dir.display()));
        }
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                paths.push(path);
            }
        }
        paths.sort();
        let mut count = 0;
        for path in paths {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            self.apply_mod_toml(&content)
                .map_err(|e| format!("Failed to apply mod {}: {e}", path.display()))?;
            count += 1;
        }
        Ok(count)
    }
}
