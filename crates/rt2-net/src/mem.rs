//! In-memory transport for tests and headless play (docs/net-protocol.md "Transports").
//!
//! A [`MemHub`] connects any number of [`MemTransport`] nodes in one process. Delivery uses the
//! logical time the callers pass to [`Transport::poll`] (no wall clock), a seeded generator for
//! the faults, and can lose, duplicate, delay and reorder frames ([`Faults`]); a node can be cut
//! off silently (a pulled cable: no close event, the silence timeout has to notice) or killed
//! (links close with events).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use crate::NetError;
use crate::transport::{LinkId, Transport, TransportEvent};

/// Faults of the simulated network.
#[derive(Clone, Copy, Debug, Default)]
pub struct Faults {
    /// Base one-way delay.
    pub latency_ms: u64,
    /// Extra random delay in `0..=jitter_ms`: frames overtake each other.
    pub jitter_ms: u64,
    /// Chance a frame is lost, in thousandths.
    pub loss_permille: u32,
    /// Chance a frame is delivered twice, in thousandths.
    pub dup_permille: u32,
}

struct Flight {
    at: u64,
    seq: u64,
    node: String,
    event: TransportEvent,
}

#[derive(Default)]
struct Inner {
    nodes: BTreeSet<String>,
    /// `(node, link)` to the other end.
    links: BTreeMap<(String, LinkId), (String, LinkId)>,
    next_link: LinkId,
    now: u64,
    rng: u64,
    seq: u64,
    faults: Faults,
    flights: Vec<Flight>,
    cut: BTreeSet<String>,
    dead: BTreeSet<String>,
}

impl Inner {
    fn rand(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn push(&mut self, at: u64, node: &str, event: TransportEvent) {
        self.seq += 1;
        self.flights.push(Flight {
            at,
            seq: self.seq,
            node: node.to_string(),
            event,
        });
    }
}

/// The shared network.
#[derive(Clone)]
pub struct MemHub(Arc<Mutex<Inner>>);

impl MemHub {
    /// A perfect network; `seed` feeds the fault generator.
    pub fn new(seed: u64) -> MemHub {
        MemHub(Arc::new(Mutex::new(Inner {
            rng: seed | 1,
            next_link: 1,
            ..Inner::default()
        })))
    }

    pub fn set_faults(&self, f: Faults) {
        self.0.lock().unwrap().faults = f;
    }

    /// A node listening at `mem:<name>`.
    pub fn node(&self, name: &str) -> MemTransport {
        let addr = format!("mem:{name}");
        self.0.lock().unwrap().nodes.insert(addr.clone());
        MemTransport {
            hub: self.clone(),
            addr,
        }
    }

    /// Drops everything to and from the node without telling anybody (a pulled cable).
    pub fn cut(&self, addr: &str) {
        self.0.lock().unwrap().cut.insert(addr.to_string());
    }

    pub fn heal(&self, addr: &str) {
        self.0.lock().unwrap().cut.remove(addr);
    }

    /// The node crashes: its links close for the others at once.
    pub fn kill(&self, addr: &str) {
        let mut h = self.0.lock().unwrap();
        h.dead.insert(addr.to_string());
        let now = h.now;
        let mine: Vec<_> = h
            .links
            .iter()
            .filter(|((n, _), _)| n == addr)
            .map(|((_, l), (pn, pl))| (*l, pn.clone(), *pl))
            .collect();
        for (l, pn, pl) in mine {
            h.links.remove(&(addr.to_string(), l));
            h.links.remove(&(pn.clone(), pl));
            h.push(now, &pn, TransportEvent::Closed(pl));
        }
    }
}

/// One node of a [`MemHub`].
pub struct MemTransport {
    hub: MemHub,
    addr: String,
}

impl Transport for MemTransport {
    fn local_addr(&self) -> String {
        self.addr.clone()
    }

    fn connect(&mut self, addr: &str) -> Result<LinkId, NetError> {
        let mut h = self.hub.0.lock().unwrap();
        if !h.nodes.contains(addr) || h.dead.contains(addr) || h.cut.contains(addr) {
            return Err(NetError::Transport(format!("no route to {addr}")));
        }
        let (mine, theirs) = (h.next_link, h.next_link + 1);
        h.next_link += 2;
        h.links
            .insert((self.addr.clone(), mine), (addr.to_string(), theirs));
        h.links
            .insert((addr.to_string(), theirs), (self.addr.clone(), mine));
        let now = h.now;
        h.push(now, addr, TransportEvent::Opened(theirs));
        Ok(mine)
    }

    fn send(&mut self, link: LinkId, frame: &[u8]) {
        let mut h = self.hub.0.lock().unwrap();
        let Some((node, peer_link)) = h.links.get(&(self.addr.clone(), link)).cloned() else {
            return;
        };
        if h.cut.contains(&self.addr) || h.cut.contains(&node) || h.dead.contains(&self.addr) {
            return;
        }
        let f = h.faults;
        if f.loss_permille > 0 && (h.rand() % 1000) < u64::from(f.loss_permille) {
            return;
        }
        let copies = if f.dup_permille > 0 && (h.rand() % 1000) < u64::from(f.dup_permille) {
            2
        } else {
            1
        };
        for _ in 0..copies {
            let jitter = if f.jitter_ms > 0 {
                h.rand() % (f.jitter_ms + 1)
            } else {
                0
            };
            let at = h.now + f.latency_ms + jitter;
            h.push(at, &node, TransportEvent::Data(peer_link, frame.to_vec()));
        }
    }

    fn close(&mut self, link: LinkId) {
        let mut h = self.hub.0.lock().unwrap();
        if let Some((node, peer_link)) = h.links.remove(&(self.addr.clone(), link)) {
            h.links.remove(&(node.clone(), peer_link));
            let now = h.now;
            h.push(now, &node, TransportEvent::Closed(peer_link));
        }
    }

    fn poll(&mut self, now_ms: u64) -> Vec<TransportEvent> {
        let mut h = self.hub.0.lock().unwrap();
        h.now = h.now.max(now_ms);
        let now = h.now;
        if h.dead.contains(&self.addr) || h.cut.contains(&self.addr) {
            // A cut node hears nothing; what was in flight to it is lost.
            let me = self.addr.clone();
            h.flights.retain(|f| f.node != me || f.at > now);
            return Vec::new();
        }
        let me = self.addr.clone();
        let mut due: Vec<Flight> = Vec::new();
        let mut keep = Vec::new();
        for f in std::mem::take(&mut h.flights) {
            if f.node == me && f.at <= now {
                due.push(f);
            } else {
                keep.push(f);
            }
        }
        h.flights = keep;
        due.sort_by_key(|f| (f.at, f.seq));
        due.into_iter().map(|f| f.event).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(hub: &MemHub) -> (MemTransport, MemTransport, LinkId, LinkId) {
        let mut a = hub.node("a");
        let mut b = hub.node("b");
        let la = a.connect("mem:b").unwrap();
        let ev = b.poll(0);
        let lb = match ev[..] {
            [TransportEvent::Opened(l)] => l,
            _ => panic!("{ev:?}"),
        };
        (a, b, la, lb)
    }

    #[test]
    fn frames_arrive_in_order_on_a_perfect_network() {
        let hub = MemHub::new(1);
        let (mut a, mut b, la, lb) = pair(&hub);
        a.send(la, b"1");
        a.send(la, b"2");
        assert_eq!(
            b.poll(0),
            vec![
                TransportEvent::Data(lb, b"1".to_vec()),
                TransportEvent::Data(lb, b"2".to_vec())
            ]
        );
    }

    #[test]
    fn latency_holds_frames_until_their_time() {
        let hub = MemHub::new(1);
        hub.set_faults(Faults {
            latency_ms: 50,
            ..Faults::default()
        });
        let (mut a, mut b, la, _) = pair(&hub);
        a.send(la, b"x");
        assert!(b.poll(49).is_empty());
        assert_eq!(b.poll(50).len(), 1);
    }

    #[test]
    fn loss_and_duplication_follow_the_seed() {
        let run = |seed| {
            let hub = MemHub::new(seed);
            hub.set_faults(Faults {
                loss_permille: 300,
                dup_permille: 200,
                ..Faults::default()
            });
            let (mut a, mut b, la, _) = pair(&hub);
            for i in 0..200u8 {
                a.send(la, &[i]);
            }
            b.poll(0).len()
        };
        let n = run(7);
        assert_eq!(n, run(7), "same seed, same faults");
        assert!(n < 200 + 40 && n > 80, "{n}");
    }

    #[test]
    fn cut_is_silent_and_kill_closes() {
        let hub = MemHub::new(1);
        let (mut a, mut b, la, lb) = pair(&hub);
        hub.cut("mem:b");
        a.send(la, b"x");
        assert!(b.poll(10).is_empty());
        hub.heal("mem:b");
        a.send(la, b"y");
        assert_eq!(b.poll(10), vec![TransportEvent::Data(lb, b"y".to_vec())]);
        hub.kill("mem:a");
        assert_eq!(b.poll(11), vec![TransportEvent::Closed(lb)]);
    }
}
