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
use rt2_sim::command::Command;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::{NewGameOptions, World};

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
    /// Game and CRT RNG state when the load starts (`advance_save.py --preseed`): a save older than
    /// 0x2348 is converted on load, which consumes the game RNG
    #[arg(long, value_parser = parse_u32)]
    preseed: Option<u32>,
    /// Start a new game on the input map (`World::new_game`) instead of loading it; the setup
    /// options (`0x574840` block, campaign block) come from `--options-from`
    #[arg(long, requires = "options_from")]
    new_game: bool,
    /// Save whose option block and campaign block are the new game's options (e.g. the
    /// original's snapshot right after the start)
    #[arg(long)]
    options_from: Option<PathBuf>,
    /// Name copied to the header (default: the input file stem)
    #[arg(long)]
    name: Option<String>,
    /// Campaign start (`0x410de0`: `0x570234 = 1`)
    #[arg(long)]
    campaign: bool,
    /// Game RNG when the scenario start begins (`rng_seed` before `0x479b20`)
    #[arg(long, value_parser = parse_u32, default_value = "0")]
    game_rng: u32,
    /// CRT rand state when the scenario start begins
    #[arg(long, value_parser = parse_u32, default_value = "0")]
    crt: u32,
    /// Do not print the step counters
    #[arg(long)]
    quiet: bool,
}

fn parse_u32(s: &str) -> Result<u32, String> {
    match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16),
        None => s.parse(),
    }
    .map_err(|e| e.to_string())
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

    let mut world = if args.new_game {
        let p = args.options_from.as_ref().expect("clap requires it");
        let opt_save = Save::parse(&std::fs::read(p).with_context(|| p.display().to_string())?)
            .with_context(|| format!("parsing {}", p.display()))?;
        let name = args.name.clone().unwrap_or_else(|| {
            args.input
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        let opts = NewGameOptions::from_save(&opt_save, &name, args.campaign, args.crt);
        let w = World::new_game(&save, &opts, &data, args.game_rng);
        if !args.quiet {
            eprintln!(
                "rng after start: {:#x} crt {:#x}",
                w.rng().state(),
                w.crt_rng().state()
            );
        }
        w
    } else {
        World::from_save_seeded(&save, &data, args.preseed.unwrap_or(0))
    };
    if !args.quiet && args.preseed.is_some() {
        eprintln!("rng after load: {:#x}", world.rng().state());
    }
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
