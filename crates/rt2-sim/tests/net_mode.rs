//! The network mode of the simulation on a real game (docs/spec/multiplayer.md 1.6, 3.4, 5.1).
//! Needs the game install (`RT2_DIR`, skipped otherwise). The single-player rules are checked by
//! the fidelity scoreboard (`scripts/fidelity.py`) and the other tests of this crate; here the
//! network world is compared with itself.

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::net::NetPlayer;
use rt2_sim::{Command, World};

fn setup() -> Option<(GameData, Save)> {
    let game = PathBuf::from(std::env::var_os("RT2_DIR")?);
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).ok()?).ok()?;
    let mut data = GameData::load(&exe).ok()?;
    data.load_sprites(&game).ok()?;
    let save = Save::parse(&std::fs::read(game.join("games/Example save game.GAM")).ok()?).ok()?;
    Some((data, save))
}

fn players(n: u32) -> Vec<NetPlayer> {
    (1..=n)
        .map(|id| NetPlayer {
            id,
            identity: 50 + id,
            name: format!("p{id}"),
        })
        .collect()
}

/// A network world with two humans (the Example has four persons in its player set).
fn world(data: &GameData, save: &Save) -> World {
    let mut w = World::from_save_seeded(save, data, 1234);
    let st = w.save_ref().unwrap().state.clone();
    let n = u32::from_le_bytes(st[0x28d..0x291].try_into().unwrap()) as usize;
    for k in 0..n.min(2) {
        let p = usize::from(u16::from_le_bytes([st[0x291 + 2 * k], st[0x292 + 2 * k]]));
        w.net_set_person_controller(p, Some(0));
    }
    w.net_begin(players(2), 1);
    w
}

fn advance(w: &mut World, minutes: i32, step: i32) {
    let mut left = minutes;
    while left > 0 {
        let dt = left.min(step);
        w.advance(dt);
        left -= dt;
    }
}

/// In a network game the trains move once a day and the RNG is re-seeded daily, so the state
/// does not depend on the size of the steps the machine runs (frame rate independence, the
/// reason the original does it).
#[test]
fn the_network_state_does_not_depend_on_the_frame_rate() {
    let Some((data, save)) = setup() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let total = 1440 * 40;
    let mut a = world(&data, &save);
    let mut b = world(&data, &save);
    assert_eq!(a.net_digest(), b.net_digest());
    advance(&mut a, total, 320);
    advance(&mut b, total, 1000);
    let (da, db) = (a.net_digest(), b.net_digest());
    assert_eq!(
        da.differs(&db),
        0,
        "{:?}",
        a.net_checkpoint().diff_lines(&b.net_checkpoint())
    );
    let log = a.hook_log().unwrap();
    assert!(log.count("trains_advance") >= 40, "daily train steps");
    assert!(log.count("net_daily_reseed") >= 40);
}

/// The same world played in local mode moves the trains with the frame step instead, and does
/// not re-seed: the two modes really differ.
#[test]
fn a_local_game_does_not_take_the_network_steps() {
    let Some((data, save)) = setup() else {
        return;
    };
    let mut w = World::from_save_seeded(&save, &data, 1234);
    advance(&mut w, 1440 * 5, 320);
    let log = w.hook_log().unwrap();
    assert_eq!(log.count("trains_advance"), 0);
    assert_eq!(log.count("net_daily_reseed"), 0);
    assert!(log.count("train_step") > 0);
}

#[test]
fn a_human_who_leaves_becomes_a_computer_and_keeps_the_company() {
    let Some((data, save)) = setup() else {
        return;
    };
    let mut w = world(&data, &save);
    let p = w.net_local_person().expect("player 1 has a person");
    let company = w.net_local_company().unwrap();
    assert_ne!(company, 0);
    advance(&mut w, 1440 * 3, 320);
    w.apply(Command::NetPlayerLeft { net_id: 1 });
    assert_eq!(w.net_local_person(), None, "the person is not his any more");
    let rec = w.save_ref().unwrap();
    let r = rec
        .persons
        .records
        .iter()
        .find(|r| r.slot as i32 == p)
        .unwrap();
    let u32_at = |o: usize| u32::from_le_bytes(r.data[o..o + 4].try_into().unwrap());
    assert_eq!(u32_at(0x122), 0, "+0x122 = 0");
    assert_eq!(r.data[0x52f], 0, "+0x52f = 0");
    assert_eq!(u32_at(0x548), 3);
    assert_eq!(u32_at(0x267), 0);
    assert_eq!(
        u32_at(0x263) as i32,
        company,
        "the chair and the company stay"
    );
    // One human left: local rules, the game keeps running.
    assert!(!w.is_network());
    advance(&mut w, 1440 * 40, 320);
}

#[test]
fn a_snapshot_of_a_real_game_rebuilds_an_equivalent_world() {
    let Some((data, save)) = setup() else {
        return;
    };
    let mut w = world(&data, &save);
    advance(&mut w, 1440 * 60 + 777, 320);
    let snap = w.net_snapshot();
    let mut r = World::from_net_snapshot(&snap, Some(&data)).unwrap();
    r.net_attach(&snap, 2);
    let (a, b) = (w.net_checkpoint(), r.net_checkpoint());
    assert_eq!(a.digest.differs(&b.digest), 0, "{:?}", a.diff_lines(&b));
    // The reloaded world and the one that kept running stay together: the load is what the
    // original does too (multiplayer.md 5.3).
    advance(&mut w, 1440 * 70, 320);
    advance(&mut r, 1440 * 70, 320);
    let (a, b) = (w.net_checkpoint(), r.net_checkpoint());
    assert_eq!(a.digest.differs(&b.digest), 0, "{:?}", a.diff_lines(&b));
}
