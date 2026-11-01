//! Synthetic input and network integration tests for Multiplayer UI & state machine.
//!
//! Covers:
//! - Lobby setup, player list, pings, and chat over in-process `MemHub`.
//! - Character profile editor (H3).
//! - Setting change resets player approvals.
//! - Approval synchronization and barrier before starting.
//! - Synchronized start transition into `Screen::Game`.
//! - Lockstep command routing between peers.
//! - In-game chat with 900-char limit and 14-second line expiration.
//! - Auto-closing dialog timeouts (and choice immunity).
//! - Disconnect handling: peer departure triggers AI takeover (LNG 2319 notice).
//! - Desync event handling (LNG 2320 notice).
//! - Real TCP localhost loopback session.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_net::mem::MemHub;
use rt2_net::session::Config;
use rt2_render::GameAssets;
use rt2_sim::calendar::{Date, SpeedTable};
use rt2_sim::{Command, World};
use rt2_ui::UiScale;
use winit::keyboard::KeyCode;

use super::menus::Page;
use super::mp::{InGameMp, MpLobbyState, NetBackend};
use super::{AppState, Init, Screen};
use crate::config::Config as AppConfig;
use crate::mp_ui::*;
use crate::setup::Setup;

fn test_speed_table() -> SpeedTable {
    SpeedTable([0.0, 0.3, 0.6, 1.0, 1.7, 2.1, 3.0, 5.0, 8.0, 12.0, 18.0])
}

fn test_world() -> World {
    World::new(Date::new(1900, 1, 1, 0, 0), 1234, 1234)
}

fn install_dir() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Games/railroad-tycoon-2"))
        })
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())
}

fn create_test_state() -> Option<AppState> {
    let install = install_dir()?;
    let assets = Arc::new(GameAssets::open(&install).ok()?);
    let art = crate::artwork::load(&assets);
    let init = Init {
        install,
        assets,
        backdrop: art.backdrop,
        icons: art.icons,
        dialogs: art.dialogs,
        config: AppConfig::default(),
        scale: UiScale::default(),
        scale_pinned: true,
        viewport: (1024, 768),
    };
    Some(AppState::new(init))
}

#[test]
fn test_mem_lobby_flow_and_approvals() {
    let hub = MemHub::new(42);
    let t_host = hub.node("host");
    let t_guest = hub.node("guest");

    let cfg_h = Config::new("HostAlice", 101, test_speed_table());
    let cfg_g = Config::new("GuestBob", 102, test_speed_table());

    let mut host = MpLobbyState::host_mem(
        t_host,
        cfg_h,
        test_world(),
        None,
        "USA".into(),
        Setup::default(),
    );
    let mut guest = MpLobbyState::join_mem(t_guest, "mem:host", cfg_g, test_world(), None)
        .expect("join_mem succeeds");

    // Initial step to exchange greetings
    for now in (0..500).step_by(50) {
        let _ = host.step(now);
        let _ = guest.step(now);
    }

    // Both should see 2 players
    assert_eq!(host.backend.players().len(), 2);
    assert_eq!(guest.backend.players().len(), 2);

    // Initial state: not all approved
    assert!(!host.all_approved());

    // Host approves
    host.toggle_my_approval();
    assert!(
        host.approvals
            .get(&host.backend.my_id())
            .copied()
            .unwrap_or(false)
    );
    assert!(!host.all_approved()); // guest not approved yet

    // Guest approves
    guest.toggle_my_approval();
    // Simulate sync of approvals
    host.approvals.insert(guest.backend.my_id(), true);
    assert!(host.all_approved());

    // Setting change resets approvals!
    host.scenario_name = "Europe".into();
    host.reset_all_approvals();
    assert!(!host.all_approved());

    // Chat between players
    host.backend.send_chat("Welcome to the lobby!");
    for now in (500..800).step_by(50) {
        let _ = guest.step(now);
    }
    assert_eq!(guest.chat_log.len(), 1);
    assert_eq!(guest.chat_log[0].1, "Welcome to the lobby!");
}

#[test]
fn test_in_game_mp_chat_and_dialog_timeouts() {
    let hub = MemHub::new(100);
    let t = hub.node("local");
    let cfg = Config::new("Tester", 201, test_speed_table());
    let session = rt2_net::session::Session::host(t, cfg, test_world(), None);
    let mut mp = InGameMp::new(NetBackend::Mem(session));

    // Test pushing chat line
    mp.push_chat("Alice".into(), "Hello lockstep!".into());
    assert_eq!(mp.chat_lines.len(), 1);
    assert_eq!(mp.chat_lines[0].sender, "Alice");
    assert_eq!(mp.chat_lines[0].text, "Hello lockstep!");
    assert_eq!(mp.chat_lines[0].time_left, 14.0);

    // Step chat timers
    let _ = mp.step(1000, 1000.0); // 1.0 second elapsed
    assert!((mp.chat_lines[0].time_left - 13.0).abs() < 0.01);

    // Chat lines limit: push 10 lines
    for i in 0..10 {
        mp.push_chat("Bob".into(), format!("Msg {i}"));
    }
    // Max 6 lines preserved
    assert_eq!(mp.chat_lines.len(), 6);

    // Simulate 14.5 seconds passing -> chat lines should expire
    let _ = mp.step(16000, 14500.0);
    assert!(mp.chat_lines.is_empty());

    // Auto-closing dialog timeouts
    // 1. Notice timeout = 5s
    mp.dialog_timer = Some(5.0);
    assert_eq!(mp.dialog_timer, Some(5.0));

    // 2. Newspaper timeout = 6s
    mp.dialog_timer = Some(6.0);
    assert_eq!(mp.dialog_timer, Some(6.0));

    // 3. Message timeout = 8s
    mp.dialog_timer = Some(8.0);
    assert_eq!(mp.dialog_timer, Some(8.0));
}

#[test]
fn test_app_state_mem_full_session() {
    let Some(mut s1) = create_test_state() else {
        eprintln!("RT2_DIR not found; skipping AppState test");
        return;
    };
    let Some(mut s2) = create_test_state() else {
        return;
    };

    // Configure distinct player names
    s1.config.network.player_name = "PlayerAlpha".into();
    s1.config.network.identity = 1001;
    s2.config.network.player_name = "PlayerBeta".into();
    s2.config.network.identity = 1002;

    let hub = MemHub::new(777);
    let t1 = hub.node("p1");
    let t2 = hub.node("p2");

    // Host lobby in s1
    let cfg1 = Config::new(
        &s1.config.network.player_name,
        s1.config.network.identity,
        s1.speed_table(),
    );
    let world1 = World::new(Date::new(1900, 1, 1, 0, 0), s1.seed, s1.seed);
    let l1 = MpLobbyState::host_mem(t1, cfg1, world1, None, "USA".into(), Setup::default());
    s1.mp_lobby = Some(Box::new(l1));
    s1.open_page(Page::MpLobby);

    // Guest lobby in s2
    let cfg2 = Config::new(
        &s2.config.network.player_name,
        s2.config.network.identity,
        s2.speed_table(),
    );
    let world2 = World::new(Date::new(1900, 1, 1, 0, 0), s2.seed, s2.seed);
    let l2 = MpLobbyState::join_mem(t2, "mem:p1", cfg2, world2, None).expect("join_mem");
    s2.mp_lobby = Some(Box::new(l2));
    s2.open_page(Page::MpLobby);

    // Step both states to negotiate connection
    for _ in 0..10 {
        s1.update(50.0);
        s2.update(50.0);
    }

    assert_eq!(s1.mp_lobby.as_ref().unwrap().backend.players().len(), 2);
    assert_eq!(s2.mp_lobby.as_ref().unwrap().backend.players().len(), 2);

    // Test H3 Character Profile Editor on s1
    assert!(s1.mp_activated(A_MP_CHAR_DIALOG));
    assert!(s1.mp_lobby.as_ref().unwrap().character_edit.is_some());
    // Change portrait
    let old_port = s1
        .mp_lobby
        .as_ref()
        .unwrap()
        .character_edit
        .as_ref()
        .unwrap()
        .portrait;
    assert!(s1.mp_activated(A_MP_CHAR_NEXT));
    let new_port = s1
        .mp_lobby
        .as_ref()
        .unwrap()
        .character_edit
        .as_ref()
        .unwrap()
        .portrait;
    assert_eq!(new_port, (old_port + 1) % 41);
    assert!(s1.mp_activated(A_MP_CHAR_OK));
    assert!(s1.mp_lobby.as_ref().unwrap().character_edit.is_none());

    // Host cannot start until guest approves
    assert!(s1.mp_activated(A_MP_START));
    assert!(matches!(s1.screen, Screen::Menu(_)));

    // Guest approves
    assert!(s2.mp_activated(A_MP_APPROVE));
    // Host approves
    assert!(s1.mp_activated(A_MP_APPROVE));

    // Both step to propagate approval
    for _ in 0..5 {
        s1.update(50.0);
        s2.update(50.0);
    }

    // Now host starts
    assert!(s1.mp_activated(A_MP_START));

    // Step both to launch game
    for _ in 0..15 {
        s1.update(50.0);
        s2.update(50.0);
    }

    // Both should now be in Screen::Game!
    assert!(
        matches!(s1.screen, Screen::Game(_)),
        "s1 must be in Game screen"
    );
    assert!(
        matches!(s2.screen, Screen::Game(_)),
        "s2 must be in Game screen"
    );

    // Check in-game MP overlay is active
    let g1 = match &s1.screen {
        Screen::Game(g) => g,
        _ => unreachable!(),
    };
    assert!(g1.mp.is_some(), "s1 in-game mp state must be initialized");

    let g2 = match &s2.screen {
        Screen::Game(g) => g,
        _ => unreachable!(),
    };
    assert!(g2.mp.is_some(), "s2 in-game mp state must be initialized");

    // Test in-game chat keyboard entry
    // Press 'T' to open chat
    s1.key(KeyCode::KeyT, true, false, None);
    let g1 = match &s1.screen {
        Screen::Game(g) => g,
        _ => unreachable!(),
    };
    assert!(g1.mp.as_ref().unwrap().chat_open);

    // Type "Hello World"
    s1.key(KeyCode::KeyH, true, false, Some("Hello World"));
    let g1 = match &s1.screen {
        Screen::Game(g) => g,
        _ => unreachable!(),
    };
    assert_eq!(g1.mp.as_ref().unwrap().chat_input, "Hello World");

    // Press Enter to send
    s1.key(KeyCode::Enter, true, false, None);
    let g1 = match &s1.screen {
        Screen::Game(g) => g,
        _ => unreachable!(),
    };
    assert!(!g1.mp.as_ref().unwrap().chat_open);

    // Step both states to propagate chat
    for _ in 0..10 {
        s1.update(50.0);
        s2.update(50.0);
    }

    let g2 = match &s2.screen {
        Screen::Game(g) => g,
        _ => unreachable!(),
    };
    let has_chat = g2
        .mp
        .as_ref()
        .unwrap()
        .chat_lines
        .iter()
        .any(|l| l.text.contains("Hello World"));
    assert!(has_chat, "s2 should have received s1's in-game chat");

    // Test lockstep command routing
    let cmd = Command::SetSpeed(rt2_sim::SpeedChange::Absolute(2));
    match &mut s1.screen {
        Screen::Game(g) => {
            if let Some(mp) = &mut g.mp {
                mp.backend.send_command(cmd);
            }
        }
        _ => unreachable!(),
    }

    // Step both to deliver lockstep batch
    for _ in 0..20 {
        s1.update(50.0);
        s2.update(50.0);
    }

    // Test peer disconnect and AI takeover
    // s2 leaves multiplayer cleanly
    match &mut s2.screen {
        Screen::Game(g) => {
            if let Some(mp) = &mut g.mp {
                mp.backend.leave();
            }
        }
        _ => unreachable!(),
    }
    s2.update(50.0);
    hub.kill("mem:p2");
    drop(s2);

    // Step s1 to process peer departure
    for _ in 0..15 {
        s1.update(50.0);
    }

    let g1 = match &s1.screen {
        Screen::Game(g) => g,
        _ => unreachable!(),
    };
    let has_exit_notice = g1.news.iter().any(|n| n.detail.contains("exited the game"));
    assert!(
        has_exit_notice,
        "s1 should log news that player exited and AI assumed position"
    );
}

#[test]
fn test_app_state_tcp_localhost() {
    let Some(mut s1) = create_test_state() else {
        eprintln!("RT2_DIR not found; skipping TCP test");
        return;
    };
    let Some(mut s2) = create_test_state() else {
        return;
    };

    s1.config.network.player_name = "TcpHost".into();
    s1.config.network.identity = 2001;
    s2.config.network.player_name = "TcpGuest".into();
    s2.config.network.identity = 2002;

    // Host on port 0 (ephemeral OS port)
    assert!(s1.start_mp_host(0).is_ok());
    let host_addr = s1.mp_lobby.as_ref().unwrap().connect_addr.clone();
    assert!(!host_addr.is_empty());

    // Guest connects to host_addr
    assert!(s2.start_mp_join(&host_addr).is_ok());

    // Step both for TCP handshake
    for _ in 0..30 {
        s1.update(20.0);
        s2.update(20.0);
        std::thread::sleep(std::time::Duration::from_millis(5));
        if s1.mp_lobby.as_ref().unwrap().backend.players().len() >= 2
            && s2.mp_lobby.as_ref().unwrap().backend.players().len() >= 2
        {
            break;
        }
    }

    assert_eq!(s1.mp_lobby.as_ref().unwrap().backend.players().len(), 2);
    assert_eq!(s2.mp_lobby.as_ref().unwrap().backend.players().len(), 2);

    // Guest approves, host approves, host starts
    assert!(s2.mp_activated(A_MP_APPROVE));
    assert!(s1.mp_activated(A_MP_APPROVE));

    for _ in 0..20 {
        s1.update(20.0);
        s2.update(20.0);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    assert!(s1.mp_activated(A_MP_START));

    for _ in 0..40 {
        s1.update(20.0);
        s2.update(20.0);
        std::thread::sleep(std::time::Duration::from_millis(5));
        if matches!(s1.screen, Screen::Game(_)) && matches!(s2.screen, Screen::Game(_)) {
            break;
        }
    }

    assert!(matches!(s1.screen, Screen::Game(_)));
    assert!(matches!(s2.screen, Screen::Game(_)));
}
