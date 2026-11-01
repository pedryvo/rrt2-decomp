//! Multiplayer UI: K1 menu, K2 connection dialog, K3 game preparation lobby,
//! H3 character profile editor, and in-game chat and player indicators.
//!
//! Follows the specifications in `docs/spec/multiplayer.md` and `docs/reference/screens.md`.

use rt2_ui::{
    Align, Anchor, Color, Fill, Fit, FontStyle, Kind, Layout, NodeId, Stack, Ui, game_font,
};

use crate::state::AppState;
use crate::state::mp::{CharacterEditState, MpLobbyState};
use crate::text::id as lng;

// Action IDs in the reserved 15000..15999 range.
#[allow(dead_code)]
pub const A_MP_HOST: u32 = 15001;
pub const A_MP_JOIN_DIALOG: u32 = 15002;
pub const A_MP_JOIN_CONNECT: u32 = 15003;
pub const A_MP_JOIN_CANCEL: u32 = 15004;
pub const A_MP_CHAR_DIALOG: u32 = 15005;
pub const A_MP_CHAR_OK: u32 = 15006;
pub const A_MP_CHAR_CANCEL: u32 = 15007;
pub const A_MP_CHAR_PREV: u32 = 15008;
pub const A_MP_CHAR_NEXT: u32 = 15009;
#[allow(dead_code)]
pub const A_MP_CHAR_FIELD_NAME: u32 = 15010;
#[allow(dead_code)]
pub const A_MP_CHAR_FIELD_DESC: u32 = 15011;
pub const A_MP_APPROVE: u32 = 15012;
pub const A_MP_START: u32 = 15013;
pub const A_MP_LEAVE: u32 = 15014;
pub const A_MP_CHAT_SEND: u32 = 15015;
pub const A_MP_MAP_PICK: u32 = 15016;
pub const A_MP_DIFF: u32 = 15020; // 15020..15025
#[allow(dead_code)]
pub const A_MP_FIN: u32 = 15030; // 15030..15033
#[allow(dead_code)]
pub const A_MP_IND: u32 = 15040; // 15040..15043
pub const A_MP_COMP_UP: u32 = 15050;
pub const A_MP_COMP_DOWN: u32 = 15051;
pub const A_MP_INGAME_CHAT_SEND: u32 = 15060;
pub const A_MP_INGAME_CHAT_TOGGLE: u32 = 15061;

const CREAM: Color = Color::rgb(0xf2, 0xe9, 0xd0);
const GOLD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd0);
const DIM: Color = Color(0, 0, 0, 180);
const GREEN_LGT: Color = Color::rgb(0x40, 0xe0, 0x40);
const RED_LGT: Color = Color::rgb(0xe0, 0x40, 0x40);

fn mfont(original_style: usize, _size: f32) -> FontStyle {
    game_font(original_style as u8)
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout {
        anchor: Anchor::TopLeft,
        dx: x,
        dy: y,
        w,
        h,
    }
}

fn lfixed(w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h)
}

impl AppState {
    /// Renders the K1 Multiplayer Menu (`setup.win` page 5) with Host, Connect, and Change.
    #[allow(clippy::too_many_arguments)]
    pub fn build_multi_menu(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        logo_bottom: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let t = &self.texts;
        let left_top = logo_bottom.min(ch * 0.46);
        let gap = 10.0 * z;
        let bh = 42.0 * z;
        let bw = 240.0 * z;
        let x = (cw * 0.16 + 32.0 * z).clamp(160.0 * z, 280.0 * z);

        let p = ui.add(
            layer,
            Kind::Panel { fill: Fill::Panel },
            at(x, left_top, bw + 32.0 * z, 5.0 * (bh + gap) + 40.0 * z),
        );

        let title = ui.add(
            p,
            Kind::Label {
                text: t.get(1676, "Multi Player"),
                font: mfont(3, 20.0 * z),
                color: GOLD,
                align: Align::Center,
                wrap: false,
            },
            at(0.0, 14.0 * z, bw + 32.0 * z, 26.0 * z),
        );
        let _ = title;

        let col = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(16.0 * z, 50.0 * z, bw, 4.0 * (bh + gap)),
        );
        ui.set_stack(col, Stack::column(gap, 0.0));

        // 1. Host Game
        let b_host = ui.add(
            col,
            Kind::Button {
                text: t.a("Host Game"),
                font: mfont(2, 14.0 * z),
                action: A_MP_HOST,
                enabled: true,
                hotkey: Some('H'),
            },
            lfixed(bw, bh),
        );
        tips.push((
            b_host,
            t.get(1714, "Host a game that other players can join.")
                .to_string(),
        ));

        // 2. Connect
        let b_join = ui.add(
            col,
            Kind::Button {
                text: t.a("Connect (IP:port)"),
                font: mfont(2, 14.0 * z),
                action: A_MP_JOIN_DIALOG,
                enabled: true,
                hotkey: Some('C'),
            },
            lfixed(bw, bh),
        );
        tips.push((
            b_join,
            t.get(1708, "Join a multi-player game hosted on another computer.")
                .to_string(),
        ));

        // 3. Change Character (H3)
        let b_char = ui.add(
            col,
            Kind::Button {
                text: t.a("Change Character"),
                font: mfont(2, 14.0 * z),
                action: A_MP_CHAR_DIALOG,
                enabled: true,
                hotkey: None,
            },
            lfixed(bw, bh),
        );
        tips.push((
            b_char,
            t.get(2912, "Change your name, description, and/or portrait")
                .to_string(),
        ));

        // 4. Back to Main Menu
        let b_back = ui.add(
            col,
            Kind::Button {
                text: t.a("Back"),
                font: mfont(2, 14.0 * z),
                action: crate::state::A_BACK,
                enabled: true,
                hotkey: Some('B'),
            },
            lfixed(bw, bh),
        );
        tips.push((b_back, t.get(lng::CANCEL, "Cancel").to_string()));
    }

    /// Renders the K2 Connect Dialog (server address input).
    pub fn build_mp_connect_dialog(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        addr: &str,
    ) {
        let t = &self.texts;
        let overlay = ui.add(
            layer,
            Kind::Panel {
                fill: Fill::Solid(DIM),
            },
            Layout::fill(),
        );

        let dw = 400.0 * z;
        let dh = 200.0 * z;
        let dx = (cw - dw) * 0.5;
        let dy = (ch - dh) * 0.5;

        let p = ui.add(
            overlay,
            Kind::Panel { fill: Fill::Panel },
            at(dx, dy, dw, dh),
        );

        ui.add(
            p,
            Kind::Label {
                text: t.a("Connect (IP:port)"),
                font: mfont(3, 18.0 * z),
                color: GOLD,
                align: Align::Center,
                wrap: false,
            },
            at(0.0, 16.0 * z, dw, 24.0 * z),
        );

        ui.add(
            p,
            Kind::Label {
                text: t.a("Server Address:"),
                font: mfont(2, 13.0 * z),
                color: CREAM,
                align: Align::Left,
                wrap: false,
            },
            at(24.0 * z, 52.0 * z, dw - 48.0 * z, 20.0 * z),
        );

        // Address input box
        let inp_bg = ui.add(
            p,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(24.0 * z, 76.0 * z, dw - 48.0 * z, 30.0 * z),
        );
        ui.add(
            inp_bg,
            Kind::Label {
                text: format!("{addr}_"),
                font: mfont(2, 14.0 * z),
                color: GOLD,
                align: Align::Left,
                wrap: false,
            },
            at(8.0 * z, 4.0 * z, dw - 64.0 * z, 22.0 * z),
        );

        // Connect & Cancel buttons
        let btn_row = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(24.0 * z, 130.0 * z, dw - 48.0 * z, 38.0 * z),
        );
        ui.set_stack(btn_row, Stack::row(16.0 * z, 0.0).centered());

        ui.add(
            btn_row,
            Kind::Button {
                text: t.a("Connect"),
                font: mfont(2, 14.0 * z),
                action: A_MP_JOIN_CONNECT,
                enabled: !addr.is_empty(),
                hotkey: Some('C'),
            },
            lfixed(120.0 * z, 36.0 * z),
        );

        ui.add(
            btn_row,
            Kind::Button {
                text: t.a("Back"),
                font: mfont(2, 14.0 * z),
                action: A_MP_JOIN_CANCEL,
                enabled: true,
                hotkey: Some('B'),
            },
            lfixed(120.0 * z, 36.0 * z),
        );
    }

    /// Renders the H3 Character Profile Dialog (name, portrait, description).
    pub fn build_mp_char_dialog(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        edit: &CharacterEditState,
    ) {
        let t = &self.texts;
        let overlay = ui.add(
            layer,
            Kind::Panel {
                fill: Fill::Solid(DIM),
            },
            Layout::fill(),
        );

        let dw = 520.0 * z;
        let dh = 420.0 * z;
        let dx = (cw - dw) * 0.5;
        let dy = (ch - dh) * 0.5;

        let p = ui.add(
            overlay,
            Kind::Panel { fill: Fill::Panel },
            at(dx, dy, dw, dh),
        );

        // Title
        ui.add(
            p,
            Kind::Label {
                text: t.a("Character Profile"),
                font: mfont(3, 20.0 * z),
                color: GOLD,
                align: Align::Center,
                wrap: false,
            },
            at(0.0, 16.0 * z, dw, 26.0 * z),
        );

        // Left column: Portrait and cycle buttons
        let port_w = 110.0 * z;
        let port_h = 130.0 * z;
        let port_x = 24.0 * z;
        let port_y = 60.0 * z;

        let port_box = ui.add(
            p,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(port_x, port_y, port_w, port_h),
        );

        if let Some(Some(sprite)) = self.icons.brand.portraits.get(edit.portrait as usize) {
            ui.add(
                port_box,
                Kind::Image {
                    sprite: *sprite,
                    fit: Fit::Contain,
                },
                Layout::fill(),
            );
        }

        // Prev / Next portrait buttons
        let cycle_row = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(port_x, port_y + port_h + 8.0 * z, port_w, 28.0 * z),
        );
        ui.set_stack(cycle_row, Stack::row(8.0 * z, 0.0).centered());

        ui.add(
            cycle_row,
            Kind::Button {
                text: "<".into(),
                font: mfont(2, 14.0 * z),
                action: A_MP_CHAR_PREV,
                enabled: true,
                hotkey: None,
            },
            lfixed(42.0 * z, 26.0 * z),
        );

        ui.add(
            cycle_row,
            Kind::Button {
                text: ">".into(),
                font: mfont(2, 14.0 * z),
                action: A_MP_CHAR_NEXT,
                enabled: true,
                hotkey: None,
            },
            lfixed(42.0 * z, 26.0 * z),
        );

        // Right column: Name and Description fields
        let rx = port_x + port_w + 24.0 * z;
        let rw = dw - rx - 24.0 * z;

        // Name
        ui.add(
            p,
            Kind::Label {
                text: t.a("Name"),
                font: mfont(2, 13.0 * z),
                color: CREAM,
                align: Align::Left,
                wrap: false,
            },
            at(rx, port_y, rw, 18.0 * z),
        );

        let name_border = if edit.focused_field == 0 { GOLD } else { MUTED };
        let name_box = ui.add(
            p,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(rx, port_y + 20.0 * z, rw, 30.0 * z),
        );
        let cursor_str = if edit.focused_field == 0 { "_" } else { "" };
        ui.add(
            name_box,
            Kind::Label {
                text: format!("{}{cursor_str}", edit.name),
                font: mfont(2, 14.0 * z),
                color: name_border,
                align: Align::Left,
                wrap: false,
            },
            at(8.0 * z, 5.0 * z, rw - 16.0 * z, 20.0 * z),
        );

        // Description
        ui.add(
            p,
            Kind::Label {
                text: t.a("Description:"),
                font: mfont(2, 13.0 * z),
                color: CREAM,
                align: Align::Left,
                wrap: false,
            },
            at(rx, port_y + 60.0 * z, rw, 18.0 * z),
        );

        let desc_border = if edit.focused_field == 1 { GOLD } else { MUTED };
        let desc_box = ui.add(
            p,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(rx, port_y + 80.0 * z, rw, 110.0 * z),
        );
        let dcursor_str = if edit.focused_field == 1 { "_" } else { "" };
        ui.add(
            desc_box,
            Kind::Label {
                text: format!("{}{dcursor_str}", edit.description),
                font: mfont(2, 13.0 * z),
                color: desc_border,
                align: Align::Left,
                wrap: true,
            },
            at(8.0 * z, 6.0 * z, rw - 16.0 * z, 98.0 * z),
        );

        // Bottom row: OK & Cancel
        let btm = ui.add(
            p,
            Kind::Panel { fill: Fill::None },
            at(24.0 * z, dh - 50.0 * z, dw - 48.0 * z, 36.0 * z),
        );
        ui.set_stack(btm, Stack::row(20.0 * z, 0.0).centered());

        ui.add(
            btm,
            Kind::Button {
                text: t.get(lng::OK, "OK").to_string(),
                font: mfont(2, 14.0 * z),
                action: A_MP_CHAR_OK,
                enabled: !edit.name.trim().is_empty(),
                hotkey: Some('O'),
            },
            lfixed(120.0 * z, 34.0 * z),
        );

        ui.add(
            btm,
            Kind::Button {
                text: t.get(lng::CANCEL, "Cancel").to_string(),
                font: mfont(2, 14.0 * z),
                action: A_MP_CHAR_CANCEL,
                enabled: true,
                hotkey: Some('C'),
            },
            lfixed(120.0 * z, 34.0 * z),
        );
    }

    /// Renders the K3 Multiplayer Lobby (Game Preparation) screen.
    #[allow(clippy::too_many_arguments)]
    pub fn build_mp_lobby_screen(
        &self,
        ui: &mut Ui,
        layer: NodeId,
        cw: f32,
        ch: f32,
        z: f32,
        lobby: &MpLobbyState,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let t = &self.texts;
        let is_host = lobby.backend.is_host();

        // Main container panel
        let pad = 20.0 * z;
        let p_w = (cw - pad * 2.0).clamp(640.0 * z, 980.0 * z);
        let p_h = (ch - pad * 2.0).clamp(480.0 * z, 700.0 * z);
        let p_x = (cw - p_w) * 0.5;
        let p_y = (ch - p_h) * 0.5;

        let root = ui.add(
            layer,
            Kind::Panel { fill: Fill::Panel },
            at(p_x, p_y, p_w, p_h),
        );

        // Title
        let title_text = if is_host {
            format!("{} ({})", t.a("Multiplayer Lobby"), t.a("Host"))
        } else {
            format!("{} ({})", t.a("Multiplayer Lobby"), t.a("Guest"))
        };
        ui.add(
            root,
            Kind::Label {
                text: title_text,
                font: mfont(3, 20.0 * z),
                color: GOLD,
                align: Align::Center,
                wrap: false,
            },
            at(0.0, 12.0 * z, p_w, 26.0 * z),
        );

        // Status message banner if any
        if let Some(status) = &lobby.status_message {
            ui.add(
                root,
                Kind::Label {
                    text: status.clone(),
                    font: mfont(2, 13.0 * z),
                    color: GOLD,
                    align: Align::Center,
                    wrap: false,
                },
                at(0.0, 36.0 * z, p_w, 18.0 * z),
            );
        }

        let content_y = 56.0 * z;
        let col_w = (p_w - 48.0 * z) * 0.5;
        let col_h = p_h - content_y - 70.0 * z;

        // Left Column: Game Settings (Scenario & Options)
        let left_panel = ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(16.0 * z, content_y, col_w, col_h),
        );

        // Scenario selection
        ui.add(
            left_panel,
            Kind::Label {
                text: format!(
                    "Scenario: {}",
                    if lobby.scenario_name.is_empty() {
                        "USA"
                    } else {
                        &lobby.scenario_name
                    }
                ),
                font: mfont(3, 15.0 * z),
                color: GOLD,
                align: Align::Left,
                wrap: false,
            },
            at(12.0 * z, 10.0 * z, col_w - 24.0 * z, 22.0 * z),
        );

        if is_host {
            let b_map = ui.add(
                left_panel,
                Kind::Button {
                    text: "Choose Map / Save...".into(),
                    font: mfont(2, 12.0 * z),
                    action: A_MP_MAP_PICK,
                    enabled: true,
                    hotkey: None,
                },
                at(12.0 * z, 34.0 * z, col_w - 24.0 * z, 26.0 * z),
            );
            tips.push((b_map, "Select a scenario map or saved game".into()));
        }

        // Difficulty level presets
        ui.add(
            left_panel,
            Kind::Label {
                text: "Difficulty Level:".into(),
                font: mfont(2, 13.0 * z),
                color: CREAM,
                align: Align::Left,
                wrap: false,
            },
            at(12.0 * z, 68.0 * z, col_w - 24.0 * z, 18.0 * z),
        );

        let diff_row = ui.add(
            left_panel,
            Kind::Panel { fill: Fill::None },
            at(12.0 * z, 88.0 * z, col_w - 24.0 * z, 26.0 * z),
        );
        ui.set_stack(diff_row, Stack::row(4.0 * z, 0.0));

        let levels = [
            ("Easy", crate::setup::Level::Easy),
            ("Med", crate::setup::Level::Medium),
            ("Hard", crate::setup::Level::Hard),
            ("Exp", crate::setup::Level::Expert),
        ];
        for (i, (name, lvl)) in levels.into_iter().enumerate() {
            let on = lobby.setup.level == lvl;
            ui.add(
                diff_row,
                Kind::Toggle {
                    text: name.into(),
                    font: mfont(2, 11.0 * z),
                    action: A_MP_DIFF + i as u32,
                    on,
                    enabled: is_host,
                },
                lfixed(48.0 * z, 24.0 * z),
            );
        }

        // Models: Financial & Industrial
        ui.add(
            left_panel,
            Kind::Label {
                text: format!("Financial: {:?}", lobby.setup.financial),
                font: mfont(2, 13.0 * z),
                color: CREAM,
                align: Align::Left,
                wrap: false,
            },
            at(12.0 * z, 124.0 * z, col_w - 24.0 * z, 18.0 * z),
        );

        ui.add(
            left_panel,
            Kind::Label {
                text: format!("Industrial: {:?}", lobby.setup.industrial),
                font: mfont(2, 13.0 * z),
                color: CREAM,
                align: Align::Left,
                wrap: false,
            },
            at(12.0 * z, 146.0 * z, col_w - 24.0 * z, 18.0 * z),
        );

        // Computer players count
        ui.add(
            left_panel,
            Kind::Label {
                text: format!("Extra Computers: {}", lobby.setup.extra_computers),
                font: mfont(2, 13.0 * z),
                color: CREAM,
                align: Align::Left,
                wrap: false,
            },
            at(12.0 * z, 172.0 * z, 160.0 * z, 22.0 * z),
        );

        if is_host {
            let comp_row = ui.add(
                left_panel,
                Kind::Panel { fill: Fill::None },
                at(col_w - 70.0 * z, 168.0 * z, 60.0 * z, 24.0 * z),
            );
            ui.set_stack(comp_row, Stack::row(4.0 * z, 0.0));
            ui.add(
                comp_row,
                Kind::Button {
                    text: "-".into(),
                    font: mfont(2, 12.0 * z),
                    action: A_MP_COMP_DOWN,
                    enabled: lobby.setup.extra_computers > 0,
                    hotkey: None,
                },
                lfixed(24.0 * z, 22.0 * z),
            );
            ui.add(
                comp_row,
                Kind::Button {
                    text: "+".into(),
                    font: mfont(2, 12.0 * z),
                    action: A_MP_COMP_UP,
                    enabled: lobby.setup.extra_computers < 8,
                    hotkey: None,
                },
                lfixed(24.0 * z, 22.0 * z),
            );
        }

        // Right Column: Connected Players & Chat
        let rx = 16.0 * z + col_w + 16.0 * z;
        let right_panel = ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(rx, content_y, col_w, col_h),
        );

        // Players roster list
        ui.add(
            right_panel,
            Kind::Label {
                text: t.get(1726, "Players").to_string(),
                font: mfont(3, 15.0 * z),
                color: GOLD,
                align: Align::Left,
                wrap: false,
            },
            at(12.0 * z, 10.0 * z, col_w - 24.0 * z, 22.0 * z),
        );

        let roster_box = ui.add(
            right_panel,
            Kind::Panel { fill: Fill::None },
            at(12.0 * z, 34.0 * z, col_w - 24.0 * z, 120.0 * z),
        );
        ui.set_stack(roster_box, Stack::column(4.0 * z, 0.0));

        let my_id = lobby.backend.my_id();
        let host_id = lobby.backend.host_id();
        for p_info in lobby.backend.players() {
            let approved = lobby.approvals.get(&p_info.id).copied().unwrap_or(false);
            let is_me = p_info.id == my_id;
            let is_p_host = p_info.id == host_id;
            let ping = lobby.backend.ping_ms(p_info.id);

            let row = ui.add(
                roster_box,
                Kind::Panel {
                    fill: if is_me {
                        Fill::Overlay(Color(0x30, 0x60, 0x30, 0x60))
                    } else {
                        Fill::None
                    },
                },
                lfixed(col_w - 24.0 * z, 22.0 * z),
            );

            // Approval light indicator
            ui.add(
                row,
                Kind::Label {
                    text: if approved { "●".into() } else { "○".into() },
                    font: mfont(2, 13.0 * z),
                    color: if approved { GREEN_LGT } else { RED_LGT },
                    align: Align::Left,
                    wrap: false,
                },
                at(4.0 * z, 1.0 * z, 16.0 * z, 20.0 * z),
            );

            // Name & Role
            let role_label = if is_p_host { "(Host)" } else { "(Guest)" };
            let me_label = if is_me { " [You]" } else { "" };
            ui.add(
                row,
                Kind::Label {
                    text: format!("{} {} {}", p_info.name, role_label, me_label),
                    font: mfont(2, 12.0 * z),
                    color: if is_me { GOLD } else { CREAM },
                    align: Align::Left,
                    wrap: false,
                },
                at(24.0 * z, 1.0 * z, col_w - 110.0 * z, 20.0 * z),
            );

            // Ping
            let ping_str = if is_me {
                "0 ms".into()
            } else {
                format!("{ping} ms")
            };
            ui.add(
                row,
                Kind::Label {
                    text: ping_str,
                    font: mfont(2, 11.0 * z),
                    color: MUTED,
                    align: Align::Right,
                    wrap: false,
                },
                at(col_w - 90.0 * z, 1.0 * z, 60.0 * z, 20.0 * z),
            );
        }

        // My Approval Toggle button
        let my_approved = lobby.approvals.get(&my_id).copied().unwrap_or(false);
        let b_app = ui.add(
            right_panel,
            Kind::Toggle {
                text: if my_approved {
                    t.a("Approved")
                } else {
                    t.a("Approve")
                },
                font: mfont(2, 13.0 * z),
                action: A_MP_APPROVE,
                on: my_approved,
                enabled: true,
            },
            at(12.0 * z, 160.0 * z, col_w - 24.0 * z, 28.0 * z),
        );
        tips.push((
            b_app,
            t.get(2221, "Click to toggle your approval of the game settings.")
                .to_string(),
        ));

        // Lobby Chat area
        let chat_y = 196.0 * z;
        let chat_h = col_h - chat_y - 40.0 * z;
        let chat_box = ui.add(
            right_panel,
            Kind::Panel {
                fill: Fill::Solid(Color(0, 0, 0, 0x80)),
            },
            at(12.0 * z, chat_y, col_w - 24.0 * z, chat_h),
        );

        // Show last 6 messages
        let msg_start = lobby.chat_log.len().saturating_sub(6);
        let chat_col = ui.add(chat_box, Kind::Panel { fill: Fill::None }, Layout::fill());
        ui.set_stack(chat_col, Stack::column(2.0 * z, 4.0 * z));

        for (sender, text) in &lobby.chat_log[msg_start..] {
            ui.add(
                chat_col,
                Kind::Label {
                    text: format!("{sender}:  {text}"),
                    font: mfont(2, 11.0 * z),
                    color: if sender == "***" { GOLD } else { CREAM },
                    align: Align::Left,
                    wrap: false,
                },
                lfixed(col_w - 32.0 * z, 16.0 * z),
            );
        }

        // Chat input field & send button
        let inp_y = chat_y + chat_h + 6.0 * z;
        let inp_box = ui.add(
            right_panel,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(12.0 * z, inp_y, col_w - 84.0 * z, 26.0 * z),
        );
        ui.add(
            inp_box,
            Kind::Label {
                text: format!("{}_", lobby.chat_input),
                font: mfont(2, 12.0 * z),
                color: CREAM,
                align: Align::Left,
                wrap: false,
            },
            at(6.0 * z, 3.0 * z, col_w - 96.0 * z, 20.0 * z),
        );

        ui.add(
            right_panel,
            Kind::Button {
                text: t.get(1724, "Send").to_string(),
                font: mfont(2, 12.0 * z),
                action: A_MP_CHAT_SEND,
                enabled: !lobby.chat_input.trim().is_empty(),
                hotkey: None,
            },
            at(col_w - 66.0 * z, inp_y, 54.0 * z, 26.0 * z),
        );

        // Bottom Action Bar: Start Game / Waiting label / Leave
        let bar_y = p_h - 54.0 * z;
        let bar = ui.add(
            root,
            Kind::Panel { fill: Fill::None },
            at(16.0 * z, bar_y, p_w - 32.0 * z, 40.0 * z),
        );
        ui.set_stack(bar, Stack::row(24.0 * z, 0.0).centered());

        if is_host {
            let all_ok = lobby.all_approved();
            let b_start = ui.add(
                bar,
                Kind::Button {
                    text: t.a("Start Game"),
                    font: mfont(3, 15.0 * z),
                    action: A_MP_START,
                    enabled: all_ok,
                    hotkey: Some('S'),
                },
                lfixed(160.0 * z, 36.0 * z),
            );
            if !all_ok {
                tips.push((
                    b_start,
                    t.get(2216, "You can't start the game until all human players have approved of the game settings...").to_string(),
                ));
            }
        } else {
            ui.add(
                bar,
                Kind::Label {
                    text: t.a("Waiting for host to start..."),
                    font: mfont(2, 13.0 * z),
                    color: MUTED,
                    align: Align::Center,
                    wrap: false,
                },
                lfixed(220.0 * z, 36.0 * z),
            );
        }

        ui.add(
            bar,
            Kind::Button {
                text: t.a("Change Character"),
                font: mfont(2, 13.0 * z),
                action: A_MP_CHAR_DIALOG,
                enabled: true,
                hotkey: None,
            },
            lfixed(160.0 * z, 36.0 * z),
        );

        ui.add(
            bar,
            Kind::Button {
                text: t.a("Leave Lobby"),
                font: mfont(2, 13.0 * z),
                action: A_MP_LEAVE,
                enabled: true,
                hotkey: Some('L'),
            },
            lfixed(140.0 * z, 36.0 * z),
        );
    }

    /// Renders in-game multiplayer overlay (HUD player count/ping badge, chat lines, input bar, alerts).
    pub fn build_in_game_mp(&self, ui: &mut Ui, cw: f32, ch: f32, z: f32) {
        let g = match &self.screen {
            crate::state::Screen::Game(g) => g,
            _ => return,
        };
        let Some(mp) = &g.mp else {
            return;
        };
        let t = &self.texts;

        let root = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());

        // 1. HUD Player & Ping Indicator (top-right of screen)
        let player_count = mp.backend.players().len();
        let my_id = mp.backend.my_id();
        let max_ping = mp
            .backend
            .players()
            .iter()
            .filter(|p| p.id != my_id)
            .map(|p| mp.backend.ping_ms(p.id))
            .max()
            .unwrap_or(0);

        let ping_badge_text = if player_count > 1 {
            format!("MP: {player_count}p ({max_ping}ms)")
        } else {
            "MP: 1p".into()
        };

        let badge = ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(Color(0, 0, 0, 0xa0)),
            },
            at(cw - 140.0 * z, 6.0 * z, 130.0 * z, 22.0 * z),
        );
        ui.add(
            badge,
            Kind::Label {
                text: ping_badge_text,
                font: mfont(2, 11.0 * z),
                color: GOLD,
                align: Align::Center,
                wrap: false,
            },
            Layout::fill(),
        );

        // 2. Desync alert banner
        if let Some((minute, _)) = mp.desync_alert {
            let desync_p = ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Solid(Color(0xb0, 0x10, 0x10, 0xe0)),
                },
                at((cw - 320.0 * z) * 0.5, 40.0 * z, 320.0 * z, 28.0 * z),
            );
            ui.add(
                desync_p,
                Kind::Label {
                    text: format!("{} (min {minute})", t.get(2320, "Out of Synch")),
                    font: mfont(3, 14.0 * z),
                    color: Color::WHITE,
                    align: Align::Center,
                    wrap: false,
                },
                Layout::fill(),
            );
        }

        // 3. Player left alert banner
        if let Some((name, _)) = &mp.player_left_alert {
            let left_p = ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x20, 0x20, 0x20, 0xe0)),
                },
                at((cw - 440.0 * z) * 0.5, 72.0 * z, 440.0 * z, 36.0 * z),
            );
            ui.add(
                left_p,
                Kind::Label {
                    text: format!("Player {name} has left. Position assumed by AI."),
                    font: mfont(2, 12.0 * z),
                    color: GOLD,
                    align: Align::Center,
                    wrap: false,
                },
                Layout::fill(),
            );
        }

        // 4. In-Game Chat Lines (up to 6 lines, 14s duration)
        // Positioned at bottom-left above the lower control panel
        if !mp.chat_lines.is_empty() {
            let chat_w = (cw * 0.45).clamp(280.0 * z, 520.0 * z);
            let line_h = 18.0 * z;
            let total_h = (mp.chat_lines.len() as f32) * line_h + 8.0 * z;
            let chat_y = ch - 160.0 * z - total_h;

            let chat_bg = ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Solid(Color(0, 0, 0, 0x70)),
                },
                at(12.0 * z, chat_y, chat_w, total_h),
            );

            let col = ui.add(chat_bg, Kind::Panel { fill: Fill::None }, Layout::fill());
            ui.set_stack(col, Stack::column(2.0 * z, 4.0 * z));

            for line in &mp.chat_lines {
                let alpha = (line.time_left / 2.0).clamp(0.2, 1.0);
                let col_val = (240.0 * alpha) as u8;
                ui.add(
                    col,
                    Kind::Label {
                        text: format!("{}:  {}", line.sender, line.text),
                        font: mfont(2, 12.0 * z),
                        color: Color(col_val, col_val, col_val, 255),
                        align: Align::Left,
                        wrap: false,
                    },
                    lfixed(chat_w - 12.0 * z, line_h),
                );
            }
        }

        // 5. Chat Input Bar (when user pressed Enter/Tab/T)
        if mp.chat_open {
            let bar_w = (cw * 0.45).clamp(280.0 * z, 520.0 * z);
            let bar_h = 32.0 * z;
            let bar_y = ch - 156.0 * z;

            let inp_panel = ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x10, 0x14, 0x10, 0xf0)),
                },
                at(12.0 * z, bar_y, bar_w, bar_h),
            );

            ui.add(
                inp_panel,
                Kind::Label {
                    text: format!("Chat: {}_", mp.chat_input),
                    font: mfont(2, 13.0 * z),
                    color: GOLD,
                    align: Align::Left,
                    wrap: false,
                },
                at(8.0 * z, 6.0 * z, bar_w - 70.0 * z, 20.0 * z),
            );

            ui.add(
                inp_panel,
                Kind::Button {
                    text: t.get(1724, "Send").to_string(),
                    font: mfont(2, 11.0 * z),
                    action: A_MP_INGAME_CHAT_SEND,
                    enabled: !mp.chat_input.trim().is_empty(),
                    hotkey: None,
                },
                at(bar_w - 60.0 * z, 4.0 * z, 52.0 * z, 24.0 * z),
            );
        }
    }
}
