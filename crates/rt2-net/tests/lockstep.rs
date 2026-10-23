//! Lockstep over the in-memory transport with the world of the simulation without pools (the
//! rules that need no game data): the same commands run at the same minute in the same order
//! on every machine, with a perfect network and with delay, jitter, loss and duplication.
//! The same flows over a real game are in `tests/real_game.rs` (needs `RT2_DIR`).

mod common;

use common::{Cluster, FRAME_MS};
use rt2_net::mem::Faults;
use rt2_sim::{Command, SpeedChange};

/// Months of game time at speed 3 (320 minutes a frame): 126 frames each.
fn months(n: u32) -> u32 {
    n * 127
}

fn play_script(c: &mut Cluster, frames: u32) {
    for f in 0..frames {
        if f % 37 == 5 {
            let n = c.s.len();
            for i in 0..n {
                if c.s[i].is_some() && f % (3 + i as u32) == 0 {
                    c.session(i)
                        .send_command(Command::SeedRng(f * 31 + i as u32));
                }
            }
        }
        if f % 211 == 17 {
            c.session(1)
                .send_command(Command::SetSpeed(SpeedChange::Delta(1)));
        }
        if f % 523 == 9 {
            c.session(0)
                .send_command(Command::SetSpeed(SpeedChange::Absolute(3)));
        }
        c.run(1);
    }
}

#[test]
fn two_instances_end_with_identical_checksums() {
    let mut c = Cluster::started(2, Faults::default(), 1);
    play_script(&mut c, months(12));
    assert_eq!(c.desyncs(), 0);
    let n = c.assert_checkpoints_agree();
    assert!(n >= 2, "compared {n} checksum points");
    assert_ne!(
        c.session(1).world().net_session().unwrap().cmd_hash(),
        0,
        "commands really ran on the guest"
    );
    assert!(c.session(0).world().minutes() > 1860 * 483_840 + 40_320 * 5);
}

#[test]
fn three_instances_end_with_identical_checksums() {
    let mut c = Cluster::started(3, Faults::default(), 2);
    play_script(&mut c, months(12));
    assert_eq!(c.desyncs(), 0);
    assert!(c.assert_checkpoints_agree() >= 2);
}

#[test]
fn delay_jitter_loss_and_duplication_do_not_break_the_lockstep() {
    let faults = Faults {
        latency_ms: 120,
        jitter_ms: 200,
        loss_permille: 150,
        dup_permille: 80,
    };
    for seed in [3, 4] {
        let mut c = Cluster::started(3, faults, seed);
        play_script(&mut c, months(8));
        assert_eq!(c.desyncs(), 0, "seed {seed}");
        assert!(c.assert_checkpoints_agree() >= 1, "seed {seed}");
        // The game advanced in spite of the faults.
        assert!(
            c.session(2).world().minutes() > 1860 * 483_840 + 40_320 * 4,
            "seed {seed}"
        );
    }
}

#[test]
fn nobody_runs_past_the_horizon_and_the_host_leads_by_about_two_seconds() {
    let mut c = Cluster::started(2, Faults::default(), 5);
    c.run(300);
    let t0 = c.session(0).world().minutes();
    let t1 = c.session(1).world().minutes();
    // 2000 ms of game time at speed 3 is 6400 minutes; machines are within the horizon.
    assert!((t0 - t1).abs() <= 2 * 6400, "{t0} vs {t1}");
    assert!(t0 > 1860 * 483_840 + 100_000);
}

#[test]
fn pause_in_a_network_game_runs_one_minute_per_second() {
    let mut c = Cluster::started(2, Faults::default(), 6);
    c.run(50);
    c.session(1)
        .send_command(Command::SetSpeed(SpeedChange::Absolute(0)));
    c.run(30);
    assert_eq!(c.session(0).world().speed(), 0, "speed 0 reached the host");
    assert_eq!(
        c.session(1).world().speed(),
        0,
        "and the sender: no vote, global"
    );
    let t = c.session(0).world().minutes();
    c.run(600); // 60 s
    let dt = c.session(0).world().minutes() - t;
    assert!(
        (55..=65).contains(&dt),
        "paused clock moved {dt} minutes in 60 s"
    );
}

#[test]
fn chat_reaches_everybody() {
    let mut c = Cluster::started(3, Faults::default(), 7);
    c.session(2).send_chat("hello all");
    c.run(5);
    for i in [0usize, 1] {
        assert!(
            c.events_of(i).iter().any(
                |e| matches!(e, rt2_net::Event::Chat { text, from: 3, .. } if text == "hello all")
            ),
            "machine {i}"
        );
    }
}

#[test]
fn a_guest_with_other_data_is_refused() {
    use rt2_net::{Config, Session, session::default_speed_table};
    let mut c = Cluster::lobby(1, Faults::default(), 8);
    let mut cfg = Config::new("odd", 9, default_speed_table());
    cfg.exe_hash = 12345;
    let mut s = Session::join(c.hub.node("9"), cfg, "mem:0", common::bare_world(), None).unwrap();
    let mut got = Vec::new();
    for k in 1..=40u64 {
        c.run(1);
        got.extend(s.step(k * FRAME_MS + c.now));
    }
    assert!(
        got.iter().any(|e| matches!(e, rt2_net::Event::Rejected(_))),
        "{got:?}"
    );
    assert_eq!(c.session(0).players().len(), 1);
}
