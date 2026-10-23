//! The session: lobby, start barrier, host-sequenced lockstep, heartbeats, host migration, join in
//! progress, checksum and resynchronisation (docs/spec/multiplayer.md 1 and 5;
//! docs/net-protocol.md "Session").
//!
//! A [`Session`] owns the [`World`] and a [`Transport`]. The caller drives it by calling
//! [`Session::step`] once per simulation frame (nominally every [`Config::frame_ms`] = 100 ms)
//! with a monotonic millisecond clock of its own; nothing in here reads a clock, so a test can
//! run hours of game time in a blink with logical time. [`Session::send_command`] is the only
//! way the local player changes the game: the command goes to the host, comes back in a batch and
//! runs at the batch's stamp on every machine, the host included.

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use rt2_formats::map::Save;
use rt2_sim::calendar::SpeedTable;
use rt2_sim::data::GameData;
use rt2_sim::net::{Checkpoint, Digest, NetEvent, NetPlayer};
use rt2_sim::{Command, World};

use crate::NetError;
use crate::link::Reliable;
use crate::msg::{
    Batch, Hello, MAX_CHAT, Msg, PROTOCOL_VERSION, Pending, PlayerInfo, SnapKind, SnapshotMsg,
    Welcome,
};
use crate::transport::{LinkId, Transport, TransportEvent};

/// The speed table of `docs/spec/time.md` 3 (a test fixture and the default; a game takes it from
/// the executable).
pub fn default_speed_table() -> SpeedTable {
    SpeedTable([0.0, 0.3, 0.6, 1.0, 1.7, 2.1, 3.0, 5.0, 8.0, 12.0, 18.0])
}

/// Tunables. The defaults are the original's (`multiplayer.md` 1.5 and 5.1).
#[derive(Clone, Debug)]
pub struct Config {
    /// Profile name shown to the others (`+0x2b`, 30 characters).
    pub name: String,
    /// Persistent identity of the player (`+0x24`): a returning player takes the same seat.
    pub identity: u32,
    /// Hash of the executable the game data came from ([`crate::hash_bytes`]); 0 for none.
    pub exe_hash: u64,
    pub speeds: SpeedTable,
    /// Length of a simulation frame at normal pacing.
    pub frame_ms: u32,
    /// An empty batch goes out after this long without commands (the 1 s heartbeat of `0x4af8c0`).
    pub batch_idle_ms: u64,
    /// The horizon: game time the host lets everyone run ahead (`2000` ms of game time).
    pub horizon_ms: u32,
    /// Extra horizon the new host grants after a migration (`8000` ms).
    pub takeover_extra_ms: u32,
    /// A heartbeat after this long without sending to a peer (10 s).
    pub send_idle_ms: u64,
    /// A probe after this long without hearing a peer (23 s), repeated every `probe_every_ms`.
    pub probe_after_ms: u64,
    pub probe_every_ms: u64,
    /// A peer silent for longer than this is removed (90.001 s).
    pub silence_ms: u64,
    /// An unacknowledged message is sent again after this long (the original: 3 s).
    pub resend_ms: u64,
    pub ping_every_ms: u64,
    /// Accept players after the game started (clone extra).
    pub join_in_progress: bool,
    pub max_players: usize,
    /// Where the desync diff dumps go (`desync-<minute>-p<id>.txt`).
    pub dump_dir: Option<PathBuf>,
    /// After a desync, the host sends its state to the peer that diverged (clone extra).
    pub resync_on_desync: bool,
}

impl Config {
    pub fn new(name: &str, identity: u32, speeds: SpeedTable) -> Config {
        Config {
            name: name.chars().take(30).collect(),
            identity,
            exe_hash: 0,
            speeds,
            frame_ms: 100,
            batch_idle_ms: 1000,
            horizon_ms: 2000,
            takeover_extra_ms: 8000,
            send_idle_ms: 10_000,
            probe_after_ms: 23_000,
            probe_every_ms: 5000,
            silence_ms: 90_001,
            resend_ms: 3000,
            ping_every_ms: 5000,
            join_in_progress: true,
            max_players: 32,
            dump_dir: None,
            resync_on_desync: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Dialed the host, waiting for the welcome (and, for a running game, the snapshot).
    Connecting,
    /// Players gather; the host may [`Session::start`].
    Lobby,
    /// The start snapshot is out; the host waits for everybody to load.
    Loading,
    Playing,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveReason {
    /// The player said goodbye (`0xc`).
    Left,
    /// Nothing heard for the silence limit (90 s).
    TimedOut,
    /// The connection closed.
    Disconnected,
}

/// What happened during a [`Session::step`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The host welcomed us.
    Connected {
        your_id: u32,
        host_id: u32,
    },
    PlayerJoined {
        id: u32,
        name: String,
    },
    PlayerLeft {
        id: u32,
        name: String,
        reason: LeaveReason,
    },
    /// The game started (or we joined a running one).
    Started,
    HostChanged {
        host: u32,
    },
    /// A person was taken over by a computer (LNG 2319) or by a returning human.
    PersonAssumedByAi {
        net_id: u32,
        person: i32,
    },
    PersonTaken {
        net_id: u32,
        person: i32,
    },
    /// The host found different checksums at `minute`.
    Desync {
        minute: i32,
        peer: u32,
        groups: Vec<&'static str>,
    },
    /// The diff of the states of the host and `peer` at `minute`.
    DesyncDump {
        minute: i32,
        peer: u32,
        path: Option<PathBuf>,
        lines: Vec<String>,
    },
    /// Our state was replaced by the host's after a desync.
    Resynced,
    Chat {
        from: u32,
        name: String,
        text: String,
    },
    /// The host refused us, or the connection failed.
    Rejected(String),
    /// A message could not be read or came from nowhere.
    Protocol(String),
    Closed,
}

struct LinkState {
    rel: Reliable,
    peer: Option<u32>,
}

#[derive(Default)]
struct Peer {
    link: LinkId,
    last_heard: u64,
    last_sent: u64,
    last_probe: u64,
    last_ping: u64,
    ping_ms: u32,
    /// Receives batches (the host adds a peer once its snapshot is out).
    active: bool,
    loaded: bool,
}

/// One player's connection to the others and to the game.
pub struct Session<T: Transport> {
    cfg: Config,
    t: T,
    data: Option<Arc<GameData>>,
    world: World,
    state: State,
    my_id: u32,
    host_id: u32,
    /// Connection roster, sorted by id (the original's `0x577e2f`).
    players: Vec<PlayerInfo>,
    links: BTreeMap<LinkId, LinkState>,
    peers: BTreeMap<u32, Peer>,
    host_link: Option<LinkId>,
    now: u64,
    events: Vec<Event>,
    // Lockstep.
    out_list: Vec<(u32, Command)>,
    /// Host: sequence number of the last batch sent.
    seq: u32,
    /// Last batch applied here.
    last_seq: u32,
    /// The horizon of the last batch applied (`0x579220`).
    horizon: i32,
    /// Host: horizon of the previous batch (`0x579214`), the stamp of the next.
    host_prev_h: i32,
    last_batch_ms: u64,
    takeover: bool,
    buffered: BTreeMap<u32, Batch>,
    queue: VecDeque<Pending>,
    last_frame_ms: u64,
    // Checksum.
    my_checkpoints: VecDeque<Checkpoint>,
    sync_table: BTreeMap<i32, BTreeMap<u32, Digest>>,
    flagged: BTreeMap<(i32, u32), ()>,
}

fn to_net(p: &PlayerInfo) -> NetPlayer {
    NetPlayer {
        id: p.id,
        identity: p.identity,
        name: p.name.clone(),
    }
}

impl<T: Transport> Session<T> {
    fn new(t: T, cfg: Config, world: World, data: Option<Arc<GameData>>) -> Session<T> {
        Session {
            cfg,
            t,
            data,
            world,
            state: State::Connecting,
            my_id: 0,
            host_id: 0,
            players: Vec::new(),
            links: BTreeMap::new(),
            peers: BTreeMap::new(),
            host_link: None,
            now: 0,
            events: Vec::new(),
            out_list: Vec::new(),
            seq: 0,
            last_seq: 0,
            horizon: 0,
            host_prev_h: 0,
            last_batch_ms: 0,
            takeover: false,
            buffered: BTreeMap::new(),
            queue: VecDeque::new(),
            last_frame_ms: 0,
            my_checkpoints: VecDeque::new(),
            sync_table: BTreeMap::new(),
            flagged: BTreeMap::new(),
        }
    }

    /// Creates a game as the host. `world` is the game before it starts (a loaded save or a new
    /// map); the players that connect before [`Session::start`] get a seat in it.
    pub fn host(t: T, cfg: Config, world: World, data: Option<Arc<GameData>>) -> Session<T> {
        let mut s = Session::new(t, cfg, world, data);
        s.my_id = 1;
        s.host_id = 1;
        s.players = vec![PlayerInfo {
            id: 1,
            identity: s.cfg.identity,
            name: s.cfg.name.clone(),
            addr: s.t.local_addr(),
        }];
        s.state = State::Lobby;
        s
    }

    /// Joins the game of the host at `host_addr`. The world is created from the snapshot the host
    /// sends; `placeholder` only stands in until then.
    pub fn join(
        mut t: T,
        cfg: Config,
        host_addr: &str,
        placeholder: World,
        data: Option<Arc<GameData>>,
    ) -> Result<Session<T>, NetError> {
        let link = t.connect(host_addr)?;
        let mut s = Session::new(t, cfg, placeholder, data);
        s.links.insert(
            link,
            LinkState {
                rel: Reliable::default(),
                peer: None,
            },
        );
        s.host_link = Some(link);
        let h = Msg::Hello(Hello {
            protocol: PROTOCOL_VERSION,
            rules_hash: s.my_rules_hash(),
            identity: s.cfg.identity,
            name: s.cfg.name.clone(),
            addr: s.t.local_addr(),
            mesh_id: None,
        });
        s.send_link(link, &h);
        Ok(s)
    }

    fn my_rules_hash(&self) -> u64 {
        crate::rules_hash(self.cfg.exe_hash, self.world.ruleset().bits())
    }

    // ---- accessors ------------------------------------------------------------------------

    pub fn world(&self) -> &World {
        &self.world
    }

    /// Mutable access to the world, for the front end's reads that need `&mut` (message queue).
    /// Never apply commands through it in a network game: use [`Session::send_command`].
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn my_id(&self) -> u32 {
        self.my_id
    }

    pub fn host_id(&self) -> u32 {
        self.host_id
    }

    pub fn is_host(&self) -> bool {
        self.my_id != 0 && self.my_id == self.host_id
    }

    pub fn players(&self) -> &[PlayerInfo] {
        &self.players
    }

    /// Round-trip time to a player in ms (0 for us or before the first answer).
    pub fn ping_ms(&self, id: u32) -> u32 {
        self.peers.get(&id).map_or(0, |p| p.ping_ms)
    }

    /// The checksum points this machine passed lately, `(game minute, digest)`, oldest first.
    pub fn checkpoints(&self) -> Vec<(i32, Digest)> {
        self.my_checkpoints
            .iter()
            .map(|c| (c.minute, c.digest))
            .collect()
    }

    /// Test hook: where the host writes desync dumps.
    pub fn debug_set_dump_dir(&mut self, dir: &std::path::Path) {
        self.cfg.dump_dir = Some(dir.to_path_buf());
    }

    /// Test hook: accept or refuse players after the start.
    pub fn debug_set_join_in_progress(&mut self, on: bool) {
        self.cfg.join_in_progress = on;
    }

    pub fn transport(&self) -> &T {
        &self.t
    }

    pub fn local_addr(&self) -> String {
        self.t.local_addr()
    }

    /// Commands waiting for their minute.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Lockstep is on with two or more players connected; alone the game runs as a local one.
    fn lockstep(&self) -> bool {
        self.players.len() >= 2
    }

    /// Test hook: applies a command to this machine's world only, outside the lockstep. This is
    /// how a test makes one machine diverge.
    pub fn debug_apply_local(&mut self, cmd: Command) {
        self.world.apply(cmd);
    }

    // ---- sending --------------------------------------------------------------------------

    fn send_link(&mut self, link: LinkId, m: &Msg) {
        let payload = m.encode();
        let now = self.now;
        let Some(st) = self.links.get_mut(&link) else {
            return;
        };
        let frame = if m.reliable() {
            st.rel.data(&payload, now)
        } else {
            Reliable::raw(&payload)
        };
        self.t.send(link, &frame);
        if let Some(id) = st.peer
            && let Some(p) = self.peers.get_mut(&id)
        {
            p.last_sent = now;
        }
    }

    fn send_peer(&mut self, id: u32, m: &Msg) {
        if let Some(link) = self.peers.get(&id).map(|p| p.link) {
            self.send_link(link, m);
        }
    }

    fn send_all_active(&mut self, m: &Msg) {
        let ids: Vec<u32> = self
            .peers
            .iter()
            .filter(|(_, p)| p.active)
            .map(|(&i, _)| i)
            .collect();
        for id in ids {
            self.send_peer(id, m);
        }
    }

    fn send_everyone(&mut self, m: &Msg) {
        let ids: Vec<u32> = self.peers.keys().copied().collect();
        for id in ids {
            self.send_peer(id, m);
        }
    }

    fn add_peer(&mut self, id: u32, link: LinkId, active: bool) {
        if let Some(st) = self.links.get_mut(&link) {
            st.peer = Some(id);
        }
        let now = self.now;
        let p = self.peers.entry(id).or_default();
        p.link = link;
        p.last_heard = now;
        p.last_sent = now;
        p.active |= active;
    }

    fn name_of(&self, id: u32) -> String {
        self.players
            .iter()
            .find(|p| p.id == id)
            .map_or_else(String::new, |p| p.name.clone())
    }

    // ---- the local player's commands ------------------------------------------------------

    /// Sends a command of the local player: to the host, which orders it into a batch. Alone,
    /// it runs at once.
    pub fn send_command(&mut self, cmd: Command) {
        if self.state != State::Playing {
            return;
        }
        if !self.lockstep() {
            self.world.apply(cmd);
            return;
        }
        if self.is_host() {
            self.out_list.push((self.my_id, cmd));
        } else if let Some(host) = self.peers.get(&self.host_id).map(|p| p.link) {
            self.send_link(host, &Msg::Cmd(cmd));
        }
    }

    /// Chat to everybody (`multiplayer.md` 4.1); the text is cut at 900 characters.
    pub fn send_chat(&mut self, text: &str) {
        let t: String = text.chars().take(MAX_CHAT).collect();
        self.send_everyone(&Msg::Chat(t));
    }

    /// Says goodbye (`0xc`) and closes.
    pub fn leave(&mut self) {
        self.send_everyone(&Msg::Leave);
        let links: Vec<LinkId> = self.links.keys().copied().collect();
        for l in links {
            self.t.close(l);
        }
        self.links.clear();
        self.peers.clear();
        self.state = State::Closed;
        self.events.push(Event::Closed);
    }

    // ---- the host starts the game ---------------------------------------------------------

    /// The host starts the game with the players connected now: everybody loads the same state
    /// and `net_begin` seeds the RNG with `game_seed` and gives each human a person
    /// (`net_game_start` case 6, `multiplayer.md` 2.6).
    pub fn start(&mut self) -> Result<(), NetError> {
        if !self.is_host() || self.state != State::Lobby {
            return Err(NetError::State("only the host in the lobby can start"));
        }
        let snap = self.world.net_snapshot();
        let ids: Vec<u32> = self.peers.keys().copied().collect();
        for id in ids {
            let m = Msg::Snapshot(Box::new(SnapshotMsg {
                kind: SnapKind::Start,
                snap: snap.clone(),
                players: self.players.clone(),
                host_id: self.host_id,
                your_id: id,
                last_seq: 0,
                horizon: 0,
                host_prev_h: 0,
                pending: Vec::new(),
            }));
            self.send_peer(id, &m);
            if let Some(p) = self.peers.get_mut(&id) {
                p.active = true;
            }
        }
        self.apply_start(&snap)?;
        self.state = State::Loading;
        self.events.push(Event::Started);
        Ok(())
    }

    fn apply_start(&mut self, snap: &rt2_sim::net::NetSnapshot) -> Result<(), NetError> {
        let mut w = World::from_net_snapshot(snap, self.data.as_deref())
            .map_err(|e| NetError::Refused(format!("cannot load the start state: {e}")))?;
        let roster = self.players.iter().map(to_net).collect();
        w.net_begin(roster, self.my_id);
        w.net_keep_checkpoints(true);
        self.world = w;
        self.queue.clear();
        self.last_seq = 0;
        self.horizon = 0;
        self.host_prev_h = 0;
        self.seq = 0;
        Ok(())
    }

    // ---- the frame ------------------------------------------------------------------------

    /// Runs the network and the simulation for one frame. Returns what happened.
    pub fn step(&mut self, now_ms: u64) -> Vec<Event> {
        self.now = now_ms;
        if self.state == State::Closed {
            return std::mem::take(&mut self.events);
        }
        for ev in self.t.poll(now_ms) {
            self.on_transport(ev);
        }
        self.resend();
        self.maintain();
        if self.is_host() {
            if self.state == State::Loading
                && self.peers.values().filter(|p| p.active).all(|p| p.loaded)
            {
                self.begin_playing();
            }
            if self.state == State::Playing && self.lockstep() {
                self.flush();
            }
        }
        if self.state == State::Playing {
            self.run_frames();
            self.drain_world_events();
        }
        std::mem::take(&mut self.events)
    }

    fn begin_playing(&mut self) {
        self.state = State::Playing;
        self.last_frame_ms = self.now;
        self.last_batch_ms = 0;
        // The batches that arrived while loading.
        self.apply_buffered();
    }

    fn resend(&mut self) {
        let (now, every) = (self.now, self.cfg.resend_ms);
        let mut out = Vec::new();
        for (&l, st) in &mut self.links {
            for f in st.rel.due(now, every) {
                out.push((l, f));
            }
        }
        for (l, f) in out {
            self.t.send(l, &f);
        }
    }

    /// Heartbeats, probes, pings and the silence limit (`0x49ce50`, `multiplayer.md` 5.1): the
    /// host watches everybody, a guest only the host.
    fn maintain(&mut self) {
        if matches!(self.state, State::Connecting | State::Closed) {
            return;
        }
        let now = self.now;
        let host = self.host_id;
        let me_host = self.is_host();
        let ids: Vec<u32> = self
            .peers
            .keys()
            .copied()
            .filter(|&i| me_host || i == host)
            .collect();
        let mut dead = Vec::new();
        for id in ids {
            let Some(p) = self.peers.get(&id) else {
                continue;
            };
            let (heard, sent, probed, pinged) =
                (p.last_heard, p.last_sent, p.last_probe, p.last_ping);
            let quiet = now.saturating_sub(heard);
            if quiet > self.cfg.silence_ms {
                dead.push(id);
                continue;
            }
            if quiet >= self.cfg.probe_after_ms
                && now.saturating_sub(probed) >= self.cfg.probe_every_ms
            {
                self.peers.get_mut(&id).unwrap().last_probe = now;
                self.send_peer(id, &Msg::Probe);
            }
            if now.saturating_sub(sent) >= self.cfg.send_idle_ms {
                self.send_peer(id, &Msg::Heartbeat);
            }
            if now.saturating_sub(pinged) >= self.cfg.ping_every_ms {
                self.peers.get_mut(&id).unwrap().last_ping = now;
                self.send_peer(id, &Msg::Ping(now));
            }
        }
        for id in dead {
            self.drop_player(id, LeaveReason::TimedOut);
        }
    }

    // ---- the host's batches ---------------------------------------------------------------

    /// `cmd_net_flush` (`0x4af8c0`): a batch leaves when commands are waiting or a second has
    /// passed since the last one.
    fn flush(&mut self) {
        let idle = self.now.saturating_sub(self.last_batch_ms) >= self.cfg.batch_idle_ms;
        if self.out_list.is_empty() && !idle {
            return;
        }
        let t = self.world.minutes();
        let speed = self.world.speed();
        let mins = |ms: u32| self.cfg.speeds.minutes_for_ms(speed, ms);
        let mut h = self
            .host_prev_h
            .max(t.wrapping_add(mins(self.cfg.horizon_ms)).wrapping_add(1));
        if self.takeover {
            // The new host: +8 s of slack, and never less than the horizon seen or `T`, plus one.
            let base = if self.horizon != 0 { self.horizon } else { t };
            h = h.max(
                t.wrapping_add(mins(self.cfg.horizon_ms))
                    .wrapping_add(mins(self.cfg.takeover_extra_ms))
                    .wrapping_add(1),
            );
            h = h.max(base.wrapping_add(1));
            self.takeover = false;
        }
        self.seq += 1;
        let b = Batch {
            seq: self.seq,
            stamp: self.host_prev_h,
            horizon: h,
            cmds: std::mem::take(&mut self.out_list),
        };
        self.host_prev_h = h;
        self.last_batch_ms = self.now;
        self.send_all_active(&Msg::Batch(b.clone()));
        self.on_batch(b);
    }

    /// `net_batch_receive`/`apply` (`0x4afc80`/`0x4afd50`): batches are applied in order of
    /// sequence; speed and pause run on arrival, everything else waits for the stamp.
    fn on_batch(&mut self, b: Batch) {
        if b.seq <= self.last_seq {
            return;
        }
        self.buffered.insert(b.seq, b);
        self.apply_buffered();
    }

    /// Applies the buffered batches that follow the last one applied. Only a running game
    /// applies them: before that (connecting, loading) they wait.
    fn apply_buffered(&mut self) {
        if self.state != State::Playing {
            return;
        }
        while let Some(b) = self.buffered.remove(&(self.last_seq + 1)) {
            self.last_seq = b.seq;
            if b.horizon != 0 {
                self.horizon = b.horizon;
            }
            for (sender, cmd) in b.cmds {
                if matches!(cmd, Command::SetSpeed(_) | Command::TogglePause) {
                    self.world.apply(cmd);
                } else {
                    self.queue.push_back(Pending {
                        stamp: b.stamp,
                        sender,
                        cmd,
                    });
                }
            }
        }
    }

    // ---- pacing and execution -------------------------------------------------------------

    fn run_frames(&mut self) {
        let mut budget = 5;
        while budget > 0
            && self.now.saturating_sub(self.last_frame_ms) >= u64::from(self.cfg.frame_ms)
        {
            self.last_frame_ms += u64::from(self.cfg.frame_ms);
            self.run_frame();
            budget -= 1;
        }
        if self.now.saturating_sub(self.last_frame_ms) >= u64::from(self.cfg.frame_ms) * 5 {
            // Far behind (the window was stalled): do not try to catch up in one go.
            self.last_frame_ms = self.now;
        }
    }

    /// The frame pacing of `game_frame_update` (`0x405540`, `multiplayer.md` 1.5): the frame's
    /// minutes are cut at the horizon, and a machine far behind it speeds up (the 10.0 and 3.0
    /// ratios are interface calibration).
    fn pace(&self, dt0: i32) -> i32 {
        let t = self.world.minutes();
        let h = self.horizon;
        if h == 0 || h <= t {
            return 0;
        }
        let speed = self.world.speed();
        let slack = i64::from(h) + i64::from(dt0) - i64::from(t);
        let mut dt = dt0;
        if slack > 0 {
            let seg = self.cfg.speeds.minutes_for_ms(speed, 1000);
            if seg <= 0 {
                // Paused: run to the horizon, which the host moves one minute a second.
                dt = h - t;
            } else {
                let ratio = slack as f32 / seg as f32;
                if ratio > 10.0 {
                    dt = h - t;
                } else if ratio > 3.0 {
                    dt = rt2_sim::fp::ftol(f64::from(ratio * dt0 as f32 * (1.0 / 3.0)));
                }
            }
        }
        dt.clamp(0, h - t)
    }

    fn exec_due(&mut self) {
        while let Some(p) = self.queue.front() {
            if p.stamp > self.world.minutes() {
                break;
            }
            let p = self.queue.pop_front().unwrap();
            self.exec(p);
        }
    }

    fn exec(&mut self, p: Pending) {
        self.world.apply(p.cmd);
        if let Command::NetPlayerLeft { net_id } = p.cmd {
            if net_id == self.my_id {
                // The host removed us (it heard nothing for too long): our seat is a
                // computer's now.
                self.events
                    .push(Event::Rejected("removed from the game".into()));
                self.state = State::Closed;
                return;
            }
            self.forget_player(net_id);
        }
    }

    fn run_frame(&mut self) {
        let dt0 = self
            .cfg
            .speeds
            .minutes_for_ms(self.world.speed(), self.cfg.frame_ms);
        let lockstep = self.lockstep();
        let mut dt = if lockstep { self.pace(dt0) } else { dt0 };
        self.exec_due();
        loop {
            let t = self.world.minutes();
            let next = self.queue.iter().map(|p| p.stamp).filter(|&s| s > t).min();
            match next {
                Some(n) if dt > n - t => {
                    self.world.advance(n - t);
                    dt -= n - t;
                    self.exec_due();
                }
                _ => break,
            }
        }
        if lockstep {
            dt = dt.min((self.horizon - self.world.minutes()).max(0));
        }
        if dt > 0 {
            self.world.advance(dt);
        }
    }

    // ---- checksum and the world's events --------------------------------------------------

    fn drain_world_events(&mut self) {
        for ev in self.world.take_net_events() {
            match ev {
                NetEvent::Sync(cp) => {
                    let (minute, digest) = (cp.minute, cp.digest);
                    self.my_checkpoints.push_back(*cp);
                    while self.my_checkpoints.len() > 3 {
                        self.my_checkpoints.pop_front();
                    }
                    if self.lockstep() {
                        if self.is_host() {
                            self.on_sync(self.my_id, minute, digest);
                        } else {
                            self.send_peer(self.host_id, &Msg::Sync { minute, digest });
                        }
                    }
                }
                NetEvent::PlayerLeft { net_id, person } => {
                    self.events
                        .push(Event::PersonAssumedByAi { net_id, person });
                }
                NetEvent::PlayerJoined { net_id, person } => {
                    self.events.push(Event::PersonTaken { net_id, person });
                }
            }
        }
    }

    /// `net_sync_check` (`0x49de20`): only the host compares. The clone always reacts: a notice
    /// to the peer, the dump of the diff and (by default) the host's state for the peer.
    fn on_sync(&mut self, from: u32, minute: i32, digest: Digest) {
        self.sync_table
            .entry(minute)
            .or_default()
            .insert(from, digest);
        let newest = self
            .sync_table
            .keys()
            .next_back()
            .copied()
            .unwrap_or(minute);
        self.sync_table
            .retain(|&m, _| m > newest.saturating_sub(8 * 40_320));
        self.flagged
            .retain(|&(m, _), _| m > newest.saturating_sub(8 * 40_320));
        let Some(mine) = self.sync_table[&minute].get(&self.my_id).copied() else {
            return;
        };
        let others: Vec<(u32, Digest)> = self.sync_table[&minute]
            .iter()
            .filter(|&(&i, _)| i != self.my_id)
            .map(|(&i, &d)| (i, d))
            .collect();
        for (id, d) in others {
            if d == mine || self.flagged.contains_key(&(minute, id)) {
                continue;
            }
            self.flagged.insert((minute, id), ());
            let flags = mine.differs(&d);
            self.events.push(Event::Desync {
                minute,
                peer: id,
                groups: Digest::group_names(flags),
            });
            self.send_peer(
                id,
                &Msg::Desync {
                    minute,
                    host: mine,
                    flags,
                },
            );
            if self.cfg.resync_on_desync {
                self.send_snapshot(id, SnapKind::Resync);
            }
        }
    }

    fn on_state_dump(&mut self, from: u32, minute: i32, digest: Digest, save: Vec<u8>) {
        let Some(mine) = self
            .my_checkpoints
            .iter()
            .find(|c| c.minute == minute)
            .cloned()
        else {
            return;
        };
        let theirs = Checkpoint {
            minute,
            digest,
            save: if save.is_empty() {
                None
            } else {
                Save::parse(&save).ok().map(Box::new)
            },
        };
        let lines = mine.diff_lines(&theirs);
        let mut path = None;
        if let Some(dir) = &self.cfg.dump_dir {
            let p = dir.join(format!("desync-{minute}-p{from}.txt"));
            let mut text = format!(
                "desync at game minute {minute}: host {} vs player {from} ({})\n",
                self.my_id,
                self.name_of(from)
            );
            for l in &lines {
                text.push_str(l);
                text.push('\n');
            }
            if std::fs::create_dir_all(dir).is_ok() && std::fs::write(&p, text).is_ok() {
                path = Some(p);
            }
        }
        self.events.push(Event::DesyncDump {
            minute,
            peer: from,
            path,
            lines,
        });
    }

    // ---- snapshots ------------------------------------------------------------------------

    fn build_snapshot(&self, kind: SnapKind, your_id: u32, players: &[PlayerInfo]) -> Msg {
        Msg::Snapshot(Box::new(SnapshotMsg {
            kind,
            snap: self.world.net_snapshot(),
            players: players.to_vec(),
            host_id: self.host_id,
            your_id,
            last_seq: self.last_seq,
            horizon: self.horizon,
            host_prev_h: self.host_prev_h,
            pending: self.queue.iter().copied().collect(),
        }))
    }

    fn send_snapshot(&mut self, id: u32, kind: SnapKind) {
        let m = self.build_snapshot(kind, id, &self.players.clone());
        self.send_peer(id, &m);
    }

    fn on_snapshot(&mut self, s: SnapshotMsg) {
        self.my_id = s.your_id;
        self.host_id = s.host_id;
        self.players = s.players.clone();
        self.players.sort_by_key(|p| p.id);
        match s.kind {
            SnapKind::Start => {
                if self.apply_start(&s.snap).is_err() {
                    self.events
                        .push(Event::Protocol("cannot start".to_string()));
                    return;
                }
                self.state = State::Playing;
                self.last_frame_ms = self.now;
                self.events.push(Event::Started);
                self.send_peer(self.host_id, &Msg::Loaded);
                self.apply_buffered();
            }
            SnapKind::Join | SnapKind::Resync => {
                let world = match World::from_net_snapshot(&s.snap, self.data.as_deref()) {
                    Ok(w) => w,
                    Err(e) => {
                        self.events
                            .push(Event::Protocol(format!("cannot load the snapshot: {e}")));
                        return;
                    }
                };
                self.world = world;
                self.world.net_attach(&s.snap, self.my_id);
                self.world.net_keep_checkpoints(true);
                self.queue = s.pending.into();
                self.last_seq = s.last_seq;
                self.horizon = s.horizon;
                self.buffered.retain(|&q, _| q > s.last_seq);
                self.state = State::Playing;
                if s.kind == SnapKind::Join {
                    self.last_frame_ms = self.now;
                    self.events.push(Event::Started);
                } else {
                    self.events.push(Event::Resynced);
                }
                // Batches that arrived before the snapshot.
                self.apply_buffered();
            }
        }
    }

    // ---- receiving ------------------------------------------------------------------------

    fn on_transport(&mut self, ev: TransportEvent) {
        match ev {
            TransportEvent::Opened(l) => {
                self.links.insert(
                    l,
                    LinkState {
                        rel: Reliable::default(),
                        peer: None,
                    },
                );
            }
            TransportEvent::Closed(l) => {
                let Some(st) = self.links.remove(&l) else {
                    return;
                };
                if self.host_link == Some(l) && self.state == State::Connecting {
                    self.events
                        .push(Event::Rejected("the host closed the connection".into()));
                    self.state = State::Closed;
                    return;
                }
                if let Some(id) = st.peer
                    && self.peers.get(&id).is_some_and(|p| p.link == l)
                {
                    self.drop_player(id, LeaveReason::Disconnected);
                }
            }
            TransportEvent::Data(l, frame) => {
                let Some(st) = self.links.get_mut(&l) else {
                    return;
                };
                let rec = match st.rel.on_frame(&frame) {
                    Ok(r) => r,
                    Err(e) => {
                        self.events.push(Event::Protocol(e.to_string()));
                        return;
                    }
                };
                let peer = st.peer;
                if let Some(ack) = rec.reply {
                    self.t.send(l, &ack);
                }
                if let Some(id) = peer
                    && let Some(p) = self.peers.get_mut(&id)
                {
                    p.last_heard = self.now;
                }
                if let Some(payload) = rec.payload {
                    match Msg::decode(&payload) {
                        Ok(m) => self.on_msg(l, peer, m),
                        Err(e) => self.events.push(Event::Protocol(e.to_string())),
                    }
                }
            }
        }
    }

    fn on_msg(&mut self, link: LinkId, peer: Option<u32>, m: Msg) {
        // Before the handshake only a hello (host side) or the answer (guest side) counts.
        let Some(from) = peer else {
            match m {
                Msg::Hello(h) => self.on_hello(link, h),
                Msg::Welcome(w) => self.on_welcome(link, w),
                Msg::Reject(why) => {
                    self.events.push(Event::Rejected(why));
                    self.state = State::Closed;
                    self.t.close(link);
                }
                _ => {}
            }
            return;
        };
        match m {
            Msg::Hello(h) => self.on_hello(link, h),
            Msg::Welcome(_) | Msg::Reject(_) => {}
            Msg::Roster { host_id, players } => self.on_roster(host_id, players),
            Msg::Cmd(c) => {
                if self.is_host() && matches!(self.state, State::Loading | State::Playing) {
                    if self.lockstep() {
                        self.out_list.push((from, c));
                    } else {
                        self.world.apply(c);
                    }
                }
            }
            Msg::Batch(b) => self.on_batch(b),
            Msg::Heartbeat => {}
            Msg::Probe => self.send_peer(from, &Msg::Heartbeat),
            Msg::Leave => self.drop_player(from, LeaveReason::Left),
            Msg::Ping(t) => self.send_peer(from, &Msg::Pong(t)),
            Msg::Pong(t) => {
                let rtt = self.now.saturating_sub(t) as u32;
                if let Some(p) = self.peers.get_mut(&from) {
                    p.ping_ms = rtt;
                }
            }
            Msg::Sync { minute, digest } => {
                if self.is_host() {
                    self.on_sync(from, minute, digest);
                }
            }
            Msg::Desync { minute, .. } => {
                let reply = match self.my_checkpoints.iter().find(|c| c.minute == minute) {
                    Some(c) => Msg::StateDump {
                        minute,
                        digest: c.digest,
                        save: c.save.as_ref().map(|s| s.to_bytes()).unwrap_or_default(),
                    },
                    None => Msg::StateDump {
                        minute,
                        digest: Digest::default(),
                        save: Vec::new(),
                    },
                };
                self.send_peer(from, &reply);
            }
            Msg::StateDump {
                minute,
                digest,
                save,
            } => {
                if self.is_host() {
                    self.on_state_dump(from, minute, digest, save);
                }
            }
            Msg::Snapshot(s) => {
                if from == self.host_id || self.state == State::Connecting {
                    self.on_snapshot(*s);
                }
            }
            Msg::Loaded => {
                if let Some(p) = self.peers.get_mut(&from) {
                    p.loaded = true;
                }
            }
            Msg::HostChanged { host } => {
                if host != self.host_id && self.peers.contains_key(&self.host_id) {
                    let old = self.host_id;
                    self.drop_player(old, LeaveReason::Disconnected);
                }
            }
            Msg::Chat(text) => {
                let name = self.name_of(from);
                self.events.push(Event::Chat { from, name, text });
            }
        }
    }

    // ---- handshake ------------------------------------------------------------------------

    fn reject(&mut self, link: LinkId, why: &str) {
        self.send_link(link, &Msg::Reject(why.to_string()));
    }

    fn on_hello(&mut self, link: LinkId, h: Hello) {
        if h.protocol != PROTOCOL_VERSION {
            return self.reject(link, "protocol version differs");
        }
        if h.rules_hash != self.my_rules_hash() {
            return self.reject(link, "different build, game data or rules");
        }
        if let Some(id) = h.mesh_id {
            // A link between two players already in the game.
            self.add_peer(id, link, false);
            return;
        }
        if !self.is_host() {
            return self.reject(link, "not the host");
        }
        if self.players.len() >= self.cfg.max_players {
            return self.reject(link, "the game is full");
        }
        let id = self.players.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let info = PlayerInfo {
            id,
            identity: h.identity,
            name: h.name.chars().take(30).collect(),
            addr: h.addr.clone(),
        };
        match self.state {
            State::Lobby => {
                self.players.push(info.clone());
                self.add_peer(id, link, false);
                let w = Msg::Welcome(Welcome {
                    your_id: id,
                    host_id: self.host_id,
                    in_progress: false,
                    players: self.players.clone(),
                });
                self.send_link(link, &w);
                self.broadcast_roster();
                self.events.push(Event::PlayerJoined {
                    id,
                    name: info.name,
                });
            }
            State::Loading | State::Playing => {
                if !self.cfg.join_in_progress {
                    return self.reject(link, "the game already started");
                }
                let seat = if self.world.save_ref().is_some() {
                    match self.world.net_free_seat(h.identity) {
                        Some(p) => p,
                        None => return self.reject(link, "no free seat in this game"),
                    }
                } else {
                    0
                };
                self.players.push(info.clone());
                self.players.sort_by_key(|p| p.id);
                self.add_peer(id, link, true);
                if let Some(p) = self.peers.get_mut(&id) {
                    p.loaded = true;
                }
                let w = Msg::Welcome(Welcome {
                    your_id: id,
                    host_id: self.host_id,
                    in_progress: true,
                    players: self.players.clone(),
                });
                self.send_link(link, &w);
                // The snapshot is the state before the join command; the joiner gets every
                // batch from here on, including the one that carries the command.
                let m = self.build_snapshot(SnapKind::Join, id, &self.players.clone());
                self.send_link(link, &m);
                let mut name = [0u8; 31];
                for (d, s) in name.iter_mut().take(30).zip(info.name.bytes()) {
                    *d = s;
                }
                self.out_list.push((
                    self.my_id,
                    Command::NetPlayerJoined {
                        net_id: id,
                        identity: h.identity,
                        person: seat,
                        name,
                    },
                ));
                self.broadcast_roster();
                self.events.push(Event::PlayerJoined {
                    id,
                    name: info.name,
                });
            }
            _ => self.reject(link, "not accepting players"),
        }
    }

    fn broadcast_roster(&mut self) {
        let m = Msg::Roster {
            host_id: self.host_id,
            players: self.players.clone(),
        };
        let ids: Vec<u32> = self
            .peers
            .iter()
            .filter(|(i, _)| **i != self.my_id)
            .map(|(&i, _)| i)
            .collect();
        for id in ids {
            self.send_peer(id, &m);
        }
    }

    fn on_welcome(&mut self, link: LinkId, w: Welcome) {
        self.my_id = w.your_id;
        self.host_id = w.host_id;
        self.players = w.players.clone();
        self.players.sort_by_key(|p| p.id);
        self.add_peer(w.host_id, link, true);
        self.state = if w.in_progress {
            State::Connecting
        } else {
            State::Lobby
        };
        self.events.push(Event::Connected {
            your_id: w.your_id,
            host_id: w.host_id,
        });
        self.dial_mesh();
    }

    /// Connects to every other player (guests keep links among themselves so that a host
    /// migration finds its successor).
    fn dial_mesh(&mut self) {
        let targets: Vec<PlayerInfo> = self
            .players
            .iter()
            .filter(|p| {
                p.id != self.my_id && p.id != self.host_id && !self.peers.contains_key(&p.id)
            })
            .cloned()
            .collect();
        for p in targets {
            self.dial(&p);
        }
    }

    fn dial(&mut self, p: &PlayerInfo) {
        let Ok(link) = self.t.connect(&p.addr) else {
            self.events
                .push(Event::Protocol(format!("no route to player {}", p.id)));
            return;
        };
        self.links.insert(
            link,
            LinkState {
                rel: Reliable::default(),
                peer: None,
            },
        );
        self.add_peer(p.id, link, false);
        let h = Msg::Hello(Hello {
            protocol: PROTOCOL_VERSION,
            rules_hash: self.my_rules_hash(),
            identity: self.cfg.identity,
            name: self.cfg.name.clone(),
            addr: self.t.local_addr(),
            mesh_id: Some(self.my_id),
        });
        self.send_link(link, &h);
    }

    fn on_roster(&mut self, host_id: u32, players: Vec<PlayerInfo>) {
        let had: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        self.players = players;
        self.players.sort_by_key(|p| p.id);
        self.host_id = host_id;
        let now_ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        for id in had.iter().filter(|i| !now_ids.contains(i)).copied() {
            let name = String::new();
            self.remove_peer_link(id);
            self.events.push(Event::PlayerLeft {
                id,
                name,
                reason: LeaveReason::Disconnected,
            });
        }
        for p in self.players.clone() {
            if !had.contains(&p.id) && p.id != self.my_id {
                self.events.push(Event::PlayerJoined {
                    id: p.id,
                    name: p.name.clone(),
                });
            }
        }
        // In the lobby a guest also dials the players that came after it.
        if matches!(self.state, State::Lobby | State::Playing) {
            self.dial_mesh();
        }
    }

    // ---- leaving, dropping, migration -----------------------------------------------------

    fn remove_peer_link(&mut self, id: u32) {
        if let Some(p) = self.peers.remove(&id) {
            self.links.remove(&p.link);
            self.t.close(p.link);
        }
    }

    /// The command `0x01` ran: forget the player in the connection roster too.
    fn forget_player(&mut self, id: u32) {
        if self.players.iter().any(|p| p.id == id) && id != self.my_id {
            let name = self.name_of(id);
            self.players.retain(|p| p.id != id);
            self.remove_peer_link(id);
            self.events.push(Event::PlayerLeft {
                id,
                name,
                reason: LeaveReason::Disconnected,
            });
        }
    }

    /// A player left, timed out or disconnected (`net_player_exit` `0x49d6f0`): if it was the
    /// host the lowest remaining id takes over; the removal reaches the world through the
    /// command `0x01` in a batch when three or more were playing, directly when two were.
    fn drop_player(&mut self, id: u32, reason: LeaveReason) {
        if !self.players.iter().any(|p| p.id == id) && !self.peers.contains_key(&id) {
            return;
        }
        let name = self.name_of(id);
        let before = self.players.len();
        let was_host = id == self.host_id;
        self.players.retain(|p| p.id != id);
        self.remove_peer_link(id);
        self.events.push(Event::PlayerLeft { id, name, reason });
        if was_host {
            self.migrate();
        }
        if before < 3 {
            // Two players: the removal is local on the machine that is left.
            self.world.net_player_left(id);
            if !self.lockstep() {
                self.go_solo();
            }
        } else if self.is_host() {
            self.out_list
                .push((self.my_id, Command::NetPlayerLeft { net_id: id }));
            self.broadcast_roster();
        }
        if self.state == State::Lobby && self.is_host() {
            self.broadcast_roster();
        }
    }

    fn migrate(&mut self) {
        let Some(new) = self.players.first().map(|p| p.id) else {
            return;
        };
        self.host_id = new;
        self.events.push(Event::HostChanged { host: new });
        if new == self.my_id {
            self.takeover = true;
            self.seq = self.last_seq;
            self.host_prev_h = self.horizon;
            self.last_batch_ms = self.now;
            for p in self.peers.values_mut() {
                p.last_heard = self.now;
                p.active = true;
            }
            self.send_everyone(&Msg::HostChanged { host: new });
        } else if let Some(p) = self.peers.get_mut(&new) {
            p.last_heard = self.now;
        } else if let Some(info) = self.players.iter().find(|p| p.id == new).cloned() {
            // No link to the successor yet.
            self.dial(&info);
        }
    }

    /// Alone again: the game is a local one (`multiplayer.md` 5.1); commands that waited for
    /// their minute run now.
    fn go_solo(&mut self) {
        let pending: Vec<Pending> = self.queue.drain(..).collect();
        for p in pending {
            self.exec(p);
        }
        let out = std::mem::take(&mut self.out_list);
        for (_, c) in out {
            self.world.apply(c);
        }
        self.host_id = self.my_id;
    }
}
