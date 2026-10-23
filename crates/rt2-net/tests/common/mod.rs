//! Shared helpers of the network tests: a cluster of sessions over a [`MemHub`] driven by
//! logical time (no wall clock, no sleeping).

#![allow(dead_code)]

use rt2_net::mem::{Faults, MemHub, MemTransport};
use rt2_net::session::default_speed_table;
use rt2_net::{Config, Event, Session, State};
use rt2_sim::World;
use rt2_sim::calendar::Date;

pub const FRAME_MS: u64 = 100;

pub fn bare_world() -> World {
    World::new(Date::new(1860, 1, 1, 0, 0), 7, 1)
}

pub fn config(name: &str, identity: u32) -> Config {
    let mut c = Config::new(name, identity, default_speed_table());
    // Short timers keep the tests fast; the defaults are the original's.
    c.silence_ms = 6000;
    c.probe_after_ms = 2000;
    c.probe_every_ms = 1000;
    c.send_idle_ms = 1000;
    c.resend_ms = 300;
    c
}

pub struct Cluster {
    pub hub: MemHub,
    pub s: Vec<Option<Session<MemTransport>>>,
    pub now: u64,
    pub log: Vec<(usize, Event)>,
}

impl Cluster {
    /// A host and `n - 1` guests in the lobby, game not started.
    pub fn lobby(n: usize, faults: Faults, seed: u64) -> Cluster {
        let hub = MemHub::new(seed);
        hub.set_faults(faults);
        let s = vec![Some(Session::host(
            hub.node("0"),
            config("host", 1000),
            bare_world(),
            None,
        ))];
        let mut c = Cluster {
            hub,
            s,
            now: 0,
            log: Vec::new(),
        };
        for i in 1..n {
            c.add_guest(i, 1000 + i as u32);
        }
        c.run(30);
        c
    }

    pub fn add_guest(&mut self, i: usize, identity: u32) {
        let sess = Session::join(
            self.hub.node(&i.to_string()),
            config(&format!("guest{i}"), identity),
            "mem:0",
            bare_world(),
            None,
        )
        .unwrap();
        while self.s.len() <= i {
            self.s.push(None);
        }
        self.s[i] = Some(sess);
    }

    /// Lobby, then start and run until everybody plays.
    pub fn started(n: usize, faults: Faults, seed: u64) -> Cluster {
        let mut c = Cluster::lobby(n, faults, seed);
        c.s[0].as_mut().unwrap().start().unwrap();
        c.run(40);
        for (i, s) in c.s.iter().enumerate() {
            if let Some(s) = s {
                assert_eq!(s.state(), State::Playing, "machine {i}");
            }
        }
        c
    }

    /// Steps every live session once per frame for `frames` frames.
    pub fn run(&mut self, frames: u32) {
        for _ in 0..frames {
            self.now += FRAME_MS;
            for i in 0..self.s.len() {
                if let Some(s) = self.s[i].as_mut() {
                    let now = self.now;
                    for e in s.step(now) {
                        self.log.push((i, e));
                    }
                }
            }
        }
    }

    pub fn session(&mut self, i: usize) -> &mut Session<MemTransport> {
        self.s[i].as_mut().unwrap()
    }

    pub fn events_of(&self, i: usize) -> Vec<&Event> {
        self.log
            .iter()
            .filter(|(j, _)| *j == i)
            .map(|(_, e)| e)
            .collect()
    }

    pub fn desyncs(&self) -> usize {
        self.log
            .iter()
            .filter(|(_, e)| matches!(e, Event::Desync { .. }))
            .count()
    }

    /// Every checksum point shared by two or more machines has the same digest everywhere.
    /// Returns how many points were compared.
    pub fn assert_checkpoints_agree(&self) -> usize {
        use std::collections::BTreeMap;
        let mut by_minute: BTreeMap<i32, Vec<(usize, rt2_sim::net::Digest)>> = BTreeMap::new();
        for (i, s) in self.s.iter().enumerate() {
            if let Some(s) = s {
                for (m, d) in s.checkpoints() {
                    by_minute.entry(m).or_default().push((i, d));
                }
            }
        }
        let mut compared = 0;
        for (m, v) in by_minute {
            if v.len() < 2 {
                continue;
            }
            for (i, d) in &v[1..] {
                assert_eq!(
                    *d, v[0].1,
                    "minute {m}: machine {i} differs from machine {}",
                    v[0].0
                );
            }
            compared += 1;
        }
        compared
    }
}
