//! Differential tests of the financial commands (`rt2_sim::fincmd`, docs/spec/commands.md 2.1,
//! docs/spec/finance.md) against the original (docs/dynamic-analysis.md section 6): `cmd_execute`
//! (`0x4b0210`) of the original is called directly with one packet on a saved game; the game is
//! saved right before the call (`<name>_pre.GAM`) and right after it (`<name>_post.GAM`). The
//! clone loads the first save, runs the same packet through [`World::apply_fin_unchecked`] (the
//! executor, which like `cmd_execute` does no window check) and must write the second save field
//! by field and end with the same RNG states.
//!
//! The original runs with its dialogs patched to answer "yes" (`0x4510a0` returns `0x66`): a human
//! shareholder asked to vote votes in favour, which the test reproduces with the vote command of
//! the local person.
//!
//! Files: `$RT2_FIDELITY_DIR/snaps_fin/` with a `manifest.txt` of lines
//! `name 0x4b0210 ecx rng crt rng_after crt_after edx pkt <hex>` (or `name 0x46bb10 ecx rng crt
//! rng_after crt_after edx`, the margin routine called by itself; made by
//! `~/.cache/rrt2/buildwork/runfin.sh`). Skipped when the variables or the files are absent.
//! `FIN_SNAP_ONLY=name` runs one case, `FIN_SNAP_SHOW=n` prints the first `n` differing fields.

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_formats::savefields::diff;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::fincmd::FinCmd;
use rt2_sim::{Command, World};

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("hex")
}

/// The person that answers the vote dialogs of the original (the local player).
const LOCAL_PERSON: i32 = 1;

#[test]
fn finance_commands_match_original_snapshots() {
    let (Some(game), Some(fid)) = (
        std::env::var_os("RT2_DIR").map(PathBuf::from),
        std::env::var_os("RT2_FIDELITY_DIR").map(PathBuf::from),
    ) else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let dir = fid.join("snaps_fin");
    let Ok(manifest) = std::fs::read_to_string(dir.join("manifest.txt")) else {
        eprintln!("no snaps_fin/manifest.txt; skipping");
        return;
    };
    let only = std::env::var("FIN_SNAP_ONLY").ok();
    let show: usize = std::env::var("FIN_SNAP_SHOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let data = GameData::load(&exe).unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let name = f[0];
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        // `0x46bb10` (the broker's margin routine) is called without a packet.
        let cmd = if f[1] == "0x46bb10" {
            None
        } else {
            assert_eq!(f[8], "pkt", "{name}: manifest line");
            let pkt: Vec<u8> = (0..f[9].len() / 2)
                .map(|i| u8::from_str_radix(&f[9][2 * i..2 * i + 2], 16).unwrap())
                .collect();
            Some(
                FinCmd::from_packet(&pkt).unwrap_or_else(|| panic!("{name}: not a finance packet")),
            )
        };
        let pre =
            Save::parse(&std::fs::read(dir.join(format!("{name}_pre.GAM"))).unwrap()).unwrap();
        let post =
            Save::parse(&std::fs::read(dir.join(format!("{name}_post.GAM"))).unwrap()).unwrap();
        let mut w = World::from_save_seeded(&pre, &data, hex(f[3]));
        w.apply(Command::SeedCrtRng(hex(f[4])));
        let outcome = match cmd {
            Some(cmd) => w.apply_fin_unchecked(cmd),
            None => {
                w.margin_check();
                Ok(rt2_sim::fincmd::FinOutcome::Done)
            }
        };
        // The dialogs of the original answered "yes" to the local human's vote.
        for _ in 0..4 {
            let pending: Vec<_> = w
                .pending_votes()
                .into_iter()
                .filter(|v| v.person == LOCAL_PERSON)
                .collect();
            let Some(v) = pending.first() else { break };
            let vote = if v.merger {
                FinCmd::MergerVote {
                    proposal: v.id,
                    person: v.person,
                    yes: true,
                }
            } else {
                FinCmd::ElectionVote {
                    election: v.id,
                    person: v.person,
                    yes: true,
                }
            };
            let _ = w.apply_fin_unchecked(vote);
        }
        for m in w.take_messages() {
            if show > 0 {
                eprintln!("  {name} message: {m:?}");
            }
        }
        let out = w.to_save();
        let left: Vec<String> = diff(&post, &out)
            .into_iter()
            .filter(|d| d.masked.is_none())
            .map(|d| format!("{}: {:?} (clone {:?})", d.path, d.a, d.b))
            .collect();
        for l in left.iter().take(show) {
            eprintln!("  {name} {l}");
        }
        let rng_ok = w.rng().state() == hex(f[5]) && w.crt_rng().state() == hex(f[6]);
        if !left.is_empty() || !rng_ok {
            failures.push(format!(
                "{name} ({outcome:?}): {} fields differ (first: {:?}), RNG {:#x}/{:#x} want {}/{}",
                left.len(),
                left.iter().take(6).collect::<Vec<_>>(),
                w.rng().state(),
                w.crt_rng().state(),
                f[5],
                f[6]
            ));
            continue;
        }
        eprintln!("{name}: identical ({outcome:?})");
        checked += 1;
    }
    assert!(failures.is_empty(), "{failures:#?}");
    eprintln!("{checked} finance snapshots match");
}
