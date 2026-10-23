//! `rrt2`: the playable Railroad Tycoon II clone (docs/ui.md).
//!
//! ```text
//! rrt2 set-path <folder>        remember the original game's install folder
//! rrt2                          main menu
//! rrt2 <file.GAM|file.MAP>      open a save or a scenario map directly
//! rrt2 --screenshot out.png     render the menu (or --screen game) offscreen and exit
//! ```

mod app;
mod app_strings;
mod artwork;
mod buildtools;
mod catalog;
mod config;
mod founding;
mod gfx;
mod info;
mod keys;
mod locales;
mod options;
mod paths;
mod pick;
mod radar;
mod saves;
mod scenario_info;
mod setup;
mod sound;
mod state;
mod stationview;
mod text;
mod version;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use rt2_render::GameAssets;
use rt2_ui::UiScale;

use crate::config::Config;
use crate::state::{AppState, Init};

#[derive(Parser)]
#[command(
    name = "rrt2",
    version = version::LONG,
    about = "Railroad Tycoon II, rebuilt: menu, map view and simulation"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Cmd>,
    /// A save (.GAM) or scenario map (.MAP/.MP2) to open straight away
    file: Option<PathBuf>,
    /// The original game's install folder (default: `game_dir` of config.toml)
    #[arg(long, env = "RT2_DIR", global = true)]
    game_dir: Option<PathBuf>,
    /// UI scale in percent, 100 to 300 (default: from the monitor DPI, or config.toml)
    #[arg(long)]
    ui_scale: Option<u32>,
    /// UI language tag such as pt-BR (default: `language` of config.toml, else the system locale)
    #[arg(long, env = "RRT2_LANG")]
    language: Option<String>,
    /// Render one frame offscreen to this PNG and exit
    #[arg(long, value_name = "PNG")]
    screenshot: Option<PathBuf>,
    /// Screen to render with --screenshot
    #[arg(long, value_enum, default_value_t = Screen::Menu)]
    screen: Screen,
    /// With `--screen menu`: the page (main, single, new-scenario, map, options, campaign, load,
    /// scores, credits). With `--screen dialog`: the dialog (goals, founding, year-end,
    /// year-end-1..3, news, dialog, win-gold, win-silver, win-bronze, lose, file, tool-0..7).
    #[arg(long)]
    page: Option<String>,
    /// With `--screen info`: finance, stock, trains, train, stations, station, cities, city,
    /// industries, industry or territory
    #[arg(long, default_value = "finance")]
    info: String,
    /// With `--screen game`: click the map at this physical pixel (X,Y) before the shot, to try
    /// the click-to-inspect pages
    #[arg(long, value_parser = parse_point)]
    click: Option<(f32, f32)>,
    /// Screenshot size in pixels, WIDTHxHEIGHT
    #[arg(long, default_value = "1920x1080", value_parser = parse_size)]
    size: (u32, u32),
    /// With --screenshot --screen game: centre the camera on this tile (X,Y)
    #[arg(long, value_parser = parse_point)]
    focus: Option<(f32, f32)>,
    /// With --focus: pixels per native pixel (1 = the original's full zoom)
    #[arg(long, default_value_t = 1.0)]
    zoom: f32,
    /// With --focus: view rotation, 0..3
    #[arg(long, default_value_t = 0)]
    rotation: u8,
    /// With --screenshot --screen game: render this many extra live frames (simulation running,
    /// interpolated cars) and print the mean time per frame
    #[arg(long, default_value_t = 0)]
    bench: u32,
    /// With --screenshot --screen game: simulated days to advance before the shot
    #[arg(long, default_value_t = 0)]
    days: u32,
    /// With `--screen game`: select a construction tool (track, station, bulldoze)
    #[arg(long)]
    tool: Option<String>,
    /// With `--tool`: the tile under the cursor (X,Y in map tiles)
    #[arg(long, value_parser = parse_point)]
    hover: Option<(f32, f32)>,
    /// With `--tool track`: the tile the drag started on (X,Y in map tiles)
    #[arg(long, value_parser = parse_point)]
    drag_from: Option<(f32, f32)>,
    /// With `--tool`: send the tool's command (as a click or the end of the drag would)
    #[arg(long)]
    apply: bool,
    /// With `--screen game`: open the dialogs a started game opens (briefing, then Start New
    /// Company when the player has no company), as the menus do
    #[arg(long)]
    opening: bool,
    /// With `--screen game`: found a company with this name (the dialog's middle sliders)
    #[arg(long)]
    found: Option<String>,
    /// With a game screen: write a save with this name into the saves folder (`RRT2_SAVES`)
    /// before the shot, to have something to list in the Load dialog
    #[arg(long)]
    save_as: Option<String>,
    /// With `--screen game`: start a scenario map as a campaign game (index 0..18)
    #[arg(long)]
    campaign: Option<usize>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Screen {
    Menu,
    NewGame,
    LoadGame,
    /// The main window over a game.
    Game,
    /// A dialog (`--page`) over the main window.
    Dialog,
    /// The Options screen over the menu (or over the game when a file is given); `--page` is the
    /// tab: display, audio, language, keys, gameplay.
    Options,
    /// The Save Game or Load Game dialog over the game; `--page` is `save` or `load`.
    Saves,
    /// An information screen over the loaded game: finance, stock, trains, train, stations,
    /// station, cities, city, industries, industry or territory (see `--info`).
    Info,
}

#[derive(Subcommand)]
enum Cmd {
    /// Remember the original game's install folder in config.toml
    SetPath { dir: PathBuf },
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    let p = |v: &str| v.parse::<u32>().map_err(|e| e.to_string());
    let size = (p(w)?, p(h)?);
    if size.0 == 0 || size.1 == 0 {
        return Err("size must be non-zero".into());
    }
    Ok(size)
}

fn parse_point(s: &str) -> Result<(f32, f32), String> {
    let (x, y) = s.split_once(',').ok_or("expected X,Y")?;
    let p = |v: &str| v.trim().parse::<f32>().map_err(|e| e.to_string());
    Ok((p(x)?, p(y)?))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config_path = paths::config_file();
    if let Some(Cmd::SetPath { dir }) = args.command {
        return set_path(&dir, config_path);
    }
    let mut config = match &config_path {
        Some(p) => Config::load(p)?,
        None => Config::default(),
    };
    if args.language.is_some() {
        config.language.clone_from(&args.language);
    }
    let install = paths::resolve_install(args.game_dir.clone(), &config)?;
    let assets = Arc::new(
        GameAssets::open(&install)
            .with_context(|| format!("opening the game in {}", install.display()))?,
    );
    let art = artwork::load(&assets);
    let pinned = args.ui_scale.or(config.ui_scale);
    let init = Init {
        install: install.clone(),
        assets,
        backdrop: art.backdrop,
        icons: art.icons.clone(),
        dialogs: art.dialogs,
        config,
        scale: pinned.map_or(UiScale::default(), UiScale::from_percent),
        scale_pinned: pinned.is_some(),
        viewport: args.size,
    };
    match &args.screenshot {
        Some(out) => screenshot(init, art.skin, &args, out),
        None => app::run(init, art.skin, args.file),
    }
}

fn set_path(dir: &std::path::Path, config_path: Option<PathBuf>) -> Result<()> {
    let dir = dir
        .canonicalize()
        .with_context(|| format!("{} does not exist", dir.display()))?;
    paths::validate_install(&dir)?;
    let Some(path) = config_path else {
        bail!("cannot find a config folder; set RRT2_CONFIG to a file path");
    };
    let mut config = Config::load(&path)?;
    config.game_dir = Some(dir.clone());
    config.save(&path)?;
    println!("game folder set to {}", dir.display());
    println!("saved in {}", path.display());
    Ok(())
}

fn screenshot(init: Init, skin: rt2_ui::Skin, args: &Args, out: &std::path::Path) -> Result<()> {
    let install = init.install.clone();
    let (device, queue) = gfx::headless_device()?;
    let mut gfx = gfx::Gfx::new(device, queue, gfx::SCREENSHOT_FORMAT, args.size, skin);
    let mut state = AppState::new(init);
    match args.screen {
        Screen::Menu => {
            if let Some(page) = &args.page
                && !state.show_menu_page(page)
            {
                bail!("unknown --page {page:?}");
            }
        }
        Screen::Options if args.file.is_none() => {
            let tab = args.page.as_deref().unwrap_or("display");
            if !state.show_options(tab) {
                bail!("unknown options tab {tab:?}");
            }
        }
        Screen::NewGame => state.show_new_game(),
        Screen::LoadGame => state.show_load_game(),
        Screen::Game | Screen::Info | Screen::Dialog | Screen::Options | Screen::Saves => {
            let file = args
                .file
                .clone()
                .unwrap_or_else(|| install.join("games").join("Example save game.GAM"));
            match args.campaign {
                Some(index) => state.start_mode(
                    &file,
                    state::StartMode::Campaign(
                        setup::Setup::default(),
                        setup::CampaignStart {
                            set: 0,
                            index,
                            difficulty: 1,
                            bonus: 0,
                        },
                    ),
                )?,
                None => state.start(&file)?,
            }
            if args.opening {
                state.queue_opening_dialogs();
            }
            if let Some(name) = &args.found {
                state.found_with_defaults(name);
            }
            state.advance_days(args.days);
            if let Some((x, y)) = args.focus {
                state.set_view(x, y, args.zoom, args.rotation);
            }
            state.update(0.0);
            if matches!(args.screen, Screen::Info) && !state.show_info_screen(&args.info) {
                bail!("unknown --info screen {:?}", args.info);
            }
            if let Some(name) = &args.save_as {
                state.save_as(name).map_err(|e| anyhow::anyhow!(e))?;
            }
            if matches!(args.screen, Screen::Options) {
                let tab = args.page.as_deref().unwrap_or("display");
                if !state.show_options(tab) {
                    bail!("unknown options tab {tab:?}");
                }
            }
            if matches!(args.screen, Screen::Saves) {
                let name = args.page.as_deref().unwrap_or("load");
                if !state.show_saves(name) {
                    bail!("unknown --page saves {name:?} (save or load)");
                }
            }
            if matches!(args.screen, Screen::Dialog) {
                let name = args.page.as_deref().unwrap_or("goals");
                if !state.show_dialog(name) {
                    bail!("unknown --page dialog {name:?}");
                }
            }
            if let Some(tool) = &args.tool {
                let t = |p: Option<(f32, f32)>| p.map(|(x, y)| (x as i32, y as i32));
                if !state.show_tool(tool, t(args.drag_from), t(args.hover)) {
                    bail!("cannot select the tool {tool:?} (unknown, or no playable save)");
                }
            }
            if args.apply {
                state.apply_tool();
            }
            if let Some((x, y)) = args.click {
                state.cursor_moved(x, y);
                state.mouse_button(true, true);
                state.mouse_button(true, false);
                state.update(0.0);
            }
        }
    }
    if args.bench > 0 {
        let t = std::time::Instant::now();
        for _ in 0..args.bench {
            state.update(16.7);
            gfx::render_offscreen(&mut gfx, &mut state, args.size)?;
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / f64::from(args.bench);
        eprintln!(
            "{ms:.2} ms/frame (incl. readback) over {} frames",
            args.bench
        );
    }
    let pixels = gfx::render_offscreen(&mut gfx, &mut state, args.size)?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    rt2_render::write_png(out, args.size, &pixels)?;
    eprintln!("wrote {}", out.display());
    Ok(())
}
