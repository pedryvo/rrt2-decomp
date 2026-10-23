//! TCP transport: `std` sockets and one reader thread per link (docs/net-protocol.md
//! "Transports"). A frame is a `u32` little-endian length and the bytes (at most 256 MB: a save
//! travels in one frame). Connection is direct by IP and port, no discovery.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::NetError;
use crate::transport::{LinkId, Transport, TransportEvent};

/// Longest frame accepted.
const MAX_FRAME: usize = 256 << 20;

enum Raw {
    Accepted(LinkId, TcpStream),
    Frame(LinkId, Vec<u8>),
    Closed(LinkId),
}

pub struct TcpTransport {
    addr: String,
    next: Arc<AtomicU32>,
    stop: Arc<AtomicBool>,
    rx: Receiver<Raw>,
    tx: Sender<Raw>,
    writers: BTreeMap<LinkId, TcpStream>,
    acceptor: Option<JoinHandle<()>>,
}

fn reader(link: LinkId, mut s: TcpStream, tx: Sender<Raw>) {
    loop {
        let mut len = [0u8; 4];
        if s.read_exact(&mut len).is_err() {
            break;
        }
        let n = u32::from_le_bytes(len) as usize;
        if n > MAX_FRAME {
            break;
        }
        let mut buf = vec![0u8; n];
        if s.read_exact(&mut buf).is_err() || tx.send(Raw::Frame(link, buf)).is_err() {
            break;
        }
    }
    let _ = tx.send(Raw::Closed(link));
}

impl TcpTransport {
    /// Listens on `bind` (`"0.0.0.0:7777"`, or port 0 for any); [`Transport::local_addr`] is the
    /// bound address, which the other players use to connect (replace the IP by the public one
    /// when behind a router).
    pub fn listen(bind: &str) -> Result<TcpTransport, NetError> {
        let l = TcpListener::bind(bind).map_err(|e| NetError::Transport(e.to_string()))?;
        l.set_nonblocking(true)
            .map_err(|e| NetError::Transport(e.to_string()))?;
        let addr = l
            .local_addr()
            .map_err(|e| NetError::Transport(e.to_string()))?
            .to_string();
        let (tx, rx) = channel();
        let next = Arc::new(AtomicU32::new(1));
        let stop = Arc::new(AtomicBool::new(false));
        let (atx, anext, astop) = (tx.clone(), next.clone(), stop.clone());
        let acceptor = std::thread::spawn(move || {
            while !astop.load(Ordering::Relaxed) {
                match l.accept() {
                    Ok((s, _)) => {
                        let _ = s.set_nonblocking(false);
                        let _ = s.set_nodelay(true);
                        let id = anext.fetch_add(1, Ordering::Relaxed);
                        let Ok(r) = s.try_clone() else { continue };
                        let rtx = atx.clone();
                        if atx.send(Raw::Accepted(id, s)).is_err() {
                            break;
                        }
                        std::thread::spawn(move || reader(id, r, rtx));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(20)),
                }
            }
        });
        Ok(TcpTransport {
            addr,
            next,
            stop,
            rx,
            tx,
            writers: BTreeMap::new(),
            acceptor: Some(acceptor),
        })
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for s in self.writers.values() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        if let Some(h) = self.acceptor.take() {
            let _ = h.join();
        }
    }
}

impl Transport for TcpTransport {
    fn local_addr(&self) -> String {
        self.addr.clone()
    }

    fn connect(&mut self, addr: &str) -> Result<LinkId, NetError> {
        let sa = addr
            .to_socket_addrs()
            .map_err(|e| NetError::Transport(format!("{addr}: {e}")))?
            .next()
            .ok_or_else(|| NetError::Transport(format!("{addr}: no address")))?;
        let s = TcpStream::connect_timeout(&sa, Duration::from_secs(5))
            .map_err(|e| NetError::Transport(format!("{addr}: {e}")))?;
        let _ = s.set_nodelay(true);
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let r = s
            .try_clone()
            .map_err(|e| NetError::Transport(e.to_string()))?;
        let tx = self.tx.clone();
        std::thread::spawn(move || reader(id, r, tx));
        self.writers.insert(id, s);
        Ok(id)
    }

    fn send(&mut self, link: LinkId, frame: &[u8]) {
        let Some(s) = self.writers.get_mut(&link) else {
            return;
        };
        let mut buf = Vec::with_capacity(frame.len() + 4);
        buf.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        buf.extend_from_slice(frame);
        if s.write_all(&buf).is_err() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }

    fn close(&mut self, link: LinkId) {
        if let Some(s) = self.writers.remove(&link) {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }

    fn poll(&mut self, _now_ms: u64) -> Vec<TransportEvent> {
        let mut out = Vec::new();
        while let Ok(r) = self.rx.try_recv() {
            match r {
                Raw::Accepted(id, s) => {
                    self.writers.insert(id, s);
                    out.push(TransportEvent::Opened(id));
                }
                Raw::Frame(id, f) => {
                    if self.writers.contains_key(&id) {
                        out.push(TransportEvent::Data(id, f));
                    }
                }
                Raw::Closed(id) => {
                    if self.writers.remove(&id).is_some() {
                        out.push(TransportEvent::Closed(id));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn wait(t: &mut TcpTransport, want: usize) -> Vec<TransportEvent> {
        let end = Instant::now() + Duration::from_secs(5);
        let mut all = Vec::new();
        while all.len() < want && Instant::now() < end {
            all.extend(t.poll(0));
            std::thread::sleep(Duration::from_millis(2));
        }
        all
    }

    #[test]
    fn frames_cross_a_real_socket_both_ways() {
        let mut a = TcpTransport::listen("127.0.0.1:0").unwrap();
        let mut b = TcpTransport::listen("127.0.0.1:0").unwrap();
        let lb = b.connect(&a.local_addr()).unwrap();
        let ev = wait(&mut a, 1);
        let TransportEvent::Opened(la) = ev[0] else {
            panic!("{ev:?}")
        };
        b.send(lb, b"ping");
        // A frame larger than a socket buffer.
        let big = vec![7u8; 3 << 20];
        a.send(la, &big);
        assert_eq!(
            wait(&mut a, 1),
            vec![TransportEvent::Data(la, b"ping".to_vec())]
        );
        assert_eq!(wait(&mut b, 1), vec![TransportEvent::Data(lb, big)]);
        b.close(lb);
        assert_eq!(wait(&mut a, 1), vec![TransportEvent::Closed(la)]);
    }
}
