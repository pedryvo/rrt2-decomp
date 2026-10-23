//! Map Options pages for the players, player pool, managers, engines, ports and robbers
//! (docs/formats/map-gam.md section 10; the original's `mapset.win` pages built by `0x485b40`,
//! `0x484130`, `0x484a50`, `0x483800`, `0x487380` and `0x482270`). The rules live in
//! `rt2_sim::editor_opts`; this file is the dialog.

use rt2_sim::editor_opts::{self as opts, Controller, OptError};
use rt2_ui::{Align, Kind};

use super::editor_modals::{
    A_OPT_BASE, Ctx, F_CASH, F_PORT_Q, F_ROB_LEVEL, F_ROB_NAME, F_ROB_YEAR, F_START_OFFSET,
    OptionsState,
};
use super::editor_ui::A_MODAL;
use super::editor_ui::{BAD, HEAD, MUTED, TEXT};
use super::{AppState, Screen};

pub(super) const A_PLR_LIST: u32 = A_OPT_BASE;
pub(super) const A_PLR_ADD: u32 = A_OPT_BASE + 1;
pub(super) const A_PLR_DEL: u32 = A_OPT_BASE + 2;
pub(super) const A_PLR_PREV: u32 = A_OPT_BASE + 3;
pub(super) const A_PLR_NEXT: u32 = A_OPT_BASE + 4;
pub(super) const A_PLR_MAND: u32 = A_OPT_BASE + 5;
pub(super) const A_PLR_OPT: u32 = A_OPT_BASE + 6;
pub(super) const A_PLR_HUMAN: u32 = A_OPT_BASE + 7;
pub(super) const A_PLR_COMP: u32 = A_OPT_BASE + 8;
pub(super) const A_PLR_EITHER: u32 = A_OPT_BASE + 9;
pub(super) const A_PLR_OVR: u32 = A_OPT_BASE + 10;
/// `+0..3`: North America, Europe, World.
pub(super) const A_POOL_PRESET: u32 = A_OPT_BASE + 15;
pub(super) const A_POOL_ITEM: u32 = A_OPT_BASE + 20; // + 0..40
pub(super) const A_MGR_ITEM: u32 = A_OPT_BASE + 60; // + 0..40
pub(super) const A_ENG_PRESET: u32 = A_OPT_BASE + 100; // + 0..3
pub(super) const A_ENG_ITEM: u32 = A_OPT_BASE + 110; // + 0..66
pub(super) const A_PORT_ANNUAL: u32 = A_OPT_BASE + 180; // + 0..4
pub(super) const A_PORT_MANUF: u32 = A_OPT_BASE + 184; // + 0..4
pub(super) const A_PORT_SUP: u32 = A_OPT_BASE + 188; // + 0..8: prev/next per row
pub(super) const A_PORT_DEM: u32 = A_OPT_BASE + 196; // + 0..8
pub(super) const A_ROB_LEVEL: u32 = A_OPT_BASE + 205; // -10, +10
/// Last action id of these pages (exclusive).
pub(super) const A_OPT_END: u32 = A_OPT_BASE + 210;
/// Actions of the Special Restrictions, Regional, Computer A.I. and Industry pages; they sit in
/// the gap between the text fields (`A_MODAL + 620..650`) and the end of the dialog range.
pub(super) const A_SPEC_BASE: u32 = A_MODAL + 650;
pub(super) const A_SPECIAL: u32 = A_SPEC_BASE; // + 0..10
pub(super) const A_START_OFF: u32 = A_SPEC_BASE + 10; // -10, +10
pub(super) const A_COST: u32 = A_SPEC_BASE + 12; // + 3 * row + level
pub(super) const A_TRAFFIC: u32 = A_SPEC_BASE + 21; // + level
pub(super) const A_AI_TRACK: u32 = A_SPEC_BASE + 24; // + 0..3: aggressive, moderate, conservative
pub(super) const A_AI_DIV: u32 = A_SPEC_BASE + 27; // + 0..3: aggressive, moderate, none
pub(super) const A_IND_TOGGLE: u32 = A_SPEC_BASE + 30;
pub(super) const A_IND_LIST: u32 = A_SPEC_BASE + 31;
pub(super) const A_SPEC_END: u32 = A_SPEC_BASE + 32;

/// A wrapped label (the hints of the pages).
fn wrap(c: &mut Ctx, x: f32, y: f32, w: f32, h: f32, text: &str, color: rt2_ui::Color) {
    let l = c.rect(x, y, w, h);
    c.ui.add(
        c.b,
        Kind::Label {
            text: text.to_owned(),
            font: c.f,
            color,
            align: Align::Left,
            wrap: true,
        },
        l,
    );
}

/// Values of the supplied-cargo list: none, then the 45 cargos.
fn supply_values() -> Vec<u8> {
    std::iter::once(opts::NO_CARGO)
        .chain(0..opts::PORT_CARGOS as u8)
        .collect()
}

fn step_value(list: &[u8], cur: u8, d: isize) -> u8 {
    let i = list.iter().position(|&v| v == cur).unwrap_or(0) as isize;
    list[(i + d).rem_euclid(list.len() as isize) as usize]
}

fn digits(s: &str, max: usize) -> String {
    s.chars().filter(char::is_ascii_digit).take(max).collect()
}

impl AppState {
    fn opt_text(&self, id: u32, fallback: &str) -> String {
        self.texts.get(id, fallback)
    }

    fn opt_save(&self) -> Option<&rt2_formats::map::Save> {
        self.editor_world().and_then(|w| w.editor_save())
    }

    fn cargo_label(&self, c: u8) -> String {
        if c == opts::NO_CARGO {
            return self.opt_text(188, "None");
        }
        match self.data.as_ref() {
            Some(d) if usize::from(c) < opts::PORT_CARGOS => {
                self.opt_text(opts::port_cargo_name_id(&d.image, usize::from(c)), "?")
            }
            _ => format!("#{c}"),
        }
    }

    fn typed(&self, o: &OptionsState, id: u32, saved: String) -> String {
        match &o.typing {
            Some((t, s)) if *t == id => s.clone(),
            _ => saved,
        }
    }

    // ---- Players --------------------------------------------------------------------------

    pub(super) fn draw_players(&self, c: &mut Ctx, o: &OptionsState) {
        let t = &self.texts;
        let Some(s) = self.opt_save() else { return };
        let list = opts::players(s);
        let y0 = c.top() + 56.0;
        let sel = o.player_sel.min(list.len().saturating_sub(1));
        let items: Vec<String> = list
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let m = if p.mandatory { 'M' } else { 'O' };
                let w = match p.controller {
                    Controller::Human => t.get(498, "Human"),
                    Controller::Computer => t.get(499, "Computer"),
                    Controller::Either => t.get(508, "Either"),
                };
                format!("{}) {} [{m}] {w}", i + 1, p.name)
            })
            .collect();
        c.list(20.0, y0, 310.0, 290.0, items, sel, A_PLR_LIST);
        c.button(20.0, y0 + 298.0, 150.0, &t.get(501, "Add"), A_PLR_ADD);
        c.button(180.0, y0 + 298.0, 150.0, &t.get(503, "Delete"), A_PLR_DEL);
        let n = opts::player_counts(s);
        let lines = [
            (t.get(2305, "Number of Players Allowed"), n[0], n[1]),
            (t.get(498, "Human"), n[2], n[3]),
            (t.get(499, "Computer"), n[4], n[5]),
        ];
        for (k, (name, a, b)) in lines.into_iter().enumerate() {
            c.label(
                20.0,
                y0 + 334.0 + 26.0 * k as f32,
                310.0,
                &format!("{name}: {a} / {b}"),
                MUTED,
            );
        }
        let Some(p) = list.get(sel) else { return };
        let x = 350.0;
        let (names, desc) = match self.data.as_ref() {
            Some(d) => opts::persona_texts(
                &d.image,
                &|id| self.texts.get(id, ""),
                p.persona.max(0) as usize,
                p.slot,
            ),
            None => (p.name.clone(), String::new()),
        };
        c.button(x, y0, 36.0, "<", A_PLR_PREV);
        c.label(x + 44.0, y0, 270.0, &names, HEAD);
        c.button(x + 322.0, y0, 36.0, ">", A_PLR_NEXT);
        let l = c.rect(x, y0 + 34.0, 358.0, 100.0);
        c.ui.add(
            c.b,
            Kind::Label {
                text: desc,
                font: c.f,
                color: MUTED,
                align: Align::Left,
                wrap: true,
            },
            l,
        );
        let y = y0 + 142.0;
        c.toggle(
            x,
            y,
            175.0,
            &t.get(506, "Mandatory"),
            A_PLR_MAND,
            p.mandatory,
        );
        c.toggle(
            x + 183.0,
            y,
            175.0,
            &t.get(507, "Optional"),
            A_PLR_OPT,
            !p.mandatory,
        );
        let y = y + 34.0;
        let w3 = 114.0;
        c.toggle(
            x,
            y,
            w3,
            &t.get(498, "Human"),
            A_PLR_HUMAN,
            p.controller == Controller::Human,
        );
        c.toggle(
            x + w3 + 8.0,
            y,
            w3,
            &t.get(499, "Computer"),
            A_PLR_COMP,
            p.controller == Controller::Computer,
        );
        c.toggle(
            x + 2.0 * (w3 + 8.0),
            y,
            w3,
            &t.get(508, "Either"),
            A_PLR_EITHER,
            p.controller == Controller::Either,
        );
        let y = y + 34.0;
        c.toggle(
            x,
            y,
            358.0,
            &t.get(505, "Allow user to override"),
            A_PLR_OVR,
            p.allow_override,
        );
        let y = y + 40.0;
        c.label(x, y, 120.0, &t.a("Cash"), MUTED);
        let cash = self.typed(o, F_CASH, format!("{}", p.cash as i64));
        c.field(x + 130.0, y, 228.0, &cash, F_CASH);
        if !o.note.is_empty() {
            c.label(x, y + 34.0, 358.0, &o.note, BAD);
        }
    }

    pub(super) fn opt_error_message(&self, e: OptError) -> String {
        self.opt_error_text(e)
    }

    fn opt_error_text(&self, e: OptError) -> String {
        match e {
            OptError::FirstMandatory => {
                self.opt_text(2290, "The first player is always mandatory.")
            }
            OptError::FirstNotAi => {
                self.opt_text(2291, "The first player may not be an AI only player.")
            }
            OptError::FirstPlayer => self.t_first_player(),
            OptError::TooManyPlayers => self.texts.a("At most 32 players."),
            OptError::NoSuchPlayer | OptError::NoSuchTerritory => String::new(),
            OptError::FirstTerritory => self.texts.a("The first territory cannot be removed."),
            OptError::Remapped(id) => self.texts.af(
                "Territory %1 remaps to this one; remap it first.",
                &[id.to_string()],
            ),
        }
    }

    fn t_first_player(&self) -> String {
        self.opt_text(2290, "The first player is always mandatory.")
    }

    /// Runs an edit of the Players page on the world's map and refreshes the header summary.
    fn player_edit(
        &mut self,
        o: &mut OptionsState,
        f: impl FnOnce(
            &mut rt2_sim::World,
            &rt2_sim::data::DataImage,
            &dyn Fn(u32) -> String,
        ) -> Result<(), OptError>,
    ) {
        let texts = &self.texts;
        let (Some(d), Screen::Game(g)) = (self.data.as_ref(), &mut self.screen) else {
            return;
        };
        let r = f(&mut g.world, &d.image, &|id| texts.get(id, ""));
        if let Some(s) = g.world.editor_save_mut() {
            opts::refresh_player_summary(s);
        }
        o.note = match r {
            Ok(()) => String::new(),
            Err(e) => self.opt_error_text(e),
        };
        o.typing = None;
        self.mark_options_dirty();
    }

    fn selected_player(&self, o: &OptionsState) -> Option<u16> {
        let s = self.opt_save()?;
        let ids = opts::player_slots(s);
        ids.get(o.player_sel.min(ids.len().saturating_sub(1)))
            .copied()
    }

    // ---- Player pool, managers, engines ---------------------------------------------------

    fn grid_header(&self, c: &mut Ctx, presets: u32, count: String, hint: &str) -> f32 {
        let t = &self.texts;
        let y = c.top() + 56.0;
        for (k, name) in [
            t.get(489, "North America"),
            t.get(491, "Europe"),
            t.get(493, "World"),
        ]
        .iter()
        .enumerate()
        {
            if presets != 0 {
                c.button(20.0 + 160.0 * k as f32, y, 152.0, name, presets + k as u32);
            }
        }
        c.label(500.0, y, 240.0, &count, HEAD);
        c.label(20.0, y + 28.0, 720.0, hint, MUTED);
        y + 62.0
    }

    pub(super) fn draw_pool(&self, c: &mut Ctx) {
        let Some(s) = self.opt_save() else { return };
        let t = &self.texts;
        let y = self.grid_header(
            c,
            A_POOL_PRESET,
            format!("{}: {}", t.get(2306, "Player Pool"), opts::pool_count(s)),
            &t.get(
                2307,
                "Players positions that are unassigned are drawn from those checked here.",
            ),
        );
        for p in 1..=opts::POOL_SIZE {
            let name = match self.data.as_ref() {
                Some(d) => opts::persona_name(&d.image, &|id| t.get(id, ""), p),
                None => format!("#{p}"),
            };
            let i = p - 1;
            c.toggle(
                20.0 + (i % 4) as f32 * 180.0,
                y + (i / 4) as f32 * 30.0,
                176.0,
                &name,
                A_POOL_ITEM + i as u32,
                opts::pool_allowed(s, p),
            );
        }
    }

    pub(super) fn draw_managers(&self, c: &mut Ctx) {
        let Some(s) = self.opt_save() else { return };
        let t = &self.texts;
        let y = self.grid_header(
            c,
            0,
            format!(
                "{} {}",
                t.get(264, "Number of managers allowed:"),
                opts::manager_count(s)
            ),
            &t.get(
                497,
                "Managers not checked are not allowed in the map, unless specifically allowed by an event.",
            ),
        );
        for m in 0..opts::MANAGER_COUNT {
            c.toggle(
                20.0 + (m % 4) as f32 * 180.0,
                y + (m / 4) as f32 * 30.0,
                176.0,
                &t.get(opts::manager_name_id(m), "?"),
                A_MGR_ITEM + m as u32,
                opts::manager_allowed(s, m),
            );
        }
    }

    pub(super) fn draw_engines(&self, c: &mut Ctx) {
        let Some(s) = self.opt_save() else { return };
        let t = &self.texts;
        let y = self.grid_header(
            c,
            A_ENG_PRESET,
            format!(
                "{} {}",
                t.get(263, "Number of engines allowed:"),
                opts::engine_count(s)
            ),
            &t.get(
                495,
                "Engines not checked are not allowed in the map, unless specifically allowed by an event or manager.",
            ),
        );
        let rows = opts::ENGINE_COUNT.div_ceil(4);
        for e in 0..opts::ENGINE_COUNT {
            let name = match self.data.as_ref() {
                Some(d) => t.get(opts::engine_name_id(&d.image, e), "?"),
                None => format!("#{e}"),
            };
            c.toggle(
                20.0 + (e / rows) as f32 * 180.0,
                y + (e % rows) as f32 * 26.0,
                176.0,
                &name,
                A_ENG_ITEM + e as u32,
                opts::engine_allowed(s, e),
            );
        }
    }

    // ---- Ports ----------------------------------------------------------------------------

    pub(super) fn draw_ports(&self, c: &mut Ctx, o: &OptionsState) {
        let Some(s) = self.opt_save() else { return };
        let t = &self.texts;
        let y0 = c.top() + 48.0;
        c.label(
            20.0,
            y0 - 4.0,
            700.0,
            &t.get(510, "Cargos Supplied and Demanded in Ports"),
            HEAD,
        );
        for k in 0..opts::PORT_COUNT {
            let Some(p) = opts::port(s, k) else { continue };
            let y = y0 + 28.0 + 104.0 * k as f32;
            c.toggle(
                20.0,
                y,
                180.0,
                &t.get(511, "Annual supply"),
                A_PORT_ANNUAL + k as u32,
                p.annual,
            );
            c.toggle(
                206.0,
                y,
                180.0,
                &t.get(513, "Manufactured supply"),
                A_PORT_MANUF + k as u32,
                !p.annual,
            );
            c.label(
                400.0,
                y,
                190.0,
                &t.get(515, "Quantity\nper year").replace('\n', " "),
                MUTED,
            );
            let q = self.typed(o, F_PORT_Q + k as u32, format!("{}", p.quantity as i32));
            c.field(596.0, y, 60.0, &q, F_PORT_Q + k as u32);
            let y = y + 34.0;
            c.label(20.0, y, 120.0, &t.get(2308, "Supplied"), MUTED);
            c.button(140.0, y, 36.0, "<", A_PORT_SUP + 2 * k as u32);
            c.label(184.0, y, 440.0, &self.cargo_label(p.supplied), TEXT);
            c.button(628.0, y, 36.0, ">", A_PORT_SUP + 2 * k as u32 + 1);
            let y = y + 34.0;
            c.label(20.0, y, 120.0, &t.get(193, "Demanded"), MUTED);
            if p.annual {
                c.label(184.0, y, 440.0, &self.cargo_label(opts::NO_CARGO), MUTED);
            } else {
                c.button(140.0, y, 36.0, "<", A_PORT_DEM + 2 * k as u32);
                c.label(184.0, y, 440.0, &self.cargo_label(p.demanded), TEXT);
                c.button(628.0, y, 36.0, ">", A_PORT_DEM + 2 * k as u32 + 1);
            }
        }
    }

    // ---- Robbers --------------------------------------------------------------------------

    pub(super) fn draw_robbers(&self, c: &mut Ctx, o: &OptionsState) {
        let Some(s) = self.opt_save() else { return };
        let t = &self.texts;
        let y0 = c.top() + 56.0;
        c.label(20.0, y0, 250.0, &t.get(2299, "Overall Robbery Level"), HEAD);
        c.button(280.0, y0, 44.0, "-10", A_ROB_LEVEL);
        let lvl = self.typed(o, F_ROB_LEVEL, opts::robbery_level(s).to_string());
        c.field(330.0, y0, 70.0, &lvl, F_ROB_LEVEL);
        c.button(406.0, y0, 44.0, "+10", A_ROB_LEVEL + 1);
        c.label(
            20.0,
            y0 + 32.0,
            720.0,
            &t.get(
                2300,
                "Sets the overall level at which robberies occur.  The Jesse James era (circa 1875) would be a level of about 200.  Low level early 1900s robbery would be about a 30.",
            ),
            MUTED,
        );
        c.label(20.0, y0 + 90.0, 720.0, &t.get(2302, "Enter the name of a robber.  If left blank, this robber will not be used in the scenario.  If no names are entered, generic names will be used."), MUTED);
        for k in 0..opts::ROBBER_COUNT {
            let y = y0 + 140.0 + 40.0 * k as f32;
            let (name, year) = opts::robber(s, k).unwrap_or_default();
            c.label(
                20.0,
                y,
                130.0,
                &t.get(2301, "Robber #%1")
                    .replace("%1", &(k + 1).to_string()),
                MUTED,
            );
            c.field(160.0, y, 300.0, &name, F_ROB_NAME + k as u32);
            c.label(480.0, y, 120.0, &t.get(2303, "Begin Year"), MUTED);
            let saved = if year == 0 {
                String::new()
            } else {
                year.to_string()
            };
            let ytext = self.typed(o, F_ROB_YEAR + k as u32, saved);
            c.field(610.0, y, 80.0, &ytext, F_ROB_YEAR + k as u32);
        }
    }

    // ---- Special restrictions, regional, computer A.I., industry types ----------------------

    pub(super) fn draw_special(&self, c: &mut Ctx, o: &OptionsState) {
        let Some(s) = self.opt_save() else { return };
        let t = &self.texts;
        let y0 = c.top() + 50.0;
        c.label(20.0, y0, 200.0, &t.get(3329, "Start offset days"), HEAD);
        c.button(230.0, y0, 44.0, "-10", A_START_OFF);
        let v = self.typed(o, F_START_OFFSET, opts::start_offset_days(s).to_string());
        c.field(280.0, y0, 70.0, &v, F_START_OFFSET);
        c.button(356.0, y0, 44.0, "+10", A_START_OFF + 1);
        wrap(
            c,
            20.0,
            y0 + 28.0,
            740.0,
            48.0,
            &t.get(3330, "Offset, in days, to start of this scenario."),
            MUTED,
        );
        for (k, f) in opts::Special::ALL.into_iter().enumerate() {
            let y = y0 + 84.0 + 40.0 * k as f32;
            c.toggle(
                20.0,
                y,
                260.0,
                &t.get(f.text_id(), "?"),
                A_SPECIAL + k as u32,
                opts::special(s, f),
            );
            wrap(
                c,
                296.0,
                y - 4.0,
                464.0,
                40.0,
                &t.get(f.text_id() + 1, ""),
                MUTED,
            );
        }
    }

    pub(super) fn draw_regional(&self, c: &mut Ctx) {
        let Some(s) = self.opt_save() else { return };
        let t = &self.texts;
        let y0 = c.top() + 56.0;
        let levels = [
            t.get(2886, "High"),
            t.get(2887, "Normal"),
            t.get(2888, "Low"),
        ];
        for row in 0..3usize {
            let y = y0 + 64.0 * row as f32;
            c.label(20.0, y, 200.0, &t.get(2880 + row as u32, "?"), HEAD);
            for (l, name) in levels.iter().enumerate() {
                c.toggle(
                    230.0 + 120.0 * l as f32,
                    y,
                    112.0,
                    name,
                    A_COST + 3 * row as u32 + l as u32,
                    usize::from(opts::cost_level(s, row)) == l,
                );
            }
            wrap(
                c,
                20.0,
                y + 28.0,
                720.0,
                30.0,
                &t.get(2883 + row as u32, ""),
                MUTED,
            );
        }
        let y = y0 + 64.0 * 3.0 + 10.0;
        c.label(20.0, y, 200.0, &t.get(2889, "Passenger Traffic:"), HEAD);
        for (l, name) in levels.iter().enumerate() {
            c.toggle(
                230.0 + 120.0 * l as f32,
                y,
                112.0,
                name,
                A_TRAFFIC + l as u32,
                usize::from(opts::traffic_level(s)) == l,
            );
        }
        wrap(c, 20.0, y + 28.0, 720.0, 60.0, &t.get(2890, ""), MUTED);
    }

    pub(super) fn draw_ai(&self, c: &mut Ctx) {
        let Some(s) = self.opt_save() else { return };
        let t = &self.texts;
        let y0 = c.top() + 56.0;
        // Track expansion: aggressive stores 2, conservative 0 (ids 0x83fc..0x83fe).
        c.label(20.0, y0, 700.0, &t.get(3408, "Track Expansion"), HEAD);
        for k in 0..3u32 {
            let y = y0 + 34.0 + 70.0 * k as f32;
            c.toggle(
                20.0,
                y,
                200.0,
                &t.get(3395 + k, "?"),
                A_AI_TRACK + k,
                u32::from(opts::ai_track(s)) == 2 - k,
            );
            wrap(c, 236.0, y - 2.0, 520.0, 60.0, &t.get(3403 - k, ""), MUTED);
        }
        let y1 = y0 + 34.0 + 70.0 * 3.0 + 20.0;
        c.label(
            20.0,
            y1,
            700.0,
            &t.get(3409, "Dividend Payout / Stock Buyback"),
            HEAD,
        );
        for k in 0..3u32 {
            let y = y1 + 34.0 + 70.0 * k as f32;
            c.toggle(
                20.0,
                y,
                200.0,
                &t.get(3398 + k, "?"),
                A_AI_DIV + k,
                u32::from(opts::ai_dividend(s)) == k,
            );
            wrap(c, 236.0, y - 2.0, 520.0, 60.0, &t.get(3404 + k, ""), MUTED);
        }
    }

    /// The per-type list of the Industries tab (`0x486d70`): every type may be allowed or not
    /// for the map, with the weighting name at the right.
    pub(super) fn draw_industry_types(&self, c: &mut Ctx, o: &OptionsState, y: f32) {
        let (Some(s), Some(d)) = (self.opt_save(), self.data.as_ref()) else {
            return;
        };
        let t = &self.texts;
        c.label(20.0, y, 500.0, &t.get(464, "Industry Weightings"), HEAD);
        let items: Vec<String> = (0..opts::INDUSTRY_TYPES)
            .filter_map(|i| opts::industry_row(s, &d.image, i))
            .map(|r| {
                format!(
                    "{}{}   [{}]",
                    t.get(r.name_id, "?"),
                    if r.allowed {
                        String::new()
                    } else {
                        t.get(478, " (disabled)")
                    },
                    t.get(r.weight_name_id, "?")
                )
            })
            .collect();
        let sel = o.ind_sel.min(opts::INDUSTRY_TYPES - 1);
        c.list(20.0, y + 30.0, 400.0, 400.0, items, sel, A_IND_LIST);
        let allowed = opts::industry_row(s, &d.image, sel).is_some_and(|r| r.allowed);
        c.toggle(
            440.0,
            y + 30.0,
            320.0,
            &t.get(509, "Allow this building type for this map"),
            A_IND_TOGGLE,
            allowed,
        );
        c.label(
            440.0,
            y + 64.0,
            320.0,
            &format!(
                "{} {}",
                opts::industry_allowed_count(s),
                t.a("types allowed")
            ),
            MUTED,
        );
    }

    // ---- actions --------------------------------------------------------------------------

    fn mark_options_dirty(&mut self) {
        if let Some(e) = self.ed_mut() {
            e.dirty = true;
        }
    }

    fn opt_with_save(&mut self, f: impl FnOnce(&mut rt2_formats::map::Save)) {
        if let Screen::Game(g) = &mut self.screen
            && let Some(s) = g.world.editor_save_mut()
        {
            f(s);
        }
        self.mark_options_dirty();
    }

    /// The buttons of the new pages; true when `a` was one of them.
    pub(super) fn options_page_action(&mut self, a: u32, o: &mut OptionsState) -> bool {
        if (A_SPEC_BASE..A_SPEC_END).contains(&a) {
            return self.spec_page_action(a, o);
        }
        if !(A_OPT_BASE..A_OPT_END).contains(&a) {
            return false;
        }
        let sel = self.selected_player(o);
        match a {
            A_PLR_LIST => {}
            A_PLR_ADD => {
                self.player_edit(o, |w, d, t| w.editor_player_add(d, t).map(|_| ()));
                let n: usize = self.opt_save().map_or(0, |s| opts::player_slots(s).len());
                o.player_sel = n.saturating_sub(1);
            }
            A_PLR_DEL => {
                if let Some(id) = sel {
                    self.player_edit(o, |w, _, _| {
                        let s = w.editor_save_mut().ok_or(OptError::NoSuchPlayer)?;
                        opts::remove_player(s, id)
                    });
                    o.player_sel = o.player_sel.saturating_sub(1);
                }
            }
            A_PLR_PREV | A_PLR_NEXT => {
                if let Some(id) = sel {
                    let d = if a == A_PLR_PREV { -1 } else { 1 };
                    self.player_edit(o, |w, im, t| w.editor_player_step(im, t, id, d));
                }
            }
            A_PLR_MAND | A_PLR_OPT => {
                if let Some(id) = sel {
                    let m = a == A_PLR_MAND;
                    self.player_edit(o, |w, _, _| {
                        let s = w.editor_save_mut().ok_or(OptError::NoSuchPlayer)?;
                        opts::set_mandatory(s, id, m)
                    });
                }
            }
            A_PLR_HUMAN | A_PLR_COMP | A_PLR_EITHER => {
                if let Some(id) = sel {
                    let c = match a {
                        A_PLR_HUMAN => Controller::Human,
                        A_PLR_COMP => Controller::Computer,
                        _ => Controller::Either,
                    };
                    self.player_edit(o, |w, _, _| {
                        let s = w.editor_save_mut().ok_or(OptError::NoSuchPlayer)?;
                        opts::set_controller(s, id, c)
                    });
                }
            }
            A_PLR_OVR => {
                if let Some(id) = sel {
                    let on = self
                        .opt_save()
                        .and_then(|s| opts::player_info(s, id))
                        .is_some_and(|p| p.allow_override);
                    self.player_edit(o, |w, _, _| {
                        let s = w.editor_save_mut().ok_or(OptError::NoSuchPlayer)?;
                        opts::set_override(s, id, !on)
                    });
                }
            }
            x if (A_POOL_PRESET..A_POOL_PRESET + 3).contains(&x) => {
                let g = (x - A_POOL_PRESET) as u8;
                if let Some(d) = self.data.as_ref().map(|d| d.image.clone()) {
                    self.opt_with_save(|s| opts::pool_preset(s, &d, g));
                }
            }
            x if (A_POOL_ITEM..A_POOL_ITEM + 40).contains(&x) => {
                let p = (x - A_POOL_ITEM) as usize + 1;
                self.opt_with_save(|s| {
                    let on = opts::pool_allowed(s, p);
                    opts::set_pool_allowed(s, p, !on);
                });
            }
            x if (A_MGR_ITEM..A_MGR_ITEM + 40).contains(&x) => {
                let m = (x - A_MGR_ITEM) as usize;
                self.opt_with_save(|s| {
                    let on = opts::manager_allowed(s, m);
                    opts::set_manager_allowed(s, m, !on);
                });
            }
            x if (A_ENG_PRESET..A_ENG_PRESET + 3).contains(&x) => {
                let col = (x - A_ENG_PRESET) as usize;
                if let Some(d) = self.data.as_ref().map(|d| d.image.clone()) {
                    self.opt_with_save(|s| opts::engine_preset(s, &d, col));
                }
            }
            x if (A_ENG_ITEM..A_ENG_ITEM + 66).contains(&x) => {
                let e = (x - A_ENG_ITEM) as usize;
                self.opt_with_save(|s| {
                    let on = opts::engine_allowed(s, e);
                    opts::set_engine_allowed(s, e, !on);
                });
            }
            x if (A_PORT_ANNUAL..A_PORT_ANNUAL + 4).contains(&x) => {
                let k = (x - A_PORT_ANNUAL) as usize;
                self.opt_with_save(|s| opts::set_port_annual(s, k, true));
                o.typing = None;
            }
            x if (A_PORT_MANUF..A_PORT_MANUF + 4).contains(&x) => {
                let k = (x - A_PORT_MANUF) as usize;
                self.opt_with_save(|s| opts::set_port_annual(s, k, false));
                o.typing = None;
            }
            x if (A_PORT_SUP..A_PORT_SUP + 8).contains(&x) => {
                let (k, d) = (
                    ((x - A_PORT_SUP) / 2) as usize,
                    if x % 2 == 0 { -1 } else { 1 },
                );
                self.opt_with_save(|s| {
                    if let Some(p) = opts::port(s, k) {
                        opts::set_port_supplied(s, k, step_value(&supply_values(), p.supplied, d));
                    }
                });
            }
            x if (A_PORT_DEM..A_PORT_DEM + 8).contains(&x) => {
                let (k, d) = (
                    ((x - A_PORT_DEM) / 2) as usize,
                    if x % 2 == 0 { -1 } else { 1 },
                );
                let cargos: Vec<u8> = (0..opts::PORT_CARGOS as u8).collect();
                self.opt_with_save(|s| {
                    if let Some(p) = opts::port(s, k) {
                        opts::set_port_demanded(s, k, step_value(&cargos, p.demanded, d));
                    }
                });
            }
            x if (A_ROB_LEVEL..A_ROB_LEVEL + 2).contains(&x) => {
                let d: i64 = if x == A_ROB_LEVEL { -10 } else { 10 };
                self.opt_with_save(|s| {
                    let v = (i64::from(opts::robbery_level(s)) + d)
                        .clamp(0, i64::from(opts::ROBBERY_MAX));
                    opts::set_robbery_level(s, v as u32);
                });
                o.typing = None;
            }
            _ => return false,
        }
        true
    }

    fn spec_page_action(&mut self, a: u32, o: &mut OptionsState) -> bool {
        match a {
            x if (A_SPECIAL..A_SPECIAL + 10).contains(&x) => {
                let f = opts::Special::ALL[(x - A_SPECIAL) as usize];
                self.opt_with_save(|s| opts::toggle_special(s, f));
            }
            x if (A_START_OFF..A_START_OFF + 2).contains(&x) => {
                let d: i64 = if x == A_START_OFF { -10 } else { 10 };
                self.opt_with_save(|s| {
                    let v = (i64::from(opts::start_offset_days(s)) + d)
                        .clamp(0, i64::from(opts::START_OFFSET_MAX));
                    opts::set_start_offset_days(s, v as u32);
                });
                o.typing = None;
            }
            x if (A_COST..A_COST + 9).contains(&x) => {
                let (row, level) = (((x - A_COST) / 3) as usize, ((x - A_COST) % 3) as u8);
                self.opt_with_save(|s| opts::set_cost_level(s, row, level));
            }
            x if (A_TRAFFIC..A_TRAFFIC + 3).contains(&x) => {
                let level = (x - A_TRAFFIC) as u8;
                self.opt_with_save(|s| opts::set_traffic_level(s, level));
            }
            x if (A_AI_TRACK..A_AI_TRACK + 3).contains(&x) => {
                let v = 2 - (x - A_AI_TRACK) as u8;
                self.opt_with_save(|s| opts::set_ai_track(s, v));
            }
            x if (A_AI_DIV..A_AI_DIV + 3).contains(&x) => {
                let v = (x - A_AI_DIV) as u8;
                self.opt_with_save(|s| opts::set_ai_dividend(s, v));
            }
            A_IND_TOGGLE => {
                let i = o.ind_sel.min(opts::INDUSTRY_TYPES - 1);
                self.opt_with_save(|s| opts::toggle_industry(s, i));
            }
            A_IND_LIST => {}
            _ => return false,
        }
        true
    }

    /// Typed characters of the new fields (ids from 30); true when `id` is one of them.
    pub(super) fn options_edit_field(
        &mut self,
        o: &mut OptionsState,
        id: u32,
        f: impl FnOnce(&mut String),
    ) -> bool {
        let saved = |s: &rt2_formats::map::Save| -> String {
            match id {
                x if (F_ROB_NAME..F_ROB_NAME + 4).contains(&x) => {
                    opts::robber(s, (x - F_ROB_NAME) as usize)
                        .unwrap_or_default()
                        .0
                }
                x if (F_ROB_YEAR..F_ROB_YEAR + 4).contains(&x) => {
                    match opts::robber(s, (x - F_ROB_YEAR) as usize) {
                        Some((_, y)) if y != 0 => y.to_string(),
                        _ => String::new(),
                    }
                }
                F_ROB_LEVEL => opts::robbery_level(s).to_string(),
                F_START_OFFSET => opts::start_offset_days(s).to_string(),
                x if (F_PORT_Q..F_PORT_Q + 4).contains(&x) => {
                    opts::port(s, (x - F_PORT_Q) as usize)
                        .map_or_else(String::new, |p| (p.quantity as i32).to_string())
                }
                _ => String::new(),
            }
        };
        let known =
            (F_ROB_NAME..F_PORT_Q + 4).contains(&id) || id == F_CASH || id == F_START_OFFSET;
        if !known {
            return false;
        }
        let first = self.selected_player(o);
        let cur = match &o.typing {
            Some((t, s)) if *t == id => s.clone(),
            _ if id == F_CASH => first
                .and_then(|p| self.opt_save().and_then(|s| opts::player_info(s, p)))
                .map_or_else(String::new, |p| format!("{}", p.cash as i64)),
            _ => self.opt_save().map(saved).unwrap_or_default(),
        };
        let mut s = cur;
        f(&mut s);
        match id {
            x if (F_ROB_NAME..F_ROB_NAME + 4).contains(&x) => {
                s.truncate(opts::ROBBER_NAME_LEN);
                let k = (x - F_ROB_NAME) as usize;
                let n = s.clone();
                self.opt_with_save(|sv| opts::set_robber_name(sv, k, &n));
                o.typing = None;
            }
            x if (F_ROB_YEAR..F_ROB_YEAR + 4).contains(&x) => {
                s = digits(&s, 4);
                let k = (x - F_ROB_YEAR) as usize;
                let v = s.parse::<i32>().unwrap_or(0);
                if s.is_empty() || (1800..=2100).contains(&v) {
                    self.opt_with_save(|sv| opts::set_robber_year(sv, k, v));
                }
                o.typing = Some((id, s));
            }
            F_ROB_LEVEL => {
                s = digits(&s, 3);
                let v = s.parse::<u32>().unwrap_or(0);
                self.opt_with_save(|sv| opts::set_robbery_level(sv, v));
                o.typing = Some((id, s));
            }
            x if (F_PORT_Q..F_PORT_Q + 4).contains(&x) => {
                s = digits(&s, 2);
                let k = (x - F_PORT_Q) as usize;
                if let Ok(v) = s.parse::<i32>() {
                    self.opt_with_save(|sv| opts::set_port_quantity(sv, k, v));
                }
                o.typing = Some((id, s));
            }
            F_START_OFFSET => {
                s = digits(&s, 3);
                let v = s.parse::<u32>().unwrap_or(0);
                self.opt_with_save(|sv| opts::set_start_offset_days(sv, v));
                o.typing = Some((id, s));
            }
            F_CASH => {
                s = digits(&s, 9);
                if let (Some(p), Ok(v)) = (first, s.parse::<i64>()) {
                    self.opt_with_save(|sv| {
                        let _ = opts::set_cash(sv, p, v);
                    });
                }
                o.typing = Some((id, s));
            }
            _ => {}
        }
        true
    }
}
