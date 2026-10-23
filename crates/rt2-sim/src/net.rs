//! Network mode of the simulation: the rules the original changes when two or more humans play
//! (docs/spec/multiplayer.md sections 1.6, 1.8, 3 and 5; "Implementacao no clone").
//!
//! "Network" means `net_player_count() >= 2` (`0x46ae50`): a [`World`] with a [`NetSession`] of at
//! least two players. A world without a session, or with one player left, is the single-player
//! simulation, untouched (the fidelity scoreboard runs it). The transport and the lockstep live
//! in the `rt2-net` crate; this module only holds what the *rules* need:
//!
//! - the roster (sorted by network id) and the person each human controls (`+0x122`/`+0x126` of
//!   the person record), [`World::net_begin`], [`World::net_player_left`] (`0x49d970`) and the late
//!   join of the clone ([`World::net_player_joined`]);
//! - the differences of the world loop: daily RNG re-seed, trains in daily steps of 1440 minutes
//!   (`tick/net.rs`), no news popups, no interactive choices, victory/defeat per player;
//! - the state checksum ([`Digest`], the clone's full-state version of `net_sync_checksum_build`
//!   `0x46c680`) and the checkpoints used to dump a desynchronisation;
//! - the snapshot a late joiner or a diverged peer loads ([`NetSnapshot`]).
//!
//! Per-machine state of the original (`local_person`/`local_company`, `0x566ca4`/`0x574414`) is a
//! *view* here ([`World::net_local_person`]): the simulation keeps its own deterministic notion
//! of "the viewed company" (the first human), so no rule reads the machine it runs on (the
//! original's latent desync sources, multiplayer.md 3.4).

use rt2_formats::map::{Pool, Save};

use crate::calendar::Date;
use crate::pool;
use crate::rng::{CrtRng, GameRng};
use crate::world::World;

/// Version of the rules of the network mode; bump it when a change makes two builds disagree
/// (it goes into the connection hash, docs/net-protocol.md).
pub const RULES_REVISION: u32 = 1;

/// Version of the simulation crate (goes into the connection hash).
pub const SIM_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `net_min_peer_version` of a game of the clone (`0x49d920`; the original's is 9012 = `0x2334`):
/// every test of the original of the form `>= 0x232b` or `>= 0x232d` is true.
pub const PEER_VERSION: u32 = 0x2334;

/// `+0x122`: the person is a human (u32).
const PE_HUMAN: usize = 0x122;
/// `+0x126`: network id of the controlling human, 0 in a local game, -1 for an AI person.
const PE_NET_ID: usize = 0x126;
/// `+0x263`: the company the person chairs.
const PE_CHAIR: usize = 0x263;
/// `+0x54d`: the persistent identity of the human (saved, multiplayer.md 5.3).
const PE_IDENTITY: usize = 0x54d;
/// `0x573f1c` `GameIsMultiPlayer`, state offset.
const ST_MULTIPLAYER: usize = 0x3A34;
/// The yearly history block of the interface (`0x570a82`, state offset): it records the viewed
/// company, so it differs per machine in the original.
const ST_HISTORY: usize = 0x59A;
const HISTORY_LEN: usize = 0x3283;
/// Header offsets of the saved local company and its flag (`tick/fincmd.rs`).
const HDR_LOCAL_COMPANY: usize = 0x8a3c;
const HDR_LOCAL_FLAG: usize = 0x89da;

/// One human of the roster (record of `0x147` bytes in the original, `+0x00` id, `+0x24`
/// identity, `+0x2b` profile).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetPlayer {
    /// Network id; the roster is sorted by it and the lowest id after the host is the successor.
    pub id: u32,
    /// Persistent identity (`+0x24`), matched with `person+0x54d` when a saved game loads.
    pub identity: u32,
    /// Profile name (`+0x2b`, 30 characters).
    pub name: String,
}

/// End of the game for one person (the "local state" of the original, kept per person so that
/// every machine computes the same).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetOutcome {
    pub person: i32,
    /// 1..=3 victory bronze/silver/gold, 4 defeat (`GameOver.state`).
    pub state: u8,
    /// Defeat because another player reached the goals first (LNG 2691).
    pub other_won: bool,
}

/// Something the front end or the lockstep layer should know, drained with
/// [`World::take_net_events`]. Output only: the rules never read it.
#[derive(Clone, Debug)]
pub enum NetEvent {
    /// The day-1 checksum point (`net_time_sync` `0x49e0b0`): the lockstep layer sends it to the
    /// host.
    Sync(Box<Checkpoint>),
    /// A human left and a computer took the person over (LNG 2319).
    PlayerLeft { net_id: u32, person: i32 },
    /// A human took a vacated or free seat (clone extra).
    PlayerJoined { net_id: u32, person: i32 },
}

/// Network state of a world.
#[derive(Clone, Debug)]
pub struct NetSession {
    pub(crate) roster: Vec<NetPlayer>,
    /// This machine's id: view only, not part of the shared state.
    pub(crate) local_id: u32,
    pub(crate) events: Vec<NetEvent>,
    /// Keep a full state clone with every [`NetEvent::Sync`] (needed to dump a diff).
    pub(crate) keep_checkpoints: bool,
    /// Hash of every command applied in network mode with the minute it ran at (speed commands
    /// excluded: they run on arrival, not at the stamp). Equal hashes prove the same commands ran
    /// in the same order at the same minutes.
    pub(crate) cmd_hash: u64,
}

impl PartialEq for NetSession {
    fn eq(&self, other: &Self) -> bool {
        self.roster == other.roster
    }
}
impl Eq for NetSession {}

impl NetSession {
    pub(crate) fn log_command(&mut self, minute: i32, cmd: &crate::Command) {
        let mut h = Fnv(self.cmd_hash);
        h.u32(minute as u32);
        h.bytes(format!("{cmd:?}").as_bytes());
        self.cmd_hash = h.finish();
    }

    pub fn cmd_hash(&self) -> u64 {
        self.cmd_hash
    }

    pub fn roster(&self) -> &[NetPlayer] {
        &self.roster
    }
    pub fn local_id(&self) -> u32 {
        self.local_id
    }
}

/// FNV-1a, 64 bit: the checksum primitive (stable across platforms, no dependencies).
#[derive(Clone, Copy)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    pub fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn finish(self) -> u64 {
        self.0
    }
}

/// The checksum record of the clone: the five groups of `net_sync_checksum_build` (RNG,
/// companies, persons, trains, industries) plus the rest of the shared state, so a divergence is
/// named by group (bits of [`Digest::differs`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Digest {
    /// Game RNG state (the original keeps the low 16 bits).
    pub rng: u32,
    pub clock: i32,
    pub roster: u64,
    pub companies: u64,
    pub persons: u64,
    pub trains: u64,
    pub industries: u64,
    pub stations: u64,
    /// Everything else that is shared: the other pools, the state block without the interface
    /// history, the map, the scenario rules, the outcomes.
    pub other: u64,
}

/// Bits of [`Digest::differs`] (the original has 0 RNG, 1 companies, 2 persons, 3 trains, 4
/// industries; the clone adds stations, the rest and the clock).
pub mod group {
    pub const RNG: u8 = 1;
    pub const COMPANIES: u8 = 2;
    pub const PERSONS: u8 = 4;
    pub const TRAINS: u8 = 8;
    pub const INDUSTRIES: u8 = 16;
    pub const STATIONS: u8 = 32;
    pub const OTHER: u8 = 64;
    pub const CLOCK_OR_ROSTER: u8 = 128;
}

impl Digest {
    /// The groups that differ.
    pub fn differs(&self, o: &Digest) -> u8 {
        let mut f = 0;
        for (bit, d) in [
            (group::RNG, self.rng != o.rng),
            (group::COMPANIES, self.companies != o.companies),
            (group::PERSONS, self.persons != o.persons),
            (group::TRAINS, self.trains != o.trains),
            (group::INDUSTRIES, self.industries != o.industries),
            (group::STATIONS, self.stations != o.stations),
            (group::OTHER, self.other != o.other),
            (
                group::CLOCK_OR_ROSTER,
                self.clock != o.clock || self.roster != o.roster,
            ),
        ] {
            if d {
                f |= bit;
            }
        }
        f
    }

    /// Names of the groups in `flags`.
    pub fn group_names(flags: u8) -> Vec<&'static str> {
        [
            (group::RNG, "rng"),
            (group::COMPANIES, "companies"),
            (group::PERSONS, "persons"),
            (group::TRAINS, "trains"),
            (group::INDUSTRIES, "industries"),
            (group::STATIONS, "stations"),
            (group::OTHER, "other"),
            (group::CLOCK_OR_ROSTER, "clock/roster"),
        ]
        .into_iter()
        .filter(|(b, _)| flags & b != 0)
        .map(|(_, n)| n)
        .collect()
    }

    /// 64 bytes: the wire form of the record.
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut o = [0u8; 64];
        o[0..4].copy_from_slice(&self.rng.to_le_bytes());
        o[4..8].copy_from_slice(&self.clock.to_le_bytes());
        for (i, v) in [
            self.roster,
            self.companies,
            self.persons,
            self.trains,
            self.industries,
            self.stations,
            self.other,
        ]
        .into_iter()
        .enumerate()
        {
            o[8 + 8 * i..16 + 8 * i].copy_from_slice(&v.to_le_bytes());
        }
        o
    }

    pub fn from_bytes(b: &[u8; 64]) -> Digest {
        let u64_at = |i: usize| u64::from_le_bytes(b[8 + 8 * i..16 + 8 * i].try_into().unwrap());
        Digest {
            rng: u32::from_le_bytes(b[0..4].try_into().unwrap()),
            clock: i32::from_le_bytes(b[4..8].try_into().unwrap()),
            roster: u64_at(0),
            companies: u64_at(1),
            persons: u64_at(2),
            trains: u64_at(3),
            industries: u64_at(4),
            stations: u64_at(5),
            other: u64_at(6),
        }
    }
}

/// The state at a checksum point: the digest and, when kept, a clone of the shared state with
/// the per-machine fields cleared, to diff against another machine's.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub minute: i32,
    pub digest: Digest,
    pub save: Option<Box<Save>>,
}

impl Checkpoint {
    /// Field-by-field differences against `other` (docs `rt2-savediff`): one line per field,
    /// `path: ours | theirs`. Without saves (a world without pools) only the digest groups.
    pub fn diff_lines(&self, other: &Checkpoint) -> Vec<String> {
        let mut out = vec![format!(
            "minute {} vs {}; groups: {}",
            self.minute,
            other.minute,
            Digest::group_names(self.digest.differs(&other.digest)).join(", ")
        )];
        if self.digest.rng != other.digest.rng {
            out.push(format!(
                "rng: {:#010x} | {:#010x}",
                self.digest.rng, other.digest.rng
            ));
        }
        if let (Some(a), Some(b)) = (&self.save, &other.save) {
            for d in rt2_formats::savefields::diff(a, b) {
                if d.masked.is_none() {
                    out.push(format!(
                        "{}: {} | {}",
                        d.path,
                        d.a.map_or("<absent>".into(), |v| v.to_string()),
                        d.b.map_or("<absent>".into(), |v| v.to_string())
                    ));
                }
            }
        }
        out
    }
}

/// A complete state to start from: the first load of a game, a late join or a resynchronisation
/// ("the host sends the save", docs/net-protocol.md). The save carries the pools; the RNG states,
/// the clock and the roster are not in a `.GAM` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetSnapshot {
    /// [`World::to_file_bytes`] (empty for a world without pools).
    pub file: Vec<u8>,
    pub minutes: i32,
    pub speed: u8,
    pub speed_resume: u8,
    pub game_seed: u32,
    pub rng: u32,
    pub crt: u32,
    pub roster: Vec<NetPlayer>,
    pub outcomes: Vec<NetOutcome>,
    /// [`NetSession::cmd_hash`].
    pub cmd_hash: u64,
}

/// One of the three passes of the person assignment (`0x479710`, multiplayer.md 5.3).
fn human_persons(sim: &crate::tick::SimState) -> Vec<usize> {
    sim.net_player_set()
        .into_iter()
        .filter(|&p| {
            pool::record(&sim.save.persons, p).is_some_and(|r| pool::u32_at(&r.data, PE_HUMAN) != 0)
        })
        .collect()
}

impl World {
    /// True with two or more humans: `net_player_count() >= 2`.
    pub fn is_network(&self) -> bool {
        self.net.as_ref().is_some_and(|n| n.roster.len() >= 2)
    }

    pub fn net_session(&self) -> Option<&NetSession> {
        self.net.as_ref()
    }

    /// Drains the network events, oldest first.
    pub fn take_net_events(&mut self) -> Vec<NetEvent> {
        self.net
            .as_mut()
            .map(|n| std::mem::take(&mut n.events))
            .unwrap_or_default()
    }

    /// Keep a clone of the state with every checksum point (for the diff dump).
    pub fn net_keep_checkpoints(&mut self, on: bool) {
        if let Some(n) = self.net.as_mut() {
            n.keep_checkpoints = on;
        }
    }

    /// The start of a network game (`net_game_start` `0x4b69b0`, case 6, multiplayer.md 2.6):
    /// the roster, `rng_seed(game_seed)`, `GameIsMultiPlayer`, and the humans take their persons
    /// ([`World::net_assign_humans`]). `roster` is sorted by id here.
    pub fn net_begin(&mut self, mut roster: Vec<NetPlayer>, local_id: u32) {
        roster.sort_by_key(|p| p.id);
        self.rng.seed(self.game_seed);
        self.net = Some(NetSession {
            roster,
            local_id,
            events: Vec::new(),
            keep_checkpoints: false,
            cmd_hash: 0,
        });
        self.net_sync_multiplayer_flag();
        self.net_assign_humans();
    }

    /// Gives a world loaded from a snapshot (the late joiner and the resynchronised peer) the
    /// session of the snapshot, without touching persons or RNG.
    pub fn net_attach(&mut self, snap: &NetSnapshot, local_id: u32) {
        let mut roster = snap.roster.clone();
        roster.sort_by_key(|p| p.id);
        let keep = self.net.as_ref().is_some_and(|n| n.keep_checkpoints);
        self.net = Some(NetSession {
            roster,
            local_id,
            events: Vec::new(),
            keep_checkpoints: keep,
            cmd_hash: snap.cmd_hash,
        });
    }

    fn net_sync_multiplayer_flag(&mut self) {
        let multi = self.is_network();
        if let Some(sim) = self.sim.as_mut()
            && sim.save.state.len() >= ST_MULTIPLAYER + 4
        {
            // `0x573f1c` is written once, when the world starts (`0x479b20`).
            pool::put_u32(&mut sim.save.state, ST_MULTIPLAYER, u32::from(multi));
        }
    }

    /// The three passes of `0x479710` over the persons that are human (`+0x122 != 0`): (0) the
    /// person whose saved identity (`+0x54d`) equals a connected player's takes that player; (1)
    /// the remaining persons take the remaining players in roster order; (2) persons left without
    /// a player become computers. Writes `+0x126` (network id) and `+0x54d` (identity).
    pub fn net_assign_humans(&mut self) {
        let Some(net) = self.net.as_ref() else {
            return;
        };
        let roster = net.roster.clone();
        let Some(sim) = self.sim.as_mut() else {
            return;
        };
        let persons = human_persons(sim);
        let mut taken: Vec<Option<usize>> = vec![None; roster.len()];
        let mut assigned: Vec<Option<usize>> = vec![None; persons.len()];
        // Pass 0: by identity.
        for (pi, &p) in persons.iter().enumerate() {
            let ident = pool::record(&sim.save.persons, p)
                .map_or(0, |r| pool::u32_at(&r.data, PE_IDENTITY));
            if ident == 0 {
                continue;
            }
            if let Some(ri) = roster
                .iter()
                .enumerate()
                .position(|(ri, n)| n.identity == ident && taken[ri].is_none())
            {
                taken[ri] = Some(p);
                assigned[pi] = Some(ri);
            }
        }
        // Pass 1: the rest, in order.
        for slot in assigned.iter_mut().filter(|a| a.is_none()) {
            if let Some(ri) = taken.iter().position(Option::is_none) {
                taken[ri] = Some(0);
                *slot = Some(ri);
            }
        }
        // Pass 2: persons without a player become computers.
        for (pi, &p) in persons.iter().enumerate() {
            let Some(r) = pool::record_mut(&mut sim.save.persons, p) else {
                continue;
            };
            match assigned[pi] {
                Some(ri) => {
                    pool::put_u32(&mut r.data, PE_HUMAN, 1);
                    pool::put_u32(&mut r.data, PE_NET_ID, roster[ri].id);
                    pool::put_u32(&mut r.data, PE_IDENTITY, roster[ri].identity);
                }
                None => {
                    pool::put_u32(&mut r.data, PE_HUMAN, 0);
                    pool::put_u32(&mut r.data, PE_NET_ID, u32::MAX);
                }
            }
        }
    }

    /// Makes `person` a human controlled by `net_id`, or a computer (`None`): the way a map
    /// start (`0x473590`: `+0x122 = (human index >= 0)`, `+0x126 = network id`) is set up.
    pub fn net_set_person_controller(&mut self, person: usize, net_id: Option<u32>) -> bool {
        let Some(sim) = self.sim.as_mut() else {
            return false;
        };
        let Some(r) = pool::record_mut(&mut sim.save.persons, person) else {
            return false;
        };
        pool::put_u32(&mut r.data, PE_HUMAN, u32::from(net_id.is_some()));
        pool::put_u32(&mut r.data, PE_NET_ID, net_id.unwrap_or(u32::MAX));
        true
    }

    /// `net_player_left` (`0x49d970`): the record leaves the roster, every person the player
    /// controlled becomes a computer (`+0x122 = 0`; with a peer version >= `0x232b`, also
    /// `+0x52f = 0`, `+0x548 = 3`, `+0x267 = 0`, so the AI can act at once). Company and cash
    /// stay. Idempotent: the second call finds no record (the duplicate removal of the original).
    /// With one player left the world is a local game again.
    pub fn net_player_left(&mut self, net_id: u32) {
        let Some(net) = self.net.as_mut() else {
            return;
        };
        let Some(i) = net.roster.iter().position(|p| p.id == net_id) else {
            return;
        };
        net.roster.remove(i);
        let mut person_ev = Vec::new();
        if let Some(sim) = self.sim.as_mut() {
            for p in pool::list_order(&sim.save.persons) {
                let Some(r) = pool::record_mut(&mut sim.save.persons, p) else {
                    continue;
                };
                if pool::u32_at(&r.data, PE_HUMAN) != 0
                    && pool::u32_at(&r.data, PE_NET_ID) == net_id
                {
                    pool::put_u32(&mut r.data, PE_HUMAN, 0);
                    r.data[0x52f] = 0;
                    pool::put_u32(&mut r.data, 0x548, 3);
                    pool::put_u32(&mut r.data, 0x267, 0);
                    person_ev.push(p as i32);
                }
            }
        }
        if let Some(net) = self.net.as_mut() {
            if person_ev.is_empty() {
                net.events.push(NetEvent::PlayerLeft { net_id, person: 0 });
            }
            for person in person_ev {
                net.events.push(NetEvent::PlayerLeft { net_id, person });
            }
        }
    }

    /// Clone extra (join in progress and reconnect): `net_id` enters the roster and takes
    /// `person` (a vacated seat chosen by the host, [`World::net_free_seat`]). Idempotent.
    pub fn net_player_joined(&mut self, player: NetPlayer, person: i32) {
        let Some(net) = self.net.as_mut() else {
            return;
        };
        if net.roster.iter().any(|p| p.id == player.id) {
            return;
        }
        let (id, identity) = (player.id, player.identity);
        let at = net
            .roster
            .iter()
            .position(|p| p.id > id)
            .unwrap_or(net.roster.len());
        net.roster.insert(at, player);
        net.events
            .push(NetEvent::PlayerJoined { net_id: id, person });
        if let Some(sim) = self.sim.as_mut()
            && person > 0
            && let Some(r) = pool::record_mut(&mut sim.save.persons, person as usize)
        {
            pool::put_u32(&mut r.data, PE_HUMAN, 1);
            pool::put_u32(&mut r.data, PE_NET_ID, id);
            pool::put_u32(&mut r.data, PE_IDENTITY, identity);
        }
        self.net_sync_multiplayer_flag();
    }

    /// The seat a returning or new player may take: the person (in player-set order) that a
    /// human controlled once (its identity `+0x54d` is set) and that is a computer now, first
    /// the one whose identity equals `identity`, else the first such seat. `None` without one.
    /// Decided by the host and carried by the join command, so every machine applies the same.
    pub fn net_free_seat(&self, identity: u32) -> Option<i32> {
        let sim = self.sim.as_ref()?;
        let seats: Vec<(usize, u32)> = sim
            .net_player_set()
            .into_iter()
            .filter_map(|p| {
                let r = pool::record(&sim.save.persons, p)?;
                let d = &r.data;
                (pool::u32_at(d, PE_HUMAN) == 0 && pool::u32_at(d, PE_IDENTITY) != 0)
                    .then(|| (p, pool::u32_at(d, PE_IDENTITY)))
            })
            .collect();
        seats
            .iter()
            .find(|&&(_, i)| i == identity)
            .or(seats.first())
            .map(|&(p, _)| p as i32)
    }

    /// The person this machine controls (view; the simulation never reads it).
    pub fn net_local_person(&self) -> Option<i32> {
        let id = self.net.as_ref()?.local_id;
        let sim = self.sim.as_ref()?;
        pool::list_order(&sim.save.persons)
            .into_iter()
            .find(|&p| {
                pool::record(&sim.save.persons, p).is_some_and(|r| {
                    pool::u32_at(&r.data, PE_HUMAN) != 0 && pool::u32_at(&r.data, PE_NET_ID) == id
                })
            })
            .map(|p| p as i32)
    }

    /// The company this machine's person chairs (view).
    pub fn net_local_company(&self) -> Option<i32> {
        let p = self.net_local_person()?;
        let sim = self.sim.as_ref()?;
        let r = pool::record(&sim.save.persons, p as usize)?;
        Some(pool::u32_at(&r.data, PE_CHAIR) as i32)
    }

    /// The end of the game for `person` in a network game: victory level, defeat, or defeat
    /// because another player won (scenarios.md 3.3, multiplayer.md 3.3). The scenario's single
    /// `game_over` of a local game is not used here.
    pub fn net_outcome(&self, person: i32) -> Option<NetOutcome> {
        self.sim
            .as_ref()?
            .net_outcomes
            .iter()
            .find(|o| o.person == person)
            .copied()
    }

    /// The checksum of the shared state now.
    pub fn net_digest(&self) -> Digest {
        digest(self.sim.as_deref(), self)
    }

    /// A checkpoint of the shared state now.
    pub fn net_checkpoint(&self) -> Checkpoint {
        checkpoint(self.sim.as_deref(), self, true)
    }

    /// The state to give a joiner or a diverged peer.
    pub fn net_snapshot(&self) -> NetSnapshot {
        let (file, outcomes) = match self.sim.as_ref() {
            Some(s) => (self.to_file_bytes(), s.net_outcomes.clone()),
            None => (Vec::new(), Vec::new()),
        };
        NetSnapshot {
            file,
            minutes: self.minutes,
            speed: self.speed,
            speed_resume: self.speed_resume,
            game_seed: self.game_seed,
            rng: self.rng.state(),
            crt: self.crt.state(),
            roster: self
                .net
                .as_ref()
                .map(|n| n.roster.clone())
                .unwrap_or_default(),
            outcomes,
            cmd_hash: self.net.as_ref().map_or(0, |n| n.cmd_hash),
        }
    }

    /// Builds the world a [`NetSnapshot`] describes (without a session: call
    /// [`World::net_attach`]). `data` is needed when the snapshot has a save.
    pub fn from_net_snapshot(
        snap: &NetSnapshot,
        data: Option<&crate::data::GameData>,
    ) -> Result<World, String> {
        let mut w = if snap.file.is_empty() {
            World::new(Date::from_minutes(snap.minutes), snap.game_seed, snap.rng)
        } else {
            let data = data.ok_or("a snapshot with a save needs the game data")?;
            let (save, rules) = World::parse_file(&snap.file).map_err(|e| e.to_string())?;
            let mut w = World::from_save_seeded(&save, data, snap.rng);
            w.set_ruleset(rules);
            if let Some(sim) = w.sim.as_mut() {
                sim.net_outcomes = snap.outcomes.clone();
            }
            w
        };
        w.minutes = snap.minutes;
        w.rng = GameRng::new(snap.rng);
        w.crt = CrtRng::new(snap.crt);
        w.game_seed = snap.game_seed;
        w.speed = snap.speed;
        w.speed_resume = snap.speed_resume;
        Ok(w)
    }
}

/// `links`: include the list order (head, tail, order, `next`/`prev`). The map animations
/// (`pool_78`, smoke puffs) are freed by `anim_object_step` on the pass after they expire, so the
/// order of their list depends on the size of the steps a machine ran; nothing reads it back.
fn hash_pool(h: &mut Fnv, p: &Pool, mask: Option<(usize, usize)>, links: bool) {
    h.u64(p.slots as u64);
    h.u64(p.count as u64);
    if links {
        h.u32(u32::from(p.head));
        h.u32(u32::from(p.tail));
    }
    for r in &p.records {
        h.u64(r.slot as u64);
        match mask {
            Some((a, b)) if r.data.len() >= b => {
                h.bytes(&r.data[..a]);
                h.bytes(&r.data[b..]);
            }
            _ => h.bytes(&r.data),
        }
    }
    h.bytes(&p.bitmap);
    if links {
        for &o in &p.order {
            h.u32(o as u32);
        }
        for &n in p.next.iter().chain(&p.prev) {
            h.u32(u32::from(n));
        }
    }
}

fn pool_hash(p: &Pool, mask: Option<(usize, usize)>) -> u64 {
    let mut h = Fnv::default();
    hash_pool(&mut h, p, mask, true);
    h.finish()
}

/// The checksum of a state. `sim` is passed apart from the world because the world loop runs
/// with the pools taken out of it.
pub(crate) fn digest(sim: Option<&crate::tick::SimState>, w: &World) -> Digest {
    let mut roster = Fnv::default();
    if let Some(n) = w.net.as_ref() {
        for p in &n.roster {
            roster.u32(p.id);
        }
    }
    let cmd_hash = w.net.as_ref().map_or(0, |n| n.cmd_hash);
    let mut d = Digest {
        rng: w.rng.state(),
        clock: w.minutes,
        roster: roster.finish(),
        ..Digest::default()
    };
    let Some(sim) = sim else {
        d.other = cmd_hash;
        return d;
    };
    let s = &sim.save;
    d.companies = pool_hash(&s.companies, None);
    d.persons = pool_hash(&s.persons, Some((PE_IDENTITY, PE_IDENTITY + 4)));
    d.trains = {
        let mut h = Fnv::default();
        hash_pool(&mut h, &s.trains, None, true);
        for e in &s.train_extras {
            h.u64(e.slot as u64);
            h.bytes(&e.cars_raw);
            h.bytes(&e.route_raw);
        }
        h.finish()
    };
    d.industries = pool_hash(&s.objects, None);
    d.stations = pool_hash(&s.stations, None);
    let mut h = Fnv::default();
    hash_pool(&mut h, &s.pool_78, None, false);
    for p in [
        &s.structures,
        &s.pool_70,
        &s.pool_58,
        &s.pool_7c,
        &s.pool_80,
        &s.track_pieces,
        &s.places,
        &s.territories,
        &s.pool_88,
    ] {
        hash_pool(&mut h, p, None, true);
    }
    // The state block without the interface history (it records the viewed company).
    let st = &s.state;
    let (a, b) = (
        ST_HISTORY.min(st.len()),
        (ST_HISTORY + HISTORY_LEN).min(st.len()),
    );
    h.bytes(&st[..a]);
    h.bytes(&st[b..]);
    // The map: the light bit (`tile+6 & 0x20`) is derived and only refreshed on a redraw.
    for t in &s.map.tiles {
        h.bytes(&[
            t.var,
            t.ground,
            t.flags2,
            t.trees,
            t.struct_id,
            t.reserved5,
            t.flags6 & !0x20,
            t.track_id,
            t.reserved8,
            t.region,
        ]);
    }
    for &e in &s.map.elevation {
        h.bytes(&e.to_le_bytes());
    }
    for r in &s.rules {
        h.bytes(format!("{r:?}").as_bytes());
    }
    for o in &sim.net_outcomes {
        h.u32(o.person as u32);
        h.bytes(&[o.state, u8::from(o.other_won)]);
    }
    h.u64(cmd_hash);
    d.other = h.finish();
    d
}

/// A checkpoint of the state, with a canonical clone of the save when the session keeps them.
pub(crate) fn checkpoint(
    sim: Option<&crate::tick::SimState>,
    w: &World,
    force_save: bool,
) -> Checkpoint {
    let d = digest(sim, w);
    let keep = force_save || w.net.as_ref().is_some_and(|n| n.keep_checkpoints);
    let save = sim.filter(|_| keep).map(|sim| {
        let mut s = sim.save.clone();
        // Clear what is per machine: the interface history and the viewed company.
        let st = &mut s.state;
        let (a, b) = (
            ST_HISTORY.min(st.len()),
            (ST_HISTORY + HISTORY_LEN).min(st.len()),
        );
        st[a..b].fill(0);
        for r in &mut s.persons.records {
            if r.data.len() >= PE_IDENTITY + 4 {
                r.data[PE_IDENTITY..PE_IDENTITY + 4].fill(0);
            }
        }
        // The animation list order (see `hash_pool`).
        let a = &mut s.pool_78;
        a.head = 0;
        a.tail = 0;
        a.next.fill(0);
        a.prev.fill(0);
        a.order.sort_unstable();
        for o in [HDR_LOCAL_COMPANY, HDR_LOCAL_FLAG] {
            if s.header.len() > o + 4 {
                s.header[o..o + 4].fill(0);
            }
        }
        // The clock and the RNG are in the digest, not in the save blocks of a running world.
        s.game_time_minutes = w.minutes;
        Box::new(s)
    });
    Checkpoint {
        minute: w.minutes,
        digest: d,
        save,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;

    fn world(seed: u32) -> World {
        World::new(Date::new(1860, 1, 1, 0, 0), 7, seed)
    }

    fn roster(n: u32) -> Vec<NetPlayer> {
        (1..=n)
            .map(|id| NetPlayer {
                id,
                identity: 100 + id,
                name: format!("p{id}"),
            })
            .collect()
    }

    #[test]
    fn network_needs_two_players() {
        let mut w = world(1);
        assert!(!w.is_network());
        w.net_begin(roster(1), 1);
        assert!(
            !w.is_network(),
            "one player is a local game (net_player_count() < 2)"
        );
        w.net_begin(roster(2), 1);
        assert!(w.is_network());
        w.net_player_left(2);
        assert!(!w.is_network(), "the last human left: local rules again");
    }

    #[test]
    fn the_rng_is_reseeded_every_day_in_a_network_game_only() {
        // multiplayer.md 1.6: rng_seed(T + game_seed + cash) at 23:59 of every day.
        let mut a = world(11);
        let mut b = world(99);
        a.net_begin(roster(2), 1);
        b.net_begin(roster(2), 2);
        // `net_begin` seeds with game_seed: equal states.
        assert_eq!(a.rng().state(), b.rng().state());
        a.rng_mut().range(0, 100);
        assert_ne!(a.rng().state(), b.rng().state());
        a.advance(1440);
        b.advance(1440);
        assert_eq!(
            a.rng().state(),
            b.rng().state(),
            "the day end re-seeds both"
        );
        // The seed is `T + game_seed` at 23:59 of the day that ended (cash 0 without persons).
        let t = Date::new(1860, 1, 1, 23, 59).to_minutes();
        assert_eq!(a.rng().state(), (t + 7) as u32);
        // A local game keeps its generator.
        let mut l = world(5);
        l.advance(1440);
        assert_eq!(l.rng().state(), 5);
    }

    #[test]
    fn the_checksum_point_is_the_end_of_day_1_and_carries_the_digest() {
        let mut w = world(1);
        w.net_begin(roster(2), 1);
        w.advance(1440 * 3);
        let evs = w.take_net_events();
        let syncs: Vec<_> = evs
            .iter()
            .filter_map(|e| match e {
                NetEvent::Sync(c) => Some(c.minute),
                _ => None,
            })
            .collect();
        assert_eq!(
            syncs,
            vec![Date::new(1860, 1, 1, 23, 59).to_minutes()],
            "only day 1 of the month"
        );
    }

    #[test]
    fn leaving_is_idempotent_and_the_roster_stays_sorted() {
        let mut w = world(1);
        w.net_begin(roster(3), 1);
        w.apply(Command::NetPlayerLeft { net_id: 2 });
        w.apply(Command::NetPlayerLeft { net_id: 2 });
        let ids: Vec<u32> = w
            .net_session()
            .unwrap()
            .roster()
            .iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids, vec![1, 3]);
        let evs = w.take_net_events();
        assert_eq!(
            evs.iter()
                .filter(|e| matches!(e, NetEvent::PlayerLeft { .. }))
                .count(),
            1
        );
        // A late joiner enters in id order.
        w.net_player_joined(
            NetPlayer {
                id: 2,
                identity: 5,
                name: "back".into(),
            },
            0,
        );
        let ids: Vec<u32> = w
            .net_session()
            .unwrap()
            .roster()
            .iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn the_digest_sees_the_commands_that_ran_and_when() {
        let mk = || {
            let mut w = world(1);
            w.net_begin(roster(2), 1);
            w
        };
        let (mut a, mut b) = (mk(), mk());
        assert_eq!(a.net_digest(), b.net_digest());
        a.apply(Command::SeedRng(5));
        b.apply(Command::SeedRng(5));
        assert_eq!(a.net_digest(), b.net_digest());
        // The same command one minute later is a different history.
        a.advance(1);
        a.apply(Command::SeedRng(6));
        b.advance(2);
        b.apply(Command::SeedRng(6));
        a.advance(1);
        assert_ne!(a.net_digest(), b.net_digest());
        // Speed changes run on arrival, not at a stamp: not part of the history.
        let (mut c, mut d) = (mk(), mk());
        c.apply(Command::SetSpeed(crate::SpeedChange::Absolute(7)));
        assert_eq!(c.net_digest(), d.net_digest());
        let _ = &mut d;
    }

    #[test]
    fn a_snapshot_rebuilds_the_same_world() {
        let mut w = world(3);
        w.net_begin(roster(2), 1);
        w.advance(5000);
        w.apply(Command::SeedRng(77));
        w.apply(Command::SetSpeed(crate::SpeedChange::Absolute(5)));
        let snap = w.net_snapshot();
        let mut r = World::from_net_snapshot(&snap, None).unwrap();
        r.net_attach(&snap, 2);
        assert_eq!(r.net_digest(), w.net_digest());
        assert_eq!((r.speed(), r.speed_resume()), (w.speed(), w.speed_resume()));
        // They stay equal while running.
        w.advance(3000);
        r.advance(3000);
        assert_eq!(r.net_digest(), w.net_digest());
    }

    #[test]
    fn the_digest_names_the_group_that_differs() {
        let a = Digest::default();
        let b = Digest {
            trains: 1,
            rng: 2,
            ..a
        };
        let f = a.differs(&b);
        assert_eq!(Digest::group_names(f), vec!["rng", "trains"]);
        assert_eq!(Digest::from_bytes(&b.to_bytes()), b);
    }
}
