//! Unit tests for GameData mods (TOML export/import/directories)
//! and fleet management (ConsistTemplate, bulk_replace_engines).

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_sim::World;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::fleet::ConsistTemplate;
use rt2_sim::trains::{SpeedMods, engine_max_speed};

fn setup_data() -> Option<GameData> {
    let game = std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/pedryvo/Games/railroad-tycoon-2"));
    let exe_bytes = std::fs::read(game.join("RT2_PLAT.EXE"))
        .or_else(|_| {
            std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../re/bin/RT2_PLAT.EXE"),
            )
        })
        .ok()?;
    let exe = Exe::from_bytes(exe_bytes).ok()?;
    let mut data = GameData::load(&exe).ok()?;
    let _ = data.load_sprites(&game);
    Some(data)
}

fn setup_world() -> Option<(GameData, Save)> {
    let game = std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/pedryvo/Games/railroad-tycoon-2"));
    let fid = std::env::var_os("RT2_FIDELITY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("/home/pedryvo/Decomp/railroad/target/wine-ts/rt2sb/games")
        });
    let raw = std::fs::read(fid.join("base_ex.GAM"))
        .or_else(|_| {
            std::fs::read("/home/pedryvo/Decomp/railroad/target/wine-ts/rt2sb/games/base_ex.GAM")
        })
        .ok()?;
    let exe_bytes = std::fs::read(game.join("RT2_PLAT.EXE"))
        .or_else(|_| {
            std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../re/bin/RT2_PLAT.EXE"),
            )
        })
        .ok()?;
    let exe = Exe::from_bytes(exe_bytes).ok()?;
    let mut data = GameData::load(&exe).ok()?;
    let _ = data.load_sprites(&game);
    Some((data, Save::parse(&raw).ok()?))
}

#[test]
fn mod_toml_round_trip() {
    let Some(data) = setup_data() else {
        eprintln!("RT2_DIR not available; skipping");
        return;
    };

    let toml_str = data.to_toml();
    assert!(!toml_str.is_empty());
    assert!(toml_str.contains("[meta]"));
    assert!(toml_str.contains("[[engines]]"));
    assert!(toml_str.contains("[[vehicles]]"));
    assert!(toml_str.contains("[[cargo]]"));
    assert!(toml_str.contains("[[buildings]]"));
    assert!(toml_str.contains("[[effects]]"));

    let mut data2 = data.clone();
    let res = data2.apply_mod_toml(&toml_str);
    assert!(res.is_ok(), "apply_mod_toml should succeed: {:?}", res);

    assert_eq!(data.engines, data2.engines);
    assert_eq!(data.vehicles, data2.vehicles);
    assert_eq!(data.cargo, data2.cargo);
    assert_eq!(data.growth.types, data2.growth.types);
    assert_eq!(data.scenario_effects, data2.scenario_effects);
    assert_eq!(data.reliability_rate, data2.reliability_rate);
    assert_eq!(data.fuel_levels, data2.fuel_levels);
    assert_eq!(data.maintenance_levels, data2.maintenance_levels);
    // Re-applying the exported TOML is idempotent on all data structures and image
    let mut data3 = data2.clone();
    let res3 = data3.apply_mod_toml(&toml_str);
    assert!(res3.is_ok());
    assert_eq!(data2.engines, data3.engines);
    assert_eq!(data2.vehicles, data3.vehicles);
    assert_eq!(data2.cargo, data3.cargo);
    assert_eq!(data2.growth.types, data3.growth.types);
    assert_eq!(data2.scenario_effects, data3.scenario_effects);
    assert_eq!(data2.reliability_rate, data3.reliability_rate);
    assert_eq!(data2.fuel_levels, data3.fuel_levels);
    assert_eq!(data2.maintenance_levels, data3.maintenance_levels);
    assert_eq!(data2.image, data3.image);
}

#[test]
fn partial_mod_overrides_engine_and_alters_simulation() {
    let Some(mut data) = setup_data() else {
        eprintln!("RT2_DIR not available; skipping");
        return;
    };

    let original_eng0 = data.engines[0];
    let original_eng1 = data.engines[1];
    let default_speed = engine_max_speed(&original_eng0, 200, 0.0, false, 0, &SpeedMods::NEUTRAL);

    let partial_toml = r#"
[[engines]]
id = 0
price = 123456
top_mph = 150
power = 12
"#;

    let res = data.apply_mod_toml(partial_toml);
    assert!(res.is_ok());

    // Check high-level struct
    assert_eq!(data.engines[0].price, 123456);
    assert_eq!(data.engines[0].top_mph, 150);
    assert_eq!(data.engines[0].power, 12);
    // Other engines unchanged
    assert_eq!(data.engines[1], original_eng1);

    // Check DataImage low-level image
    assert_eq!(data.image.i32(0x0053_cec0 + 0x0c), 123456);
    assert_eq!(data.image.i32(0x0053_cec0 + 0x14), 150);
    assert_eq!(data.image.i32(0x0053_cec0 + 0x18), 12);

    // Check simulation speed
    let modded_speed = engine_max_speed(&data.engines[0], 200, 0.0, false, 0, &SpeedMods::NEUTRAL);
    assert!(
        modded_speed > default_speed * 2,
        "modded speed ({modded_speed}) should be much higher than default ({default_speed})"
    );
}

#[test]
fn apply_mod_dir_in_alphabetical_order() {
    let Some(mut data) = setup_data() else {
        eprintln!("RT2_DIR not available; skipping");
        return;
    };

    let tmp_dir = std::env::temp_dir().join(format!("rt2_mod_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let mod1 = r#"
[[engines]]
id = 2
price = 50000
"#;
    let mod2 = r#"
[[engines]]
id = 2
price = 99999
"#;

    std::fs::write(tmp_dir.join("01_mod.toml"), mod1).unwrap();
    std::fs::write(tmp_dir.join("02_mod.toml"), mod2).unwrap();
    std::fs::write(tmp_dir.join("ignore_me.txt"), "not a toml file").unwrap();

    let count = data.apply_mod_dir(&tmp_dir).unwrap();
    assert_eq!(count, 2);
    // 02_mod.toml should overwrite 01_mod.toml
    assert_eq!(data.engines[2].price, 99999);

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn bulk_replace_engines_and_consist_template() {
    let Some((data, save)) = setup_world() else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not available; skipping");
        return;
    };

    let mut w = World::from_save(&save, &data);
    let company = 5u8;
    let initial_cash = w.company_cash(company);
    assert!(initial_cash > 0.0, "initial cash should be positive");

    // 1. Bulk replace engines for company 5 to engine 2
    let replaced = w.bulk_replace_engines(company, None, 2).unwrap();
    assert!(
        replaced > 0,
        "should have replaced at least one train engine"
    );

    let cash_after = w.company_cash(company);
    assert!(
        cash_after < initial_cash,
        "cash after ({cash_after}) should be lower than initial ({initial_cash})"
    );

    // Replacing again to engine 2 should do nothing (already engine 2)
    let replaced_again = w.bulk_replace_engines(company, None, 2).unwrap();
    assert_eq!(replaced_again, 0);

    // Replacing with non-matching from_engine does nothing
    let replaced_filtered = w.bulk_replace_engines(company, Some(99), 3).unwrap();
    assert_eq!(replaced_filtered, 0);

    // 2. Consist Templates
    let tpl = ConsistTemplate {
        name: "Express Passenger".into(),
        cars: vec![0, 0, 0, 1, 1], // 3 passenger, 2 mail
    };
    let train_id = 1u16;
    let res = w.apply_consist_template(train_id, 0, &tpl.cars);
    assert!(
        res.is_ok(),
        "apply_consist_template should succeed: {:?}",
        res
    );

    // Verify stop 0 on train 1 has the cars
    let s = w.save_ref().unwrap();
    let extra = s
        .train_extras
        .iter()
        .find(|e| e.slot == train_id as usize)
        .unwrap();
    let car_count = extra.cars_raw[2];
    assert_eq!(car_count, 5);
    assert_eq!(&extra.cars_raw[4..9], &[0, 0, 0, 1, 1]);

    // Test consist template with 8 cars
    let tpl8 = ConsistTemplate {
        name: "Long Freight".into(),
        cars: vec![7, 7, 8, 8, 9, 9, 10, 10],
    };
    let res8 = w.apply_consist_template(train_id, 0xff, &tpl8.cars);
    assert!(res8.is_ok());

    let s2 = w.save_ref().unwrap();
    let extra2 = s2
        .train_extras
        .iter()
        .find(|e| e.slot == train_id as usize)
        .unwrap();
    let car_count2 = extra2.cars_raw[2];
    assert_eq!(car_count2, 8);
    assert_eq!(&extra2.cars_raw[4..12], &[7, 7, 8, 8, 9, 9, 10, 10]);
}
