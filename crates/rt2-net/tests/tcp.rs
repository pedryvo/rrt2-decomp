//! The same lockstep over real TCP sockets on localhost: two and three instances in one process
//! play a sequence of commands and end with identical checksums; a guest that closes is replaced
//! by a computer.

use std::time::Duration;

use rt2_net::session::default_speed_table;
use rt2_net::tcp::TcpTransport;
use rt2_net::{Config, Event, Session, State};
use rt2_sim::calendar::Date;
use rt2_sim::{Command, SpeedChange, World};

fn world() -> World {
    World::new(Date::new(1860, 1, 1, 0, 0), 7, 1)
}

fn cfg(name: &str, identity: u32) -> Config {
    Config::new(name, identity, default_speed_table())
}

struct Net {
    s: Vec<Option<Session<TcpTransport>>>,
    now: u64,
    log: Vec<(usize, Event)>,
}

impl Net {
    /// Logical frames with a short real pause so the socket threads deliver.
    fn run(&mut self, frames: u32) {
        for _ in 0..frames {
            self.now += 100;
            for i in 0..self.s.len() {
                if let Some(s) = self.s[i].as_mut() {
                    for e in s.step(self.now) {
                        self.log.push((i, e));
                    }
                }
            }
            std::thread::sleep(Duration::from_micros(300));
        }
    }
}

fn start(n: usize) -> Net {
    let host = Session::host(
        TcpTransport::listen("127.0.0.1:0").unwrap(),
        cfg("host", 100),
        world(),
        None,
    );
    let addr = host.local_addr();
    let mut net = Net {
        s: vec![Some(host)],
        now: 0,
        log: Vec::new(),
    };
    for i in 1..n {
        let g = Session::join(
            TcpTransport::listen("127.0.0.1:0").unwrap(),
            cfg(&format!("guest{i}"), 100 + i as u32),
            &addr,
            world(),
            None,
        )
        .unwrap();
        net.s.push(Some(g));
        net.run(60);
    }
    net.run(60);
    assert_eq!(net.s[0].as_ref().unwrap().players().len(), n);
    net.s[0].as_mut().unwrap().start().unwrap();
    net.run(200);
    for s in net.s.iter().flatten() {
        assert_eq!(s.state(), State::Playing);
    }
    net
}

fn agree(net: &Net) -> usize {
    use std::collections::BTreeMap;
    let mut by: BTreeMap<i32, Vec<rt2_sim::net::Digest>> = BTreeMap::new();
    for s in net.s.iter().flatten() {
        for (m, d) in s.checkpoints() {
            by.entry(m).or_default().push(d);
        }
    }
    let mut n = 0;
    for (m, v) in by {
        if v.len() > 1 {
            assert!(v.iter().all(|d| *d == v[0]), "minute {m}");
            n += 1;
        }
    }
    n
}

fn play(net: &mut Net, frames: u32) {
    for f in 0..frames {
        let n = net.s.len();
        if f % 29 == 3 {
            for i in 0..n {
                if let Some(s) = net.s[i].as_mut()
                    && s.state() == State::Playing
                {
                    s.send_command(Command::SeedRng(f * 7 + i as u32));
                }
            }
        }
        if f % 400 == 100
            && let Some(s) = net.s[1].as_mut()
        {
            s.send_command(Command::SetSpeed(SpeedChange::Delta(1)));
        }
        net.run(1);
    }
}

#[test]
fn two_instances_over_tcp_agree() {
    let mut net = start(2);
    play(&mut net, 127 * 6);
    let n = agree(&net);
    assert!(n >= 2, "{n}");
    let desync = net
        .log
        .iter()
        .any(|(_, e)| matches!(e, Event::Desync { .. }));
    assert!(!desync);
}

#[test]
fn three_instances_over_tcp_agree_and_survive_a_departure() {
    let mut net = start(3);
    play(&mut net, 127 * 4);
    assert!(agree(&net) >= 1);
    net.s[2].as_mut().unwrap().leave();
    net.s[2] = None;
    net.run(200);
    assert_eq!(
        net.s[0]
            .as_ref()
            .unwrap()
            .world()
            .net_session()
            .unwrap()
            .roster()
            .len(),
        2
    );
    assert_eq!(
        net.s[1]
            .as_ref()
            .unwrap()
            .world()
            .net_session()
            .unwrap()
            .roster()
            .len(),
        2
    );
    play(&mut net, 127 * 3);
    assert!(agree(&net) >= 2);
    assert!(
        !net.log
            .iter()
            .any(|(_, e)| matches!(e, Event::Desync { .. }))
    );
}
