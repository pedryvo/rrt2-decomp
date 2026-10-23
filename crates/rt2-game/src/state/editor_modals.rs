//! The editor's dialogs: New Map, Open, Save As, "discard changes?", Map Options (general,
//! victory goals, restrictions, economy, industries), the scenario Rules editor, the city /
//! region / label setup (`cityset.win`) and Territory Setup (`bpterrit`, `0x4c6e70`).
//!
//! Typed text goes to the focused field (`EditorState::focus`, `field id + 1`); everything else is
//! buttons, toggles and lists. A rule's condition is validated live through the scenario
//! compiler (`rt2_sim::scenario::edit::validate`), which also derives the loop flags the way
//! `rule_set_condition` does.

#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_sim::editor::{self, PlaceInfo, TerritoryInfo};
use rt2_sim::scenario::edit as sedit;
use rt2_sim::scenario::{MessageKind, Rule};
use rt2_ui::{Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Ui, game_font};
use winit::keyboard::KeyCode;

use super::editor_ui::{
    A_MODAL, BAD, GOOD, HEAD, MUTED, Modal, Pending, TEXT, at, design_scale, lbl,
};
use super::{AppState, Screen};
use crate::text::id as lng;

// Common.
const A_OK: u32 = A_MODAL;
const A_CANCEL: u32 = A_MODAL + 1;
const A_DELETE: u32 = A_MODAL + 2;
const A_FIELD: u32 = A_MODAL + 10; // + field id (0..30)
/// Fields 30.. (`F_ROB_NAME` ...): their actions are `A_FIELD2 + id - 30`.
const A_FIELD2: u32 = A_MODAL + 620;
// New map.
const A_NM_STEP: u32 = A_MODAL + 40; // +0..3: width -/+, height -/+
const A_NM_CLASS: u32 = A_MODAL + 50; // + slot
const A_NM_IMAGE: u32 = A_MODAL + 66;
const A_OPEN_LIST: u32 = A_MODAL + 70;
// Options.
const A_OPT_TAB: u32 = A_MODAL + 80;
const A_OPT_PERM: u32 = A_MODAL + 100; // + 0..22
const A_OPT_FIN: u32 = A_MODAL + 130;
const A_OPT_IND: u32 = A_MODAL + 133;
const A_OPT_SET: u32 = A_MODAL + 136;
const A_OPT_YEAR: u32 = A_MODAL + 140; // -10, -1, +1, +10
const A_OPT_CASH: u32 = A_MODAL + 144; // -, +
/// First action of the Players, pool, managers, engines, ports and robbers pages.
pub(super) const A_OPT_BASE: u32 = A_MODAL + 400;
/// Tabs of Map Options (`OPT_TABS`).
pub(super) const A_OPT_TOTAL: usize = 11;
// Rules.
const A_RULE_LIST: u32 = A_MODAL + 200;
const A_RULE_ADD: u32 = A_MODAL + 210;
const A_RULE_DEL: u32 = A_MODAL + 211;
const A_RULE_UP: u32 = A_MODAL + 212;
const A_RULE_DOWN: u32 = A_MODAL + 213;
const A_RULE_DUP: u32 = A_MODAL + 214;
const A_RULE_TRIG: u32 = A_MODAL + 220; // + 0..8
const A_RULE_KIND: u32 = A_MODAL + 230; // + 0..3
const A_RULE_FLAG: u32 = A_MODAL + 235; // + 0..9
const A_RULE_SET: u32 = A_MODAL + 246; // + 0..2
const A_EFF_LIST: u32 = A_MODAL + 250;
const A_EFF_MINUS: u32 = A_MODAL + 251;
const A_EFF_PLUS: u32 = A_MODAL + 252;
const A_EFF_REMOVE: u32 = A_MODAL + 253;
const A_EFF_ADD: u32 = A_MODAL + 254;
const A_EFF_PICK: u32 = A_MODAL + 255;
const A_EFF_PERM: u32 = A_MODAL + 256;
const A_EFF_DUR: u32 = A_MODAL + 257; // -, +
const A_EFF_BIG: u32 = A_MODAL + 259; // -10, +10
// Place.
const A_PL_TAB: u32 = A_MODAL + 300;
const A_PL_KIND: u32 = A_MODAL + 303;
const A_PL_SIZE: u32 = A_MODAL + 306;
const A_PL_STYLE: u32 = A_MODAL + 312;
const A_PL_TREND: u32 = A_MODAL + 320;
const A_PL_LIST: u32 = A_MODAL + 324;
const A_PL_W: u32 = A_MODAL + 325; // -10, -1, +1, +10
const A_PL_ALL0: u32 = A_MODAL + 330;
const A_PL_ALL100: u32 = A_MODAL + 331;
// Territory.
const A_TD_TOGGLE: u32 = A_MODAL + 340; // + 0..3: visible, remap click, separate rights
const A_TD_GOOD: u32 = A_MODAL + 344; // -, +
const A_TD_PARENT: u32 = A_MODAL + 346;
const A_TD_RIGHT: u32 = A_MODAL + 350; // + 0..4
const A_TD_SALE: u32 = A_MODAL + 355; // + 0..4

/// Field ids.
const F_NAME: u32 = 0;
const F_DESC: u32 = 1;
const F_COND: u32 = 2;
const F_MSG: u32 = 3;
const F_LABEL1: u32 = 4;
const F_LABEL2: u32 = 5;
const F_TITLE: u32 = 6;
const F_GOAL: u32 = 10; // + 0..4 conditions
const F_GOAL_MSG: u32 = 20; // + 0..4 messages
pub(super) const F_ROB_NAME: u32 = 30; // + 0..4
pub(super) const F_ROB_YEAR: u32 = 34; // + 0..4
pub(super) const F_ROB_LEVEL: u32 = 38;
pub(super) const F_CASH: u32 = 39;
pub(super) const F_PORT_Q: u32 = 40; // + 0..4
const F_IMG: u32 = 44;
const F_SCALE: u32 = 45;

/// The tabs of Map Options in order; `--tool` names of the screenshots and tests.
pub(super) const OPT_TABS: [&str; A_OPT_TOTAL] = [
    "options",
    "goals",
    "players",
    "pool",
    "managers",
    "economy",
    "engines",
    "industries",
    "restrictions",
    "ports",
    "robbers",
];
const T_GENERAL: usize = 0;
const T_GOALS: usize = 1;
const T_PLAYERS: usize = 2;
const T_POOL: usize = 3;
const T_MANAGERS: usize = 4;
const T_ECONOMY: usize = 5;
const T_ENGINES: usize = 6;
const T_INDUSTRIES: usize = 7;
const T_RESTRICTIONS: usize = 8;
const T_PORTS: usize = 9;
const T_ROBBERS: usize = 10;

/// The action of text field `id`.
fn field_action(id: u32) -> u32 {
    if id < 30 {
        A_FIELD + id
    } else {
        A_FIELD2 + id - 30
    }
}

#[derive(Clone, Debug)]
pub(super) struct NewMap {
    pub name: String,
    pub w: usize,
    pub h: usize,
    pub class_slot: usize,
    /// From Image: the PCX file and the height scale as typed.
    pub image: String,
    pub scale: String,
}

#[derive(Clone, Debug)]
pub(super) struct OpenList {
    pub entries: Vec<(String, PathBuf)>,
    pub sel: usize,
}

#[derive(Clone, Debug)]
pub(super) struct OptionsState {
    pub tab: usize,
    /// The text of each goal condition as typed (it may not compile yet).
    pub goal_cond: [String; 4],
    /// Selected row of the Players list.
    pub player_sel: usize,
    /// The numeric field being typed into (its id and text): applied to the map as soon as it
    /// is valid, shown as typed while it has the focus.
    pub typing: Option<(u32, String)>,
    /// The last refusal of the Players page.
    pub note: String,
}

#[derive(Clone, Debug)]
pub(super) struct RulesState {
    pub sel: usize,
    /// The condition as typed.
    pub cond: String,
    /// Effect set being edited (0 first button or only set, 1 second button).
    pub set: usize,
    pub eff_sel: usize,
    pub picking: bool,
    pub pick_sel: usize,
    /// The map as it was when the dialog opened (Cancel restores it).
    pub before: Box<Save>,
}

#[derive(Clone, Debug)]
pub(super) struct PlaceDlg {
    pub info: PlaceInfo,
    pub is_new: bool,
    pub tab: usize,
    pub wsel: usize,
}

#[derive(Clone, Debug)]
pub(super) struct TerritoryDlg {
    pub info: TerritoryInfo,
    pub is_new: bool,
}

fn fnt(n: u8, dz: f32) -> FontStyle {
    let f = game_font(n);
    f.with_size((f.size * dz).round())
}

const TRIGGERS: [&str; 8] = [
    "Start of year",
    "End of year",
    "Start of month",
    "End of month",
    "Track or station changed",
    "Company created",
    "Briefing",
    "Status line",
];
const GOALS: [&str; 4] = ["Bronze", "Silver", "Gold", "Defeat"];
fn opts_scale_default() -> String {
    rt2_sim::editor_pcx::SCALE_DEFAULT.to_string()
}
const STYLE_BYTES: [u8; 5] = [0, 1, 2, 3, 5];
const SIZES: [&str; 5] = ["Village", "Town", "City", "Metropolis", "Megalopolis"];

impl AppState {
    // ---- opening ----------------------------------------------------------------------------

    pub(super) fn editor_new_dialog(&mut self) {
        let name = self.texts.a("New map");
        self.set_modal(Modal::NewMap(NewMap {
            name,
            w: 120,
            h: 120,
            class_slot: 5,
            image: "EXAMPLE.PCX".into(),
            scale: opts_scale_default(),
        }));
        self.set_focus(F_NAME + 1);
    }

    pub(super) fn editor_open_dialog(&mut self) {
        let mut entries: Vec<(String, PathBuf)> = Vec::new();
        if let Some(d) = self.own_maps_dir()
            && let Ok(rd) = std::fs::read_dir(d)
        {
            let mut own: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("mp2"))
                })
                .collect();
            own.sort();
            for p in own {
                let n = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                entries.push((format!("* {n}"), p));
            }
        }
        for e in crate::catalog::list_maps(&self.install) {
            entries.push((e.title.clone(), e.path));
        }
        self.set_modal(Modal::Open(OpenList { entries, sel: 0 }));
    }

    pub(super) fn editor_save_as_dialog(&mut self) {
        let name = self
            .game()
            .map(|g| g.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "map".into());
        self.set_modal(Modal::SaveAs(name));
        self.set_focus(F_NAME + 1);
    }

    pub(super) fn editor_confirm(&mut self, p: Pending) {
        self.set_modal(Modal::Confirm(p));
    }

    pub(super) fn editor_options_dialog(&mut self) {
        let goals = self.read_goal_conditions();
        self.set_modal(Modal::Options(OptionsState {
            tab: 0,
            goal_cond: goals,
            player_sel: 0,
            typing: None,
            note: String::new(),
        }));
        self.set_focus(0);
    }

    pub(super) fn editor_rules_dialog(&mut self) {
        let Some(before) = self.editor_world().and_then(|w| w.editor_save()).cloned() else {
            return;
        };
        let st = RulesState {
            sel: 0,
            cond: String::new(),
            set: 0,
            eff_sel: 0,
            picking: false,
            pick_sel: 0,
            before: Box::new(before),
        };
        self.set_modal(Modal::Rules(Box::new(st)));
        self.rules_select(0);
    }

    pub(super) fn editor_place_dialog(&mut self, id: u16, is_new: bool) {
        let Some(info) = self.editor_world().and_then(|w| w.editor_place(id)) else {
            return;
        };
        self.set_modal(Modal::Place(Box::new(PlaceDlg {
            info,
            is_new,
            tab: 0,
            wsel: 0,
        })));
        self.set_focus(F_NAME + 1);
    }

    pub(super) fn editor_territory_new(&mut self) {
        let id = self.editor_world().map_or(1, |w| {
            (1u8..=31)
                .find(|i| !w.editor_territories().iter().any(|t| t.id == *i))
                .unwrap_or(31)
        });
        let info = TerritoryInfo {
            id,
            name: self.texts.get(0x1d, "Unnamed"),
            goodwill: 0,
            visible: true,
            merged_flag: true,
            parent: id,
            separate_rights: false,
            rights: [true; 4],
            for_sale: [true; 4],
        };
        self.set_modal(Modal::Territory(Box::new(TerritoryDlg {
            info,
            is_new: true,
        })));
        self.set_focus(F_NAME + 1);
    }

    pub(super) fn editor_territory_edit(&mut self) {
        let Some(sel) = self.ed().map(|e| e.terr_sel) else {
            return;
        };
        let Some(info) = self
            .editor_world()
            .and_then(|w| w.editor_territories().get(sel).cloned())
        else {
            return;
        };
        self.set_modal(Modal::Territory(Box::new(TerritoryDlg {
            info,
            is_new: false,
        })));
        self.set_focus(F_NAME + 1);
    }

    fn set_modal(&mut self, m: Modal) {
        if let Some(e) = self.ed_mut() {
            e.modal = Some(m);
            e.focus = 0;
        }
        self.rebuild_ui();
    }

    fn set_focus(&mut self, f: u32) {
        if let Some(e) = self.ed_mut() {
            e.focus = f;
        }
    }

    fn close_modal(&mut self) {
        if let Some(e) = self.ed_mut() {
            e.modal = None;
            e.focus = 0;
        }
        self.rebuild_ui();
    }

    // ---- text input -------------------------------------------------------------------------

    pub(super) fn editor_modal_key(&mut self, code: KeyCode, text: Option<&str>) -> bool {
        let focus = self.ed().map_or(0, |e| e.focus);
        if code == KeyCode::Escape {
            if focus != 0 {
                self.set_focus(0);
                self.rebuild_ui();
            } else {
                self.editor_modal_cancel();
            }
            return true;
        }
        if focus == 0 {
            if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
                self.editor_modal_activated(A_OK);
            }
            return true;
        }
        let id = focus - 1;
        let multi = matches!(id, F_DESC | F_MSG);
        match code {
            KeyCode::Backspace => {
                self.edit_field(id, |s| {
                    s.pop();
                });
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                if multi {
                    self.edit_field(id, |s| s.push('\n'));
                } else {
                    self.set_focus(0);
                }
            }
            _ => {
                if let Some(t) = text {
                    let add: String = t.chars().filter(|c| (' '..='~').contains(c)).collect();
                    if !add.is_empty() {
                        self.edit_field(id, |s| s.push_str(&add));
                    }
                }
            }
        }
        self.rebuild_ui();
        true
    }

    /// Edits the text of field `id` of the open dialog, and applies it to the map.
    fn edit_field(&mut self, id: u32, f: impl FnOnce(&mut String)) {
        let Some(m) = self.ed().and_then(|e| e.modal.clone()) else {
            return;
        };
        match m {
            Modal::NewMap(mut n) => {
                match id {
                    F_IMG => {
                        f(&mut n.image);
                        n.image.truncate(200);
                    }
                    F_SCALE => {
                        f(&mut n.scale);
                        n.scale = n
                            .scale
                            .chars()
                            .filter(char::is_ascii_digit)
                            .take(4)
                            .collect();
                    }
                    _ => {
                        f(&mut n.name);
                        n.name.truncate(30);
                    }
                }
                self.replace_modal(Modal::NewMap(n));
            }
            Modal::SaveAs(mut s) => {
                f(&mut s);
                s.truncate(40);
                self.replace_modal(Modal::SaveAs(s));
            }
            Modal::Place(mut p) => {
                f(&mut p.info.name);
                p.info.name.truncate(21);
                self.replace_modal(Modal::Place(p));
            }
            Modal::Territory(mut t) => {
                f(&mut t.info.name);
                t.info.name.truncate(30);
                self.replace_modal(Modal::Territory(t));
            }
            Modal::Options(mut o) => {
                if id >= 30 {
                    if self.options_edit_field(&mut o, id, f) {
                        self.replace_modal(Modal::Options(o));
                        self.mark_dirty();
                    }
                    return;
                }
                if id == F_NAME {
                    let mut s = self.map_text(true);
                    f(&mut s);
                    s.truncate(60);
                    self.with_save(|sv| editor::set_map_name(sv, &s));
                } else if id == F_DESC {
                    let mut s = self.map_text(false);
                    f(&mut s);
                    s.truncate(299);
                    self.with_save(|sv| editor::set_map_description(sv, &s));
                } else if (F_GOAL..F_GOAL + 4).contains(&id) {
                    let k = (id - F_GOAL) as usize;
                    f(&mut o.goal_cond[k]);
                    let cond = o.goal_cond[k].clone();
                    self.apply_goal_condition(k, &cond);
                    self.replace_modal(Modal::Options(o));
                } else if (F_GOAL_MSG..F_GOAL_MSG + 4).contains(&id) {
                    let k = (id - F_GOAL_MSG) as usize;
                    let mut s = self.goal_message(k);
                    f(&mut s);
                    self.apply_goal_message(k, &s);
                }
                self.mark_dirty();
            }
            Modal::Rules(mut r) => {
                self.rules_edit_text(&mut r, id, f);
                self.replace_modal(Modal::Rules(r));
                self.mark_dirty();
            }
            _ => {}
        }
    }

    fn replace_modal(&mut self, m: Modal) {
        if let Some(e) = self.ed_mut() {
            e.modal = Some(m);
        }
    }

    fn mark_dirty(&mut self) {
        if let Some(e) = self.ed_mut() {
            e.dirty = true;
        }
    }

    fn with_save<R>(&mut self, f: impl FnOnce(&mut Save) -> R) -> Option<R> {
        match &mut self.screen {
            Screen::Game(g) => g.world.editor_save_mut().map(f),
            Screen::Menu(_) => None,
        }
    }

    fn map_text(&self, name: bool) -> String {
        self.editor_world()
            .and_then(|w| w.editor_save())
            .map(|s| {
                if name {
                    editor::map_name(s)
                } else {
                    editor::map_description(s)
                }
            })
            .unwrap_or_default()
    }

    // ---- rules and goals --------------------------------------------------------------------

    fn rules(&self) -> Vec<Rule> {
        self.editor_world()
            .and_then(|w| w.editor_save())
            .map(|s| s.rules.iter().map(Rule::from_file).collect())
            .unwrap_or_default()
    }

    fn store_rule(&mut self, i: usize, rule: &Rule) {
        self.with_save(|s| {
            if let Some(raw) = s.rules.get(i) {
                let b = sedit::build(Some(raw), rule);
                s.rules[i] = b;
            }
        });
        self.mark_dirty();
    }

    fn rules_select(&mut self, i: usize) {
        let rules = self.rules();
        let cond = rules
            .get(i)
            .map(|r| r.condition.clone())
            .unwrap_or_default();
        if let Some(Modal::Rules(r)) = self.ed_mut().and_then(|e| e.modal.as_mut()) {
            r.sel = i;
            r.cond = cond;
            r.eff_sel = 0;
            r.picking = false;
            r.set = 0;
        }
        self.rebuild_ui();
    }

    fn rules_edit_text(&mut self, r: &mut RulesState, id: u32, f: impl FnOnce(&mut String)) {
        let mut rules = self.rules();
        let Some(rule) = rules.get_mut(r.sel) else {
            return;
        };
        match id {
            F_COND => {
                f(&mut r.cond);
                let text = r.cond.clone();
                if sedit::set_condition(rule, &text).is_ok() {
                    let rule = rule.clone();
                    self.store_rule(r.sel, &rule);
                }
            }
            F_MSG => {
                f(&mut rule.message);
                let rule = rule.clone();
                self.store_rule(r.sel, &rule);
            }
            F_LABEL1 => {
                f(&mut rule.label1);
                let rule = rule.clone();
                self.store_rule(r.sel, &rule);
            }
            F_LABEL2 => {
                f(&mut rule.label2);
                let rule = rule.clone();
                self.store_rule(r.sel, &rule);
            }
            F_TITLE => {
                f(&mut rule.title);
                let rule = rule.clone();
                self.store_rule(r.sel, &rule);
            }
            _ => {}
        }
    }

    /// The rule that sets goal `k` (game over effect `k + 1` in effect set 0), if any.
    fn goal_rule(rules: &[Rule], k: usize) -> Option<usize> {
        rules
            .iter()
            .position(|r| r.effects[0].get(k + 1).is_some_and(|&b| b != 0))
    }

    fn read_goal_conditions(&self) -> [String; 4] {
        let rules = self.rules();
        std::array::from_fn(|k| {
            Self::goal_rule(&rules, k)
                .map(|i| rules[i].condition.clone())
                .unwrap_or_default()
        })
    }

    fn goal_message(&self, k: usize) -> String {
        let rules = self.rules();
        Self::goal_rule(&rules, k)
            .map(|i| rules[i].message.clone())
            .unwrap_or_default()
    }

    /// Writes a goal condition: an empty text removes the rule, a valid one creates or updates
    /// it (trigger "end of month", once only, the game-over effect).
    fn apply_goal_condition(&mut self, k: usize, cond: &str) {
        let rules = self.rules();
        let at = Self::goal_rule(&rules, k);
        if cond.trim().is_empty() {
            if let Some(i) = at {
                self.with_save(|s| {
                    s.rules.remove(i);
                });
            }
            return;
        }
        if sedit::validate(cond).is_err() {
            return;
        }
        match at {
            Some(i) => {
                let mut r = rules[i].clone();
                if sedit::set_condition(&mut r, cond).is_ok() {
                    self.store_rule(i, &r);
                }
            }
            None => {
                let mut r = sedit::new_rule();
                if sedit::set_condition(&mut r, cond).is_ok() {
                    r.effects[0][k + 1] = 1;
                    r.title = GOALS[k].to_owned();
                    // Losing is checked at the end of each month and victory at its end too;
                    // the shipped scenarios use triggers 1 and 3 (scenarios.md 5).
                    r.trigger = if k == 3 { 2 } else { 3 };
                    let raw = sedit::build(None, &r);
                    self.with_save(|s| s.rules.push(raw));
                }
            }
        }
    }

    fn apply_goal_message(&mut self, k: usize, text: &str) {
        let rules = self.rules();
        if let Some(i) = Self::goal_rule(&rules, k) {
            let mut r = rules[i].clone();
            r.message = text.to_owned();
            self.store_rule(i, &r);
        }
    }

    // ---- activation -------------------------------------------------------------------------

    pub(super) fn editor_modal_cancel(&mut self) {
        let Some(m) = self.ed().and_then(|e| e.modal.clone()) else {
            return;
        };
        match m {
            Modal::Rules(r) => {
                self.with_restore(*r.before);
                self.close_modal();
            }
            Modal::Place(p) if p.is_new => {
                let _ = self.editor_world_mut().editor_place_remove(p.info.id);
                if let Some(e) = self.ed_mut() {
                    e.undo.pop();
                }
                self.close_modal();
                self.editor_changed(false);
            }
            _ => self.close_modal(),
        }
    }

    fn editor_world_mut(&mut self) -> &mut rt2_sim::World {
        match &mut self.screen {
            Screen::Game(g) => &mut g.world,
            Screen::Menu(_) => unreachable!("the editor is open"),
        }
    }

    fn with_restore(&mut self, s: Save) {
        self.editor_world_mut().editor_restore(s);
    }

    pub(super) fn editor_modal_row(&mut self, action: u32, row: usize) -> bool {
        match action {
            A_OPEN_LIST => {
                if let Some(Modal::Open(o)) = self.ed_mut().and_then(|e| e.modal.as_mut()) {
                    o.sel = row;
                }
            }
            A_RULE_LIST => self.rules_select(row),
            A_EFF_LIST => {
                if let Some(Modal::Rules(r)) = self.ed_mut().and_then(|e| e.modal.as_mut()) {
                    if r.picking {
                        r.pick_sel = row;
                    } else {
                        r.eff_sel = row;
                    }
                }
            }
            super::editor_options_ui::A_PLR_LIST => {
                if let Some(Modal::Options(o)) = self.ed_mut().and_then(|e| e.modal.as_mut()) {
                    o.player_sel = row;
                    o.note.clear();
                    o.typing = None;
                }
            }
            A_PL_LIST => {
                if let Some(Modal::Place(p)) = self.ed_mut().and_then(|e| e.modal.as_mut()) {
                    p.wsel = row;
                }
            }
            _ => return false,
        }
        self.rebuild_ui();
        true
    }

    pub(super) fn editor_modal_activated(&mut self, action: u32) -> bool {
        if (A_FIELD..A_FIELD + 30).contains(&action) || (A_FIELD2..A_FIELD2 + 30).contains(&action)
        {
            let id = if action >= A_FIELD2 {
                action - A_FIELD2 + 30
            } else {
                action - A_FIELD
            };
            self.set_focus(id + 1);
            self.rebuild_ui();
            return true;
        }
        let Some(m) = self.ed().and_then(|e| e.modal.clone()) else {
            return false;
        };
        match m {
            Modal::NewMap(n) => self.act_new_map(action, n),
            Modal::Open(o) => self.act_open(action, o),
            Modal::SaveAs(name) => self.act_save_as(action, name),
            Modal::Confirm(p) => self.act_confirm(action, p),
            Modal::Options(o) => self.act_options(action, o),
            Modal::Rules(r) => self.act_rules(action, *r),
            Modal::Place(p) => self.act_place(action, *p),
            Modal::Territory(t) => self.act_territory(action, *t),
        }
        true
    }

    fn act_new_map(&mut self, a: u32, mut n: NewMap) {
        match a {
            A_OK => {
                self.close_modal();
                self.editor_new_map(
                    n.w,
                    n.h,
                    usize::from(editor::SOLID_CLASSES[n.class_slot]),
                    &n.name,
                );
                return;
            }
            A_CANCEL => {
                self.close_modal();
                return;
            }
            A_NM_IMAGE => {
                self.new_map_from_image(n);
                return;
            }
            x if (A_NM_STEP..A_NM_STEP + 4).contains(&x) => {
                let d: isize = if (x - A_NM_STEP).is_multiple_of(2) {
                    -10
                } else {
                    10
                };
                let v = if x - A_NM_STEP < 2 {
                    &mut n.w
                } else {
                    &mut n.h
                };
                *v = (*v as isize + d).clamp(20, 500) as usize;
            }
            x if (A_NM_CLASS..A_NM_CLASS + 16).contains(&x) => {
                n.class_slot = (x - A_NM_CLASS) as usize
            }
            _ => {}
        }
        self.replace_modal(Modal::NewMap(n));
        self.rebuild_ui();
    }

    /// New Map > From Image (`0x4694f0`): reads the PCX (as typed, else next to the game's data),
    /// builds the map and gives it the heights of the image (`0x469670`).
    fn new_map_from_image(&mut self, n: NewMap) {
        let typed = PathBuf::from(n.image.trim());
        let candidates = [typed.clone(), self.install.join(&typed)];
        let img = candidates.iter().find_map(|p| {
            std::fs::read(p)
                .ok()
                .and_then(|raw| rt2_sim::editor_pcx::Pcx::parse(&raw).ok())
        });
        let Some(img) = img else {
            let m = self
                .texts
                .a("The image could not be read (an indexed or 24-bit PCX is needed).");
            self.close_modal();
            self.editor_new_dialog();
            self.ed_msg(m, false);
            return;
        };
        let scale = n
            .scale
            .parse::<i32>()
            .unwrap_or(rt2_sim::editor_pcx::SCALE_DEFAULT);
        self.close_modal();
        self.editor_new_map(n.w, n.h, rt2_sim::editor_pcx::IMPORT_CLASS, &n.name);
        self.editor_world_mut().editor_import_image(&img, scale);
        self.editor_changed(true);
    }

    fn act_open(&mut self, a: u32, o: OpenList) {
        match a {
            A_OK | A_OPEN_LIST => {
                if let Some((_, p)) = o.entries.get(o.sel) {
                    let p = p.clone();
                    self.close_modal();
                    self.editor_open_file(&p);
                }
            }
            _ => self.close_modal(),
        }
    }

    fn act_save_as(&mut self, a: u32, name: String) {
        if a != A_OK {
            self.close_modal();
            return;
        }
        let stem = crate::saves::sanitize_stem(&name);
        let Some(dir) = self.own_maps_dir() else {
            self.close_modal();
            return;
        };
        let path = dir.join(format!("{stem}.MP2"));
        self.close_modal();
        self.with_save(|s| editor::set_map_name(s, &name));
        let r = self.editor_write(&path);
        self.editor_saved(r);
    }

    fn act_confirm(&mut self, a: u32, p: Pending) {
        self.close_modal();
        if a != A_OK {
            return;
        }
        match p {
            Pending::New => self.editor_new_dialog(),
            Pending::Open => self.editor_open_dialog(),
            Pending::Exit => self.editor_leave(),
        }
    }

    fn act_options(&mut self, a: u32, mut o: OptionsState) {
        if self.options_page_action(a, &mut o) {
            self.replace_modal(Modal::Options(o));
            self.rebuild_ui();
            return;
        }
        match a {
            A_OK | A_CANCEL => {
                self.close_modal();
                return;
            }
            x if (A_OPT_TAB..A_OPT_TAB + A_OPT_TOTAL as u32).contains(&x) => {
                o.tab = (x - A_OPT_TAB) as usize;
                o.typing = None;
                o.note.clear();
                self.set_focus(0);
            }
            x if (A_OPT_PERM..A_OPT_PERM + 22).contains(&x) => {
                let i = (x - A_OPT_PERM) as usize;
                self.with_save(|s| {
                    let on = editor::permission(s, i);
                    editor::set_permission(s, i, !on);
                });
                self.mark_dirty();
            }
            x if (A_OPT_FIN..A_OPT_FIN + 3).contains(&x) => {
                self.with_save(|s| editor::set_financial_model(s, (x - A_OPT_FIN) as u8));
                self.mark_dirty();
            }
            x if (A_OPT_IND..A_OPT_IND + 3).contains(&x) => {
                self.with_save(|s| editor::set_industrial_model(s, (x - A_OPT_IND) as u8));
                self.mark_dirty();
            }
            x if (A_OPT_SET..A_OPT_SET + 2).contains(&x) => {
                self.with_save(|s| editor::set_industry_set(s, (x - A_OPT_SET) as u8));
                self.mark_dirty();
            }
            x if (A_OPT_YEAR..A_OPT_YEAR + 4).contains(&x) => {
                let d = [-10i32, -1, 1, 10][(x - A_OPT_YEAR) as usize];
                self.with_save(|s| {
                    let y = (i32::from(editor::start_year(s)) + d).clamp(1700, 2100);
                    editor::set_start_year(s, y as u16);
                });
                self.mark_dirty();
            }
            x if (A_OPT_CASH..A_OPT_CASH + 2).contains(&x) => {
                let up = x == A_OPT_CASH + 1;
                self.with_save(|s| {
                    let c = editor::player_cash(s);
                    let step = if c >= 1_000_000.0 {
                        500_000.0
                    } else {
                        50_000.0
                    };
                    let v = (c + if up { step } else { -step }).clamp(0.0, 50_000_000.0);
                    editor::set_player_cash(s, v);
                });
                self.mark_dirty();
            }
            _ => {}
        }
        self.replace_modal(Modal::Options(o));
        self.rebuild_ui();
    }

    fn act_rules(&mut self, a: u32, mut r: RulesState) {
        let mut rules = self.rules();
        let n = rules.len();
        match a {
            A_OK => {
                self.close_modal();
                return;
            }
            A_CANCEL => {
                self.editor_modal_cancel();
                return;
            }
            A_RULE_ADD => {
                let mut rule = sedit::new_rule();
                rule.message = String::new();
                let raw = sedit::build(None, &rule);
                self.with_save(|s| s.rules.push(raw));
                self.mark_dirty();
                r.sel = n;
                self.replace_modal(Modal::Rules(Box::new(r)));
                self.rules_select(n);
                self.set_focus(F_COND + 1);
                self.rebuild_ui();
                return;
            }
            A_RULE_DEL if n > 0 => {
                let i = r.sel.min(n - 1);
                self.with_save(|s| {
                    s.rules.remove(i);
                });
                self.mark_dirty();
                let i = i.min(n.saturating_sub(2));
                self.replace_modal(Modal::Rules(Box::new(r)));
                self.rules_select(i);
                self.rebuild_ui();
                return;
            }
            A_RULE_DUP if n > 0 => {
                let i = r.sel.min(n - 1);
                self.with_save(|s| {
                    let c = s.rules[i].clone();
                    s.rules.insert(i + 1, c);
                });
                self.mark_dirty();
                self.replace_modal(Modal::Rules(Box::new(r)));
                self.rules_select(i + 1);
                self.rebuild_ui();
                return;
            }
            A_RULE_UP | A_RULE_DOWN if n > 1 => {
                let i = r.sel.min(n - 1);
                let j = if a == A_RULE_UP {
                    i.checked_sub(1)
                } else {
                    (i + 1 < n).then_some(i + 1)
                };
                if let Some(j) = j {
                    self.with_save(|s| s.rules.swap(i, j));
                    self.mark_dirty();
                    self.replace_modal(Modal::Rules(Box::new(r)));
                    self.rules_select(j);
                    self.rebuild_ui();
                }
                return;
            }
            _ => {}
        }
        let sel = r.sel;
        let Some(rule) = rules.get_mut(sel) else {
            self.replace_modal(Modal::Rules(Box::new(r)));
            self.rebuild_ui();
            return;
        };
        let table_rows = self
            .data
            .as_ref()
            .map(|d| sedit::effect_rows(&d.scenario_effects))
            .unwrap_or_default();
        let mut changed = true;
        match a {
            x if (A_RULE_TRIG..A_RULE_TRIG + 8).contains(&x) => {
                rule.trigger = (x - A_RULE_TRIG) as u8
            }
            x if (A_RULE_KIND..A_RULE_KIND + 3).contains(&x) => {
                rule.message_kind = [
                    MessageKind::Dialog,
                    MessageKind::Newspaper,
                    MessageKind::Choice,
                ][(x - A_RULE_KIND) as usize];
                if rule.message_kind != MessageKind::Choice {
                    r.set = 0;
                }
            }
            x if (A_RULE_FLAG..A_RULE_FLAG + 11).contains(&x) => match x - A_RULE_FLAG {
                0 => rule.once = !rule.once,
                1 => rule.per_company = !rule.per_company,
                2 => rule.per_player = !rule.per_player,
                3 => rule.per_territory = !rule.per_territory,
                4 => rule.ai_companies = !rule.ai_companies,
                5 => rule.human_companies = !rule.human_companies,
                6 => rule.ai_players = !rule.ai_players,
                7 => rule.human_players = !rule.human_players,
                8 => rule.show_when_false = !rule.show_when_false,
                9 => rule.first_true_only = !rule.first_true_only,
                _ => rule.territory_filter = if rule.territory_filter > 0 { 0 } else { 1 },
            },
            x if (A_RULE_SET..A_RULE_SET + 2).contains(&x) => {
                r.set = (x - A_RULE_SET) as usize;
                r.eff_sel = 0;
                r.picking = false;
                changed = false;
            }
            A_EFF_ADD => {
                r.picking = true;
                changed = false;
            }
            A_EFF_PICK => {
                if let Some(&(e, min, max, _)) = table_rows.get(r.pick_sel) {
                    let v = if max >= 1 { 1 } else { min.max(-1) };
                    rule.effects[r.set][e] = v;
                }
                r.picking = false;
            }
            A_EFF_REMOVE => {
                let cur = current_effects(rule, r.set);
                if let Some(&e) = cur.get(r.eff_sel) {
                    rule.effects[r.set][e] = 0;
                    r.eff_sel = r.eff_sel.saturating_sub(1);
                }
            }
            A_EFF_MINUS | A_EFF_PLUS | A_EFF_BIG | 260 => {
                let d: i32 = match a {
                    A_EFF_MINUS => -1,
                    A_EFF_PLUS => 1,
                    A_EFF_BIG => -10,
                    _ => 10,
                };
                let cur = current_effects(rule, r.set);
                if let (Some(&e), Some(row)) = (
                    cur.get(r.eff_sel),
                    self.data.as_ref().and_then(|d| {
                        d.scenario_effects
                            .rows
                            .get(cur.get(r.eff_sel).copied().unwrap_or(0))
                    }),
                ) {
                    let v = i32::from(rule.effects[r.set][e]) + d;
                    let (lo, hi) = (i32::from(row.min), i32::from(row.max));
                    let mut v = v.clamp(lo, hi);
                    if v == 0 {
                        // 0 means "no effect": step over it in the direction of travel.
                        v = if d > 0 { 1.min(hi) } else { (-1).max(lo) };
                        if v == 0 {
                            v = i32::from(rule.effects[r.set][e]);
                        }
                    }
                    rule.effects[r.set][e] = v as i8;
                }
            }
            A_EFF_PERM => rule.permanent[r.set] = !rule.permanent[r.set],
            x if (A_EFF_DUR..A_EFF_DUR + 2).contains(&x) => {
                let d = if x == A_EFF_DUR { -1 } else { 1 };
                rule.duration[r.set] = (rule.duration[r.set] + d).clamp(1, 120);
            }
            _ => changed = false,
        }
        if changed {
            let rule = rule.clone();
            self.store_rule(sel, &rule);
        }
        self.replace_modal(Modal::Rules(Box::new(r)));
        self.rebuild_ui();
    }

    fn act_place(&mut self, a: u32, mut p: PlaceDlg) {
        match a {
            A_OK => {
                let r = self.editor_world_mut().editor_place_set(&p.info);
                if r.is_ok() {
                    self.mark_dirty();
                }
                self.close_modal();
                self.editor_changed(false);
                return;
            }
            A_CANCEL => {
                self.editor_modal_cancel();
                return;
            }
            A_DELETE => {
                let _ = self.editor_world_mut().editor_place_remove(p.info.id);
                self.mark_dirty();
                self.close_modal();
                self.editor_changed(false);
                return;
            }
            x if (A_PL_TAB..A_PL_TAB + 2).contains(&x) => {
                p.tab = (x - A_PL_TAB) as usize;
                self.set_focus(0);
            }
            x if (A_PL_KIND..A_PL_KIND + 3).contains(&x) => p.info.kind = (x - A_PL_KIND) as u8,
            x if (A_PL_SIZE..A_PL_SIZE + 5).contains(&x) => p.info.size = (x - A_PL_SIZE) as u8,
            x if (A_PL_STYLE..A_PL_STYLE + 5).contains(&x) => {
                p.info.style = STYLE_BYTES[(x - A_PL_STYLE) as usize]
            }
            x if (A_PL_TREND..A_PL_TREND + 2).contains(&x) => {
                let d = if x == A_PL_TREND { -1 } else { 1 };
                p.info.trend = (i32::from(p.info.trend) + d).clamp(-2, 6) as i8;
            }
            x if (A_PL_W..A_PL_W + 4).contains(&x) => {
                let d = [-10i32, -1, 1, 10][(x - A_PL_W) as usize];
                let types = self.weight_types();
                if let Some(&t) = types.get(p.wsel) {
                    let v = (i32::from(p.info.weights[t]) + d).clamp(0, 200);
                    p.info.weights[t] = v as u16;
                }
            }
            A_PL_ALL0 => p.info.weights.iter_mut().for_each(|w| *w = 0),
            A_PL_ALL100 => p.info.weights.iter_mut().for_each(|w| *w = 100),
            _ => {}
        }
        self.replace_modal(Modal::Place(Box::new(p)));
        self.rebuild_ui();
    }

    /// Building types that have a name (the ones the weights list shows).
    fn weight_types(&self) -> Vec<usize> {
        (1..53usize)
            .filter(|&t| !self.type_name(t as u8).is_empty())
            .collect()
    }

    fn act_territory(&mut self, a: u32, mut t: TerritoryDlg) {
        match a {
            A_OK => {
                if t.is_new {
                    match self.editor_world_mut().editor_territory_add(&t.info.name) {
                        Ok(id) => t.info.id = id,
                        Err(e) => {
                            let m = self.editor_error_text(e);
                            self.close_modal();
                            self.ed_msg(m, false);
                            self.rebuild_ui();
                            return;
                        }
                    }
                    if t.info.parent == 0 || t.info.parent == 31 {
                        t.info.parent = t.info.id;
                    }
                }
                let _ = self.editor_world_mut().editor_territory_set(&t.info);
                self.mark_dirty();
                let count = self
                    .editor_world()
                    .map_or(0, |w| w.editor_territories().len());
                if t.is_new
                    && let Some(e) = self.ed_mut()
                {
                    e.terr_sel = count.saturating_sub(1);
                }
                self.close_modal();
                return;
            }
            A_CANCEL => {
                self.close_modal();
                return;
            }
            A_DELETE => {
                match self.editor_world_mut().editor_territory_remove(t.info.id) {
                    Ok(()) => {
                        self.mark_dirty();
                        if let Some(e) = self.ed_mut() {
                            e.terr_sel = 0;
                        }
                        self.close_modal();
                        self.editor_changed(false);
                    }
                    Err(e) => {
                        let m = self.opt_error_message(e);
                        self.close_modal();
                        self.ed_msg(m, false);
                        self.rebuild_ui();
                    }
                }
                return;
            }
            x if (A_TD_TOGGLE..A_TD_TOGGLE + 3).contains(&x) => match x - A_TD_TOGGLE {
                0 => t.info.visible = !t.info.visible,
                1 => t.info.merged_flag = !t.info.merged_flag,
                _ => t.info.separate_rights = !t.info.separate_rights,
            },
            x if (A_TD_GOOD..A_TD_GOOD + 2).contains(&x) => {
                let d = if x == A_TD_GOOD { -500 } else { 500 };
                t.info.goodwill = (i32::from(t.info.goodwill) + d).clamp(0, 10_000) as i16;
            }
            x if (A_TD_PARENT..A_TD_PARENT + 2).contains(&x) => {
                let d = if x == A_TD_PARENT { -1 } else { 1 };
                t.info.parent = (i32::from(t.info.parent) + d).clamp(1, 31) as u8;
            }
            x if (A_TD_RIGHT..A_TD_RIGHT + 4).contains(&x) => {
                let i = (x - A_TD_RIGHT) as usize;
                t.info.rights[i] = !t.info.rights[i];
            }
            x if (A_TD_SALE..A_TD_SALE + 4).contains(&x) => {
                let i = (x - A_TD_SALE) as usize;
                t.info.for_sale[i] = !t.info.for_sale[i];
            }
            _ => {}
        }
        self.replace_modal(Modal::Territory(Box::new(t)));
        self.rebuild_ui();
    }

    // ---- drawing ----------------------------------------------------------------------------

    pub(super) fn build_editor_modal(&mut self, ui: &mut Ui) {
        let Some(m) = self.ed().and_then(|e| e.modal.clone()) else {
            return;
        };
        let focus = self.ed().map_or(0, |e| e.focus);
        let (cw, ch) = ui.logical_size();
        let dz = design_scale(ch);
        let (w, h) = match &m {
            Modal::NewMap(_) => (520.0, 480.0),
            Modal::Open(_) => (520.0, 420.0),
            Modal::SaveAs(_) | Modal::Confirm(_) => (460.0, 190.0),
            Modal::Options(_) => (780.0, 650.0),
            Modal::Rules(_) => (cw / dz - 24.0, ch / dz - 24.0),
            Modal::Place(_) => (560.0, 420.0),
            Modal::Territory(_) => (560.0, 430.0),
        };
        let (w, h) = ((w * dz).min(cw - 8.0), (h * dz).min(ch - 8.0));
        let dim = ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(Color(0, 0, 0, 0x90)),
            },
            Layout::fill(),
        );
        let b = ui.add(
            dim,
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, w, h),
        );
        let mut c = Ctx {
            ui,
            b,
            dz,
            w,
            h,
            focus,
            f: fnt(1, dz * 0.9),
        };
        match m {
            Modal::NewMap(n) => self.draw_new_map(&mut c, &n),
            Modal::Open(o) => self.draw_open(&mut c, &o),
            Modal::SaveAs(s) => self.draw_save_as(&mut c, &s),
            Modal::Confirm(_) => self.draw_confirm(&mut c),
            Modal::Options(o) => self.draw_options(&mut c, &o),
            Modal::Rules(r) => self.draw_rules(&mut c, &r),
            Modal::Place(p) => self.draw_place(&mut c, &p),
            Modal::Territory(t) => self.draw_territory(&mut c, &t),
        }
    }

    fn draw_new_map(&self, c: &mut Ctx, n: &NewMap) {
        let t = &self.texts;
        c.title(&t.a("New Map"));
        let mut y = c.top();
        c.label(20.0, y, 90.0, &t.a("Name"), MUTED);
        c.field(120.0, y, c.w / c.dz - 140.0, &n.name, F_NAME);
        y += 34.0;
        for (i, (name, v)) in [(t.a("Width"), n.w), (t.a("Height"), n.h)]
            .into_iter()
            .enumerate()
        {
            c.label(20.0, y, 90.0, &name, MUTED);
            c.button(120.0, y, 40.0, "-10", A_NM_STEP + 2 * i as u32);
            c.label(166.0, y, 60.0, &v.to_string(), TEXT);
            c.button(230.0, y, 40.0, "+10", A_NM_STEP + 2 * i as u32 + 1);
            y += 30.0;
        }
        c.label(20.0, y, 120.0, &t.a("Base terrain"), MUTED);
        y += 28.0;
        let cols = 4;
        let bw = (c.w / c.dz - 40.0) / cols as f32;
        for (i, class) in editor::SOLID_CLASSES.iter().enumerate() {
            c.toggle(
                20.0 + (i % cols) as f32 * bw,
                y + (i / cols) as f32 * 28.0,
                bw - 4.0,
                &self.class_label(*class),
                A_NM_CLASS + i as u32,
                n.class_slot == i,
            );
        }
        let y = y + 4.0 * 28.0 + 8.0;
        c.label(20.0, y, 150.0, &t.a("Image file"), MUTED);
        c.field(180.0, y, c.w / c.dz - 200.0, &n.image, F_IMG);
        c.label(
            20.0,
            y + 32.0,
            150.0,
            &t.a("Height scale (1 to 2000)"),
            MUTED,
        );
        c.field(180.0, y + 32.0, 80.0, &n.scale, F_SCALE);
        c.ok_cancel(&t.a("From Scratch"), &t.get(lng::CANCEL, "Cancel"));
        let by = c.h / c.dz - 38.0;
        c.button(14.0, by, 150.0, &t.a("From Image"), A_NM_IMAGE);
    }

    fn class_label(&self, class: u8) -> String {
        match class {
            0..=12 => self.texts.get(700 + u32::from(class), ""),
            18 => self.texts.get(713, "ocean"),
            c => self.texts.af("Terrain %1", &[c.to_string()]),
        }
    }

    fn draw_open(&self, c: &mut Ctx, o: &OpenList) {
        let t = &self.texts;
        c.title(&t.a("Open Map"));
        let items: Vec<String> = o.entries.iter().map(|e| e.0.clone()).collect();
        let y = c.top();
        c.list(
            20.0,
            y,
            c.w / c.dz - 40.0,
            c.h / c.dz - y - 60.0,
            items,
            o.sel,
            A_OPEN_LIST,
        );
        c.ok_cancel(&t.get(lng::OK, "OK"), &t.get(lng::CANCEL, "Cancel"));
    }

    fn draw_save_as(&self, c: &mut Ctx, s: &str) {
        let t = &self.texts;
        c.title(&t.a("Save Map As"));
        let y = c.top();
        c.label(20.0, y, 90.0, &t.a("Name"), MUTED);
        c.field(120.0, y, c.w / c.dz - 140.0, s, F_NAME);
        c.label(
            20.0,
            y + 36.0,
            c.w / c.dz - 40.0,
            &t.a("Saved as an .MP2 map in your maps folder."),
            MUTED,
        );
        c.ok_cancel(&t.get(lng::OK, "OK"), &t.get(lng::CANCEL, "Cancel"));
    }

    fn draw_confirm(&self, c: &mut Ctx) {
        let t = &self.texts;
        c.title(&t.a("Unsaved changes"));
        let y = c.top();
        c.label(
            20.0,
            y,
            c.w / c.dz - 40.0,
            &t.a("Discard the changes to this map?"),
            TEXT,
        );
        c.ok_cancel(&t.a("Discard"), &t.get(lng::CANCEL, "Cancel"));
    }

    fn draw_options(&self, c: &mut Ctx, o: &OptionsState) {
        let t = &self.texts;
        c.title(&t.a("Map Options"));
        let tabs = [
            t.get(1308, "General"),
            t.get(1309, "Victory Conditions"),
            t.get(1311, "Players"),
            t.get(1319, "Player Pool"),
            t.get(1312, "Managers"),
            t.get(1313, "Economy"),
            t.get(1314, "Engines"),
            t.get(1315, "Industry"),
            t.get(1316, "Restrictions"),
            t.get(1317, "Ports"),
            t.get(1318, "Robbers"),
        ];
        let per_row = A_OPT_TOTAL.div_ceil(2);
        let tw = (c.w / c.dz - 40.0) / per_row as f32;
        for (i, name) in tabs.iter().enumerate() {
            c.toggle(
                20.0 + (i % per_row) as f32 * tw,
                c.top() - 6.0 + (i / per_row) as f32 * 30.0,
                tw - 3.0,
                name,
                A_OPT_TAB + i as u32,
                o.tab == i,
            );
        }
        let mut y = c.top() + 56.0;
        let Some(s) = self.editor_world().and_then(|w| w.editor_save()) else {
            return;
        };
        match o.tab {
            T_PLAYERS => self.draw_players(c, o),
            T_POOL => self.draw_pool(c),
            T_MANAGERS => self.draw_managers(c),
            T_ENGINES => self.draw_engines(c),
            T_PORTS => self.draw_ports(c, o),
            T_ROBBERS => self.draw_robbers(c, o),
            T_GENERAL => {
                c.label(20.0, y, 120.0, &t.a("Name"), MUTED);
                c.field(150.0, y, c.w / c.dz - 170.0, &editor::map_name(s), F_NAME);
                y += 34.0;
                c.label(20.0, y, 120.0, &t.a("Briefing text"), MUTED);
                c.field_tall(
                    150.0,
                    y,
                    c.w / c.dz - 170.0,
                    120.0,
                    &editor::map_description(s),
                    F_DESC,
                );
                y += 134.0;
                c.label(20.0, y, 120.0, &t.a("Start year"), MUTED);
                c.button(150.0, y, 44.0, "-10", A_OPT_YEAR);
                c.button(198.0, y, 36.0, "-1", A_OPT_YEAR + 1);
                c.label(240.0, y, 60.0, &editor::start_year(s).to_string(), TEXT);
                c.button(304.0, y, 36.0, "+1", A_OPT_YEAR + 2);
                c.button(344.0, y, 44.0, "+10", A_OPT_YEAR + 3);
                y += 32.0;
                c.label(20.0, y, 120.0, &t.a("Starting cash"), MUTED);
                c.button(150.0, y, 36.0, "-", A_OPT_CASH);
                c.label(
                    192.0,
                    y,
                    140.0,
                    &crate::info::money(editor::player_cash(s)),
                    TEXT,
                );
                c.button(336.0, y, 36.0, "+", A_OPT_CASH + 1);
            }
            T_GOALS => {
                c.label(20.0, y, c.w / c.dz - 40.0, &t.a("Each goal is a rule: the condition below must hold. Leave a goal empty for none."), MUTED);
                y += 30.0;
                for k in 0..4 {
                    let name = match k {
                        0 => t.a("Bronze"),
                        1 => t.a("Silver"),
                        2 => t.a("Gold"),
                        _ => t.a("Defeat"),
                    };
                    c.label(20.0, y, 80.0, &name, HEAD);
                    c.field(
                        104.0,
                        y,
                        c.w / c.dz - 124.0,
                        &o.goal_cond[k],
                        F_GOAL + k as u32,
                    );
                    let (msg, color) = match sedit::validate(&o.goal_cond[k]) {
                        Ok(()) => (self.goal_message(k), MUTED),
                        Err(e) => (sedit::describe(&e), BAD),
                    };
                    if color == BAD {
                        c.label(104.0, y + 28.0, c.w / c.dz - 124.0, &msg, BAD);
                    } else {
                        c.field(
                            104.0,
                            y + 28.0,
                            c.w / c.dz - 124.0,
                            &msg,
                            F_GOAL_MSG + k as u32,
                        );
                    }
                    y += 62.0;
                }
            }
            T_RESTRICTIONS => {
                let cols = 2;
                let bw = (c.w / c.dz - 40.0) / cols as f32;
                for i in 0..22usize {
                    let text = t.get(1267 + i as u32, "");
                    c.toggle(
                        20.0 + (i % cols) as f32 * bw,
                        y + (i / cols) as f32 * 28.0,
                        bw - 4.0,
                        &text,
                        A_OPT_PERM + i as u32,
                        editor::permission(s, i),
                    );
                }
            }
            T_ECONOMY => {
                let names = [t.a("Basic"), t.a("Advanced"), t.a("Expert")];
                c.label(20.0, y, 160.0, &t.a("Financial model"), MUTED);
                for (i, n) in names.iter().enumerate() {
                    c.toggle(
                        190.0 + i as f32 * 110.0,
                        y,
                        104.0,
                        n,
                        A_OPT_FIN + i as u32,
                        editor::financial_model(s) as usize == i,
                    );
                }
                y += 36.0;
                c.label(20.0, y, 160.0, &t.a("Industrial model"), MUTED);
                for (i, n) in names.iter().enumerate() {
                    c.toggle(
                        190.0 + i as f32 * 110.0,
                        y,
                        104.0,
                        n,
                        A_OPT_IND + i as u32,
                        editor::industrial_model(s) as usize == i,
                    );
                }
            }
            T_INDUSTRIES => {
                c.label(20.0, y, 160.0, &t.a("Industry set"), MUTED);
                let names = [t.a("Original industries"), t.a("Expansion industries")];
                for (i, n) in names.iter().enumerate() {
                    c.toggle(
                        190.0 + i as f32 * 200.0,
                        y,
                        190.0,
                        n,
                        A_OPT_SET + i as u32,
                        usize::from(editor::industry_set(s)) == i,
                    );
                }
            }
            _ => {}
        }
        c.ok_cancel(&t.get(lng::OK, "OK"), "");
    }

    fn draw_rules(&self, c: &mut Ctx, r: &RulesState) {
        let t = &self.texts;
        let rules = self.rules();
        c.title(&t.a("Scenario Rules"));
        let lw = (c.w / c.dz * 0.30).max(180.0);
        let top = c.top();
        let items: Vec<String> = rules
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let c = if r.condition.is_empty() {
                    "-".to_owned()
                } else {
                    r.condition.chars().take(40).collect()
                };
                format!(
                    "{:>2}  {}  {}",
                    i + 1,
                    t.a(TRIGGERS[usize::from(r.trigger.min(7))]),
                    c
                )
            })
            .collect();
        c.list(
            14.0,
            top,
            lw,
            c.h / c.dz - top - 100.0,
            items,
            r.sel,
            A_RULE_LIST,
        );
        let by = c.h / c.dz - 96.0;
        let bw = (lw - 8.0) / 3.0;
        c.button(14.0, by, bw, &t.a("Add"), A_RULE_ADD);
        c.button(14.0 + bw + 4.0, by, bw, &t.a("Delete"), A_RULE_DEL);
        c.button(14.0 + 2.0 * (bw + 4.0), by, bw, &t.a("Copy"), A_RULE_DUP);
        c.button(14.0, by + 30.0, bw * 1.5, &t.a("Up"), A_RULE_UP);
        c.button(
            14.0 + bw * 1.5 + 4.0,
            by + 30.0,
            bw * 1.5,
            &t.a("Down"),
            A_RULE_DOWN,
        );
        let Some(rule) = rules.get(r.sel) else {
            c.label(lw + 30.0, top, 400.0, &t.a("No rules yet. Add one."), MUTED);
            c.ok_cancel(&t.get(lng::OK, "OK"), &t.get(lng::CANCEL, "Cancel"));
            return;
        };
        let x0 = lw + 28.0;
        let rw = c.w / c.dz - x0 - 14.0;
        let mut y = top;
        c.label(x0, y, 80.0, &t.a("When"), MUTED);
        let tw = (rw - 80.0) / 4.0;
        for i in 0..8usize {
            c.toggle(
                x0 + 80.0 + (i % 4) as f32 * tw,
                y + (i / 4) as f32 * 26.0,
                tw - 3.0,
                &t.a(TRIGGERS[i]),
                A_RULE_TRIG + i as u32,
                usize::from(rule.trigger) == i,
            );
        }
        y += 56.0;
        c.label(x0, y, 80.0, &t.a("Condition"), MUTED);
        c.field(x0 + 80.0, y, rw - 80.0, &r.cond, F_COND);
        y += 28.0;
        match sedit::validate(&r.cond) {
            Ok(()) => c.label(
                x0 + 80.0,
                y,
                rw - 80.0,
                &t.a("The condition is valid."),
                GOOD,
            ),
            Err(e) => c.label(x0 + 80.0, y, rw - 80.0, &sedit::describe(&e), BAD),
        }
        y += 24.0;
        let flags: [(String, bool); 11] = [
            (t.a("Only once"), rule.once),
            (t.a("Each company"), rule.per_company),
            (t.a("Each player"), rule.per_player),
            (t.a("Each territory"), rule.per_territory),
            (t.a("AI companies"), rule.ai_companies),
            (t.a("Human companies"), rule.human_companies),
            (t.a("AI players"), rule.ai_players),
            (t.a("Human players"), rule.human_players),
            (t.a("Show if false (status)"), rule.show_when_false),
            (t.a("Message once per check"), rule.first_true_only),
            (t.a("First territory only"), rule.territory_filter > 0),
        ];
        let fw = rw / 4.0;
        for (i, (name, on)) in flags.iter().enumerate() {
            c.toggle(
                x0 + (i % 4) as f32 * fw,
                y + (i / 4) as f32 * 26.0,
                fw - 3.0,
                name,
                A_RULE_FLAG + i as u32,
                *on,
            );
        }
        y += 82.0;
        c.label(x0, y, 80.0, &t.a("Message"), MUTED);
        for (i, name) in [t.a("Dialog"), t.a("Newspaper"), t.a("Choice")]
            .iter()
            .enumerate()
        {
            c.toggle(
                x0 + 80.0 + i as f32 * 110.0,
                y,
                104.0,
                name,
                A_RULE_KIND + i as u32,
                rule.message_kind as usize == i,
            );
        }
        y += 28.0;
        c.field(x0 + 80.0, y, rw - 80.0, &rule.message, F_MSG);
        c.label(x0, y, 80.0, &t.a("Text"), MUTED);
        y += 28.0;
        if rule.message_kind == MessageKind::Choice {
            c.label(x0, y, 80.0, &t.a("Buttons"), MUTED);
            c.field(x0 + 80.0, y, (rw - 90.0) / 2.0, &rule.label1, F_LABEL1);
            c.field(
                x0 + 90.0 + (rw - 90.0) / 2.0,
                y,
                (rw - 90.0) / 2.0,
                &rule.label2,
                F_LABEL2,
            );
            y += 28.0;
        } else if rule.message_kind == MessageKind::Newspaper {
            c.label(x0, y, 80.0, &t.a("Subtitle"), MUTED);
            c.field(x0 + 80.0, y, rw - 80.0, &rule.label1, F_LABEL1);
            y += 28.0;
        }
        c.label(x0, y, 80.0, &t.a("Comment"), MUTED);
        c.field(x0 + 80.0, y, rw - 80.0, &rule.title, F_TITLE);
        y += 32.0;
        // Effects.
        c.label(x0, y, 80.0, &t.a("Effects"), HEAD);
        if rule.message_kind == MessageKind::Choice {
            for k in 0..2usize {
                c.toggle(
                    x0 + 80.0 + k as f32 * 110.0,
                    y,
                    104.0,
                    &format!("{} {}", t.a("Button"), k + 1),
                    A_RULE_SET + k as u32,
                    r.set == k,
                );
            }
        }
        let list_y = y + 28.0;
        let list_h = (c.h / c.dz - list_y - 58.0).max(60.0);
        let rows = self.data.as_ref().map(|d| &d.scenario_effects);
        let effect_name = |e: usize| -> String {
            rows.and_then(|t| t.rows.get(e))
                .map(|row| self.texts.get(u32::from(row.name_lng), ""))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("#{e}"))
        };
        let lw2 = rw * 0.62;
        if r.picking {
            let named = self
                .data
                .as_ref()
                .map(|d| sedit::effect_rows(&d.scenario_effects))
                .unwrap_or_default();
            let items: Vec<String> = named.iter().map(|&(e, ..)| effect_name(e)).collect();
            c.list(x0, list_y, lw2, list_h, items, r.pick_sel, A_EFF_LIST);
            c.button(
                x0 + lw2 + 8.0,
                list_y,
                rw - lw2 - 8.0,
                &t.a("Use this effect"),
                A_EFF_PICK,
            );
            if let Some(&(e, min, max, inst)) = named.get(r.pick_sel) {
                c.label(
                    x0 + lw2 + 8.0,
                    list_y + 30.0,
                    rw - lw2 - 8.0,
                    &t.af("Range %1 to %2", &[min.to_string(), max.to_string()]),
                    MUTED,
                );
                if inst {
                    c.label(
                        x0 + lw2 + 8.0,
                        list_y + 54.0,
                        rw - lw2 - 8.0,
                        &t.a("Instant: not undone."),
                        MUTED,
                    );
                }
                let _ = e;
            }
        } else {
            let cur = current_effects(rule, r.set);
            let items: Vec<String> = cur
                .iter()
                .map(|&e| {
                    format!(
                        "{}: {}",
                        effect_name(e),
                        effect_value_text(
                            rule.effects[r.set][e],
                            rows.and_then(|t| t.rows.get(e)).map_or(0, |x| x.min)
                        )
                    )
                })
                .collect();
            let warn = effect_warnings(&self.texts, rule, &cur, rows);
            c.list(
                x0,
                list_y,
                lw2,
                list_h,
                items,
                r.eff_sel.min(cur.len().saturating_sub(1)),
                A_EFF_LIST,
            );
            let bx = x0 + lw2 + 8.0;
            let bw = rw - lw2 - 8.0;
            c.button(bx, list_y, bw / 2.0 - 2.0, "-", A_EFF_MINUS);
            c.button(bx + bw / 2.0 + 2.0, list_y, bw / 2.0 - 2.0, "+", A_EFF_PLUS);
            c.button(bx, list_y + 28.0, bw / 2.0 - 2.0, "-10", A_EFF_BIG);
            c.button(
                bx + bw / 2.0 + 2.0,
                list_y + 28.0,
                bw / 2.0 - 2.0,
                "+10",
                A_EFF_BIG + 1,
            );
            c.button(bx, list_y + 56.0, bw, &t.a("Remove effect"), A_EFF_REMOVE);
            c.button(bx, list_y + 84.0, bw, &t.a("Add effect..."), A_EFF_ADD);
            c.toggle(
                bx,
                list_y + 112.0,
                bw,
                &t.a("Permanent"),
                A_EFF_PERM,
                rule.permanent[r.set],
            );
            if !rule.permanent[r.set] {
                c.button(bx, list_y + 140.0, 36.0, "-", A_EFF_DUR);
                c.label(
                    bx + 40.0,
                    list_y + 140.0,
                    bw - 80.0,
                    &t.af("%1 months", &[rule.duration[r.set].to_string()]),
                    TEXT,
                );
                c.button(bx + bw - 36.0, list_y + 140.0, 36.0, "+", A_EFF_DUR + 1);
            }
            if let Some(w) = warn {
                c.label(bx, list_y + 172.0, bw, &w, BAD);
            }
        }
        c.ok_cancel(&t.get(lng::OK, "OK"), &t.get(lng::CANCEL, "Cancel"));
    }

    fn draw_place(&self, c: &mut Ctx, p: &PlaceDlg) {
        let t = &self.texts;
        let i = &p.info;
        c.title(&if p.is_new {
            t.a("New place")
        } else {
            t.a("Place")
        });
        for (k, name) in [t.a("General"), t.a("Industries")].iter().enumerate() {
            c.toggle(
                20.0 + k as f32 * 130.0,
                c.top() - 6.0,
                126.0,
                name,
                A_PL_TAB + k as u32,
                p.tab == k,
            );
        }
        let mut y = c.top() + 30.0;
        if p.tab == 0 {
            c.label(20.0, y, 100.0, &t.a("Name"), MUTED);
            c.field(130.0, y, c.w / c.dz - 150.0, &i.name, F_NAME);
            y += 34.0;
            c.label(20.0, y, 100.0, &t.a("Type"), MUTED);
            for (k, id) in [1146u32, 1147, 1148].into_iter().enumerate() {
                c.toggle(
                    130.0 + k as f32 * 110.0,
                    y,
                    104.0,
                    &t.get(id, ""),
                    A_PL_KIND + k as u32,
                    usize::from(i.kind) == k,
                );
            }
            y += 34.0;
            c.label(20.0, y, 100.0, &t.a("Size"), MUTED);
            let sw = (c.w / c.dz - 150.0) / 5.0;
            for k in 0..5usize {
                c.toggle(
                    130.0 + k as f32 * sw,
                    y,
                    sw - 3.0,
                    &t.get(1152 + k as u32, SIZES[k]),
                    A_PL_SIZE + k as u32,
                    usize::from(i.size) == k,
                );
            }
            y += 34.0;
            c.label(20.0, y, 100.0, &t.a("Architecture"), MUTED);
            let names = ["Victorian", "Colonial", "Mission", "Tudor", "Kyoto"];
            for k in 0..5usize {
                c.toggle(
                    130.0 + k as f32 * sw,
                    y,
                    sw - 3.0,
                    &t.a(names[k]),
                    A_PL_STYLE + k as u32,
                    i.style == STYLE_BYTES[k],
                );
            }
            y += 34.0;
            if i.kind == 1 {
                c.label(20.0, y, 100.0, &t.a("Trend"), MUTED);
                c.button(130.0, y, 36.0, "-", A_PL_TREND);
                c.label(172.0, y, 50.0, &i.trend.to_string(), TEXT);
                c.button(226.0, y, 36.0, "+", A_PL_TREND + 1);
            }
        } else {
            let types = self.weight_types();
            let items: Vec<String> = types
                .iter()
                .map(|&ty| format!("{:<28} {:>3}", self.type_name(ty as u8), i.weights[ty]))
                .collect();
            let lw = c.w / c.dz * 0.62;
            c.list(20.0, y, lw, c.h / c.dz - y - 60.0, items, p.wsel, A_PL_LIST);
            let bx = 40.0 + lw;
            let bw = c.w / c.dz - bx - 20.0;
            for (k, label) in ["-10", "-1", "+1", "+10"].iter().enumerate() {
                c.button(
                    bx + (k % 2) as f32 * (bw / 2.0),
                    y + (k / 2) as f32 * 30.0,
                    bw / 2.0 - 3.0,
                    label,
                    A_PL_W + k as u32,
                );
            }
            c.button(bx, y + 70.0, bw, &t.a("All 0"), A_PL_ALL0);
            c.button(bx, y + 100.0, bw, &t.a("All 100"), A_PL_ALL100);
            c.label(
                bx,
                y + 134.0,
                bw,
                &t.a("Weights of the buildings this place builds, 0 to 200."),
                MUTED,
            );
        }
        c.ok_cancel_delete(
            &t.get(lng::OK, "OK"),
            &t.get(lng::CANCEL, "Cancel"),
            &t.a("Delete"),
            !p.is_new,
        );
    }

    fn draw_territory(&self, c: &mut Ctx, tdl: &TerritoryDlg) {
        let t = &self.texts;
        let i = &tdl.info;
        c.title(&t.get(2403, "Territory Setup"));
        let mut y = c.top();
        c.label(20.0, y, 120.0, &t.a("Name"), MUTED);
        c.field(150.0, y, c.w / c.dz - 170.0, &i.name, F_NAME);
        y += 34.0;
        c.label(
            20.0,
            y,
            250.0,
            &t.get(2395, "Default Goodwill (1 to 100)"),
            MUTED,
        );
        c.button(280.0, y, 36.0, "-", A_TD_GOOD);
        c.label(
            322.0,
            y,
            60.0,
            &if i.goodwill == 0 {
                "-".to_owned()
            } else {
                (i32::from(i.goodwill) / 100).to_string()
            },
            TEXT,
        );
        c.button(386.0, y, 36.0, "+", A_TD_GOOD + 1);
        y += 34.0;
        c.toggle(
            20.0,
            y,
            250.0,
            &t.get(2411, "Border is visible"),
            A_TD_TOGGLE,
            i.visible,
        );
        y += 32.0;
        if !i.visible {
            c.label(20.0, y, 250.0, &t.get(2396, "Remap To Territory ID"), MUTED);
            c.button(280.0, y, 36.0, "-", A_TD_PARENT);
            c.label(322.0, y, 50.0, &i.parent.to_string(), TEXT);
            c.button(376.0, y, 36.0, "+", A_TD_PARENT + 1);
            y += 32.0;
            c.toggle(
                20.0,
                y,
                330.0,
                &t.get(2397, "Remap right mouse click"),
                A_TD_TOGGLE + 1,
                i.merged_flag,
            );
        } else {
            c.toggle(
                20.0,
                y,
                330.0,
                &t.get(2398, "Allow separate rights purchases"),
                A_TD_TOGGLE + 2,
                i.separate_rights,
            );
            y += 32.0;
            c.label(
                20.0,
                y,
                250.0,
                &t.get(2399, "Access Rights - Default Statuses"),
                HEAD,
            );
            y += 28.0;
            let names = [
                t.a("Trains"),
                t.a("Track"),
                t.a("Buildings"),
                t.a("Stations"),
            ];
            let bw = (c.w / c.dz - 40.0) / 4.0;
            for (k, n) in names.iter().enumerate() {
                c.toggle(
                    20.0 + k as f32 * bw,
                    y,
                    bw - 3.0,
                    n,
                    A_TD_RIGHT + k as u32,
                    i.rights[k],
                );
                c.toggle(
                    20.0 + k as f32 * bw,
                    y + 28.0,
                    bw - 3.0,
                    &t.get(2401, "Buyable"),
                    A_TD_SALE + k as u32,
                    i.for_sale[k],
                );
            }
        }
        c.ok_cancel_delete(
            &t.get(lng::OK, "OK"),
            &t.get(lng::CANCEL, "Cancel"),
            &t.a("Remove Territory"),
            !tdl.is_new && tdl.info.id != 1,
        );
    }
}

/// Effects of set `k` that are set (non-zero), in table order.
fn current_effects(rule: &Rule, k: usize) -> Vec<usize> {
    rule.effects[k]
        .iter()
        .enumerate()
        .filter(|(_, b)| **b != 0)
        .map(|(e, _)| e)
        .collect()
}

/// The slider's value as the editor shows it: `-100` on a row that allows it is "trigger value".
fn effect_value_text(v: i8, min: i8) -> String {
    if v == -100 && min == -100 {
        "(TRIGGER)".to_owned()
    } else {
        format!("{v:+}")
    }
}

/// `rule_warn_scopes` (`0x458050`): an effect that needs a company, player or territory the rule
/// does not test against (LNG `0x89c`..`0x89e`).
fn effect_warnings(
    texts: &crate::text::Texts,
    rule: &Rule,
    cur: &[usize],
    rows: Option<&rt2_sim::scenario::EffectTable>,
) -> Option<String> {
    let rows = rows?;
    for &e in cur {
        let r = rows.rows.get(e)?;
        if r.needs_company && !rule.per_company {
            return Some(texts.a("This effect needs the rule to test each company."));
        }
        if r.needs_player && !rule.per_player {
            return Some(texts.a("This effect needs the rule to test each player."));
        }
        if r.needs_territory && !rule.per_territory {
            return Some(texts.a("This effect needs the rule to test each territory."));
        }
    }
    None
}

/// One line of at most `max` characters: line breaks shown as `|`, the tail while editing (the
/// caret is there) and the head with an ellipsis otherwise.
fn fit_text(text: &str, editing: bool, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' { '|' } else { c })
        .collect();
    let n = flat.chars().count();
    let max = max.max(8);
    if editing {
        let tail: String = flat.chars().skip(n.saturating_sub(max - 1)).collect();
        format!("{tail}_")
    } else if n > max {
        let head: String = flat.chars().take(max - 3).collect();
        format!("{head}...")
    } else {
        flat
    }
}

/// Drawing context of a dialog: the box and the sizes, with the widget helpers in logical units
/// relative to the box.
pub(super) struct Ctx<'a> {
    pub(super) ui: &'a mut Ui,
    pub(super) b: NodeId,
    pub(super) dz: f32,
    pub(super) w: f32,
    pub(super) h: f32,
    pub(super) focus: u32,
    pub(super) f: FontStyle,
}

impl Ctx<'_> {
    pub(super) fn top(&self) -> f32 {
        44.0
    }

    pub(super) fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Layout {
        at(x * self.dz, y * self.dz, w * self.dz, h * self.dz)
    }

    pub(super) fn title(&mut self, text: &str) {
        let l = self.rect(16.0, 8.0, self.w / self.dz - 32.0, 30.0);
        self.ui.add(
            self.b,
            lbl(&text.to_uppercase(), fnt(2, self.dz), HEAD, Align::Left),
            l,
        );
    }

    pub(super) fn label(&mut self, x: f32, y: f32, w: f32, text: &str, color: Color) {
        let l = self.rect(x, y, w, 26.0);
        self.ui
            .add(self.b, lbl(text, self.f, color, Align::Left), l);
    }

    pub(super) fn button(&mut self, x: f32, y: f32, w: f32, text: &str, action: u32) {
        let l = self.rect(x, y, w, 26.0);
        self.ui.add(
            self.b,
            Kind::Button {
                text: text.to_owned(),
                font: self.f,
                action,
                enabled: true,
                hotkey: None,
            },
            l,
        );
    }

    pub(super) fn toggle(&mut self, x: f32, y: f32, w: f32, text: &str, action: u32, on: bool) {
        let l = self.rect(x, y, w, 26.0);
        self.ui.add(
            self.b,
            Kind::Toggle {
                text: text.to_owned(),
                font: self.f,
                action,
                on,
                enabled: true,
            },
            l,
        );
    }

    /// A text field: a toggle showing the text (with a caret while it has the focus).
    pub(super) fn field(&mut self, x: f32, y: f32, w: f32, text: &str, id: u32) {
        let on = self.focus == id + 1;
        let shown = fit_text(text, on, (w * self.dz / (self.f.size * 0.62)) as usize);
        let l = self.rect(x, y, w, 26.0);
        self.ui.add(
            self.b,
            Kind::Toggle {
                text: shown,
                font: self.f,
                action: field_action(id),
                on,
                enabled: true,
            },
            l,
        );
    }

    pub(super) fn field_tall(&mut self, x: f32, y: f32, w: f32, h: f32, text: &str, id: u32) {
        let on = self.focus == id + 1;
        let l = self.rect(x, y, w, h);
        let node = self.ui.add(
            self.b,
            Kind::Toggle {
                text: String::new(),
                font: self.f,
                action: field_action(id),
                on,
                enabled: true,
            },
            l,
        );
        let shown = if on {
            format!("{text}_")
        } else {
            text.to_owned()
        };
        self.ui.add(
            node,
            Kind::Label {
                text: shown,
                font: self.f,
                color: TEXT,
                align: Align::Left,
                wrap: true,
            },
            Layout::fill().offset(6.0 * self.dz, 4.0 * self.dz),
        );
    }

    pub(super) fn list(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        items: Vec<String>,
        sel: usize,
        action: u32,
    ) {
        let l = self.rect(x, y, w, h);
        let sel = sel.min(items.len().saturating_sub(1));
        self.ui.add(
            self.b,
            Kind::List {
                items,
                selected: Some(sel),
                scroll: sel.saturating_sub(4),
                row_h: (18.0 * self.dz).round(),
                font: self.f,
                action,
            },
            l,
        );
    }

    pub(super) fn ok_cancel(&mut self, ok: &str, cancel: &str) {
        let y = self.h / self.dz - 38.0;
        let w = 110.0;
        let x = self.w / self.dz - 14.0 - w;
        if !cancel.is_empty() {
            self.button(x, y, w, cancel, A_CANCEL);
            self.button(x - w - 8.0, y, w, ok, A_OK);
        } else {
            self.button(x, y, w, ok, A_OK);
        }
    }

    pub(super) fn ok_cancel_delete(
        &mut self,
        ok: &str,
        cancel: &str,
        delete: &str,
        show_delete: bool,
    ) {
        self.ok_cancel(ok, cancel);
        if show_delete {
            let y = self.h / self.dz - 38.0;
            self.button(14.0, y, 110.0, delete, A_DELETE);
        }
    }
}

// The `Anchor` import is used by the box placement above.
const _: Anchor = Anchor::Center;
