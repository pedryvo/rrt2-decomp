//! Lockstep multiplayer of the Railroad Tycoon II clone (docs/spec/multiplayer.md, docs/net-protocol.md).
//!
//! The rules come from the original: only commands travel, the host orders them into numbered
//! batches stamped with a game minute and a time horizon, and every machine runs the whole
//! simulation. The transport is the clone's own: [`transport::Transport`] is an abstract
//! datagram carrier with a reliable layer on top ([`link`]), with an in-memory implementation
//! that can lose, delay and reorder ([`mem`]) and a TCP one ([`tcp`]).
//!
//! | Module | Role |
//! |---|---|
//! | [`wire`] | binary writer/reader and the codec of `rt2_sim::Command` |
//! | [`msg`] | the messages (hello, batch, sync, snapshot...) and their encoding |
//! | [`link`] | acknowledgements, retransmission and duplicate removal per link |
//! | [`transport`], [`mem`], [`tcp`] | the abstract carrier and its two implementations |
//! | [`session`] | the state machine: lobby, start barrier, batches, pacing, heartbeats, host migration, join in progress, checksum and resynchronisation |

pub mod link;
pub mod mem;
pub mod msg;
pub mod session;
pub mod tcp;
pub mod transport;
pub mod wire;

pub use msg::{PROTOCOL_VERSION, PlayerInfo};
pub use session::{Config, Event, Session, State};
pub use transport::{LinkId, Transport, TransportEvent};

/// Errors of the network layer.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("malformed message: {0}")]
    Malformed(&'static str),
    #[error("transport: {0}")]
    Transport(String),
    #[error("refused: {0}")]
    Refused(String),
    #[error("{0}")]
    State(&'static str),
}

/// FNV-1a over `bytes` (the hash of the connection check).
fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The hash two peers must share to play together (docs/net-protocol.md "Handshake"): the
/// protocol version, the version and rules revision of the simulation, the hash of the
/// executable the game data was read from (`exe_hash`, 0 for none) and the enabled fixes of
/// original bugs (`ruleset_bits`).
pub fn rules_hash(exe_hash: u64, ruleset_bits: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    h = fnv(h, &PROTOCOL_VERSION.to_le_bytes());
    h = fnv(h, rt2_sim::net::SIM_VERSION.as_bytes());
    h = fnv(h, &rt2_sim::net::RULES_REVISION.to_le_bytes());
    h = fnv(h, &exe_hash.to_le_bytes());
    fnv(h, &ruleset_bits.to_le_bytes())
}

/// Hash of the bytes of the executable (or any data file), for [`rules_hash`].
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    fnv(0xcbf2_9ce4_8422_2325, bytes)
}
