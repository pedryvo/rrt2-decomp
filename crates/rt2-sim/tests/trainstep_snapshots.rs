//! Function-level differential tests of the train step against the original
//! (docs/spec/trains.md section 10, docs/dynamic-analysis.md "snapshots").
//!
//! `re/scripts/wine/snap_gdb.py` stops the original (Wine + gdb) at the K-th call of a function,
//! saves the game right there (`<name>_pre.GAM`), runs the call to its return and saves again
//! (`<name>_post.GAM`), logging the train, the game RNG and the CRT `rand` states. The clone
//! loads the first save, runs its port of the same function with the same RNG states and must
//! write the second save field by field, and end with the same RNG states.
//!
//! The files live in `$RT2_FIDELITY_DIR/snaps/` with a `manifest.txt` of lines
//! `name function train rng crt rng_after crt_after [dt]`. Skipped when the variables or the
//! files are absent.

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_formats::savefields::diff;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::hooks::HookLog;
use rt2_sim::rng::{CrtRng, GameRng};
use rt2_sim::trainstep::{Globals, Ts};

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("hex")
}

#[test]
fn train_step_functions_match_original_snapshots() {
    let (Some(game), Some(fid)) = (
        std::env::var_os("RT2_DIR").map(PathBuf::from),
        std::env::var_os("RT2_FIDELITY_DIR").map(PathBuf::from),
    ) else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let dir = fid.join("snaps");
    let Ok(manifest) = std::fs::read_to_string(dir.join("manifest.txt")) else {
        eprintln!("no snaps/manifest.txt; skipping");
        return;
    };
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let mut data = GameData::load(&exe).unwrap();
    data.load_sprites(&game).unwrap();
    let mut checked = 0;
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, func, slot) = (f[0], f[1], f[2].parse::<i32>().unwrap());
        let mut save =
            Save::parse(&std::fs::read(dir.join(format!("{name}_pre.GAM"))).unwrap()).unwrap();
        let post =
            Save::parse(&std::fs::read(dir.join(format!("{name}_post.GAM"))).unwrap()).unwrap();
        let year = save.date.year;
        let mut types = data.industries[0].clone();
        rt2_sim::industry::apply_load_fixups(&mut types, &save.header, &data.cargo);
        for t in &mut types {
            t.rebuild_inputs(i32::from(year));
        }
        let mut rng = GameRng::new(hex(f[3]));
        let mut crt = CrtRng::new(hex(f[4]));
        let mut log = HookLog::default();
        let now = save.game_time_minutes as u32;
        let (f229, f236) = (save.header[0x86f1], save.header[0x86fe]);
        {
            let mut ts = Ts {
                s: &mut save,
                img: &data.image,
                anim_frames: &data.anim_frames,
                types: &types,
                cargo: &data.cargo,
                revenue: &data.revenue,
                now,
                year,
                rng: &mut rng,
                crt: &mut crt,
                log: &mut log,
                g: Globals::default(),
                flag_570229: f229,
                flag_570236: f236,
            };
            match func {
                "step" => ts.train_step(slot, f[7].parse().unwrap(), true),
                "arrive" => ts.train_arrive(slot),
                "place" => ts.train_consist_at_stop(slot),
                "left" => ts.train_left_station(slot),
                "breakdown" => ts.train_breakdown(slot),
                "monthly" => ts.train_monthly_costs(slot),
                "yearly" => ts.train_year_step(slot),
                other => panic!("unknown function {other}"),
            }
        }
        let left: Vec<String> = diff(&post, &save)
            .into_iter()
            .filter(|d| d.masked.is_none())
            .map(|d| d.path)
            .collect();
        assert_eq!(left, Vec::<String>::new(), "{name} ({func})");
        assert_eq!(rng.state(), hex(f[5]), "{name}: game RNG after");
        assert_eq!(crt.state(), hex(f[6]), "{name}: CRT rand after");
        assert_eq!(
            log.skipped_steps().count(),
            0,
            "{name}: missing steps reached"
        );
        checked += 1;
    }
    assert!(checked > 0);
    eprintln!("{checked} snapshots match");
}
