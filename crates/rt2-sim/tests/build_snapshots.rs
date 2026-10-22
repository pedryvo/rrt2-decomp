//! Function-level differential tests of the construction primitives (`rt2_sim::build`) against
//! the original (docs/dynamic-analysis.md section 6, docs/spec/terrain-track.md).
//!
//! The original (Wine + gdb) is stopped at the K-th call of a function; the game is saved right
//! before the call (`<name>_pre.GAM`) and right after it returns (`<name>_post.GAM`). The clone
//! loads the first save, runs its port with the same arguments and RNG states and must write the second save
//! field by field and end with the same RNG states.
//!
//! Files: `$RT2_FIDELITY_DIR/snaps_build/` with a `manifest.txt` of lines
//! `name function ecx rng crt rng_after crt_after edx arg1 .. arg12` (stack arguments in
//! order). Skipped when the variables or the files are absent. `BUILD_SNAP_ONLY=name` runs one
//! case, `BUILD_SNAP_SHOW=n` prints the first `n` differing fields.

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_formats::savefields::diff;
use rt2_sim::ai::Ai;
use rt2_sim::build::route::LayRoute;
use rt2_sim::build::station::NewBuilding;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::hooks::HookLog;
use rt2_sim::rng::{CrtRng, GameRng};
use rt2_sim::trainstep::{Globals, Ts};

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("hex")
}

/// Runs the port of `func`; `false` when the function is not ported.
fn run(
    ai: &mut Ai<'_, '_>,
    func: &str,
    ecx: i32,
    edx: i32,
    a: &[i32],
    pending: Option<(i32, i32, i32, i32)>,
    pkt_hex: &str,
) -> bool {
    match func {
        "0x4cc060" => {
            ai.track_lay_route(LayRoute {
                company: ecx,
                x1: edx,
                y1: a[0],
                x2: a[1],
                y2: a[2],
                d1: a[3],
                d2: a[4],
                single: a[6] != 0,
                electric: a[7] != 0,
                bridge: a[8],
            });
        }
        "0x46a370" => ai.station_networks_rebuild(),
        "0x409010" => {
            // The original's map repaint still has the rectangle of the preceding
            // track_lay_route of the same turn (case lay1) pending.
            if let Some(r) = pending {
                ai.dirty(r.0, r.1, r.2, r.3);
            }
            ai.building_create(NewBuilding {
                x: ecx,
                y: edx,
                shape: a[0],
                ghost: a[1] != 0,
                company: a[2],
                place: a[3],
                hint: a[4],
            });
        }
        "0x4b0210" => {
            let pkt: Vec<u8> = (0..pkt_hex.len() / 2)
                .map(|i| u8::from_str_radix(&pkt_hex[2 * i..2 * i + 2], 16).unwrap())
                .collect();
            let cmd = rt2_sim::build::cmd::BuildCmd::from_packet(&pkt).expect("packet");
            let _ = ai.execute(cmd);
        }
        "0x46aef0" => {
            ai.station_buy_improvements(ecx, edx as u32);
        }
        "0x434850" => ai.ai_station_upgrades(edx),
        _ => return false,
    }
    true
}

#[test]
fn build_primitives_match_original_snapshots() {
    let (Some(game), Some(fid)) = (
        std::env::var_os("RT2_DIR").map(PathBuf::from),
        std::env::var_os("RT2_FIDELITY_DIR").map(PathBuf::from),
    ) else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let dir = fid.join("snaps_build");
    let Ok(manifest) = std::fs::read_to_string(dir.join("manifest.txt")) else {
        eprintln!("no snaps_build/manifest.txt; skipping");
        return;
    };
    let only = std::env::var("BUILD_SNAP_ONLY").ok();
    let show: usize = std::env::var("BUILD_SNAP_SHOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let mut data = GameData::load(&exe).unwrap();
    data.load_sprites(&game).unwrap();
    let lng = std::fs::read(game.join("default2.lng"))
        .ok()
        .and_then(|b| rt2_formats::lng::Lng::parse(&b).ok())
        .map(std::rc::Rc::new);
    // Redraw rectangles left pending by an earlier case of the same original turn.
    let pending: std::collections::HashMap<&str, (i32, i32, i32, i32)> =
        [("bc1", (167, 202, 228, 234))].into_iter().collect();
    let (mut checked, mut partial) = (0, 0);
    let mut failures = Vec::new();
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, func) = (f[0], f[1]);
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let ecx = f[2].parse::<i64>().unwrap() as i32;
        let edx = f[7].parse::<i64>().unwrap() as i32;
        let pkt_hex = if f.get(8) == Some(&"pkt") { f[9] } else { "" };
        let args: Vec<i32> = if pkt_hex.is_empty() {
            f[8..]
                .iter()
                .map(|v| v.parse::<i64>().unwrap() as i32)
                .collect()
        } else {
            Vec::new()
        };
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
        let ported;
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
            let mut ai = Ai::new(&mut ts);
            ai.ai_turn = pkt_hex.is_empty();
            ai.lng = lng.clone();
            ported = run(
                &mut ai,
                func,
                ecx,
                edx,
                &args,
                pending.get(name).copied(),
                pkt_hex,
            );
        }
        if !ported {
            eprintln!("{name}: {func} not ported; skipped");
            continue;
        }
        // A default stop (`0x4d3f40`) leaves the bytes past its cargo list uninitialized
        // (stack memory of the original): taken from the original for new trains.
        let pre_trains: Vec<usize> =
            Save::parse(&std::fs::read(dir.join(format!("{name}_pre.GAM"))).unwrap())
                .unwrap()
                .train_extras
                .iter()
                .map(|e| e.slot)
                .collect();
        for e in &mut save.train_extras {
            if pre_trains.contains(&e.slot) {
                continue;
            }
            if let Some(o) = post.train_extras.iter().find(|o| o.slot == e.slot) {
                for k in 0..e.cars_raw.len().min(o.cars_raw.len()) / 14 {
                    let n = usize::from(e.cars_raw[14 * k + 2]);
                    for b in 4 + n..14 {
                        e.cars_raw[14 * k + b] = o.cars_raw[14 * k + b];
                    }
                }
            }
        }
        let left: Vec<String> = diff(&post, &save)
            .into_iter()
            .filter(|d| d.masked.is_none())
            .map(|d| format!("{}: {:?} (clone {:?})", d.path, d.a, d.b))
            .collect();
        for l in left.iter().take(show) {
            eprintln!("  {name} {l}");
        }
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
        eprintln!("{name}: identical");
        checked += 1;
    }
    assert!(failures.is_empty(), "{failures:#?}");
    eprintln!("{checked} build snapshots match, {partial} partial");
}
