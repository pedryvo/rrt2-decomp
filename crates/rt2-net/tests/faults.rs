//! Desynchronisation, disconnection, host migration, join in progress and reconnection, over the
//! in-memory transport with the world without pools.

mod common;

use common::Cluster;
use rt2_net::mem::Faults;
use rt2_net::{Event, State};
use rt2_sim::Command;
use rt2_sim::calendar::MINUTES_PER_MONTH;

const START: i32 = 1860 * 483_840;

fn run_months(c: &mut Cluster, n: u32) {
    c.run(n * 127);
}

#[test]
fn a_divergent_machine_is_detected_dumped_and_resynchronised() {
    let dir = std::env::temp_dir().join(format!("rt2-net-desync-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut c = Cluster::lobby(3, Faults::default(), 11);
    c.session(0).debug_set_dump_dir(&dir);
    c.session(0).start().unwrap();
    c.run(40);
    run_months(&mut c, 2);
    assert_eq!(c.desyncs(), 0);
    // Machine 2 runs a command nobody else runs: its state diverges.
    c.session(2).debug_apply_local(Command::SeedRng(4242));
    run_months(&mut c, 2);
    let d: Vec<_> = c
        .log
        .iter()
        .filter_map(|(_, e)| match e {
            Event::Desync { peer, groups, .. } => Some((*peer, groups.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].0, 3, "the third player (machine 2) diverged");
    assert!(
        d[0].1.contains(&"other") || d[0].1.contains(&"rng"),
        "{:?}",
        d[0].1
    );
    // The dump of the diff.
    let dump = c
        .log
        .iter()
        .find_map(|(_, e)| match e {
            Event::DesyncDump { path, lines, .. } => Some((path.clone(), lines.clone())),
            _ => None,
        })
        .expect("a dump event");
    let path = dump.0.expect("written to the dump directory");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("desync at game minute"), "{text}");
    assert!(!dump.1.is_empty());
    // The host's state replaced the divergent one: later checkpoints agree again.
    assert!(
        c.events_of(2).iter().any(|e| matches!(e, Event::Resynced)),
        "machine 2 was resynchronised"
    );
    let before = c.desyncs();
    run_months(&mut c, 3);
    assert_eq!(c.desyncs(), before, "no new desync after the resync");
    c.assert_checkpoints_agree();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_guest_that_vanishes_becomes_a_computer_and_the_game_goes_on() {
    let mut c = Cluster::started(3, Faults::default(), 12);
    run_months(&mut c, 2);
    // Pulled cable: nothing tells the others; 6 s of silence cut the player.
    c.hub.cut("mem:2");
    c.s[2] = None;
    c.run(150);
    for i in [0usize, 1] {
        assert!(
            c.events_of(i)
                .iter()
                .any(|e| matches!(e, Event::PlayerLeft { id: 3, .. })),
            "machine {i} saw the player leave"
        );
    }
    // The removal ran through the batch on both machines (three players: command 0x01).
    assert_eq!(
        c.session(0).world().net_session().unwrap().roster().len(),
        2
    );
    assert_eq!(
        c.session(1).world().net_session().unwrap().roster().len(),
        2
    );
    assert!(c.session(0).world().is_network());
    let m0 = c.session(0).world().minutes();
    run_months(&mut c, 3);
    assert!(c.session(0).world().minutes() > m0 + 2 * MINUTES_PER_MONTH);
    assert_eq!(c.desyncs(), 0);
    c.assert_checkpoints_agree();
}

#[test]
fn closing_a_connection_is_noticed_at_once() {
    let mut c = Cluster::started(3, Faults::default(), 13);
    run_months(&mut c, 1);
    c.session(2).leave();
    c.run(20);
    for i in [0usize, 1] {
        assert!(
            c.events_of(i).iter().any(|e| matches!(
                e,
                Event::PlayerLeft {
                    id: 3,
                    reason: rt2_net::session::LeaveReason::Left,
                    ..
                }
            )),
            "machine {i}"
        );
    }
    c.run(150);
    assert_eq!(
        c.session(1).world().net_session().unwrap().roster().len(),
        2
    );
}

#[test]
fn the_host_dies_and_the_lowest_id_takes_over() {
    let mut c = Cluster::started(3, Faults::default(), 14);
    run_months(&mut c, 2);
    // The host crashes: the guests are told by the closed links.
    c.hub.kill("mem:0");
    c.s[0] = None;
    c.run(100);
    assert_eq!(c.session(1).host_id(), 2, "lowest id after the host");
    assert_eq!(c.session(2).host_id(), 2);
    assert!(c.session(1).is_host());
    for i in [1usize, 2] {
        assert!(
            c.events_of(i)
                .iter()
                .any(|e| matches!(e, Event::HostChanged { host: 2 })),
            "machine {i}"
        );
        assert_eq!(
            c.session(i).world().net_session().unwrap().roster().len(),
            2,
            "machine {i}"
        );
    }
    let m = c.session(1).world().minutes();
    run_months(&mut c, 3);
    assert!(
        c.session(1).world().minutes() > m + 2 * MINUTES_PER_MONTH,
        "the clock keeps running"
    );
    assert_eq!(c.desyncs(), 0);
    c.assert_checkpoints_agree();
    // Commands of the remaining guest go through the new host.
    c.session(2).send_command(Command::SeedRng(5));
    c.run(60);
    assert_eq!(
        c.session(1).world().net_session().unwrap().cmd_hash(),
        c.session(2).world().net_session().unwrap().cmd_hash(),
        "both ran the command"
    );
}

#[test]
fn a_silent_host_is_replaced_after_the_silence_limit() {
    let mut c = Cluster::started(3, Faults::default(), 15);
    run_months(&mut c, 1);
    c.hub.cut("mem:0");
    c.s[0] = None;
    c.run(150); // 15 s > 6 s of silence
    assert_eq!(c.session(1).host_id(), 2);
    assert_eq!(c.session(2).host_id(), 2);
    run_months(&mut c, 2);
    c.assert_checkpoints_agree();
}

#[test]
fn the_last_human_left_plays_a_local_game() {
    let mut c = Cluster::started(2, Faults::default(), 16);
    run_months(&mut c, 1);
    c.session(1).leave();
    c.s[1] = None;
    c.run(30);
    assert!(
        !c.session(0).world().is_network(),
        "one player: single-player rules"
    );
    assert_eq!(c.session(0).players().len(), 1);
    // Commands run at once now.
    let h = c.session(0).world().net_session().map(|n| n.cmd_hash());
    c.session(0).send_command(Command::SeedRng(1));
    assert_ne!(c.session(0).world().net_session().map(|n| n.cmd_hash()), h);
    let m = c.session(0).world().minutes();
    c.run(100);
    assert!(c.session(0).world().minutes() > m);
}

#[test]
fn a_player_joins_a_running_game_from_the_snapshot() {
    let mut c = Cluster::started(2, Faults::default(), 17);
    run_months(&mut c, 2);
    c.add_guest(2, 2002);
    c.run(200);
    assert_eq!(c.session(2).state(), State::Playing);
    assert_eq!(c.session(2).players().len(), 3);
    for i in 0..3 {
        assert_eq!(
            c.session(i).world().net_session().unwrap().roster().len(),
            3,
            "machine {i}"
        );
    }
    run_months(&mut c, 3);
    assert_eq!(c.desyncs(), 0);
    let n = c.assert_checkpoints_agree();
    assert!(n >= 2);
    // The joiner has checkpoints of its own, equal to the others'.
    assert!(!c.session(2).checkpoints().is_empty());
}

#[test]
fn a_player_reconnects_after_dropping() {
    let mut c = Cluster::started(3, Faults::default(), 18);
    run_months(&mut c, 2);
    c.session(2).leave();
    c.s[2] = None;
    c.run(150);
    assert_eq!(
        c.session(0).world().net_session().unwrap().roster().len(),
        2
    );
    // Same identity comes back and gets a new network id.
    c.add_guest(2, 1002);
    c.run(250);
    assert_eq!(c.session(2).state(), State::Playing);
    for i in 0..3 {
        assert_eq!(
            c.session(i).world().net_session().unwrap().roster().len(),
            3,
            "machine {i}"
        );
    }
    run_months(&mut c, 3);
    assert_eq!(c.desyncs(), 0);
    c.assert_checkpoints_agree();
    assert!(c.session(2).world().minutes() > START);
}

#[test]
fn joining_is_refused_when_the_option_is_off() {
    let mut c = Cluster::started(2, Faults::default(), 19);
    c.session(0).debug_set_join_in_progress(false);
    c.add_guest(2, 55);
    c.run(100);
    assert!(
        c.events_of(2)
            .iter()
            .any(|e| matches!(e, Event::Rejected(_)))
    );
    assert_eq!(c.session(0).players().len(), 2);
}
