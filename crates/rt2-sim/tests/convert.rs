//! The loader's conversion of the shipped 0x233C files (`rt2_sim::convert`, docs/formats/map-gam.md
//! section 9) against the original.
//!
//! Needs the game install (`RT2_DIR`) and a directory of conversions made by the original
//! (`RT2_CONVERT_DIR`): for each line `name|path` of `index.txt` (a `.MAP`/`.GAM` of the install),
//! `name_d0.GAM` is what the original saved right after loading `path` with
//! `advance_save.py <sb> <path> 0 1234 <out> --preseed 1234` (game RNG 1234 when the load starts).
//! The clone converts the same file from the same RNG state and the two saves must be equal field
//! by field (volatile fields masked). Skipped when either variable is unset.

use std::path::PathBuf;

use rt2_formats::map::{MAGIC_2348, Save};
use rt2_formats::savefields::diff;
use rt2_sim::World;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn data(game: &std::path::Path) -> GameData {
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let mut data = GameData::load(&exe).unwrap();
    data.load_sprites(game).unwrap();
    data
}

#[test]
fn shipped_files_convert_like_the_original() {
    let (Some(game), Some(dir)) = (env_dir("RT2_DIR"), env_dir("RT2_CONVERT_DIR")) else {
        return;
    };
    let data = data(&game);
    let index = std::fs::read_to_string(dir.join("index.txt")).unwrap();
    let mut checked = 0;
    for line in index.lines() {
        let Some((name, path)) = line.split_once('|') else {
            continue;
        };
        let orig_path = dir.join(format!("{name}_d0.GAM"));
        if !orig_path.exists() {
            continue;
        }
        let src = Save::parse(&std::fs::read(path).unwrap()).unwrap();
        let orig = Save::parse(&std::fs::read(&orig_path).unwrap()).unwrap();
        let w = World::from_save_seeded(&src, &data, 1234);
        let clone = w.to_save();
        assert_eq!(clone.layout.version, MAGIC_2348);
        let bad: Vec<String> = diff(&orig, &clone)
            .into_iter()
            .filter(|d| d.masked.is_none())
            .map(|d| d.path)
            .collect();
        assert!(
            bad.is_empty(),
            "{name} ({path}): {:?}",
            &bad[..bad.len().min(20)]
        );
        checked += 1;
    }
    assert!(checked > 0, "no conversions in {}", dir.display());
}

/// Converting a shipped file gives a save that parses back to itself in the 0x2348 layout.
#[test]
fn converted_save_round_trips() {
    let Some(game) = env_dir("RT2_DIR") else {
        return;
    };
    let data = data(&game);
    let src =
        Save::parse(&std::fs::read(game.join("games/Example save game.GAM")).unwrap()).unwrap();
    let s = World::from_save_seeded(&src, &data, 1).to_save();
    let bytes = s.to_bytes();
    let back = Save::parse(&bytes).unwrap();
    assert_eq!(back.layout.version, MAGIC_2348);
    assert_eq!(back.to_bytes(), bytes);
    assert_eq!(back.stations.record_size, 0x4C7);
    assert_eq!(back.companies.record_size, 0x4E86);
}

/// New Game on the first campaign maps (`World::new_game`): starts at the option block's year, runs
/// the initial growth and gives a save that parses back. The options are the map's own block
/// (the setup screen is not ported).
#[test]
fn new_game_starts_campaign_maps() {
    let Some(game) = env_dir("RT2_DIR") else {
        return;
    };
    let data = data(&game);
    let mut maps: Vec<_> = std::fs::read_dir(game.join("maps"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_uppercase();
            n.starts_with('#') && n.ends_with(".MAP")
        })
        .collect();
    maps.sort();
    assert!(!maps.is_empty());
    for p in maps.iter().take(3) {
        let map = Save::parse(&std::fs::read(p).unwrap()).unwrap();
        let name = p.file_stem().unwrap().to_string_lossy().into_owned();
        let opts = rt2_sim::NewGameOptions::from_save(&map, &name, true, 1);
        let w = World::new_game(&map, &opts, &data, 777);
        let s = w.to_save();
        assert_eq!(s.layout.version, MAGIC_2348);
        assert!(
            s.objects.count >= map.objects.count,
            "{name}: initial growth"
        );
        let back = Save::parse(&s.to_bytes()).unwrap();
        assert_eq!(back.header[0x86FC], 1);
    }
}
