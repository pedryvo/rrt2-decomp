//! Single-player build undo (PLAN.md section 12, `World::undo_last_build`)
//! and company info modification (cmd 0x27, `World::set_company_info`).

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_sim::build::cmd::BuildCmd;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::pool;
use rt2_sim::{Command, World};

const CO_CASH: usize = 0x4766;
const CO_NAME: usize = 0x04;
const CO_LOGO: usize = 0x2b;

fn setup() -> Option<(GameData, Save)> {
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

fn company_cash(w: &World, co: i32) -> f64 {
    let s = w.save_ref().unwrap();
    let rec = pool::record(&s.companies, co as usize).unwrap();
    pool::f64_at(&rec.data, CO_CASH)
}

fn company_name(w: &World, co: i32) -> String {
    let s = w.save_ref().unwrap();
    let rec = pool::record(&s.companies, co as usize).unwrap();
    let end = rec.data[CO_NAME..CO_NAME + 31]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(31);
    String::from_utf8_lossy(&rec.data[CO_NAME..CO_NAME + end]).to_string()
}

fn company_logo(w: &World, co: i32) -> u8 {
    let s = w.save_ref().unwrap();
    let rec = pool::record(&s.companies, co as usize).unwrap();
    rec.data[CO_LOGO]
}

#[test]
fn undo_last_build_restores_state() {
    let Some((data, save)) = setup() else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let mut w = World::from_save(&save, &data);
    w.apply(Command::SeedRng(1234));
    let initial_save = w.to_save();
    let co = 5;
    let initial_cash = company_cash(&w, co);

    // Initial state: nothing to undo
    assert!(!w.can_undo_last_build());
    assert!(!w.undo_last_build());

    // 1. Build a station (command 0x17)
    let cmd = BuildCmd::Build {
        company: co,
        x: 170,
        y: 232,
        shape: 4,
        hint: -1,
    };
    let res = w.apply_build(cmd);
    assert!(res.is_ok(), "station build succeeds");
    let after_build_cash = company_cash(&w, co);
    assert!(
        after_build_cash < initial_cash,
        "cash was deducted for station"
    );
    assert!(w.can_undo_last_build(), "undo is now available");

    // 2. Undo the station build
    assert!(w.undo_last_build(), "undo succeeds");
    assert!(!w.can_undo_last_build(), "undo is consumed");
    assert_eq!(company_cash(&w, co), initial_cash, "cash is fully restored");
    assert_eq!(
        w.to_save(),
        initial_save,
        "world save is restored to initial state"
    );

    // 3. Time advance clears undo snapshot
    let res2 = w.apply_build(cmd);
    assert!(res2.is_ok());
    assert!(w.can_undo_last_build());
    w.advance(320); // time moves forward
    assert!(
        !w.can_undo_last_build(),
        "time advance clears undo snapshot"
    );
    assert!(!w.undo_last_build(), "cannot undo after time advanced");

    // 4. In multiplayer, undo is disabled
    let players = vec![
        rt2_sim::net::NetPlayer {
            id: 1,
            identity: 51,
            name: "p1".into(),
        },
        rt2_sim::net::NetPlayer {
            id: 2,
            identity: 52,
            name: "p2".into(),
        },
    ];
    w.net_begin(players, 1);
    let cmd_mp = BuildCmd::Build {
        company: co,
        x: 172,
        y: 232,
        shape: 4,
        hint: -1,
    };
    let res3 = w.apply_build(cmd_mp);
    assert!(res3.is_ok());
    assert!(!w.can_undo_last_build(), "undo is disabled in multiplayer");
    assert!(!w.undo_last_build(), "undo returns false in multiplayer");
}

#[test]
fn set_company_info_updates_name_and_logo() {
    let Some((data, save)) = setup() else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let mut w = World::from_save(&save, &data);
    let co = 5;

    let mut name = [0u8; 31];
    let new_name = b"Western Pacific";
    name[..new_name.len()].copy_from_slice(new_name);

    // Test through Command::SetCompanyInfo
    w.apply(Command::SetCompanyInfo {
        company: co as u8,
        name,
        logo: 9,
    });
    assert_eq!(company_name(&w, co), "Western Pacific");
    assert_eq!(company_logo(&w, co), 9);

    // Test through World::set_company_info
    let mut name2 = [0u8; 31];
    let new_name2 = b"Union Pacific Express";
    name2[..new_name2.len()].copy_from_slice(new_name2);
    let res = w.set_company_info(co as u8, name2, 14);
    assert!(res.is_ok());
    assert_eq!(company_name(&w, co), "Union Pacific Express");
    assert_eq!(company_logo(&w, co), 14);
}

fn train_throttle(w: &World, t: i32) -> f32 {
    let s = w.save_ref().unwrap();
    let rec = pool::record(&s.trains, t as usize).unwrap();
    f32::from_bits(u32::from_le_bytes(rec.data[0xdb..0xdf].try_into().unwrap()))
}

fn train_priority(w: &World, t: i32) -> u8 {
    let s = w.save_ref().unwrap();
    let rec = pool::record(&s.trains, t as usize).unwrap();
    rec.data[0xa8]
}

#[test]
fn train_throttle_and_breakdown_chance() {
    let Some((data, save)) = setup() else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let mut w = World::from_save(&save, &data);
    let train_id = 1;

    // Check initial breakdown chance
    let chance_initial = w.train_breakdown_chance(train_id);
    assert!(
        chance_initial > 0.0,
        "initial breakdown chance should be positive, got {chance_initial}"
    );

    // Test invalid train returns 0.0
    assert_eq!(w.train_breakdown_chance(9999), 0.0);
    assert_eq!(w.train_breakdown_chance(0), 0.0);
    assert_eq!(w.train_breakdown_chance(-1), 0.0);

    // Test setting throttle > 65 via World::set_train_throttle
    let res = w.set_train_throttle(train_id as u16, 80);
    assert!(res.is_ok());
    assert_eq!(train_throttle(&w, train_id), 80.0);
    let prio_80 = train_priority(&w, train_id);
    assert!(
        prio_80 > 0,
        "priority should be non-zero for throttle > 65, got {prio_80}"
    );

    let chance_80 = w.train_breakdown_chance(train_id);
    assert!(chance_80 > 0.0);

    // Test setting throttle <= 65 via Command::SetThrottle
    w.apply(Command::SetThrottle {
        train: train_id as u16,
        throttle: 50,
    });
    assert_eq!(train_throttle(&w, train_id), 50.0);
    assert_eq!(
        train_priority(&w, train_id),
        0,
        "priority should be 0 for throttle <= 65"
    );

    let chance_50 = w.train_breakdown_chance(train_id);
    assert!(chance_50 > 0.0);
    // Lower throttle should result in lower breakdown chance
    assert!(
        chance_50 < chance_80,
        "chance at 50% ({chance_50}) should be less than chance at 80% ({chance_80})"
    );
}
