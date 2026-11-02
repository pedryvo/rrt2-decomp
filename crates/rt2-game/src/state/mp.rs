//! Multiplayer state machine and network driver (docs/spec/multiplayer.md, docs/net-protocol.md).
//!
//! Owns the [`rt2_net::session::Session`], translates lobby interactions and network events,
//! routes in-game commands through lockstep batches, and maintains the in-game chat and player
//! indicators.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use rt2_net::mem::MemTransport;
use rt2_net::session::{Config, Event, Session, State as NetSessionState};
use rt2_net::tcp::TcpTransport;
use rt2_net::{NetError, PlayerInfo};
use rt2_sim::data::GameData;
use rt2_sim::{Command, World};

/// Carrier backend: either real TCP sockets or in-process simulated memory.
pub enum NetBackend {
    Tcp(Session<TcpTransport>),
    Mem(Session<MemTransport>),
}

impl NetBackend {
    pub fn step(&mut self, now_ms: u64) -> Vec<Event> {
        match self {
            NetBackend::Tcp(s) => s.step(now_ms),
            NetBackend::Mem(s) => s.step(now_ms),
        }
    }

    pub fn send_command(&mut self, cmd: Command) {
        match self {
            NetBackend::Tcp(s) => s.send_command(cmd),
            NetBackend::Mem(s) => s.send_command(cmd),
        }
    }

    pub fn send_chat(&mut self, text: &str) {
        match self {
            NetBackend::Tcp(s) => s.send_chat(text),
            NetBackend::Mem(s) => s.send_chat(text),
        }
    }

    pub fn leave(&mut self) {
        match self {
            NetBackend::Tcp(s) => s.leave(),
            NetBackend::Mem(s) => s.leave(),
        }
    }

    pub fn start(&mut self) -> Result<(), NetError> {
        match self {
            NetBackend::Tcp(s) => s.start(),
            NetBackend::Mem(s) => s.start(),
        }
    }

    pub fn state(&self) -> NetSessionState {
        match self {
            NetBackend::Tcp(s) => s.state(),
            NetBackend::Mem(s) => s.state(),
        }
    }

    pub fn is_host(&self) -> bool {
        match self {
            NetBackend::Tcp(s) => s.is_host(),
            NetBackend::Mem(s) => s.is_host(),
        }
    }

    pub fn my_id(&self) -> u32 {
        match self {
            NetBackend::Tcp(s) => s.my_id(),
            NetBackend::Mem(s) => s.my_id(),
        }
    }

    pub fn host_id(&self) -> u32 {
        match self {
            NetBackend::Tcp(s) => s.host_id(),
            NetBackend::Mem(s) => s.host_id(),
        }
    }

    pub fn players(&self) -> &[PlayerInfo] {
        match self {
            NetBackend::Tcp(s) => s.players(),
            NetBackend::Mem(s) => s.players(),
        }
    }

    pub fn ping_ms(&self, id: u32) -> u32 {
        match self {
            NetBackend::Tcp(s) => s.ping_ms(id),
            NetBackend::Mem(s) => s.ping_ms(id),
        }
    }

    pub fn world(&self) -> &World {
        match self {
            NetBackend::Tcp(s) => s.world(),
            NetBackend::Mem(s) => s.world(),
        }
    }

    #[allow(dead_code)]
    pub fn world_mut(&mut self) -> &mut World {
        match self {
            NetBackend::Tcp(s) => s.world_mut(),
            NetBackend::Mem(s) => s.world_mut(),
        }
    }

    #[allow(dead_code)]
    pub fn local_addr(&self) -> String {
        match self {
            NetBackend::Tcp(s) => s.local_addr(),
            NetBackend::Mem(s) => s.local_addr(),
        }
    }
}

/// Character profile editor state (H3, LNG 1704 / 2912).
#[derive(Clone, Debug)]
pub struct CharacterEditState {
    pub name: String,
    pub description: String,
    pub portrait: u16,
    pub focused_field: usize,
}

impl CharacterEditState {
    pub fn new(name: &str, description: &str, portrait: u16) -> Self {
        CharacterEditState {
            name: name.chars().take(30).collect(),
            description: description.chars().take(250).collect(),
            portrait: portrait % 41,
            focused_field: 0,
        }
    }
}

/// Lobby state while players gather and configure the game (K1–K3).
pub struct MpLobbyState {
    pub backend: NetBackend,
    pub approvals: BTreeMap<u32, bool>,
    pub connect_addr: String,
    pub status_message: Option<String>,
    pub chat_log: Vec<(String, String)>,
    pub chat_input: String,
    pub scenario_name: String,
    pub setup: crate::setup::Setup,
    pub character_edit: Option<CharacterEditState>,
    pub connect_dialog_open: bool,
    #[allow(dead_code)]
    pub save_path: Option<PathBuf>,
}

impl MpLobbyState {
    pub fn host_tcp(
        port: u16,
        cfg: Config,
        world: World,
        data: Option<Arc<GameData>>,
        scenario_name: String,
        setup: crate::setup::Setup,
    ) -> Result<Self, NetError> {
        let bind = format!("0.0.0.0:{port}");
        let t = TcpTransport::listen(&bind)?;
        let s = Session::host(t, cfg, world, data);
        let local = s.local_addr();
        let my_id = s.my_id();
        let mut approvals = BTreeMap::new();
        approvals.insert(my_id, false);
        Ok(MpLobbyState {
            backend: NetBackend::Tcp(s),
            approvals,
            connect_addr: local,
            status_message: None,
            chat_log: Vec::new(),
            chat_input: String::new(),
            scenario_name,
            setup,
            character_edit: None,
            connect_dialog_open: false,
            save_path: None,
        })
    }

    pub fn host_mem(
        t: MemTransport,
        cfg: Config,
        world: World,
        data: Option<Arc<GameData>>,
        scenario_name: String,
        setup: crate::setup::Setup,
    ) -> Self {
        let s = Session::host(t, cfg, world, data);
        let my_id = s.my_id();
        let mut approvals = BTreeMap::new();
        approvals.insert(my_id, false);
        MpLobbyState {
            backend: NetBackend::Mem(s),
            approvals,
            connect_addr: "mem:host".into(),
            status_message: None,
            chat_log: Vec::new(),
            chat_input: String::new(),
            scenario_name,
            setup,
            character_edit: None,
            connect_dialog_open: false,
            save_path: None,
        }
    }

    pub fn join_tcp(
        addr: &str,
        cfg: Config,
        world_placeholder: World,
        data: Option<Arc<GameData>>,
    ) -> Result<Self, NetError> {
        let t = TcpTransport::listen("0.0.0.0:0")?;
        let s = Session::join(t, cfg, addr, world_placeholder, data)?;
        Ok(MpLobbyState {
            backend: NetBackend::Tcp(s),
            approvals: BTreeMap::new(),
            connect_addr: addr.to_string(),
            status_message: Some(format!("Connecting to {addr}...")),
            chat_log: Vec::new(),
            chat_input: String::new(),
            scenario_name: String::new(),
            setup: crate::setup::Setup::preset(crate::setup::Level::Medium),
            character_edit: None,
            connect_dialog_open: false,
            save_path: None,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn join_mem(
        t: MemTransport,
        addr: &str,
        cfg: Config,
        world_placeholder: World,
        data: Option<Arc<GameData>>,
    ) -> Result<Self, NetError> {
        let s = Session::join(t, cfg, addr, world_placeholder, data)?;
        Ok(MpLobbyState {
            backend: NetBackend::Mem(s),
            approvals: BTreeMap::new(),
            connect_addr: addr.to_string(),
            status_message: Some(format!("Connecting to {addr}...")),
            chat_log: Vec::new(),
            chat_input: String::new(),
            scenario_name: String::new(),
            setup: crate::setup::Setup::preset(crate::setup::Level::Medium),
            character_edit: None,
            connect_dialog_open: false,
            save_path: None,
        })
    }

    /// Toggles the local player's approval.
    pub fn toggle_my_approval(&mut self) {
        let my_id = self.backend.my_id();
        let cur = self.approvals.get(&my_id).copied().unwrap_or(false);
        let next = !cur;
        self.approvals.insert(my_id, next);
        self.backend.send_chat(&format!("[MP_APPROVE:{next}]"));
    }

    /// Resets all players' approval bits to false (upon setting change by host).
    pub fn reset_all_approvals(&mut self) {
        for v in self.approvals.values_mut() {
            *v = false;
        }
        self.broadcast_setup();
    }

    /// Broadcasts current scenario name and setup options.
    pub fn broadcast_setup(&mut self) {
        if !self.backend.is_host() {
            return;
        }
        let s = format!(
            "[MP_SETUP:{}:{}:{:?}:{:?}:{}:{}:{}:{}]",
            self.scenario_name,
            self.setup.level as u8,
            self.setup.financial,
            self.setup.industrial,
            self.setup.sandbox,
            self.setup.human_revenue,
            self.setup.computer_revenue,
            self.setup.extra_computers,
        );
        self.backend.send_chat(&s);
    }

    /// True if all connected human players have approved.
    pub fn all_approved(&self) -> bool {
        let players = self.backend.players();
        if players.is_empty() {
            return false;
        }
        players
            .iter()
            .all(|p| self.approvals.get(&p.id).copied().unwrap_or(false))
    }

    /// Steps the lobby session and processes events.
    pub fn step(&mut self, now_ms: u64) -> Vec<Event> {
        let events = self.backend.step(now_ms);
        for ev in &events {
            match ev {
                Event::Connected { your_id, .. } => {
                    self.approvals.insert(*your_id, false);
                    self.status_message = None;
                }
                Event::PlayerJoined { id, name } => {
                    self.approvals.insert(*id, false);
                    self.chat_log
                        .push(("***".into(), format!("Player {name} entered the lobby")));
                    if self.backend.is_host() {
                        self.broadcast_setup();
                    }
                }
                Event::PlayerLeft { id, name, .. } => {
                    self.approvals.remove(id);
                    self.chat_log
                        .push(("***".into(), format!("Player {name} left the lobby")));
                }
                Event::Chat { from, name, text } => {
                    if let Some(rest) = text.strip_prefix("[MP_APPROVE:")
                        && let Some(val_str) = rest.strip_suffix(']')
                    {
                        if let Ok(b) = val_str.parse::<bool>() {
                            self.approvals.insert(*from, b);
                        }
                    } else if let Some(rest) = text.strip_prefix("[MP_SETUP:")
                        && let Some(body) = rest.strip_suffix(']')
                    {
                        let parts: Vec<&str> = body.split(':').collect();
                        if parts.len() >= 8 {
                            self.scenario_name = parts[0].to_string();
                            if let Ok(lvl) = parts[1].parse::<u8>() {
                                self.setup.level = match lvl {
                                    0 => crate::setup::Level::Easy,
                                    1 => crate::setup::Level::Medium,
                                    2 => crate::setup::Level::Hard,
                                    3 => crate::setup::Level::Expert,
                                    _ => crate::setup::Level::Custom,
                                };
                            }
                            self.setup.financial = match parts[2] {
                                "Basic" => crate::setup::Model::Basic,
                                "Advanced" => crate::setup::Model::Advanced,
                                _ => crate::setup::Model::Expert,
                            };
                            self.setup.industrial = match parts[3] {
                                "Basic" => crate::setup::Model::Basic,
                                "Advanced" => crate::setup::Model::Advanced,
                                _ => crate::setup::Model::Expert,
                            };
                            self.setup.sandbox = parts[4] == "true";
                            if let Ok(v) = parts[5].parse::<i32>() {
                                self.setup.human_revenue = v;
                            }
                            if let Ok(v) = parts[6].parse::<i32>() {
                                self.setup.computer_revenue = v;
                            }
                            if let Ok(v) = parts[7].parse::<usize>() {
                                self.setup.extra_computers = v;
                            }
                        }
                        // Any setup change resets local approval.
                        let my_id = self.backend.my_id();
                        self.approvals.insert(my_id, false);
                    } else {
                        self.chat_log.push((name.clone(), text.clone()));
                    }
                }
                Event::Rejected(why) => {
                    self.status_message = Some(format!("Connection rejected: {why}"));
                }
                Event::Closed => {
                    self.status_message = Some("Disconnected from host".into());
                }
                _ => {}
            }
        }
        events
    }
}

/// One line of the in-game chat overlay (lasts 14 seconds).
#[derive(Clone, Debug)]
pub struct InGameChatLine {
    pub sender: String,
    pub text: String,
    pub time_left: f32,
}

/// In-game multiplayer state.
pub struct InGameMp {
    pub backend: NetBackend,
    pub chat_lines: Vec<InGameChatLine>,
    pub chat_open: bool,
    pub chat_input: String,
    pub desync_alert: Option<(i32, f32)>,
    pub player_left_alert: Option<(String, f32)>,
    pub host_migrated_alert: Option<(String, f32)>,
    pub dialog_timer: Option<f32>,
}

impl InGameMp {
    pub fn new(backend: NetBackend) -> Self {
        InGameMp {
            backend,
            chat_lines: Vec::new(),
            chat_open: false,
            chat_input: String::new(),
            desync_alert: None,
            player_left_alert: None,
            host_migrated_alert: None,
            dialog_timer: None,
        }
    }

    /// Adds a line to the chat overlay (max 6 lines, 14 s each).
    pub fn push_chat(&mut self, sender: String, text: String) {
        self.chat_lines.push(InGameChatLine {
            sender,
            text,
            time_left: 14.0,
        });
        if self.chat_lines.len() > 6 {
            self.chat_lines.remove(0);
        }
    }

    /// Advances in-game multiplayer timers and processes events.
    pub fn step(&mut self, now_ms: u64, dt_ms: f32) -> Vec<Event> {
        let dt_secs = dt_ms / 1000.0;
        // Age chat lines.
        for line in &mut self.chat_lines {
            line.time_left -= dt_secs;
        }
        self.chat_lines.retain(|l| l.time_left > 0.0);

        // Age alerts.
        if let Some((_, t)) = &mut self.desync_alert {
            *t -= dt_secs;
            if *t <= 0.0 {
                self.desync_alert = None;
            }
        }
        if let Some((_, t)) = &mut self.player_left_alert {
            *t -= dt_secs;
            if *t <= 0.0 {
                self.player_left_alert = None;
            }
        }
        if let Some((_, t)) = &mut self.host_migrated_alert {
            *t -= dt_secs;
            if *t <= 0.0 {
                self.host_migrated_alert = None;
            }
        }

        let events = self.backend.step(now_ms);
        for ev in &events {
            match ev {
                Event::Chat { name, text, .. } => {
                    self.push_chat(name.clone(), text.clone());
                }
                Event::PlayerLeft { name, .. } => {
                    self.player_left_alert = Some((name.clone(), 8.0));
                }
                Event::HostChanged { host } => {
                    let host_name = self
                        .backend
                        .players()
                        .iter()
                        .find(|p| p.id == *host)
                        .map_or_else(|| format!("Player {host}"), |p| p.name.clone());
                    self.host_migrated_alert = Some((host_name, 8.0));
                }
                Event::Desync { minute, .. } => {
                    self.desync_alert = Some((*minute, 5.0));
                }
                _ => {}
            }
        }
        events
    }
}

// -------------------------------------------------------------------------------------------
// AppState integration
// -------------------------------------------------------------------------------------------

use crate::mp_ui::*;
use crate::state::build_ui::BuildState;
use crate::state::{
    AppState, CenterState, Effect, GameState, MainIds, NewsItem, Screen, format_date, player_cash,
    scene_fingerprint,
};
use rt2_render::live::LiveFrame;
use rt2_render::{Scene, SceneOptions};
use winit::keyboard::KeyCode;

impl AppState {
    /// Handles multiplayer action buttons (15000..15999).
    pub fn mp_activated(&mut self, action: u32) -> bool {
        match action {
            A_MP_HOST => {
                let port = self.config.network.default_port;
                let _ = self.start_mp_host(port);
                self.rebuild_ui();
                true
            }
            A_MP_JOIN_DIALOG => {
                self.open_mp_connect_dialog();
                self.rebuild_ui();
                true
            }
            A_MP_JOIN_CONNECT => {
                let addr = self
                    .mp_lobby
                    .as_ref()
                    .map_or_else(|| "127.0.0.1:7777".to_string(), |l| l.connect_addr.clone());
                let _ = self.start_mp_join(&addr);
                self.rebuild_ui();
                true
            }
            A_MP_JOIN_CANCEL => {
                if let Some(lobby) = &mut self.mp_lobby {
                    lobby.connect_dialog_open = false;
                }
                self.rebuild_ui();
                true
            }
            A_MP_CHAR_DIALOG => {
                self.open_mp_character_edit();
                self.rebuild_ui();
                true
            }
            A_MP_CHAR_OK => {
                if let Some(lobby) = &mut self.mp_lobby
                    && let Some(edit) = lobby.character_edit.take()
                {
                    self.config.network.player_name = edit.name;
                    self.config.network.player_description = edit.description;
                    self.config.network.player_portrait = edit.portrait;
                    self.effects
                        .push(Effect::SaveConfig(Box::new(self.config.clone())));
                }
                self.rebuild_ui();
                true
            }
            A_MP_CHAR_CANCEL => {
                if let Some(lobby) = &mut self.mp_lobby {
                    lobby.character_edit = None;
                }
                self.rebuild_ui();
                true
            }
            A_MP_CHAR_PREV => {
                if let Some(lobby) = &mut self.mp_lobby
                    && let Some(edit) = &mut lobby.character_edit
                {
                    edit.portrait = (edit.portrait + 40) % 41;
                }
                self.rebuild_ui();
                true
            }
            A_MP_CHAR_NEXT => {
                if let Some(lobby) = &mut self.mp_lobby
                    && let Some(edit) = &mut lobby.character_edit
                {
                    edit.portrait = (edit.portrait + 1) % 41;
                }
                self.rebuild_ui();
                true
            }
            A_MP_APPROVE => {
                if let Some(lobby) = &mut self.mp_lobby {
                    lobby.toggle_my_approval();
                }
                self.rebuild_ui();
                true
            }
            A_MP_START => {
                if let Some(lobby) = &mut self.mp_lobby
                    && lobby.backend.is_host()
                    && lobby.all_approved()
                {
                    let _ = lobby.backend.start();
                }
                self.rebuild_ui();
                true
            }
            A_MP_LEAVE => {
                if let Some(mut lobby) = self.mp_lobby.take() {
                    lobby.backend.leave();
                }
                self.open_page(crate::state::menus::Page::Main);
                true
            }
            A_MP_CHAT_SEND => {
                if let Some(lobby) = &mut self.mp_lobby {
                    let text = std::mem::take(&mut lobby.chat_input);
                    if !text.trim().is_empty() {
                        lobby.backend.send_chat(&text);
                        let name = self.config.network.player_name.clone();
                        lobby.chat_log.push((name, text));
                    }
                }
                self.rebuild_ui();
                true
            }
            A_MP_MAP_PICK => {
                // Host cycle through scenarios
                if let Some(lobby) = &mut self.mp_lobby
                    && lobby.backend.is_host()
                {
                    let next = match lobby.scenario_name.as_str() {
                        "USA" => "Europe",
                        "Europe" => "Britain",
                        "Britain" => "Germany",
                        _ => "USA",
                    };
                    lobby.scenario_name = next.to_string();
                    lobby.reset_all_approvals();
                }
                self.rebuild_ui();
                true
            }
            a if (A_MP_DIFF..A_MP_DIFF + 5).contains(&a) => {
                if let Some(lobby) = &mut self.mp_lobby
                    && lobby.backend.is_host()
                {
                    let idx = (a - A_MP_DIFF) as usize;
                    let lvl = crate::setup::Level::ALL[idx.min(4)];
                    lobby.setup.set_level(lvl);
                    lobby.reset_all_approvals();
                }
                self.rebuild_ui();
                true
            }
            A_MP_COMP_UP => {
                if let Some(lobby) = &mut self.mp_lobby
                    && lobby.backend.is_host()
                {
                    lobby.setup.extra_computers = (lobby.setup.extra_computers + 1).min(8);
                    lobby.reset_all_approvals();
                }
                self.rebuild_ui();
                true
            }
            A_MP_COMP_DOWN => {
                if let Some(lobby) = &mut self.mp_lobby
                    && lobby.backend.is_host()
                {
                    lobby.setup.extra_computers = lobby.setup.extra_computers.saturating_sub(1);
                    lobby.reset_all_approvals();
                }
                self.rebuild_ui();
                true
            }
            A_MP_INGAME_CHAT_SEND => {
                if let Screen::Game(g) = &mut self.screen
                    && let Some(mp) = &mut g.mp
                {
                    let text = std::mem::take(&mut mp.chat_input);
                    if !text.trim().is_empty() {
                        mp.backend.send_chat(&text);
                        let my_name = self.config.network.player_name.clone();
                        mp.push_chat(my_name, text);
                    }
                    mp.chat_open = false;
                }
                self.rebuild_ui();
                true
            }
            A_MP_INGAME_CHAT_TOGGLE => {
                if let Screen::Game(g) = &mut self.screen
                    && let Some(mp) = &mut g.mp
                {
                    mp.chat_open = !mp.chat_open;
                    if mp.chat_open {
                        mp.chat_input.clear();
                    }
                }
                self.rebuild_ui();
                true
            }
            _ => false,
        }
    }

    /// Handles keyboard events for multiplayer text input and hotkeys.
    pub fn mp_key(&mut self, code: KeyCode, _repeat: bool, text: Option<&str>) -> bool {
        // 1. In-Game multiplayer chat
        if let Screen::Game(g) = &mut self.screen
            && let Some(mp) = &mut g.mp
        {
            if mp.chat_open {
                match code {
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        let text = std::mem::take(&mut mp.chat_input);
                        if !text.trim().is_empty() {
                            mp.backend.send_chat(&text);
                            let my_name = self.config.network.player_name.clone();
                            mp.push_chat(my_name, text);
                        }
                        mp.chat_open = false;
                        self.rebuild_ui();
                        return true;
                    }
                    KeyCode::Escape => {
                        mp.chat_open = false;
                        mp.chat_input.clear();
                        self.rebuild_ui();
                        return true;
                    }
                    KeyCode::Backspace => {
                        mp.chat_input.pop();
                        self.rebuild_ui();
                        return true;
                    }
                    _ => {
                        if let Some(t) = text {
                            for c in t.chars() {
                                if !c.is_control() && mp.chat_input.len() < 900 {
                                    mp.chat_input.push(c);
                                }
                            }
                            self.rebuild_ui();
                            return true;
                        }
                    }
                }
            } else {
                // Open chat with Enter, KeyT, or Tab
                if matches!(code, KeyCode::Enter | KeyCode::KeyT | KeyCode::Tab) {
                    mp.chat_open = true;
                    mp.chat_input.clear();
                    self.rebuild_ui();
                    return true;
                }
            }
        }

        // 2. Lobby & Menu inputs
        let Some(lobby) = &mut self.mp_lobby else {
            return false;
        };

        if lobby.connect_dialog_open {
            match code {
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    let addr = lobby.connect_addr.clone();
                    let _ = self.start_mp_join(&addr);
                    self.rebuild_ui();
                    return true;
                }
                KeyCode::Escape => {
                    lobby.connect_dialog_open = false;
                    self.rebuild_ui();
                    return true;
                }
                KeyCode::Backspace => {
                    lobby.connect_addr.pop();
                    self.rebuild_ui();
                    return true;
                }
                _ => {
                    if let Some(t) = text {
                        for c in t.chars() {
                            if !c.is_control() && lobby.connect_addr.len() < 40 {
                                lobby.connect_addr.push(c);
                            }
                        }
                        self.rebuild_ui();
                        return true;
                    }
                }
            }
        }

        if let Some(edit) = &mut lobby.character_edit {
            match code {
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    return self.mp_activated(A_MP_CHAR_OK);
                }
                KeyCode::Escape => {
                    return self.mp_activated(A_MP_CHAR_CANCEL);
                }
                KeyCode::Tab => {
                    edit.focused_field = (edit.focused_field + 1) % 2;
                    self.rebuild_ui();
                    return true;
                }
                KeyCode::ArrowLeft => {
                    edit.portrait = (edit.portrait + 40) % 41;
                    self.rebuild_ui();
                    return true;
                }
                KeyCode::ArrowRight => {
                    edit.portrait = (edit.portrait + 1) % 41;
                    self.rebuild_ui();
                    return true;
                }
                KeyCode::Backspace => {
                    if edit.focused_field == 0 {
                        edit.name.pop();
                    } else {
                        edit.description.pop();
                    }
                    self.rebuild_ui();
                    return true;
                }
                _ => {
                    if let Some(t) = text {
                        for c in t.chars() {
                            if !c.is_control() {
                                if edit.focused_field == 0 && edit.name.len() < 30 {
                                    edit.name.push(c);
                                } else if edit.focused_field == 1 && edit.description.len() < 250 {
                                    edit.description.push(c);
                                }
                            }
                        }
                        self.rebuild_ui();
                        return true;
                    }
                }
            }
        }

        // Typing in lobby chat box
        if !lobby.connect_dialog_open && lobby.character_edit.is_none() {
            match code {
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    return self.mp_activated(A_MP_CHAT_SEND);
                }
                KeyCode::Backspace => {
                    lobby.chat_input.pop();
                    self.rebuild_ui();
                    return true;
                }
                _ => {
                    if let Some(t) = text {
                        for c in t.chars() {
                            if !c.is_control() && lobby.chat_input.len() < 200 {
                                lobby.chat_input.push(c);
                            }
                        }
                        self.rebuild_ui();
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Ticks the multiplayer lobby while in the menu.
    pub fn mp_lobby_update(&mut self) {
        let now_ms = self.clock_ms;
        let Some(lobby) = &mut self.mp_lobby else {
            return;
        };
        let _ = lobby.step(now_ms);
        if lobby.backend.state() == NetSessionState::Playing {
            self.launch_mp_game();
        }
    }

    /// Transitions from the lobby to the in-game screen.
    pub fn launch_mp_game(&mut self) {
        let Some(lobby) = self.mp_lobby.take() else {
            return;
        };
        let scenario_name = if lobby.scenario_name.is_empty() {
            "Multiplayer".to_string()
        } else {
            lobby.scenario_name
        };
        let setup = lobby.setup;
        self.open_multiplayer_game(lobby.backend, scenario_name, setup);
    }

    /// Opens the in-game screen driven by `backend`.
    pub fn open_multiplayer_game(
        &mut self,
        backend: NetBackend,
        scenario_name: String,
        setup: crate::setup::Setup,
    ) {
        let world = backend.world().clone();
        let speeds = self.speed_table();
        let (save, map) = if let Some(s) = world.save_ref() {
            (s.clone(), s.map.clone())
        } else if let Ok(s) = self.blank_template() {
            let m = s.map.clone();
            (s, m)
        } else {
            return;
        };
        let scene = Scene::build_with(
            self.assets.clone(),
            &map,
            Some(&save),
            SceneOptions { sprites: true },
            self.vehicle_names.clone(),
        );
        let live_frame = LiveFrame::from_save(&save);
        let scene_fp = scene_fingerprint(&save);
        let cash = player_cash(&save);
        self.pending_scene = Some(Arc::new(scene));
        self.pending_live = Some((live_frame, true));
        self.live_alpha = 1.0;
        let camera = self.camera_for_view(&save.view(), map.width, map.height);
        let last_year = world.date().year;
        let world_month = world.date().month;

        let welcome = NewsItem {
            date: format_date(world.date(), &self.texts),
            detail: format!("Multiplayer game started: {scenario_name}"),
            text: format!("Multiplayer game started: {scenario_name}"),
            category: crate::notices::Category::Scenario,
            pos: None,
            shown: true,
        };

        let in_game_mp = Box::new(InGameMp::new(backend));

        self.screen = Screen::Game(Box::new(GameState {
            world,
            name: scenario_name,
            map_size: (map.width, map.height),
            camera,
            speeds,
            accum_ms: 0.0,
            cash,
            cash_timer: 500.0,
            toast: None,
            sound_subtitle: None,
            hud: MainIds::default(),
            scenario: None,
            setup,
            dialogs: Vec::new(),
            center: CenterState::default(),
            news: vec![welcome],
            radar: Some(crate::radar::Radar::new(&map)),
            radar_view: None,
            last_year,
            center_day: -1,
            over_shown: false,
            info: None,
            scene_fp,
            build: BuildState::default(),
            last_autosave: (last_year, world_month),
            reports: Default::default(),
            editor: None,
            campaign: None,
            mp: Some(in_game_mp),
        }));
        self.rebuild_ui();
    }

    /// Opens the character profile editor dialog.
    pub fn open_mp_character_edit(&mut self) {
        let name = self.config.network.player_name.clone();
        let desc = self.config.network.player_description.clone();
        let port = self.config.network.player_portrait;
        let edit = CharacterEditState::new(&name, &desc, port);
        if let Some(lobby) = &mut self.mp_lobby {
            lobby.character_edit = Some(edit);
        } else {
            // Create dummy lobby container if in K1 menu
            let cfg = Config::new(&name, self.config.network.identity, self.speed_table());
            let placeholder = World::new(
                rt2_sim::calendar::Date::new(1900, 1, 1, 0, 0),
                self.seed,
                self.seed,
            );
            let hub = rt2_net::mem::MemHub::new(1);
            let t = hub.node("dummy");
            let mut l = MpLobbyState::host_mem(
                t,
                cfg,
                placeholder,
                None,
                "Lobby".into(),
                crate::setup::Setup::default(),
            );
            l.character_edit = Some(edit);
            self.mp_lobby = Some(Box::new(l));
        }
        self.rebuild_ui();
    }

    /// Opens the connect dialog.
    pub fn open_mp_connect_dialog(&mut self) {
        let addr = format!("127.0.0.1:{}", self.config.network.default_port);
        if let Some(lobby) = &mut self.mp_lobby {
            lobby.connect_dialog_open = true;
            lobby.connect_addr = addr;
        } else {
            let name = self.config.network.player_name.clone();
            let cfg = Config::new(&name, self.config.network.identity, self.speed_table());
            let placeholder = World::new(
                rt2_sim::calendar::Date::new(1900, 1, 1, 0, 0),
                self.seed,
                self.seed,
            );
            let hub = rt2_net::mem::MemHub::new(1);
            let t = hub.node("dummy");
            let mut l = MpLobbyState::host_mem(
                t,
                cfg,
                placeholder,
                None,
                "Lobby".into(),
                crate::setup::Setup::default(),
            );
            l.connect_dialog_open = true;
            l.connect_addr = addr;
            self.mp_lobby = Some(Box::new(l));
        }
        self.rebuild_ui();
    }

    /// Loads a scenario or blank template for a hosted multiplayer game.
    pub fn initial_mp_world(
        &mut self,
        scenario_name: &str,
        setup: &crate::setup::Setup,
    ) -> (World, String) {
        let maps = crate::catalog::list_maps(&self.install);
        let entry = maps
            .iter()
            .find(|e| {
                e.title.eq_ignore_ascii_case(scenario_name)
                    || e.path
                        .file_stem()
                        .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(scenario_name))
            })
            .or_else(|| maps.first());

        if let Some(entry) = entry
            && let Ok(raw) = std::fs::read(&entry.path)
            && let Ok((_map, Some(save))) = crate::catalog::load_any(&raw)
            && let Ok(data) = self.game_data().cloned()
        {
            let seed = self.seed;
            let mut o = setup.new_game_options(&save, &entry.title, None, seed);
            o.lng = self.texts.english().cloned().map(Arc::new);
            let world = World::new_game(&save, &o, &data, seed);
            return (world, entry.title.clone());
        }

        let world = World::new(
            rt2_sim::calendar::Date::new(1900, 1, 1, 0, 0),
            self.seed,
            self.seed,
        );
        (world, scenario_name.to_string())
    }

    /// Starts hosting on TCP `port`.
    pub fn start_mp_host(&mut self, port: u16) -> Result<(), String> {
        let _ = self.game_data();
        let speeds = self.speed_table();
        let name = self.config.network.player_name.clone();
        let cfg = Config::new(&name, self.config.network.identity, speeds);
        let data = self.data.clone().map(Arc::new);
        let setup = crate::setup::Setup::preset(crate::setup::Level::Medium);
        let (world, scenario_name) = self.initial_mp_world("USA", &setup);
        let lobby = MpLobbyState::host_tcp(port, cfg, world, data, scenario_name, setup)
            .map_err(|e| e.to_string())?;
        self.mp_lobby = Some(Box::new(lobby));
        self.open_page(crate::state::menus::Page::MpLobby);
        Ok(())
    }

    /// Connects to a host on TCP `addr`.
    pub fn start_mp_join(&mut self, addr: &str) -> Result<(), String> {
        let _ = self.game_data();
        let speeds = self.speed_table();
        let name = self.config.network.player_name.clone();
        let cfg = Config::new(&name, self.config.network.identity, speeds);
        let data = self.data.clone().map(Arc::new);
        let placeholder = World::new(
            rt2_sim::calendar::Date::new(1900, 1, 1, 0, 0),
            self.seed,
            self.seed,
        );
        let lobby =
            MpLobbyState::join_tcp(addr, cfg, placeholder, data).map_err(|e| e.to_string())?;
        self.mp_lobby = Some(Box::new(lobby));
        self.open_page(crate::state::menus::Page::MpLobby);
        Ok(())
    }

    /// Sets up a mock lobby for screenshots and testing.
    pub fn ensure_mock_mp_lobby(&mut self) {
        let _ = self.game_data();
        let speeds = self.speed_table();
        let name = self.config.network.player_name.clone();
        let cfg = Config::new(&name, self.config.network.identity, speeds);
        let placeholder = World::new(
            rt2_sim::calendar::Date::new(1900, 1, 1, 0, 0),
            self.seed,
            self.seed,
        );
        let hub = rt2_net::mem::MemHub::new(1);
        let t = hub.node("host");
        let mut l = MpLobbyState::host_mem(
            t,
            cfg,
            placeholder,
            None,
            "USA".into(),
            crate::setup::Setup::preset(crate::setup::Level::Medium),
        );
        l.approvals.insert(1, true);
        l.chat_log
            .push(("Player 1".into(), "Ready to start!".into()));
        self.mp_lobby = Some(Box::new(l));
    }
}
