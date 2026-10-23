//! The lockstep over a real game: a map of the original with pools, trains, industries, AI and
//! the full rules in network mode. Needs the game install (`RT2_DIR`; skipped otherwise), so it
//! is a local test, not a CI one (docs/net-protocol.md "Tests").

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::{FRAME_MS, config};
use rt2_formats::map::Save;
use rt2_net::mem::{Faults, MemHub, MemTransport};
use rt2_net::{Event, Session, State};
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::fincmd::FinCmd;
use rt2_sim::{Command, World};

fn setup() -> Option<(Arc<GameData>, Save)> {
    setup_save("maps/Alaska.MP2")
}

fn setup_save(file: &str) -> Option<(Arc<GameData>, Save)> {
    let game = PathBuf::from(std::env::var_os("RT2_DIR")?);
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).ok()?).ok()?;
    let mut data = GameData::load(&exe).ok()?;
    data.load_sprites(&game).ok()?;
    let save = Save::parse(&std::fs::read(game.join(file)).ok()?).ok()?;
    Some((Arc::new(data), save))
}

/// `n` humans among the persons of the map.
fn world(data: &GameData, save: &Save, humans: usize) -> World {
    let mut w = World::from_save_seeded(save, data, 1234);
    // The player set of the converted state (an old save is converted on load).
    let st = w.save_ref().unwrap().state.clone();
    let n = u32::from_le_bytes(st[0x28d..0x291].try_into().unwrap()) as usize;
    assert!(n >= humans, "the map has {n} persons in the player set");
    for k in 0..n {
        let p = usize::from(u16::from_le_bytes([st[0x291 + 2 * k], st[0x292 + 2 * k]]));
        w.net_set_person_controller(p, (k < humans).then_some(0));
    }
    w
}

struct Game {
    hub: MemHub,
    s: Vec<Option<Session<MemTransport>>>,
    now: u64,
    log: Vec<(usize, Event)>,
    data: Arc<GameData>,
    /// Checksum points compared so far (the machines keep only the last three).
    verified: std::collections::BTreeSet<i32>,
}

impl Game {
    fn run(&mut self, frames: u32) {
        for _ in 0..frames {
            self.now += FRAME_MS;
            for i in 0..self.s.len() {
                if let Some(s) = self.s[i].as_mut() {
                    for e in s.step(self.now) {
                        self.log.push((i, e));
                    }
                }
            }
            if self.now.is_multiple_of(1000) {
                self.agree();
            }
        }
    }

    /// Compares the checksum points the machines hold now; returns how many were verified
    /// over the whole run.
    fn agree(&mut self) -> usize {
        use std::collections::BTreeMap;
        let mut by: BTreeMap<i32, Vec<(usize, rt2_sim::net::Digest)>> = BTreeMap::new();
        for (i, s) in self.s.iter().enumerate() {
            for (m, d) in s.iter().flat_map(|s| s.checkpoints()) {
                by.entry(m).or_default().push((i, d));
            }
        }
        let mut n = 0;
        for (m, v) in by {
            if v.len() > 1 {
                for (i, d) in &v[1..] {
                    assert_eq!(*d, v[0].1, "minute {m}: machine {i} vs {}", v[0].0);
                }
                n += 1;
                self.verified.insert(m);
            }
        }
        let _ = n;
        self.verified.len()
    }

    fn desyncs(&self) -> usize {
        self.log
            .iter()
            .filter(|(_, e)| matches!(e, Event::Desync { .. }))
            .count()
    }
}

fn start(data: Arc<GameData>, save: &Save, n: usize) -> Game {
    start_with(data, save, n, Faults::default())
}

fn start_with(data: Arc<GameData>, save: &Save, n: usize, faults: Faults) -> Game {
    let hub = MemHub::new(5);
    hub.set_faults(faults);
    let w = world(&data, save, n);
    let host = Session::host(hub.node("0"), config("host", 10), w, Some(data.clone()));
    let mut g = Game {
        hub,
        s: vec![Some(host)],
        now: 0,
        log: Vec::new(),
        data,
        verified: Default::default(),
    };
    for i in 1..n {
        let guest = Session::join(
            g.hub.node(&i.to_string()),
            config(&format!("g{i}"), 10 + i as u32),
            "mem:0",
            World::new(rt2_sim::calendar::Date::new(1860, 1, 1, 0, 0), 0, 0),
            Some(g.data.clone()),
        )
        .unwrap();
        g.s.push(Some(guest));
    }
    g.run(40);
    g.s[0].as_mut().unwrap().start().unwrap();
    g.run(60);
    for s in g.s.iter().flatten() {
        assert_eq!(s.state(), State::Playing);
    }
    g
}

fn company_of_first_human(w: &World) -> i32 {
    w.net_local_company().unwrap_or(0)
}

fn commands(g: &mut Game, frames: u32) {
    for f in 0..frames {
        if f % 150 == 10 {
            for i in 0..g.s.len() {
                if let Some(s) = g.s[i].as_mut() {
                    let co = company_of_first_human(s.world());
                    let v = 0.25 + (f % 7) as f32 * 0.05;
                    s.send_command(Command::Finance(FinCmd::set_dividend(co.max(1), v)));
                }
            }
        }
        g.run(1);
    }
}

#[test]
fn two_instances_of_a_real_map_end_with_identical_checksums() {
    // The example game of the original: trains, stations, industries, computer companies.
    let Some((data, save)) = setup_save("games/Example save game.GAM") else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut g = start(data, &save, 2);
    commands(&mut g, 127 * 14);
    assert_eq!(
        g.desyncs(),
        0,
        "{:?}",
        g.log
            .iter()
            .filter(|(_, e)| matches!(e, Event::Desync { .. } | Event::DesyncDump { .. }))
            .take(3)
            .collect::<Vec<_>>()
    );
    let n = g.agree();
    assert!(n >= 8, "compared {n} monthly checksum points");
    let w = g.s[0].as_ref().unwrap().world();
    eprintln!(
        "real game: {:?}, {} trains, {} persons, digest {:?}",
        w.date(),
        w.save_ref().unwrap().trains.count,
        w.save_ref().unwrap().persons.count,
        w.net_digest()
    );
    // Network rules: both machines know they are in a network game; each controls its person.
    for (i, s) in g.s.iter().flatten().enumerate() {
        assert!(s.world().is_network());
        assert!(
            s.world().net_local_person().is_some(),
            "machine {i} has a person"
        );
    }
    let p0 = g.s[0].as_ref().unwrap().world().net_local_person();
    let p1 = g.s[1].as_ref().unwrap().world().net_local_person();
    assert_ne!(p0, p1);
}

#[test]
fn a_third_player_joins_a_real_game_and_a_leaver_becomes_a_computer() {
    let Some((data, save)) = setup() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let mut g = start(data, &save, 2);
    commands(&mut g, 127 * 3);
    // A third human takes the seat of a person that is a computer... there is none free: the
    // join is refused with "no free seat" until someone leaves.
    let late = Session::join(
        g.hub.node("2"),
        config("late", 12),
        "mem:0",
        World::new(rt2_sim::calendar::Date::new(1860, 1, 1, 0, 0), 0, 0),
        Some(g.data.clone()),
    )
    .unwrap();
    g.s.push(Some(late));
    g.run(100);
    assert!(
        g.log
            .iter()
            .any(|(i, e)| *i == 2 && matches!(e, Event::Rejected(_)))
    );
    g.s[2] = None;
    // The second human leaves: his person becomes a computer, the company stays.
    let p1 = g.s[1].as_ref().unwrap().world().net_local_person().unwrap();
    g.s[1].as_mut().unwrap().leave();
    g.s[1] = None;
    g.run(40);
    let w = g.s[0].as_ref().unwrap().world();
    assert!(!w.is_network(), "one human left: local rules");
    let rec = w.save_ref().unwrap();
    let r = rec
        .persons
        .records
        .iter()
        .find(|r| r.slot as i32 == p1)
        .unwrap();
    assert_eq!(
        u32::from_le_bytes(r.data[0x122..0x126].try_into().unwrap()),
        0,
        "+0x122 = 0: a computer"
    );
    commands(&mut g, 127 * 2);
    // The returning player (same identity) gets his seat back through a snapshot.
    let back = Session::join(
        g.hub.node("3"),
        config("g1", 11),
        "mem:0",
        World::new(rt2_sim::calendar::Date::new(1860, 1, 1, 0, 0), 0, 0),
        Some(g.data.clone()),
    )
    .unwrap();
    g.s.push(Some(back));
    g.run(150);
    assert_eq!(
        g.s[3].as_ref().unwrap().state(),
        State::Playing,
        "{:?}",
        g.log.iter().filter(|(i, _)| *i == 3).collect::<Vec<_>>()
    );
    commands(&mut g, 127 * 4);
    assert_eq!(
        g.desyncs(),
        0,
        "{:?}",
        g.log
            .iter()
            .filter(|(_, e)| matches!(e, Event::DesyncDump { .. }))
            .take(2)
            .collect::<Vec<_>>()
    );
    assert!(g.agree() >= 2);
    let _ = Faults::default();
}

/// Delay, jitter, loss and duplication make the machines run different frame sizes (they catch up
/// with the horizon at their own pace): the state must not care.
#[test]
fn a_real_game_survives_delay_loss_and_uneven_pacing() {
    let Some((data, save)) = setup_save("games/Example save game.GAM") else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let faults = Faults {
        latency_ms: 150,
        jitter_ms: 250,
        loss_permille: 100,
        dup_permille: 50,
    };
    let mut g = start_with(data, &save, 2, faults);
    commands(&mut g, 127 * 10);
    assert_eq!(g.desyncs(), 0);
    assert!(g.agree() >= 5);
    // Both machines kept up with the clock: within a few seconds of game time.
    let t: Vec<i32> = g.s.iter().flatten().map(|s| s.world().minutes()).collect();
    assert!((t[0] - t[1]).abs() < 20_000, "{t:?}");
}
