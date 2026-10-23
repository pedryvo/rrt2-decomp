//! End to end, no window: a game started from scratch the way the menus start it (`New Scenario`
//! on B&O through `World::new_game` with the options of the setup screen), the player founds a
//! company with the dialog's numbers (command `0x26`), builds track and two stations, buys a
//! train (`BuyTrain` + `TrainMark`), and the world runs 60 days: revenue and messages appear.
//! Also: every shipped `.GAM` loads through `World::from_save_seeded` and runs.
//!
//! Needs the install (`RT2_DIR`, default `~/Games/railroad-tycoon-2`); without it the tests are
//! skipped.

#![allow(dead_code)]

#[path = "../src/buildtools.rs"]
mod buildtools;
#[path = "../src/founding.rs"]
mod founding;
#[path = "../src/setup.rs"]
mod setup;

use std::path::PathBuf;

use buildtools::*;

const PE_HUMAN: usize = 0x122;
const PE_CASH: usize = 0x12a;
use rt2_formats::map::Save;
use rt2_sim::build::cmd::{BuildCmd, CmdError};
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::found::{FoundCompany, founding_preview};
use rt2_sim::{Command, World};

fn install() -> Option<PathBuf> {
    let p = std::env::var_os("RT2_DIR").map(PathBuf::from).or_else(|| {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Games/railroad-tycoon-2"))
    })?;
    p.join("RT2_PLAT.EXE").is_file().then_some(p)
}

fn data(dir: &std::path::Path) -> GameData {
    let exe = Exe::from_bytes(std::fs::read(dir.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    GameData::load(&exe).unwrap()
}

fn save_of(w: &World) -> &Save {
    w.save_ref().expect("a world from a save")
}

/// The player: the first person with the human flag, as `info::founder` picks.
fn human_person(save: &Save) -> i32 {
    save.persons
        .ordered()
        .find(|r| Rec(&r.data).u8(PE_HUMAN) != 0)
        .or_else(|| save.persons.ordered().next())
        .expect("a person")
        .slot as i32
}

fn person_cash(save: &Save, person: i32) -> f64 {
    rec(&save.persons, person).map_or(0.0, |p| p.f64(PE_CASH))
}

fn income(save: &Save, co: i32) -> i64 {
    company_trains(save, co)
        .into_iter()
        .map(|t| i64::from(rec(&save.trains, t).map_or(0, |r| r.i32(0xfd))))
        .sum()
}

#[test]
fn new_scenario_found_company_build_and_earn() {
    let Some(dir) = install() else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    let data = data(&dir);
    // The campaign maps come with the player's company (B&O: company 1, chaired by the human
    // person): Start New Company is refused for them. Scenarios such as "Africa After the
    // Flood" have no active company: the founding flow below.
    let b_o = std::fs::read_dir(dir.join("maps"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("#01"))
        })
        .expect("the B&O map");
    let b_o = Save::parse(&std::fs::read(&b_o).unwrap()).unwrap();
    let o = setup::Setup::default().new_game_options(&b_o, "B&O", None, 1);
    let mut w = World::new_game(&b_o, &o, &data, 1);
    let owner = human_company(save_of(&w)).expect("B&O starts with the player's company");
    let person = human_person(save_of(&w));
    let p = founding_preview(100_000, 400_000);
    let again = FoundCompany::new(person, "Second", -1, p.capital, 100_000);
    assert_eq!(w.apply_found(again), Err(CmdError::AlreadyChairman));
    assert_eq!(human_company(save_of(&w)), Some(owner));

    let map =
        Save::parse(&std::fs::read(dir.join("maps/Africa After the Flood.MP2")).unwrap()).unwrap();

    // 1. New Scenario with the default setup (the options of the Game Setup screen).
    let setup = setup::Setup::default();
    let seed = 1;
    let opts = setup.new_game_options(&map, "Africa", None, seed);
    let mut world = World::new_game(&map, &opts, &data, seed);
    world.apply(Command::SeedRng(seed));
    assert!(
        human_company(save_of(&world)).is_none(),
        "a new scenario starts without a player company"
    );

    // 2. Start New Company with the dialog's numbers and a name.
    let person = human_person(save_of(&world));
    let cash = person_cash(save_of(&world), person);
    assert!(cash >= 100_000.0, "the founder has money: {cash}");
    let confidence = founding::investor_confidence(true, 0, None);
    let mut dlg = founding::Founding::new(cash, confidence);
    dlg.move_player(1);
    let p = founding_preview(dlg.player_value(), dlg.outside_value());
    assert_eq!(
        p.founder_shares % 1000,
        0,
        "the founder buys in lots of 1000"
    );
    let cmd = FoundCompany::new(
        person,
        "Play Test Railroad",
        -1,
        p.capital,
        dlg.player_value(),
    );
    let co = world.apply_found(cmd).expect("found company");
    assert_eq!(human_company(save_of(&world)), Some(co));
    // Refusals the dialog must show: a second company by the same person.
    assert_eq!(
        world.apply_found(cmd),
        Err(CmdError::AlreadyChairman),
        "a chairman cannot found a second company"
    );
    let cash0 = bt_cash(&world, co);
    assert!(cash0 > 0.0);

    // 3. Track between two towns of the map, two stations, one train.
    let (a, b) = two_sites(&world, &data, co);
    let lay = BuildCmd::LayTrack {
        company: co,
        x1: a.0,
        y1: a.1,
        x2: b.0,
        y2: b.1,
        d1: -1,
        d2: -1,
        single: false,
        electric: false,
        bridge: 1,
    };
    let price = preview(&world, co, lay).result.expect("a route exists");
    assert!(price > 0);
    assert_eq!(world.apply_build(lay).expect("lay track"), price);
    let mut stations = Vec::new();
    for (x, y) in [a, b] {
        build_station_near(&mut world, &data, co, x, y);
        // The command answers with the building; the station is the company's newest.
        stations.push(
            company_stations(save_of(&world), co)
                .into_iter()
                .max()
                .expect("a station"),
        );
    }
    let d = world.date();
    let rows = engine_rows(
        &data,
        save_of(&world),
        co,
        i32::from(d.year),
        i32::from(d.month),
    );
    let engine = rows.iter().find(|r| r.kind != 2).expect("an engine").index;
    let train = world
        .apply_build(BuildCmd::BuyTrain {
            company: co,
            station: 0,
            engine,
            copy_from: 0,
        })
        .expect("buy train");
    world
        .apply_build(BuildCmd::TrainMark { train })
        .expect("mark train");
    for (k, station) in [(2, stations[0]), (3, stations[1])] {
        world
            .apply_build(BuildCmd::InsertStop {
                train,
                k,
                station,
                waypoint: false,
                x: 0,
                y: 0,
            })
            .expect("insert stop");
    }
    for _ in 0..2 {
        world
            .apply_build(BuildCmd::DeleteStop { train, k: 0 })
            .expect("delete stop");
    }

    // 4. Sixty days.
    let income0 = income(save_of(&world), co);
    let mut messages = world.take_messages();
    for _ in 0..60 * 1440 / 320 {
        world.advance(320);
        messages.extend(world.take_messages());
    }
    let income1 = income(save_of(&world), co);
    eprintln!(
        "income {income0} -> {income1}, {} messages, first: {:?}",
        messages.len(),
        messages.first()
    );
    assert!(income1 > income0, "the company's trains earned revenue");
    assert!(!messages.is_empty(), "the world produced messages");
    assert!(world.take_messages().is_empty(), "the queue is drained");
}

fn bt_cash(w: &World, co: i32) -> f64 {
    cash(save_of(w), co)
}

/// Two tiles far apart on the same land where a station can be built after the track: the
/// first two stations of the map's own towns would do, but a new scenario may have none, so the
/// sites come from the first two cities (`places` with `+0x10 == 0`).
fn two_sites(world: &World, data: &GameData, co: i32) -> ((i32, i32), (i32, i32)) {
    let save = save_of(world);
    let cities: Vec<(i32, i32)> = save
        .places
        .ordered()
        .filter(|r| r.data[0x10] == 0)
        .map(|r| {
            let i = |o: usize| i32::from(i16::from_le_bytes([r.data[o], r.data[o + 1]]));
            (i(4), i(6))
        })
        .collect();
    assert!(cities.len() >= 2, "the map has cities");
    // Pairs from nearest first: a short, buildable route.
    let mut pairs = Vec::new();
    for (i, a) in cities.iter().enumerate() {
        for b in &cities[i + 1..] {
            let d = (a.0 - b.0).abs() + (a.1 - b.1).abs();
            pairs.push((d, *a, *b));
        }
    }
    pairs.sort();
    for (_, a, b) in pairs {
        let lay = BuildCmd::LayTrack {
            company: co,
            x1: a.0,
            y1: a.1,
            x2: b.0,
            y2: b.1,
            d1: -1,
            d2: -1,
            single: false,
            electric: false,
            bridge: 1,
        };
        let _ = data;
        if preview(world, co, lay).result.is_ok() {
            return (a, b);
        }
    }
    panic!("no pair of cities can be joined by track");
}

/// Builds a small station at the first accepted site around `(x, y)`.
fn build_station_near(world: &mut World, data: &GameData, co: i32, x: i32, y: i32) {
    for r in 0..=8 {
        for (dx, dy) in (-r..=r).flat_map(|a| [(a, -r), (a, r), (-r, a), (r, a)]) {
            let (sx, sy) = (x + dx, y + dy);
            let (o, p) = find_station(world, data, co, StationSize::Small, sx, sy, -1, None);
            if p.result.is_ok() {
                let cmd = station_command(data, co, StationSize::Small.shape(o), sx, sy, -1);
                world.apply_build(cmd).expect("build station");
                return;
            }
        }
    }
    panic!("no station site near ({x}, {y})");
}

/// Every shipped saved game loads (the old layout is converted) and runs 3 days.
#[test]
fn shipped_saves_load_and_run() {
    let Some(dir) = install() else {
        return;
    };
    let data = data(&dir);
    for name in ["Example save game.GAM", "Tutorial.GAM"] {
        let path = dir.join("games").join(name);
        let save = Save::parse(&std::fs::read(&path).unwrap()).unwrap();
        let mut w = World::from_save_seeded(&save, &data, 1);
        w.apply(Command::SeedRng(1));
        for _ in 0..3 * 1440 / 320 {
            w.advance(320);
        }
        assert!(playable(save_of(&w)), "{name}: converted to 0x2348");
        let _ = w.take_messages();
    }
}

/// Shipped scenario maps start a new game: B&O, one from each end of the list and, with
/// `RT2_ALL_MAPS=1`, all of them (slow in a debug build).
#[test]
fn shipped_maps_start() {
    let Some(dir) = install() else {
        return;
    };
    let data = data(&dir);
    let mut n = 0;
    let mut files: Vec<_> = std::fs::read_dir(dir.join("maps"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    files.sort();
    let all = std::env::var_os("RT2_ALL_MAPS").is_some();
    let n_files = files.len();
    let picked: Vec<_> = files
        .into_iter()
        .enumerate()
        .filter(|(i, _)| all || [0, 20, n_files - 1].contains(i))
        .map(|(_, p)| p)
        .collect();
    for path in picked {
        let Ok(map) = Save::parse(&std::fs::read(&path).unwrap()) else {
            panic!("{} does not parse as a save", path.display());
        };
        let o = setup::Setup::default().new_game_options(&map, "x", None, 1);
        let mut w = World::new_game(&map, &o, &data, 1);
        w.advance(320);
        n += 1;
    }
    assert!(n > 0);
}

/// The start year of a new game is the scenario's own (B&O: 1830), not the stale year of the
/// map file's option block (1880).
#[test]
fn new_game_starts_in_the_scenario_year() {
    let Some(dir) = install() else {
        return;
    };
    let data = data(&dir);
    let map = Save::parse(&std::fs::read(dir.join("maps/#01B&O~1.MAP")).unwrap()).unwrap();
    let o = setup::Setup::default().new_game_options(&map, "B&O", None, 1);
    let w = World::new_game(&map, &o, &data, 1);
    assert_eq!(w.date().year, 1830);
}
