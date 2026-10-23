//! Messages of the protocol and their encoding (docs/net-protocol.md).
//!
//! The types follow the message table of the original (`docs/spec/multiplayer.md` 1.3): command
//! packet (7), batch (8), host change (9), leave (0xc), heartbeat (0xd), probe (0xe), checksum
//! (0xf), map/save (0x10), loaded (0x11) and chat (4); the clone adds the connection handshake
//! with versions and hashes, the roster, the desync notice with state dump, and the snapshot
//! that also serves join-in-progress and resynchronisation.

use rt2_sim::Command;
use rt2_sim::net::{Digest, NetOutcome, NetPlayer, NetSnapshot};

use crate::NetError;
use crate::wire::{Reader, Writer, get_command, put_command};

/// First bytes of a [`Hello`].
pub const MAGIC: [u8; 4] = *b"RT2N";
/// Version of the message formats. A peer with another value is refused.
pub const PROTOCOL_VERSION: u16 = 1;
/// Longest chat text (`multiplayer.md` 4.1: 900 characters).
pub const MAX_CHAT: usize = 900;

/// A player as the host announces it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerInfo {
    pub id: u32,
    pub identity: u32,
    pub name: String,
    /// Where the others connect to it (mesh links, host migration).
    pub addr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    pub protocol: u16,
    /// [`crate::rules_hash`]: the build of the rules and the executable the data came from.
    pub rules_hash: u64,
    pub identity: u32,
    pub name: String,
    pub addr: String,
    /// `Some(id)`: a link between two players already in the game (not a join request).
    pub mesh_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Welcome {
    pub your_id: u32,
    pub host_id: u32,
    /// The game is running: a snapshot follows.
    pub in_progress: bool,
    pub players: Vec<PlayerInfo>,
}

/// A batch of the host (`cmd_net_flush` `0x4af8c0`): `(seq, stamp, horizon, commands)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    /// Sequence number, 1-based; applied in order.
    pub seq: u32,
    /// The game minute every command of the batch runs at (the horizon of the previous batch).
    pub stamp: i32,
    /// The minute no machine may pass until a later batch says otherwise.
    pub horizon: i32,
    /// `(sender network id, command)` in the order the host received them.
    pub cmds: Vec<(u32, Command)>,
}

/// A command waiting for its minute (the `cmd_queue` of the original).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub stamp: i32,
    pub sender: u32,
    pub cmd: Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    /// The game starts: everyone loads the same state, then `net_begin`.
    Start,
    /// A player joins a running game.
    Join,
    /// A peer that diverged takes the host's state.
    Resync,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotMsg {
    pub kind: SnapKind,
    pub snap: NetSnapshot,
    pub players: Vec<PlayerInfo>,
    pub host_id: u32,
    pub your_id: u32,
    /// Last batch applied by the host; the receiver expects `last_seq + 1`.
    pub last_seq: u32,
    pub horizon: i32,
    pub host_prev_h: i32,
    pub pending: Vec<Pending>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    Hello(Hello),
    Welcome(Welcome),
    Reject(String),
    Roster {
        host_id: u32,
        players: Vec<PlayerInfo>,
    },
    /// Client to host (`7`).
    Cmd(Command),
    Batch(Batch),
    /// Nothing to say (`0xd`).
    Heartbeat,
    /// "Are you there?" (`0xe`): answered with a heartbeat.
    Probe,
    /// The sender is leaving (`0xc`).
    Leave,
    Ping(u64),
    Pong(u64),
    /// Checksum of the state at the first day of a month (`0xf`).
    Sync {
        minute: i32,
        digest: Digest,
    },
    /// The host found a different checksum for `minute` (groups in `flags`); the receiver sends a
    /// [`Msg::StateDump`].
    Desync {
        minute: i32,
        host: Digest,
        flags: u8,
    },
    /// The state at the checksum point, for the diff dump: a canonical save (empty without pools).
    StateDump {
        minute: i32,
        digest: Digest,
        save: Vec<u8>,
    },
    Snapshot(Box<SnapshotMsg>),
    /// "I loaded the game" (`0x11`).
    Loaded,
    /// The new host after a migration (`9`).
    HostChanged {
        host: u32,
    },
    Chat(String),
}

impl Msg {
    /// Lost messages of these types are not resent (heartbeats and pings are repeated anyway,
    /// chat is fire and forget like the original's); the others are acknowledged.
    pub fn reliable(&self) -> bool {
        !matches!(
            self,
            Msg::Heartbeat | Msg::Probe | Msg::Ping(_) | Msg::Pong(_) | Msg::Chat(_)
        )
    }
}

fn put_players(w: &mut Writer, p: &[PlayerInfo]) {
    w.u32(p.len() as u32);
    for i in p {
        w.u32(i.id).u32(i.identity).str(&i.name).str(&i.addr);
    }
}

fn get_players(r: &mut Reader) -> Result<Vec<PlayerInfo>, NetError> {
    let n = r.u32()? as usize;
    if n > 64 {
        return Err(NetError::Malformed("too many players"));
    }
    (0..n)
        .map(|_| {
            Ok(PlayerInfo {
                id: r.u32()?,
                identity: r.u32()?,
                name: r.str()?,
                addr: r.str()?,
            })
        })
        .collect()
}

fn put_snapshot(w: &mut Writer, s: &NetSnapshot) {
    w.bytes(&s.file)
        .i32(s.minutes)
        .u8(s.speed)
        .u8(s.speed_resume)
        .u32(s.game_seed)
        .u32(s.rng)
        .u32(s.crt)
        .u64(s.cmd_hash);
    w.u32(s.roster.len() as u32);
    for p in &s.roster {
        w.u32(p.id).u32(p.identity).str(&p.name);
    }
    w.u32(s.outcomes.len() as u32);
    for o in &s.outcomes {
        w.i32(o.person).u8(o.state).bool(o.other_won);
    }
}

fn get_snapshot(r: &mut Reader) -> Result<NetSnapshot, NetError> {
    let file = r.bytes()?;
    let minutes = r.i32()?;
    let speed = r.u8()?;
    let speed_resume = r.u8()?;
    let game_seed = r.u32()?;
    let rng = r.u32()?;
    let crt = r.u32()?;
    let cmd_hash = r.u64()?;
    let n = r.u32()? as usize;
    if n > 64 {
        return Err(NetError::Malformed("too many players"));
    }
    let roster = (0..n)
        .map(|_| {
            Ok(NetPlayer {
                id: r.u32()?,
                identity: r.u32()?,
                name: r.str()?,
            })
        })
        .collect::<Result<Vec<_>, NetError>>()?;
    let n = r.u32()? as usize;
    if n > 64 {
        return Err(NetError::Malformed("too many outcomes"));
    }
    let outcomes = (0..n)
        .map(|_| {
            Ok(NetOutcome {
                person: r.i32()?,
                state: r.u8()?,
                other_won: r.bool()?,
            })
        })
        .collect::<Result<Vec<_>, NetError>>()?;
    Ok(NetSnapshot {
        file,
        minutes,
        speed,
        speed_resume,
        game_seed,
        rng,
        crt,
        roster,
        outcomes,
        cmd_hash,
    })
}

const T_HELLO: u8 = 1;
const T_WELCOME: u8 = 2;
const T_REJECT: u8 = 3;
const T_ROSTER: u8 = 4;
const T_CMD: u8 = 7;
const T_BATCH: u8 = 8;
const T_HOST_CHANGED: u8 = 9;
const T_PING: u8 = 0x0a;
const T_PONG: u8 = 0x0b;
const T_LEAVE: u8 = 0x0c;
const T_HEARTBEAT: u8 = 0x0d;
const T_PROBE: u8 = 0x0e;
const T_SYNC: u8 = 0x0f;
const T_SNAPSHOT: u8 = 0x10;
const T_LOADED: u8 = 0x11;
const T_DESYNC: u8 = 0x14;
const T_STATE_DUMP: u8 = 0x15;
const T_CHAT: u8 = 0x16;

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Msg::Hello(h) => {
                w.u8(T_HELLO)
                    .raw(&MAGIC)
                    .u16(h.protocol)
                    .u64(h.rules_hash)
                    .u32(h.identity)
                    .str(&h.name)
                    .str(&h.addr)
                    .bool(h.mesh_id.is_some())
                    .u32(h.mesh_id.unwrap_or(0));
            }
            Msg::Welcome(m) => {
                w.u8(T_WELCOME)
                    .u32(m.your_id)
                    .u32(m.host_id)
                    .bool(m.in_progress);
                put_players(&mut w, &m.players);
            }
            Msg::Reject(s) => {
                w.u8(T_REJECT).str(s);
            }
            Msg::Roster { host_id, players } => {
                w.u8(T_ROSTER).u32(*host_id);
                put_players(&mut w, players);
            }
            Msg::Cmd(c) => {
                w.u8(T_CMD);
                put_command(&mut w, c);
            }
            Msg::Batch(b) => {
                w.u8(T_BATCH).u32(b.seq).i32(b.stamp).i32(b.horizon);
                w.u32(b.cmds.len() as u32);
                for (sender, c) in &b.cmds {
                    w.u32(*sender);
                    put_command(&mut w, c);
                }
            }
            Msg::HostChanged { host } => {
                w.u8(T_HOST_CHANGED).u32(*host);
            }
            Msg::Ping(t) => {
                w.u8(T_PING).u64(*t);
            }
            Msg::Pong(t) => {
                w.u8(T_PONG).u64(*t);
            }
            Msg::Leave => {
                w.u8(T_LEAVE);
            }
            Msg::Heartbeat => {
                w.u8(T_HEARTBEAT);
            }
            Msg::Probe => {
                w.u8(T_PROBE);
            }
            Msg::Sync { minute, digest } => {
                w.u8(T_SYNC).i32(*minute).raw(&digest.to_bytes());
            }
            Msg::Snapshot(s) => {
                w.u8(T_SNAPSHOT).u8(match s.kind {
                    SnapKind::Start => 0,
                    SnapKind::Join => 1,
                    SnapKind::Resync => 2,
                });
                put_snapshot(&mut w, &s.snap);
                put_players(&mut w, &s.players);
                w.u32(s.host_id)
                    .u32(s.your_id)
                    .u32(s.last_seq)
                    .i32(s.horizon)
                    .i32(s.host_prev_h);
                w.u32(s.pending.len() as u32);
                for p in &s.pending {
                    w.i32(p.stamp).u32(p.sender);
                    put_command(&mut w, &p.cmd);
                }
            }
            Msg::Loaded => {
                w.u8(T_LOADED);
            }
            Msg::Desync {
                minute,
                host,
                flags,
            } => {
                w.u8(T_DESYNC).i32(*minute).raw(&host.to_bytes()).u8(*flags);
            }
            Msg::StateDump {
                minute,
                digest,
                save,
            } => {
                w.u8(T_STATE_DUMP)
                    .i32(*minute)
                    .raw(&digest.to_bytes())
                    .bytes(save);
            }
            Msg::Chat(t) => {
                w.u8(T_CHAT).str(t);
            }
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Result<Msg, NetError> {
        let mut r = Reader::new(b);
        let m = match r.u8()? {
            T_HELLO => {
                if r.raw::<4>()? != MAGIC {
                    return Err(NetError::Malformed("not an RT2 network peer"));
                }
                let protocol = r.u16()?;
                let rules_hash = r.u64()?;
                let identity = r.u32()?;
                let name = r.str()?;
                let addr = r.str()?;
                let has = r.bool()?;
                let id = r.u32()?;
                Msg::Hello(Hello {
                    protocol,
                    rules_hash,
                    identity,
                    name,
                    addr,
                    mesh_id: has.then_some(id),
                })
            }
            T_WELCOME => Msg::Welcome(Welcome {
                your_id: r.u32()?,
                host_id: r.u32()?,
                in_progress: r.bool()?,
                players: get_players(&mut r)?,
            }),
            T_REJECT => Msg::Reject(r.str()?),
            T_ROSTER => Msg::Roster {
                host_id: r.u32()?,
                players: get_players(&mut r)?,
            },
            T_CMD => Msg::Cmd(get_command(&mut r)?),
            T_BATCH => {
                let seq = r.u32()?;
                let stamp = r.i32()?;
                let horizon = r.i32()?;
                let n = r.u32()? as usize;
                if n > 100_000 {
                    return Err(NetError::Malformed("batch too large"));
                }
                let mut cmds = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    let s = r.u32()?;
                    cmds.push((s, get_command(&mut r)?));
                }
                Msg::Batch(Batch {
                    seq,
                    stamp,
                    horizon,
                    cmds,
                })
            }
            T_HOST_CHANGED => Msg::HostChanged { host: r.u32()? },
            T_PING => Msg::Ping(r.u64()?),
            T_PONG => Msg::Pong(r.u64()?),
            T_LEAVE => Msg::Leave,
            T_HEARTBEAT => Msg::Heartbeat,
            T_PROBE => Msg::Probe,
            T_SYNC => Msg::Sync {
                minute: r.i32()?,
                digest: Digest::from_bytes(&r.raw::<64>()?),
            },
            T_SNAPSHOT => {
                let kind = match r.u8()? {
                    0 => SnapKind::Start,
                    1 => SnapKind::Join,
                    2 => SnapKind::Resync,
                    _ => return Err(NetError::Malformed("unknown snapshot kind")),
                };
                let snap = get_snapshot(&mut r)?;
                let players = get_players(&mut r)?;
                let host_id = r.u32()?;
                let your_id = r.u32()?;
                let last_seq = r.u32()?;
                let horizon = r.i32()?;
                let host_prev_h = r.i32()?;
                let n = r.u32()? as usize;
                if n > 1_000_000 {
                    return Err(NetError::Malformed("too many pending commands"));
                }
                let mut pending = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    let stamp = r.i32()?;
                    let sender = r.u32()?;
                    pending.push(Pending {
                        stamp,
                        sender,
                        cmd: get_command(&mut r)?,
                    });
                }
                Msg::Snapshot(Box::new(SnapshotMsg {
                    kind,
                    snap,
                    players,
                    host_id,
                    your_id,
                    last_seq,
                    horizon,
                    host_prev_h,
                    pending,
                }))
            }
            T_LOADED => Msg::Loaded,
            T_DESYNC => Msg::Desync {
                minute: r.i32()?,
                host: Digest::from_bytes(&r.raw::<64>()?),
                flags: r.u8()?,
            },
            T_STATE_DUMP => Msg::StateDump {
                minute: r.i32()?,
                digest: Digest::from_bytes(&r.raw::<64>()?),
                save: r.bytes()?,
            },
            T_CHAT => {
                let t = r.str()?;
                if t.chars().count() > MAX_CHAT {
                    return Err(NetError::Malformed("chat text too long"));
                }
                Msg::Chat(t)
            }
            _ => return Err(NetError::Malformed("unknown message type")),
        };
        r.finish()?;
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rt2_sim::SpeedChange;

    fn digest(n: u64) -> Digest {
        Digest {
            rng: n as u32,
            clock: -5,
            roster: n,
            companies: n + 1,
            persons: n + 2,
            trains: n + 3,
            industries: n + 4,
            stations: n + 5,
            other: n + 6,
        }
    }

    fn players() -> Vec<PlayerInfo> {
        vec![
            PlayerInfo {
                id: 1,
                identity: 11,
                name: "Ada".into(),
                addr: "mem:1".into(),
            },
            PlayerInfo {
                id: 2,
                identity: 22,
                name: "Grace".into(),
                addr: "127.0.0.1:4000".into(),
            },
        ]
    }

    #[test]
    fn every_message_round_trips() {
        let snap = NetSnapshot {
            file: vec![1, 2, 3],
            minutes: 42,
            speed: 3,
            speed_resume: 3,
            game_seed: 7,
            rng: 8,
            crt: 9,
            roster: vec![NetPlayer {
                id: 1,
                identity: 2,
                name: "x".into(),
            }],
            outcomes: vec![NetOutcome {
                person: 3,
                state: 4,
                other_won: true,
            }],
            cmd_hash: 77,
        };
        let msgs = vec![
            Msg::Hello(Hello {
                protocol: PROTOCOL_VERSION,
                rules_hash: 5,
                identity: 6,
                name: "n".into(),
                addr: "a".into(),
                mesh_id: Some(4),
            }),
            Msg::Hello(Hello {
                protocol: PROTOCOL_VERSION,
                rules_hash: 5,
                identity: 6,
                name: "n".into(),
                addr: "a".into(),
                mesh_id: None,
            }),
            Msg::Welcome(Welcome {
                your_id: 2,
                host_id: 1,
                in_progress: true,
                players: players(),
            }),
            Msg::Reject("no".into()),
            Msg::Roster {
                host_id: 1,
                players: players(),
            },
            Msg::Cmd(Command::SetSpeed(SpeedChange::Absolute(5))),
            Msg::Batch(Batch {
                seq: 9,
                stamp: -3,
                horizon: 10,
                cmds: vec![(1, Command::TogglePause), (2, Command::SeedRng(5))],
            }),
            Msg::Heartbeat,
            Msg::Probe,
            Msg::Leave,
            Msg::Ping(5),
            Msg::Pong(6),
            Msg::Sync {
                minute: 4,
                digest: digest(1),
            },
            Msg::Desync {
                minute: 4,
                host: digest(2),
                flags: 9,
            },
            Msg::StateDump {
                minute: 4,
                digest: digest(3),
                save: vec![9; 10],
            },
            Msg::Snapshot(Box::new(SnapshotMsg {
                kind: SnapKind::Join,
                snap,
                players: players(),
                host_id: 1,
                your_id: 3,
                last_seq: 12,
                horizon: 100,
                host_prev_h: 90,
                pending: vec![Pending {
                    stamp: 90,
                    sender: 2,
                    cmd: Command::SeedRng(1),
                }],
            })),
            Msg::Loaded,
            Msg::HostChanged { host: 2 },
            Msg::Chat("hello".into()),
        ];
        for m in msgs {
            assert_eq!(Msg::decode(&m.encode()).unwrap(), m, "{m:?}");
        }
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(Msg::decode(&[]).is_err());
        assert!(Msg::decode(&[0xff]).is_err());
        assert!(Msg::decode(&[T_HELLO, b'X', b'X', b'X', b'X']).is_err());
        let mut ok = Msg::Heartbeat.encode();
        ok.push(0);
        assert!(Msg::decode(&ok).is_err(), "trailing bytes");
        let long = Msg::Chat("x".repeat(MAX_CHAT + 1)).encode();
        assert!(Msg::decode(&long).is_err());
    }
}
