//! Plays the original's sound effects, music and videos through the clone's audio layer.
//!
//! ```text
//! rt2-play --list                  every sound with its format, duration and events; music; videos
//! rt2-play buttu61m                one sound effect (name, with or without .raw, or table id)
//! rt2-play music                   music following the original's rules (category 1, in game)
//! rt2-play music --menu            category 0 (main menu); --track NAME plays one file
//! rt2-play --video intro_01        convert the Smacker video to the cache and print the MP4 path
//! ```

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use clap::Parser;
use rt2_audio::{
    Audio, GameFiles, PlayParams, SoundId,
    events::EVENTS,
    music::{MusicDirector, PlayOrder},
    rng::AudioRng,
    video,
};
use rt2_formats::{
    pk2::Pk2,
    sound::{self, SOUNDS},
};

#[derive(Parser)]
#[command(about = "Play RT2 sound effects and music, or convert a Smacker video")]
struct Args {
    /// Sound name or id, or `music`
    target: Option<String>,
    /// Game folder (the one containing `data/`); defaults to $RT2_DIR
    #[arg(short, long, env = "RT2_DIR")]
    dir: PathBuf,
    /// List sounds, music tracks and videos
    #[arg(short, long)]
    list: bool,
    /// Volume in percent
    #[arg(short, long, default_value_t = 100)]
    volume: u8,
    /// Loop an effect for this many seconds (default: play once)
    #[arg(long)]
    loop_secs: Option<f32>,
    /// Music: main-menu category (0) instead of the in-game one (1)
    #[arg(long)]
    menu: bool,
    /// Music: play one file of `data2` instead of the category rules
    #[arg(long)]
    track: Option<String>,
    /// Music: next track in name order instead of the original's random rule
    #[arg(long)]
    sequential: bool,
    /// Music: skip to the next track after this many seconds, repeatedly
    #[arg(long)]
    skip_every: Option<f32>,
    /// Music: stop after this many seconds
    #[arg(long, default_value_t = 60.0)]
    seconds: f32,
    /// Convert this Smacker video (name in `anim`, e.g. intro_01) and print the cached MP4
    #[arg(long)]
    video: Option<String>,
    /// With --video: also open the result with ffplay or mpv if present
    #[arg(long)]
    show: bool,
}

fn resolve(target: &str) -> Option<SoundId> {
    target
        .parse::<u8>()
        .ok()
        .map(SoundId)
        .filter(|id| id.entry().is_some())
        .or_else(|| SoundId::from_name(target))
        .or_else(|| {
            // Unique-enough prefix: `chug1` for `chug161m.raw`.
            let t = target.to_ascii_lowercase();
            SOUNDS
                .iter()
                .find(|e| e.is_used() && e.name.to_ascii_lowercase().starts_with(&t))
                .map(|e| SoundId(e.id))
        })
}

fn list(files: &GameFiles) -> anyhow::Result<()> {
    let raws = files.read_archives()?;
    let archives = raws
        .iter()
        .map(|r| Pk2::parse(r))
        .collect::<Result<Vec<_>, _>>()?;
    println!("id  name          fmt            secs  events");
    for e in SOUNDS.iter().filter(|e| e.is_used()) {
        let secs = sound::blob(&archives, e).map_or(f64::NAN, |b| {
            (b.len() / e.frame_bytes()) as f64 / f64::from(e.sample_rate)
        });
        let events: Vec<String> = EVENTS
            .iter()
            .filter(|r| r.sound.0 == e.id)
            .map(|r| format!("{:?}", r.event))
            .collect();
        println!(
            "{:<3} {:<13} {}ch {:>2}bit {:>5}Hz{} {:>5.2}  {}",
            e.id,
            e.name,
            e.channels,
            e.bits,
            e.sample_rate,
            if e.variable_pitch { "*" } else { " " },
            secs,
            events.join(",")
        );
    }
    println!("(* = pitch follows train speed)");
    println!("\nmusic (data2):");
    let d = MusicDirector::new(files.music_files()?, AudioRng::new(0));
    for t in d.tracks() {
        println!(
            "  cat {}{}  {}",
            t.category,
            if t.looped { " loop" } else { "     " },
            t.file
        );
    }
    println!("\nvideos (anim):");
    match video::list_animations(&files.anim_dir()) {
        Ok(v) => {
            let names: Vec<_> = v
                .iter()
                .filter_map(|p| p.file_stem()?.to_str().map(str::to_lowercase))
                .collect();
            println!("  {}", names.join(" "));
        }
        Err(e) => println!("  (no anim folder: {e})"),
    }
    Ok(())
}

fn play_sound(files: &GameFiles, target: &str, args: &Args) -> anyhow::Result<()> {
    let id = resolve(target).with_context(|| format!("unknown sound {target:?} (try --list)"))?;
    let entry = id.entry().context("empty slot")?;
    let mut audio = Audio::open(files)?;
    let looping = args.loop_secs.is_some();
    let _voice = audio
        .play_with(
            id,
            &PlayParams {
                volume: args.volume,
                looping,
                ..PlayParams::default()
            },
        )
        .with_context(|| format!("{} is not in the archives", entry.name))?;
    let secs = args.loop_secs.unwrap_or_else(|| {
        let blob_len = files
            .read_archives()
            .ok()
            .and_then(|raws| {
                let archives: Vec<_> = raws.iter().filter_map(|r| Pk2::parse(r).ok()).collect();
                sound::blob(&archives, entry).map(<[u8]>::len)
            })
            .unwrap_or(0);
        (blob_len / entry.frame_bytes()) as f32 / entry.sample_rate as f32 + 0.3
    });
    println!("playing {} (id {}) for {secs:.1}s", entry.name, id.0);
    std::thread::sleep(Duration::from_secs_f32(secs));
    Ok(())
}

fn play_music(files: &GameFiles, args: &Args) -> anyhow::Result<()> {
    let mut audio = Audio::open(files)?;
    if args.sequential {
        audio.set_music_order(PlayOrder::Sequential);
    }
    audio.set_music_volume(args.volume);
    if let Some(track) = &args.track {
        audio.music_play_file(track)?;
    } else {
        audio.music_request(if args.menu { 0 } else { 1 })?;
    }
    let start = Instant::now();
    let mut last_skip = Instant::now();
    let mut shown = String::new();
    while start.elapsed().as_secs_f32() < args.seconds {
        audio.update()?;
        if args
            .skip_every
            .is_some_and(|s| last_skip.elapsed().as_secs_f32() >= s)
        {
            audio.music_skip()?;
            last_skip = Instant::now();
        }
        if let Some(t) = audio.music_current()
            && t.file != shown
        {
            println!("now playing: {} (category {})", t.file, t.category);
            shown.clone_from(&t.file);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn convert_video(files: &GameFiles, name: &str, show: bool) -> anyhow::Result<()> {
    let wanted = name.trim_end_matches(".smk").to_lowercase();
    let smk = video::list_animations(&files.anim_dir())?
        .into_iter()
        .find(|p| {
            p.file_stem()
                .is_some_and(|s| s.to_string_lossy().to_lowercase() == wanted)
        })
        .with_context(|| format!("no video {name:?} in anim (try --list)"))?;
    if !video::ffmpeg_available() {
        bail!("ffmpeg is not installed; it is needed to convert Smacker videos");
    }
    let mp4 = video::ensure_converted(&smk, &video::default_cache_dir())?;
    println!("{}", mp4.display());
    if show {
        for player in ["ffplay", "mpv"] {
            let args: &[&str] = if player == "ffplay" {
                &["-autoexit", "-loglevel", "error"]
            } else {
                &[]
            };
            if std::process::Command::new(player)
                .args(args)
                .arg(&mp4)
                .status()
                .is_ok()
            {
                break;
            }
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let files = GameFiles::new(&args.dir);
    if args.list {
        return list(&files);
    }
    if let Some(v) = &args.video {
        return convert_video(&files, v, args.show);
    }
    match args.target.as_deref() {
        Some("music") => play_music(&files, &args),
        Some(t) => play_sound(&files, t, &args),
        None => bail!("give a sound name, `music`, --video NAME or --list"),
    }
}
