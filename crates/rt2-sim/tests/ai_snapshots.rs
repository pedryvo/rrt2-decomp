//! Function-level differential tests of the computer opponents against the original
//! (docs/spec/ai.md, docs/dynamic-analysis.md section 6).
//!
//! The original (Wine + gdb, `re/scripts/wine/snap_gdb.py`) is stopped at the K-th call of an AI
//! function; the game is saved right before the call (`<name>_pre.GAM`) and right after it
//! returns (`<name>_post.GAM`), with the game RNG and the CRT `rand` states. The clone loads the
//! first save, runs its port of the function with the same RNG states and must write the second
//! save field by field and end with the same RNG states.
//!
//! The files live in `$RT2_FIDELITY_DIR/snaps_ai/` with a `manifest.txt` of lines
//! `name function arg rng crt rng_after crt_after` (`function` = address: `0x436b30`
//! `ai_company_turn(arg)`, `0x436d60` `ai_monthly_turn`). Skipped when the variables or the
//! files are absent. A case whose port reaches a step that is not implemented yet (a counted
//! `todo_hook`) is reported and not asserted.

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
fn ai_functions_match_original_snapshots() {
    let (Some(game), Some(fid)) = (
        std::env::var_os("RT2_DIR").map(PathBuf::from),
        std::env::var_os("RT2_FIDELITY_DIR").map(PathBuf::from),
    ) else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let dir = fid.join("snaps_ai");
    let Ok(manifest) = std::fs::read_to_string(dir.join("manifest.txt")) else {
        eprintln!("no snaps_ai/manifest.txt; skipping");
        return;
    };
    let only = std::env::var("AI_SNAP_ONLY").ok();
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let mut data = GameData::load(&exe).unwrap();
    data.load_sprites(&game).unwrap();
    let (mut checked, mut partial) = (0, 0);
    let mut failures = Vec::new();
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, func, arg) = (f[0], f[1], f[2].parse::<i32>().unwrap());
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
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
                "0x436b30" => {
                    rt2_sim::ai::company_turn(&mut ts, arg);
                }
                "0x436d60" => {
                    rt2_sim::ai::monthly_turn(&mut ts);
                }
                other => panic!("unknown function {other}"),
            }
        }
        let left: Vec<String> = diff(&post, &save)
            .into_iter()
            .filter(|d| d.masked.is_none())
            .map(|d| format!("{}: {:?} (clone {:?})", d.path, d.a, d.b))
            .collect();
        let skipped: Vec<String> = log
            .skipped_steps()
            .map(|(k, s)| format!("{k} x{}", s.hits))
            .collect();
        let rng_ok = rng.state() == hex(f[5]) && crt.state() == hex(f[6]);
        if !skipped.is_empty() {
            eprintln!(
                "{name}: partial port ({}); {} fields differ, RNG {}",
                skipped.join(", "),
                left.len(),
                if rng_ok { "equal" } else { "differs" }
            );
            partial += 1;
            continue;
        }
        if !left.is_empty() || !rng_ok {
            failures.push(format!(
                "{name}: {} fields differ (first: {:?}), RNG {:#x}/{:#x} want {}/{}",
                left.len(),
                left.iter().take(8).collect::<Vec<_>>(),
                rng.state(),
                crt.state(),
                f[5],
                f[6]
            ));
            continue;
        }
        checked += 1;
    }
    assert!(failures.is_empty(), "{failures:#?}");
    eprintln!("{checked} AI snapshots match, {partial} partial");
}
