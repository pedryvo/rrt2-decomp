//! Level-2 fidelity of the world loop against the original (docs/PLAN.md section 11,
//! docs/fidelity-status.md).
//!
//! Needs the game install (`RT2_DIR`, for `RT2_PLAT.EXE`) and a directory of saves produced by the
//! original with `re/scripts/wine/advance_save.py` (`RT2_FIDELITY_DIR`): `base_tut.GAM` (the
//! Tutorial advanced 0 days, i.e. the 0x2348 conversion the original makes on load) and
//! `orig_tut_<days>.GAM` (the same advanced `<days>` days with seed 1234). The clone advances
//! `base_tut.GAM` in 320-minute steps with the same seed and the two are compared field by field
//! with the volatile fields masked. Skipped when either variable is unset.

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_formats::savefields::diff;
use rt2_sim::World;
use rt2_sim::command::Command;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn clone_advance(base: &Save, data: &GameData, days: i64) -> (World, Save) {
    let mut w = World::from_save(base, data);
    w.apply(Command::SeedRng(1234));
    let mut left = days * 1440;
    while left > 0 {
        let dt = left.min(320) as i32;
        w.advance(dt);
        left -= i64::from(dt);
    }
    let s = w.to_save();
    (w, s)
}

fn setup() -> Option<(PathBuf, GameData, Save)> {
    let (game, fid) = (env_dir("RT2_DIR")?, env_dir("RT2_FIDELITY_DIR")?);
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let data = GameData::load(&exe).unwrap();
    let base = Save::parse(&std::fs::read(fid.join("base_tut.GAM")).unwrap()).unwrap();
    Some((fid, data, base))
}

/// Paths that differ after the masks, between the original's file and the clone's.
fn unmasked(fid: &std::path::Path, name: &str, clone: &Save) -> Vec<String> {
    let orig = Save::parse(&std::fs::read(fid.join(name)).unwrap()).unwrap();
    diff(&orig, clone)
        .into_iter()
        .filter(|d| d.masked.is_none())
        .map(|d| d.path)
        .collect()
}

/// The Tutorial has no trains and no stations: one day only moves the clock.
#[test]
fn tutorial_one_day_is_equal() {
    let Some((fid, data, base)) = setup() else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let (w, s) = clone_advance(&base, &data, 1);
    assert_eq!(unmasked(&fid, "orig_tut_1.GAM", &s), Vec::<String>::new());
    // 1440 = 4 x 320 + 160: five external calls and one recursive call at the midnight crossing.
    let log = w.hook_log().unwrap();
    assert_eq!(log.count("sim_advance"), 6);
    assert_eq!(log.count("day_end"), 1);
}

/// Seven days: world-tick.md section 6 counts for the Tutorial (39 `sim_advance`, 1 weekly step,
/// 387 recounts and productions, no RNG), and the weekly production matches the original except
/// for the three Port industries (see docs/fidelity-status.md).
#[test]
fn tutorial_seven_days() {
    let Some((fid, data, base)) = setup() else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let (w, s) = clone_advance(&base, &data, 7);
    let log = w.hook_log().unwrap();
    assert_eq!(log.count("sim_advance"), 39);
    assert_eq!(log.count("sim_week_end"), 1);
    assert_eq!(log.count("industry_recount_piles"), 387);
    assert_eq!(log.count("industry_production_tick"), 387);
    assert_eq!(w.rng().state(), 1234, "no RNG is drawn in a week");
    assert_eq!(
        unmasked(&fid, "orig_tut_7.GAM", &s),
        [
            "objects[88].raw[0x8c..0x90]",
            "objects[104].raw[0x8c..0x90]",
            "objects[236].raw[0x8c..0x90]"
        ]
    );
}

/// Thirty days cross the month: economic state step (2 RNG draws), the town bonus roll (1 draw) and
/// the personal cash interest all match; the final RNG state is the one the original reports.
#[test]
fn tutorial_thirty_days() {
    let Some((fid, data, base)) = setup() else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let (w, s) = clone_advance(&base, &data, 30);
    assert_eq!(w.rng().state(), 0x051b_c859);
    let log = w.hook_log().unwrap();
    assert_eq!(log.count("economy_state_step"), 1);
    assert_eq!(log.count("person_month_step"), 4);
    assert_eq!(
        unmasked(&fid, "orig_tut_30.GAM", &s),
        [
            "objects[88].raw[0x8c..0x90]",
            "objects[104].raw[0x8c..0x90]",
            "objects[236].raw[0x8c..0x90]"
        ]
    );
}

/// Loading a save into a `World` and writing it back without advancing is byte-identical, so
/// every state the loop does not simulate yet survives untouched.
#[test]
fn world_round_trip_is_byte_identical() {
    let Some(fid) = env_dir("RT2_FIDELITY_DIR") else {
        eprintln!("RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let Some(game) = env_dir("RT2_DIR") else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let data = GameData::load(&exe).unwrap();
    for name in ["base_tut.GAM", "base_ex.GAM"] {
        let Ok(raw) = std::fs::read(fid.join(name)) else {
            continue;
        };
        let w = World::from_save(&Save::parse(&raw).unwrap(), &data);
        assert!(w.to_save().to_bytes() == raw, "{name}");
    }
}
