//! The abstract carrier (docs/net-protocol.md "Transport").
//!
//! A [`Transport`] moves opaque frames between *links*. It may lose, duplicate, delay and
//! reorder them (the original's DirectPlay did: `multiplayer.md` 1.2): acknowledgements and
//! retransmission are the job of [`crate::link`], above it. It never reads a clock: the caller
//! passes `now_ms` to [`Transport::poll`], so the in-memory implementation is driven by the test's
//! logical time.

use crate::NetError;

/// A connection to one peer, local to a transport.
pub type LinkId = u32;

#[derive(Debug, PartialEq, Eq)]
pub enum TransportEvent {
    /// A peer connected to us.
    Opened(LinkId),
    /// The link is gone (closed by the peer, reset, or [`Transport::close`]).
    Closed(LinkId),
    Data(LinkId, Vec<u8>),
}

pub trait Transport: Send {
    /// The address other peers use to [`Transport::connect`] to this one.
    fn local_addr(&self) -> String;
    /// Opens a link to `addr`. The peer sees [`TransportEvent::Opened`].
    fn connect(&mut self, addr: &str) -> Result<LinkId, NetError>;
    /// Sends one frame; fire and forget.
    fn send(&mut self, link: LinkId, frame: &[u8]);
    fn close(&mut self, link: LinkId);
    /// What arrived since the last call, in arrival order.
    fn poll(&mut self, now_ms: u64) -> Vec<TransportEvent>;
}
