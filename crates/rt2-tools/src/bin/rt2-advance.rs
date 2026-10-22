//! Advances a save N days in the clone: the counterpart of `re/scripts/wine/advance_save.py`
//! (docs/dynamic-analysis.md section 5), used by the level-2 fidelity comparison
//! (`docs/fidelity-status.md`).
//!
//! Like the original driver it seeds the game RNG with `<seed>` right after the load, then calls
//! `sim_advance` in steps of 320 game minutes (`--step`) until `1440 * days` minutes have passed,
//! and writes the save back. The data tables come from `RT2_PLAT.EXE` under `--game-dir` /
//! `RT2_DIR`; nothing of them is stored in the repository.

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Parser;
use rt2_formats::map::Save;
use rt2_sim::World;
use rt2_sim::command::Command;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;

#[derive(Parser)]
#[command(about = "Advance a .GAM save N days in the clone (mirrors advance_save.py)")]
struct Args {
    input: PathBuf,
    days: u32,
    seed: u32,
    output: PathBuf,
    /// Game install directory (for RT2_PLAT.EXE)
    #[arg(
        long,
        env = "RT2_DIR",
        default_value = "/home/pedryvo/Games/railroad-tycoon-2"
    )]
    game_dir: PathBuf,
    /// Minutes per sim_advance call (320 = 100 ms at speed 3)
    #[arg(long, default_value_t = 320)]
    step: i32,
    /// Do not print the step counters
    #[arg(long)]
    quiet: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.step <= 0 {
        bail!("--step must be positive");
    }
    let raw = std::fs::read(&args.input).with_context(|| args.input.display().to_string())?;
    let save = Save::parse(&raw).with_context(|| format!("parsing {}", args.input.display()))?;
    let exe_path = args.game_dir.join("RT2_PLAT.EXE");
    let exe =
        Exe::from_bytes(std::fs::read(&exe_path).with_context(|| exe_path.display().to_string())?)
            .context("reading RT2_PLAT.EXE")?;
    let mut data = GameData::load(&exe).context("loading the data tables")?;
    data.load_sprites(&args.game_dir)
        .map_err(anyhow::Error::msg)
        .context("reading SMOKE.imb")?;

    let mut world = World::from_save(&save, &data);
    // Same order as the driver: load, then seed the game RNG.
    world.apply(Command::SeedRng(args.seed));
    world.apply(Command::SeedCrtRng(args.seed));
    let start = world.minutes();
    let total = i64::from(args.days) * 1440;
    let mut done = 0i64;
    while done < total {
        let dt = i64::from(args.step).min(total - done) as i32;
        world.advance(dt);
        done += i64::from(dt);
    }
    if i64::from(world.minutes()) != i64::from(start) + total {
        bail!(
            "the clock moved {} minutes, expected {total}",
            world.minutes() - start
        );
    }
    let out = world.to_save().to_bytes();
    std::fs::write(&args.output, &out).with_context(|| args.output.display().to_string())?;
    if !args.quiet {
        let d = world.date();
        eprintln!(
            "{:04}-{:02}-{:02} {:02}:{:02}, rng state {:#x}, {} bytes",
            d.year,
            d.month,
            d.day,
            d.hour,
            d.minute,
            world.rng().state(),
            out.len()
        );
        if let Some(log) = world.hook_log() {
            for (name, n) in log.ran_steps() {
                eprintln!("ran      {name}: {n}");
            }
            for (name, s) in log.skipped_steps() {
                eprintln!("MISSING  {name}: {} hits  ({}; {})", s.hits, s.what, s.spec);
            }
        }
    }
    Ok(())
}
