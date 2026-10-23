//! The map and scenario editor (docs/PLAN.md phase 5; docs/reference/screens.md section L).
//!
//! The editor is the main window with the simulation held and a different toolbar, as in the
//! original (`ui_set_editor_mode` `0x495700`, docs/spec/commands.md 3.1): the map with, over it,
//! a bottom panel holding the tool groups (height, paint, fill, trees, territories, add,
//! bulldoze, overview), the palette of the chosen group (the `bp*.win` panels: `bpraisea`,
//! `bppaintt`, `bpfillte`, `bpaddtre`/`bpremtre`, `bpterrit`, `bpaddbui`, `bpbulldz`) and the file
//! and option buttons. Every tool calls the rule in `rt2_sim::editor` that ports the original's
//! click handler (`map_tool_click` `0x48eb30`); the dialogs (Map Options, Rules, city and territory
//! setup, new/open/save) are in `editor_modals.rs`.

#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rt2_formats::map::Save;
use rt2_render::live::LiveFrame;
use rt2_render::{Scene, SceneOptions};
use rt2_sim::editor::{
    self, DOZER_RADIUS, Elevation, FillMode, PAINT_RADIUS, PlaceKind, RAISE_RADIUS, SMOOTH_AMOUNT,
    SOLID_CLASSES, TREE_CLASSES, TREE_RADIUS,
};
use rt2_sim::{Command, SpeedChange, World};
use rt2_ui::{Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::{AppState, GameState, Screen, build_ui::STYLES, scene_fingerprint};
use crate::text::id as lng;

pub(super) const A_GROUP: u32 = 7000;
pub(super) const A_SUB: u32 = 7010;
pub(super) const A_BRUSH: u32 = 7040;
pub(super) const A_ITEM: u32 = 7100;
pub(super) const A_STYLE: u32 = 7080;
pub(super) const A_FILL_MODE: u32 = 7090;
/// Scroll the paint palette up or down by a row (the bars beside the `bppaintt` swatches).
pub(super) const A_PAL_UP: u32 = 7220;
pub(super) const A_PAL_DOWN: u32 = 7221;
pub(super) const A_UNDO: u32 = 7200;
pub(super) const A_OPTIONS: u32 = 7201;
pub(super) const A_RULES: u32 = 7202;
pub(super) const A_SAVE: u32 = 7203;
pub(super) const A_SAVE_AS: u32 = 7204;
pub(super) const A_OPEN: u32 = 7205;
pub(super) const A_NEW: u32 = 7206;
pub(super) const A_EXIT: u32 = 7207;
pub(super) const A_ADD_LIST: u32 = 7210;
pub(super) const A_TERR_LIST: u32 = 7211;
pub(super) const A_TERR_NEW: u32 = 7212;
pub(super) const A_TERR_EDIT: u32 = 7213;
/// First action id of the modal dialogs (`editor_modals.rs`).
pub(super) const A_MODAL: u32 = 7300;

/// Largest number of undo steps kept (each is a copy of the map).
const UNDO_MAX: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
    Select,
    Height,
    Paint,
    Fill,
    Trees,
    Territory,
    Add,
    Bulldoze,
    Overview,
}

impl Group {
    pub(super) const ALL: [Group; 9] = [
        Group::Select,
        Group::Height,
        Group::Paint,
        Group::Fill,
        Group::Trees,
        Group::Territory,
        Group::Add,
        Group::Bulldoze,
        Group::Overview,
    ];

    pub(super) fn uses_brush(self) -> bool {
        !matches!(
            self,
            Group::Select | Group::Fill | Group::Overview | Group::Add
        )
    }
}

/// An open dialog of the editor.
#[derive(Clone, Debug)]
pub(super) enum Modal {
    NewMap(super::editor_modals::NewMap),
    Open(super::editor_modals::OpenList),
    SaveAs(String),
    Confirm(Pending),
    Options(super::editor_modals::OptionsState),
    Rules(Box<super::editor_modals::RulesState>),
    Place(Box<super::editor_modals::PlaceDlg>),
    Territory(Box<super::editor_modals::TerritoryDlg>),
}

/// What waits for the answer of "discard the changes?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pending {
    New,
    Open,
    Exit,
}

pub(super) struct EditorState {
    pub group: Group,
    /// Tool inside the group (height: drag, smooth, lower; paint: solid, mixed; fill: adjacent,
    /// all; trees: add, remove).
    pub sub: usize,
    pub brush: usize,
    pub class_slot: usize,
    pub mix_slot: usize,
    pub tree_slot: usize,
    /// First visible row of the paint palette when the original panel art shows two rows.
    pub pal_row: usize,
    pub fill_mode: FillMode,
    /// Row of the add list (0..6 the special entries, then the 98 shapes by name).
    pub add_row: usize,
    pub style: usize,
    pub terr_sel: usize,
    pub hover: Option<(i32, i32)>,
    /// A height drag in progress: corner and the pixel row it started at.
    pub press: Option<(i32, i32, f32)>,
    pub undo: Vec<Save>,
    pub dirty: bool,
    pub path: Option<PathBuf>,
    pub msg: Option<(String, bool)>,
    pub modal: Option<Modal>,
    /// The building shapes of the add list: (name, shape), sorted by name.
    pub shapes: Vec<(String, i32)>,
    /// The terrain changed since the radar was drawn.
    pub radar_stale: bool,
    /// Milliseconds before the scene may be rebuilt again (a stroke makes many edits).
    pub scene_wait: f32,
    pub scene_pending: bool,
    /// Which text field of the dialog takes the typed characters.
    pub focus: u32,
}

pub(super) fn fnt(n: u8, dz: f32) -> FontStyle {
    let f = game_font(n);
    f.with_size((f.size * dz).round())
}

pub(super) fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

pub(super) fn lbl(text: &str, f: FontStyle, color: Color, align: Align) -> Kind {
    Kind::Label {
        text: text.to_owned(),
        font: f,
        color,
        align,
        wrap: false,
    }
}

pub(super) const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
pub(super) const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
pub(super) const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
pub(super) const BAD: Color = Color::rgb(0xf0, 0x8c, 0x7a);
pub(super) const GOOD: Color = Color::rgb(0x9a, 0xe0, 0x8a);

/// Size of everything, grown with the canvas height like the main window.
pub(super) fn design_scale(ch: f32) -> f32 {
    (ch / 620.0).clamp(1.0, 2.0)
}

fn bottom_height(ch: f32, dz: f32) -> f32 {
    (230.0 * dz).min(ch * 0.46).max(170.0).round()
}

/// The editor opens a blank map from the template when it has none to start from.
const DEFAULT_SIDE: usize = 120;

impl EditorState {
    fn new(path: Option<PathBuf>, shapes: Vec<(String, i32)>) -> EditorState {
        EditorState {
            group: Group::Height,
            sub: 0,
            brush: 1,
            class_slot: 5,
            mix_slot: 6,
            tree_slot: 0,
            pal_row: 0,
            fill_mode: FillMode::Match,
            add_row: 6,
            style: 0,
            terr_sel: 0,
            hover: None,
            press: None,
            undo: Vec::new(),
            dirty: false,
            path,
            msg: None,
            modal: None,
            shapes,
            radar_stale: false,
            scene_wait: 0.0,
            scene_pending: false,
            focus: 0,
        }
    }
}

impl AppState {
    // ---- plumbing ---------------------------------------------------------------------------

    pub(super) fn editor_active(&self) -> bool {
        self.game().is_some_and(|g| g.editor.is_some())
    }

    pub(super) fn ed(&self) -> Option<&EditorState> {
        self.game()?.editor.as_deref()
    }

    pub(super) fn ed_mut(&mut self) -> Option<&mut EditorState> {
        match &mut self.screen {
            Screen::Game(g) => g.editor.as_deref_mut(),
            Screen::Menu(_) => None,
        }
    }

    pub(super) fn editor_modal_open(&self) -> bool {
        self.ed().is_some_and(|e| e.modal.is_some())
    }

    fn gs_mut(&mut self) -> Option<&mut GameState> {
        match &mut self.screen {
            Screen::Game(g) => Some(g),
            Screen::Menu(_) => None,
        }
    }

    pub(super) fn editor_world(&self) -> Option<&World> {
        self.game().map(|g| &g.world)
    }

    pub(super) fn ed_msg(&mut self, text: String, good: bool) {
        if let Some(e) = self.ed_mut() {
            e.msg = Some((text, good));
        }
    }

    /// Names of the building shapes `24..121`, sorted like the original's list (`strcmpi`).
    fn shape_list(&self) -> Vec<(String, i32)> {
        let Some(data) = self.data.as_ref() else {
            return Vec::new();
        };
        let mut v: Vec<(String, i32)> = (24..122)
            .map(|shape| {
                let ty = rt2_sim::tilemark::shape_type(&data.image, shape as usize);
                (self.type_name(ty), shape)
            })
            .collect();
        v.sort_by(|a, b| {
            a.0.to_lowercase()
                .cmp(&b.0.to_lowercase())
                .then(a.1.cmp(&b.1))
        });
        v
    }

    /// Name of a building type: LNG `920 + type`, or `3010 + type - 38` for the expansion types.
    pub(super) fn type_name(&self, ty: u8) -> String {
        let id = if ty >= 38 {
            3010 + u32::from(ty) - 38
        } else {
            920 + u32::from(ty)
        };
        self.texts.get(id, "")
    }

    // ---- starting and leaving ---------------------------------------------------------------

    /// Opens the editor on `save` (a loaded or new map). The world is held: the simulation never
    /// advances in the editor.
    pub(super) fn start_editor(&mut self, save: Save, path: Option<PathBuf>) -> anyhow::Result<()> {
        let data = self.game_data()?.clone();
        let seed = self.seed;
        let mut world = World::from_save_seeded(&save, &data, seed);
        world.apply(Command::SeedRng(seed));
        world.apply(Command::SetSpeed(SpeedChange::Absolute(0)));
        let speeds = data.speed_table.clone();
        let live = world.save_ref().unwrap_or(&save);
        let map = live.map.clone();
        let scene = Scene::build_with(
            self.assets.clone(),
            &map,
            Some(live),
            SceneOptions { sprites: true },
            self.vehicle_names.clone(),
        );
        let live_frame = LiveFrame::from_save(live);
        let scene_fp = scene_fingerprint(live);
        self.pending_scene = Some(Arc::new(scene));
        self.pending_live = Some((live_frame, true));
        self.live_alpha = 1.0;
        let camera = self.fit_camera(map.width, map.height);
        let name = path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| editor::map_name(live));
        let year = world.date().year;
        let month = world.date().month;
        let shapes = {
            self.data = Some(data);
            self.shape_list()
        };
        let mut ed = EditorState::new(path, shapes);
        ed.radar_stale = false;
        self.screen = Screen::Game(Box::new(GameState {
            cash: None,
            radar: Some(crate::radar::Radar::new(&map)),
            radar_view: None,
            scenario: None,
            setup: crate::setup::Setup::default(),
            dialogs: Vec::new(),
            center: Default::default(),
            news: Vec::new(),
            last_year: year,
            center_day: -1,
            world,
            name,
            map_size: (map.width, map.height),
            camera,
            speeds,
            accum_ms: 0.0,
            cash_timer: 500.0,
            toast: None,
            hud: Default::default(),
            over_shown: false,
            info: None,
            scene_fp,
            build: super::build_ui::BuildState::default(),
            last_autosave: (year, month),
            editor: Some(Box::new(ed)),
            campaign: None,
        }));
        self.rebuild_ui();
        Ok(())
    }

    /// The editor's menu entries: a blank map from a shipped scenario with no companies.
    fn blank_template(&self) -> anyhow::Result<Save> {
        let maps = self.install.join("maps");
        for name in [
            "Local Industry.MP2",
            "Sunset and Lonley.MP2",
            "ROBBER Baron.MP2",
        ] {
            if let Ok(raw) = std::fs::read(maps.join(name))
                && let Ok(s) = Save::parse(&raw)
            {
                return Ok(s);
            }
        }
        let mut best: Option<(usize, Save)> = None;
        for e in crate::catalog::list_maps(&self.install) {
            let Ok(raw) = std::fs::read(&e.path) else {
                continue;
            };
            let Ok(s) = Save::parse(&raw) else { continue };
            if s.layout.version == 0x2348
                && s.companies.count == 0
                && best.as_ref().is_none_or(|(n, _)| raw.len() < *n)
            {
                best = Some((raw.len(), s));
            }
        }
        best.map(|(_, s)| s)
            .ok_or_else(|| anyhow::anyhow!("no scenario to use as the template of a new map"))
    }

    /// A new blank map (`width` x `height` of terrain class `class`) as the editor's map.
    pub(super) fn editor_new_map(&mut self, width: usize, height: usize, class: usize, name: &str) {
        match self.blank_template() {
            Ok(t) => {
                let s = editor::new_map(&t, width, height, class, name);
                if let Err(e) = self.start_editor(s, None) {
                    self.ed_msg(format!("{e:#}"), false);
                } else if let Some(ed) = self.ed_mut() {
                    ed.dirty = true;
                }
            }
            Err(e) => self.ed_msg(format!("{e:#}"), false),
        }
    }

    /// The Editor entry of the main menu: a blank default map with the New Map or Open dialog.
    pub(super) fn editor_enter(&mut self, open: bool) {
        self.editor_new_map(DEFAULT_SIDE, DEFAULT_SIDE, 5, "New map");
        if let Some(ed) = self.ed_mut() {
            ed.dirty = false;
        }
        if open {
            self.editor_open_dialog();
        } else {
            self.editor_new_dialog();
        }
    }

    pub(super) fn editor_leave(&mut self) {
        self.go_to_menu();
    }

    // ---- edits ------------------------------------------------------------------------------

    fn push_undo(&mut self) {
        let Some(g) = self.gs_mut() else { return };
        let Some(save) = g.world.save_ref().cloned() else {
            return;
        };
        if let Some(e) = g.editor.as_deref_mut() {
            e.undo.push(save);
            if e.undo.len() > UNDO_MAX {
                e.undo.remove(0);
            }
            e.dirty = true;
        }
    }

    pub(super) fn editor_undo(&mut self) {
        let Some(g) = self.gs_mut() else { return };
        let Some(e) = g.editor.as_deref_mut() else {
            return;
        };
        if let Some(save) = e.undo.pop() {
            g.world.editor_restore(save);
            e.scene_pending = true;
            e.radar_stale = true;
            e.msg = None;
        }
        self.editor_changed(true);
    }

    /// After an edit: rebuild the scene (throttled) and the panel.
    pub(super) fn editor_changed(&mut self, terrain: bool) {
        if let Some(e) = self.ed_mut() {
            e.scene_pending = true;
            e.radar_stale |= terrain;
        }
        self.rebuild_ui();
    }

    /// Called every frame: the throttled scene and radar refresh.
    pub(super) fn editor_tick(&mut self, dt_ms: f32) {
        let held = self.press.is_some() || self.ed().is_some_and(|e| e.press.is_some());
        let Some(g) = self.gs_mut() else { return };
        let Some(e) = g.editor.as_deref_mut() else {
            return;
        };
        e.scene_wait = (e.scene_wait - dt_ms).max(0.0);
        if e.scene_pending && e.scene_wait <= 0.0 {
            e.scene_pending = false;
            e.scene_wait = 250.0;
            let radar = e.radar_stale && !held;
            if radar {
                e.radar_stale = false;
            }
            if let Some(save) = g.world.save_ref() {
                g.scene_fp = scene_fingerprint(save);
                if radar {
                    g.radar = Some(crate::radar::Radar::new(&save.map));
                }
                self.pending_live = Some((LiveFrame::from_save(save), true));
            }
            self.rebuild_scene = true;
            self.rebuild_ui();
        }
    }

    /// Applies the tool of the current group at a tile.
    pub(super) fn editor_click(&mut self, x: i32, y: i32, first: bool) {
        let Some(e) = self.ed() else { return };
        let (group, sub, brush) = (e.group, e.sub, e.brush);
        let (class, mix, tree) = (
            SOLID_CLASSES[e.class_slot.min(15)],
            e.mix_slot,
            TREE_CLASSES[e.tree_slot.min(7)],
        );
        let (mode, add_row, style, terr) = (e.fill_mode, e.add_row, e.style, e.terr_sel);
        let flatten = self.shift || self.ctrl;
        match group {
            Group::Select => {
                if let Some(id) = self
                    .editor_world()
                    .and_then(|w| w.editor_save())
                    .map(|s| editor::place_on_tile(s, x, y))
                    .filter(|&id| id != 0)
                {
                    self.editor_place_dialog(id, false);
                }
                return;
            }
            Group::Height => {
                if sub == 0 {
                    return;
                }
                self.push_undo();
                let r = RAISE_RADIUS[brush];
                let w = self.world_mut();
                match sub {
                    1 if flatten => {
                        w.editor_elevation(Elevation::Flatten, x, y, r, editor::FLATTEN_AMOUNT)
                    }
                    1 => w.editor_elevation(Elevation::Smooth, x, y, r, SMOOTH_AMOUNT),
                    _ => w.editor_elevation(Elevation::Lower, x, y, r, editor::LOWER_AMOUNT),
                }
            }
            Group::Paint => {
                if !first && sub == 1 {
                    return;
                }
                self.push_undo();
                let w = self.world_mut();
                if sub == 0 {
                    w.editor_paint(x, y, PAINT_RADIUS[brush], usize::from(class));
                } else {
                    w.editor_paint_mixed(x, y, PAINT_RADIUS[brush], mix);
                }
            }
            Group::Fill => {
                if !first {
                    return;
                }
                self.push_undo();
                let ok = self
                    .world_mut()
                    .editor_fill(x, y, usize::from(class), mode, sub == 0);
                if !ok {
                    self.editor_undo_silent();
                }
            }
            Group::Trees => {
                self.push_undo();
                let strength = if sub == 0 { 1 } else { -1 };
                self.world_mut()
                    .editor_trees(x, y, TREE_RADIUS[brush], strength, tree);
            }
            Group::Territory => {
                let ids: Vec<u8> = self
                    .editor_world()
                    .map(|w| w.editor_territories().iter().map(|t| t.id).collect())
                    .unwrap_or_default();
                let Some(&id) = ids.get(terr) else { return };
                self.push_undo();
                self.world_mut()
                    .editor_territory_paint(x, y, PAINT_RADIUS[brush], id);
            }
            Group::Add => {
                if !first {
                    return;
                }
                self.editor_add_click(x, y, add_row, style);
                return;
            }
            Group::Bulldoze => {
                self.push_undo();
                let r = self.world_mut().editor_bulldoze(x, y, DOZER_RADIUS[brush]);
                if let Err(err) = r {
                    self.editor_undo_silent();
                    let m = self.editor_error_text(err);
                    self.ed_msg(m, false);
                }
            }
            Group::Overview => return,
        }
        self.editor_changed(true);
    }

    fn world_mut(&mut self) -> &mut World {
        match &mut self.screen {
            Screen::Game(g) => &mut g.world,
            Screen::Menu(_) => unreachable!("the editor is open"),
        }
    }

    /// Drops the last undo step without applying it (the edit was refused).
    fn editor_undo_silent(&mut self) {
        if let Some(e) = self.ed_mut() {
            e.undo.pop();
        }
    }

    pub(super) fn editor_error_text(&self, e: editor::EditorError) -> String {
        use editor::EditorError as E;
        let t = &self.texts;
        match e {
            E::InvalidPlacement => t.get(534, "You can't put this building here!"),
            E::PlaceHere => t.get(395, "You can't add one city over another."),
            E::TooManyTerritories => t.a("The map already has 31 territories."),
            E::NoSuchPlace | E::NoSuchTerritory | E::NoMap => t.a("That object no longer exists."),
            E::Command(c) => self.cmd_error_text(c, 1126),
        }
    }

    /// Tool 7, 0xd, 0xf, 0x10 and 0x13: the entry of the add list.
    fn editor_add_click(&mut self, x: i32, y: i32, row: usize, style: usize) {
        match row {
            0..=2 => {
                let kind = [PlaceKind::City, PlaceKind::Region, PlaceKind::Label][row];
                self.push_undo();
                let name = self.texts.get(0x1d, "Unnamed");
                match self.world_mut().editor_place_add(x, y, kind, &name) {
                    Ok(id) => {
                        // The dialog of `0x433eb0`: OK keeps the place, Cancel removes it.
                        self.editor_place_dialog(id, true);
                    }
                    Err(e) => {
                        self.editor_undo_silent();
                        let m = self.editor_error_text(e);
                        self.ed_msg(m, false);
                    }
                }
            }
            3 => {
                self.push_undo();
                let n = if self.shift { 1 } else { 0 };
                let _ = n;
                self.world_mut().editor_river(x, y, true);
            }
            4 => {
                self.push_undo();
                let r = DOZER_RADIUS[self.ed().map_or(1, |e| e.brush)];
                self.world_mut().editor_reserve(x, y, r);
            }
            5 => {
                self.push_undo();
                self.world_mut().editor_road(x, y, true);
            }
            _ => {
                let shape = self
                    .ed()
                    .and_then(|e| e.shapes.get(row - 6))
                    .map(|s| s.1)
                    .unwrap_or(24);
                self.push_undo();
                let hint = STYLES[style.min(5)].1;
                if let Err(e) = self.world_mut().editor_build(x, y, shape, hint) {
                    self.editor_undo_silent();
                    let m = self.editor_error_text(e);
                    self.ed_msg(m, false);
                }
            }
        }
        self.editor_changed(true);
    }

    // ---- mouse and keys ---------------------------------------------------------------------

    /// Left press on the map with the editor open. Returns true when it was taken.
    pub(super) fn editor_press(&mut self) -> bool {
        if !self.editor_active() || self.editor_modal_open() {
            return false;
        }
        let Some(pos) = self.cursor else { return true };
        let Some(tile) = self.editor_tile_under(pos) else {
            return true;
        };
        let Some(e) = self.ed() else { return true };
        if e.group == Group::Height && e.sub == 0 {
            self.push_undo();
            if let Some(e) = self.ed_mut() {
                e.press = Some((tile.0, tile.1, pos.1));
            }
        } else {
            self.editor_click(tile.0, tile.1, true);
            if let Some(e) = self.ed_mut() {
                e.press = Some((tile.0, tile.1, pos.1));
            }
        }
        true
    }

    pub(super) fn editor_release(&mut self) {
        let drag = self.ed_mut().and_then(|e| e.press.take());
        if drag.is_some() {
            self.editor_changed(true);
        }
    }

    pub(super) fn editor_tile_under(&self, pos: (f32, f32)) -> Option<(i32, i32)> {
        let g = self.game()?;
        let save = g.world.save_ref()?;
        crate::pick::screen_to_tile(&g.camera, self.viewport, pos, &save.map)
    }

    /// The cursor moved: the hover tile, and the stroke of a held button.
    pub(super) fn editor_hover(&mut self, pos: (f32, f32)) {
        if !self.editor_active() {
            return;
        }
        let tile = if self.ui.over_ui() || self.editor_modal_open() {
            None
        } else {
            self.editor_tile_under(pos)
        };
        let Some(e) = self.ed() else { return };
        let (changed, press, group, sub, brush) =
            (e.hover != tile, e.press, e.group, e.sub, e.brush);
        if changed && let Some(e) = self.ed_mut() {
            e.hover = tile;
        }
        if let Some((px, py, y0)) = press
            && self.pointer_down()
        {
            if group == Group::Height && sub == 0 {
                // Dragging up raises the ground under the press, down lowers it
                // (`0x48d730`: the pixel step scaled to heights).
                let dy = y0 - pos.1;
                if dy.abs() >= 2.0 {
                    let amount = (dy * 0.75).round() as i32;
                    self.world_mut().editor_elevation(
                        Elevation::Raise,
                        px,
                        py,
                        RAISE_RADIUS[brush],
                        amount,
                    );
                    if let Some(e) = self.ed_mut() {
                        e.press = Some((px, py, pos.1));
                    }
                    self.editor_changed(true);
                }
                return;
            }
            if changed && let Some((x, y)) = tile {
                self.editor_click(x, y, false);
            }
            return;
        }
        if changed {
            self.rebuild_ui();
        }
    }

    fn pointer_down(&self) -> bool {
        self.press.is_none() && self.ed().is_some_and(|e| e.press.is_some())
    }

    /// Keys of the editor; true when consumed.
    pub(super) fn editor_key(
        &mut self,
        code: winit::keyboard::KeyCode,
        text: Option<&str>,
    ) -> bool {
        use winit::keyboard::KeyCode as K;
        if !self.editor_active() {
            return false;
        }
        if self.editor_modal_open() {
            return self.editor_modal_key(code, text);
        }
        match code {
            K::Escape => {
                self.editor_exit_request();
                true
            }
            K::KeyZ if self.ctrl => {
                self.editor_undo();
                true
            }
            K::KeyS if self.ctrl => {
                self.editor_save_request(false);
                true
            }
            K::F5 => {
                self.editor_save_request(false);
                true
            }
            _ => false,
        }
    }

    // ---- file actions -----------------------------------------------------------------------

    pub(super) fn editor_dirty(&self) -> bool {
        self.ed().is_some_and(|e| e.dirty)
    }

    pub(super) fn editor_exit_request(&mut self) {
        if self.editor_dirty() {
            self.editor_confirm(Pending::Exit);
        } else {
            self.editor_leave();
        }
    }

    /// Folder of our maps: `<saves>/maps`.
    pub(super) fn own_maps_dir(&self) -> Option<PathBuf> {
        self.saves_dir.as_ref().map(|d| d.join("maps"))
    }

    pub(super) fn editor_save_request(&mut self, as_new: bool) {
        let path = self.ed().and_then(|e| e.path.clone());
        match path {
            Some(p) if !as_new => {
                let r = self.editor_write(&p);
                self.editor_saved(r);
            }
            _ => self.editor_save_as_dialog(),
        }
    }

    /// Writes the edited map: place boxes recomputed, the overview refreshed, the checksum set.
    pub(super) fn editor_write(&mut self, path: &Path) -> Result<PathBuf, String> {
        if let Ok(install) = self.install.canonicalize()
            && let Some(parent) = path.parent()
            && parent
                .canonicalize()
                .map(|p| p.starts_with(&install))
                .unwrap_or(false)
        {
            return Err(self.texts.a("The original game's folder is read only."));
        }
        let bytes = {
            let Screen::Game(g) = &mut self.screen else {
                return Err(self.texts.a("No game state to show."));
            };
            g.world.editor_recompute();
            let mut s = g.world.to_save();
            // Editor mode: the saver keeps the player summary of the Players page (`0x477540`).
            rt2_sim::editor_opts::refresh_player_summary(&mut s);
            editor::refresh_preview(&mut s);
            s.to_bytes()
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        if let Some(e) = self.ed_mut() {
            e.dirty = false;
            e.path = Some(path.to_path_buf());
        }
        Ok(path.to_path_buf())
    }

    pub(super) fn editor_saved(&mut self, r: Result<PathBuf, String>) {
        let (m, good) = match r {
            Ok(p) => (
                self.texts.af(
                    "Saved %1",
                    &[p.file_name()
                        .map_or(String::new(), |n| n.to_string_lossy().into_owned())],
                ),
                true,
            ),
            Err(e) => (self.texts.af("Cannot save: %1", &[e]), false),
        };
        self.ed_msg(m, good);
        self.rebuild_ui();
    }

    /// Opens a map file in the editor.
    pub(super) fn editor_open_file(&mut self, path: &Path) {
        let r = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|raw| Save::parse(&raw).map_err(|e| e.to_string()));
        match r {
            Ok(save) => {
                let own = self.own_maps_dir().is_some_and(|d| path.starts_with(d));
                let keep = own.then(|| path.to_path_buf());
                if let Err(e) = self.start_editor(save, keep) {
                    self.ed_msg(format!("{e:#}"), false);
                }
            }
            Err(e) => {
                let m = self.texts.af("Cannot load: %1", &[e]);
                self.ed_msg(m, false);
                self.rebuild_ui();
            }
        }
    }

    // ---- activation -------------------------------------------------------------------------

    pub(super) fn editor_activated(&mut self, action: u32) -> bool {
        if !self.editor_active() {
            return false;
        }
        if (A_MODAL..A_MODAL + 700).contains(&action) {
            return self.editor_modal_activated(action);
        }
        if self.editor_modal_open() {
            return true;
        }
        let Some(e) = self.ed_mut() else {
            return false;
        };
        match action {
            a if (A_GROUP..A_GROUP + 9).contains(&a) => {
                e.group = Group::ALL[(a - A_GROUP) as usize];
                e.sub = 0;
                e.msg = None;
            }
            A_PAL_UP => e.pal_row = e.pal_row.saturating_sub(1),
            A_PAL_DOWN => e.pal_row = (e.pal_row + 1).min(1),
            a if (A_SUB..A_SUB + 16).contains(&a) => e.sub = (a - A_SUB) as usize,
            a if (A_BRUSH..A_BRUSH + 5).contains(&a) => e.brush = (a - A_BRUSH) as usize,
            a if (A_STYLE..A_STYLE + 6).contains(&a) => e.style = (a - A_STYLE) as usize,
            a if (A_FILL_MODE..A_FILL_MODE + 3).contains(&a) => {
                e.fill_mode =
                    [FillMode::Match, FillMode::Above, FillMode::Below][(a - A_FILL_MODE) as usize]
            }
            a if (A_ITEM..A_ITEM + 100).contains(&a) => {
                let i = (a - A_ITEM) as usize;
                match (e.group, e.sub) {
                    (Group::Paint, 1) => e.mix_slot = i,
                    (Group::Trees, _) => e.tree_slot = i,
                    _ => e.class_slot = i,
                }
            }
            A_UNDO => {
                self.editor_undo();
                return true;
            }
            A_OPTIONS => {
                self.editor_options_dialog();
                return true;
            }
            A_RULES => {
                self.editor_rules_dialog();
                return true;
            }
            A_SAVE => {
                self.editor_save_request(false);
                return true;
            }
            A_SAVE_AS => {
                self.editor_save_request(true);
                return true;
            }
            A_OPEN => {
                if self.editor_dirty() {
                    self.editor_confirm(Pending::Open);
                } else {
                    self.editor_open_dialog();
                }
                return true;
            }
            A_NEW => {
                if self.editor_dirty() {
                    self.editor_confirm(Pending::New);
                } else {
                    self.editor_new_dialog();
                }
                return true;
            }
            A_EXIT => {
                self.editor_exit_request();
                return true;
            }
            A_TERR_NEW => {
                self.editor_territory_new();
                return true;
            }
            A_TERR_EDIT => {
                self.editor_territory_edit();
                return true;
            }
            _ => return false,
        }
        self.rebuild_ui();
        true
    }

    pub(super) fn editor_row(&mut self, action: u32, row: usize) -> bool {
        if !self.editor_active() {
            return false;
        }
        if (A_MODAL..A_MODAL + 700).contains(&action) {
            return self.editor_modal_row(action, row);
        }
        let Some(e) = self.ed_mut() else {
            return false;
        };
        match action {
            A_ADD_LIST => e.add_row = row,
            A_TERR_LIST => e.terr_sel = row,
            _ => return false,
        }
        self.rebuild_ui();
        true
    }

    // ---- the panel --------------------------------------------------------------------------

    pub(super) fn build_editor(&mut self, ui: &mut Ui) {
        let (cw, ch) = ui.logical_size();
        let dz = design_scale(ch);
        let ph = bottom_height(ch, dz);
        let inset = (8.0 * dz).round();
        let t = &self.texts;
        let Some(ed) = self.ed() else { return };
        let (group, sub, brush) = (ed.group, ed.sub, ed.brush);
        // The original panel art (`bp*.imb`) carries its own message strip, so the clone's status
        // bar is dropped when it is drawn (editor_panel.rs).
        let art = self.editor_panel_art(group, sub);
        let sl = if art.is_some() {
            0.0
        } else {
            (22.0 * dz).round()
        };
        let mut tips: Vec<(NodeId, String)> = Vec::new();
        // The highlights over the map go first: the panel and the dialogs draw over them.
        self.build_editor_overlay(ui);

        let bottom = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Bottom, 0.0, ph),
        );
        let mut status = None;
        if art.is_none() {
            let status_bar = ui.add(
                bottom,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x10, 0x20, 0x14, 0xe0)),
                },
                Layout::at(Anchor::Bottom, 0.0, sl).offset(inset, 4.0),
            );
            status = Some(ui.add(
                status_bar,
                lbl("", fnt(2, dz * 0.9), TEXT, Align::Left),
                Layout::fill().offset(8.0 * dz, 0.0),
            ));
        }
        let body_y = inset;
        let body_h = ph - sl - 2.0 * inset - if art.is_some() { 0.0 } else { 4.0 };

        // Left: the tool groups.
        let names = [
            t.a("Select"),
            t.a("Height"),
            t.a("Paint"),
            t.a("Fill"),
            t.a("Trees"),
            t.a("Territories"),
            t.a("Add"),
            t.a("Bulldoze"),
            t.a("Overview"),
        ];
        let tips_g = [
            t.a("Click a city, region or label to edit it."),
            t.a("Raise, lower and smooth the ground."),
            t.a("Paint terrain, solid or mixed."),
            t.a("Fill an area with terrain."),
            t.a("Add and cut trees."),
            t.a("Draw territories on the map."),
            t.a("Add buildings, industries, cities, rivers and roads."),
            t.a("Clear track, trees and buildings."),
            t.a("See the territories on the radar."),
        ];
        let gw = 112.0 * dz;
        let gh = (body_h / 9.0).floor();
        for (i, name) in names.iter().enumerate() {
            let n = ui.add(
                bottom,
                Kind::Toggle {
                    text: name.clone(),
                    font: fnt(1, dz * 0.9),
                    action: A_GROUP + i as u32,
                    on: Group::ALL[i] == group,
                    enabled: true,
                },
                at(inset, body_y + i as f32 * gh, gw, gh - 2.0),
            );
            tips.push((n, tips_g[i].clone()));
        }

        // Right: file and option buttons, and the radar.
        let rw = 124.0 * dz;
        let rx = cw - inset - rw;
        let buttons = [
            (t.a("Undo"), A_UNDO, t.a("Undo the last change.")),
            (
                t.a("Map Options..."),
                A_OPTIONS,
                t.a("Name, goals, restrictions and economy of the scenario."),
            ),
            (
                t.a("Rules..."),
                A_RULES,
                t.a("Edit the scenario's events and victory rules."),
            ),
            (t.a("Save"), A_SAVE, t.a("Save the map.")),
            (
                t.a("Save As..."),
                A_SAVE_AS,
                t.a("Save the map under a new name."),
            ),
            (t.a("Open..."), A_OPEN, t.a("Open another map.")),
            (t.a("New Map..."), A_NEW, t.a("Start a new, empty map.")),
            (t.get(lng::QUIT, "Quit"), A_EXIT, t.a("Leave the editor.")),
        ];
        // With the panel art the Undo button sits on the plate of the `bpmisc` strip.
        let buttons: Vec<_> = buttons
            .into_iter()
            .filter(|(_, a, _)| art.is_none() || *a != A_UNDO)
            .collect();
        let bh = (body_h / buttons.len() as f32).floor();
        for (i, (text, action, tip)) in buttons.into_iter().enumerate() {
            let enabled = action != A_UNDO || !ed.undo.is_empty();
            let n = ui.add(
                bottom,
                Kind::Button {
                    text,
                    font: fnt(1, dz * 0.9),
                    action,
                    enabled,
                    hotkey: None,
                },
                at(rx, body_y + i as f32 * bh, rw, bh - 2.0),
            );
            tips.push((n, tip));
        }

        // Radar between the palette and the buttons.
        let side = (body_h).min(150.0 * dz);
        let radar_x = rx - side - inset;
        let mut radar_node = None;
        if let Screen::Game(g) = &self.screen
            && let Some(r) = &g.radar
        {
            let (cols, rows) = (r.cols as f32, r.rows as f32);
            let fit = (side - 4.0) / cols.max(rows);
            let box_ = ui.add(
                bottom,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x0c, 0x0f, 0x0c, 0xd8)),
                },
                at(radar_x, body_y, side, side),
            );
            let colors = if group == Group::Overview || group == Group::Territory {
                self.region_colors(r.cols, r.rows)
            } else {
                r.colors_with_view(&g.camera, self.viewport)
            };
            let n = ui.add(
                box_,
                Kind::Cells {
                    cols: r.cols,
                    rows: r.rows,
                    colors: Arc::new(colors),
                    action: super::mainwin::A_RADAR,
                },
                Layout::at(Anchor::Center, cols * fit, rows * fit),
            );
            tips.push((n, t.get(1772, "Radar area - click to recenter main view")));
            radar_node = Some(n);
        }
        let _ = radar_node;

        // Middle: the palette.
        let px = inset * 2.0 + gw;
        let pw = (radar_x - inset - px).max(100.0);
        if let Some(art) = art {
            let area = rt2_ui::Rect::new(px, body_y, pw, body_h);
            let s = self.build_art_panel(ui, bottom, art, area, dz, &mut tips);
            status = Some(s);
        } else {
            let pal = ui.add(
                bottom,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x0c, 0x0f, 0x0c, 0x90)),
                },
                at(px, body_y, pw, body_h),
            );
            self.build_palette(ui, pal, pw, body_h, dz, group, sub, brush, &mut tips);
        }

        // A message or the hint at the top edge of the panel.
        if art.is_some() {
            if let Some((m, good)) = &self.ed().and_then(|e| e.msg.clone()) {
                self.build_art_message(ui, m, *good, ph, dz);
            }
        } else if let Some((m, good)) = &self.ed().and_then(|e| e.msg.clone()) {
            ui.add(
                bottom,
                Kind::Label {
                    text: m.clone(),
                    font: fnt(8, dz * 0.62),
                    color: if *good { GOOD } else { BAD },
                    align: Align::Right,
                    wrap: false,
                },
                Layout::at(Anchor::BottomRight, cw * 0.5, sl).offset(inset + 8.0 * dz, 4.0),
            );
        }
        if let Screen::Game(g) = &mut self.screen {
            g.hud.status = status;
            g.hud.tips = tips;
        }
        if self.editor_modal_open() {
            self.build_editor_modal(ui);
        }
    }

    /// Territory colours for the radar cells (the overview of the territories): the territory
    /// number picks a hue, water stays dark.
    fn region_colors(&self, cols: usize, rows: usize) -> Vec<Color> {
        let Some(s) = self.editor_world().and_then(|w| w.editor_save()) else {
            return Vec::new();
        };
        let (w, h) = (s.map.width, s.map.height);
        let mut out = Vec::with_capacity(cols * rows);
        for cy in 0..rows {
            for cx in 0..cols {
                let x = (((cx as f32 + 0.5) * w as f32 / cols as f32) as usize).min(w - 1);
                let y = (((cy as f32 + 0.5) * h as f32 / rows as f32) as usize).min(h - 1);
                let tile = &s.map.tiles[y * w + x];
                out.push(if editor::class_at(s, x as i32, y as i32) == 18 {
                    Color::rgb(0x18, 0x30, 0x50)
                } else {
                    region_color(tile.region)
                });
            }
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn build_palette(
        &self,
        ui: &mut Ui,
        pal: NodeId,
        pw: f32,
        ph: f32,
        dz: f32,
        group: Group,
        sub: usize,
        brush: usize,
        tips: &mut Vec<(NodeId, String)>,
    ) {
        let t = &self.texts;
        let Some(ed) = self.ed() else { return };
        let pad = 6.0 * dz;
        let row_h = (24.0 * dz).round();
        let f = fnt(1, dz * 0.85);
        let toggle = |ui: &mut Ui, x: f32, y: f32, w: f32, text: String, action: u32, on: bool| {
            ui.add(
                pal,
                Kind::Toggle {
                    text,
                    font: f,
                    action,
                    on,
                    enabled: true,
                },
                at(x, y, w, row_h - 2.0),
            )
        };
        let mut y = pad;
        let title = match group {
            Group::Select => t.a("Select"),
            Group::Height => t.a("Height"),
            Group::Paint => t.a("Paint"),
            Group::Fill => t.a("Fill"),
            Group::Trees => t.a("Trees"),
            Group::Territory => t.a("Territories"),
            Group::Add => t.a("Add"),
            Group::Bulldoze => t.a("Bulldoze"),
            Group::Overview => t.a("Overview"),
        };
        ui.add(
            pal,
            lbl(&title.to_uppercase(), fnt(2, dz * 0.9), HEAD, Align::Left),
            at(pad, y, pw * 0.4, row_h),
        );
        // Sub-tools, to the right of the title.
        let subs: Vec<String> = match group {
            Group::Height => vec![
                t.a("Raise / lower (drag)"),
                t.a("Smooth (Shift: flatten)"),
                t.a("Lower to level 0"),
            ],
            Group::Paint => vec![t.a("Solid"), t.a("Mixed")],
            Group::Fill => vec![t.a("Adjacent"), t.a("All")],
            Group::Trees => vec![t.a("Add trees"), t.a("Cut trees")],
            _ => Vec::new(),
        };
        if !subs.is_empty() {
            let w = ((pw * 0.6 - pad) / subs.len() as f32).min(190.0 * dz);
            for (i, s) in subs.iter().enumerate() {
                toggle(
                    ui,
                    pw * 0.4 + i as f32 * w,
                    y,
                    w - 3.0,
                    s.clone(),
                    A_SUB + i as u32,
                    sub == i,
                );
            }
        }
        y += row_h + 2.0;
        // Brush sizes.
        if group.uses_brush() {
            ui.add(
                pal,
                lbl(&t.a("Brush"), f, MUTED, Align::Left),
                at(pad, y, 60.0 * dz, row_h),
            );
            for i in 0..5 {
                let n = toggle(
                    ui,
                    pad + 60.0 * dz + i as f32 * 34.0 * dz,
                    y,
                    32.0 * dz,
                    format!("{}", i + 1),
                    A_BRUSH + i as u32,
                    brush == i,
                );
                tips.push((n, t.a("The size of the brush.")));
            }
            y += row_h;
        }
        let avail_h = ph - y - pad;
        let cols_for = |n: usize, min_w: f32| {
            ((pw - 2.0 * pad) / (min_w * dz))
                .floor()
                .max(1.0)
                .min(n as f32) as usize
        };
        match group {
            Group::Select => {
                ui.add(
                    pal,
                    Kind::Label {
                        text: t.a("Click a city, region or label on the map to change its name, size, architecture and industries."),
                        font: f,
                        color: TEXT,
                        align: Align::Left,
                        wrap: true,
                    },
                    at(pad, y, pw - 2.0 * pad, avail_h),
                );
            }
            Group::Height => {
                let hint = match sub {
                    0 => t.a("Press on the map and drag up to raise the ground, down to lower it."),
                    1 => t.a("Click to smooth the ground. Hold Shift or Ctrl to level it to the height under the click."),
                    _ => t.a("Click to pull the ground down to level 0."),
                };
                ui.add(
                    pal,
                    Kind::Label {
                        text: hint,
                        font: f,
                        color: TEXT,
                        align: Align::Left,
                        wrap: true,
                    },
                    at(pad, y + 4.0, pw - 2.0 * pad, avail_h),
                );
            }
            Group::Paint | Group::Fill => {
                if group == Group::Fill {
                    let modes = [
                        (FillMode::Match, t.a("Match")),
                        (FillMode::Above, t.a("Above")),
                        (FillMode::Below, t.a("Below")),
                    ];
                    ui.add(
                        pal,
                        lbl(&t.a("Replace"), f, MUTED, Align::Left),
                        at(pad, y, 60.0 * dz, row_h),
                    );
                    for (i, (m, name)) in modes.into_iter().enumerate() {
                        let n = toggle(
                            ui,
                            pad + 60.0 * dz + i as f32 * 80.0 * dz,
                            y,
                            76.0 * dz,
                            name,
                            A_FILL_MODE + i as u32,
                            ed.fill_mode == m,
                        );
                        tips.push((
                            n,
                            [
                                t.a("Fill tiles of the same terrain as the one clicked."),
                                t.a("Fill tiles at least as high as the one clicked."),
                                t.a("Fill tiles at most as high as the one clicked."),
                            ][i]
                                .clone(),
                        ));
                    }
                    y += row_h;
                }
                let mixed = group == Group::Paint && sub == 1;
                let n_items = if mixed {
                    editor::MIX_COUNT
                } else {
                    SOLID_CLASSES.len()
                };
                let cols = cols_for(n_items, 96.0).max(4);
                let rows = n_items.div_ceil(cols);
                let w = (pw - 2.0 * pad) / cols as f32;
                let h = ((avail_h - (y - (ph - avail_h - pad))) / rows as f32)
                    .clamp(row_h * 0.8, row_h * 1.3);
                for i in 0..n_items {
                    let (cx, cy) = (i % cols, i / cols);
                    let (text, on, tip) = if mixed {
                        (
                            self.texts.af("Mix %1", &[(i + 1).to_string()]),
                            ed.mix_slot == i,
                            mix_text(i),
                        )
                    } else {
                        let c = SOLID_CLASSES[i];
                        (self.class_name(c), ed.class_slot == i, String::new())
                    };
                    let n = ui.add(
                        pal,
                        Kind::Toggle {
                            text,
                            font: f,
                            action: A_ITEM + i as u32,
                            on,
                            enabled: true,
                        },
                        at(pad + cx as f32 * w, y + cy as f32 * h, w - 3.0, h - 2.0),
                    );
                    if !tip.is_empty() {
                        tips.push((n, tip));
                    }
                }
            }
            Group::Trees => {
                let cols = cols_for(8, 90.0).max(4);
                let w = (pw - 2.0 * pad) / cols as f32;
                for i in 0..8 {
                    let (cx, cy) = (i % cols, i / cols);
                    toggle(
                        ui,
                        pad + cx as f32 * w,
                        y + cy as f32 * row_h,
                        w - 3.0,
                        self.texts.af("Trees %1", &[(i + 1).to_string()]),
                        A_ITEM + i as u32,
                        ed.tree_slot == i,
                    );
                }
            }
            Group::Territory => {
                let items: Vec<String> = self
                    .editor_world()
                    .map(|w| {
                        w.editor_territories()
                            .iter()
                            .map(|t| format!("{:>2}  {}", t.id, t.name))
                            .collect()
                    })
                    .unwrap_or_default();
                let lw = pw * 0.6;
                let list_h = (ph - y - pad).max(40.0);
                let sel = ed.terr_sel.min(items.len().saturating_sub(1));
                ui.add(
                    pal,
                    Kind::List {
                        items,
                        selected: Some(sel),
                        scroll: sel.saturating_sub(3),
                        row_h: (18.0 * dz).round(),
                        font: f,
                        action: A_TERR_LIST,
                    },
                    at(pad, y, lw, list_h),
                );
                let bx = pad * 2.0 + lw;
                let bw = pw - bx - pad;
                ui.add(
                    pal,
                    Kind::Button {
                        text: t.a("New territory..."),
                        font: f,
                        action: A_TERR_NEW,
                        enabled: true,
                        hotkey: None,
                    },
                    at(bx, y, bw, row_h),
                );
                ui.add(
                    pal,
                    Kind::Button {
                        text: t.a("Edit territory..."),
                        font: f,
                        action: A_TERR_EDIT,
                        enabled: true,
                        hotkey: None,
                    },
                    at(bx, y + row_h + 2.0, bw, row_h),
                );
                ui.add(
                    pal,
                    Kind::Label {
                        text: t.a("Choose a territory, then paint its tiles on the map."),
                        font: f,
                        color: MUTED,
                        align: Align::Left,
                        wrap: true,
                    },
                    at(bx, y + 2.0 * row_h + 6.0, bw, avail_h - 2.0 * row_h),
                );
            }
            Group::Add => {
                let items = self.add_items();
                let lw = pw * 0.55;
                let list_h = (ph - y - pad).max(40.0);
                let sel = ed.add_row.min(items.len().saturating_sub(1));
                ui.add(
                    pal,
                    Kind::List {
                        items,
                        selected: Some(sel),
                        scroll: sel.saturating_sub(4),
                        row_h: (18.0 * dz).round(),
                        font: f,
                        action: A_ADD_LIST,
                    },
                    at(pad, y, lw, list_h),
                );
                let bx = pad * 2.0 + lw;
                let bw = pw - bx - pad;
                if sel >= 6 {
                    ui.add(
                        pal,
                        lbl(&t.a("Architecture"), f, MUTED, Align::Left),
                        at(bx, y, bw, row_h),
                    );
                    for (i, (n, _)) in STYLES.iter().enumerate() {
                        let cols = 3usize;
                        let w = bw / cols as f32;
                        toggle(
                            ui,
                            bx + (i % cols) as f32 * w,
                            y + row_h + (i / cols) as f32 * row_h,
                            w - 3.0,
                            t.a(n),
                            A_STYLE + i as u32,
                            ed.style == i,
                        );
                    }
                }
                let hint = match sel {
                    0..=2 => t.a("Click the map to put a new place; its dialog opens next."),
                    3 => t.a("Click the map to lay a river tile."),
                    4 => t.a("Click the map to toggle reserved cells (use the brush size)."),
                    5 => t.a("Click the map to lay a road tile."),
                    _ => t.a("Click the map to place the building. Green marks where it fits."),
                };
                ui.add(
                    pal,
                    Kind::Label {
                        text: hint,
                        font: f,
                        color: MUTED,
                        align: Align::Left,
                        wrap: true,
                    },
                    at(bx, y + 3.0 * row_h + 4.0, bw, avail_h - 3.0 * row_h),
                );
                if sel == 4 {
                    for i in 0..5 {
                        toggle(
                            ui,
                            bx + i as f32 * 34.0 * dz,
                            y,
                            32.0 * dz,
                            format!("{}", i + 1),
                            A_BRUSH + i as u32,
                            brush == i,
                        );
                    }
                }
            }
            Group::Bulldoze => {
                ui.add(
                    pal,
                    Kind::Label {
                        text: t.a("Click to bulldoze the shaded area: track, trees, houses and industries. Stations cannot be bulldozed."),
                        font: f,
                        color: TEXT,
                        align: Align::Left,
                        wrap: true,
                    },
                    at(pad, y + 4.0, pw - 2.0 * pad, avail_h),
                );
            }
            Group::Overview => {
                ui.add(
                    pal,
                    Kind::Label {
                        text: t.a("The radar shows each territory in its own colour."),
                        font: f,
                        color: TEXT,
                        align: Align::Left,
                        wrap: true,
                    },
                    at(pad, y + 4.0, pw - 2.0 * pad, avail_h),
                );
            }
        }
    }

    pub(super) fn class_name(&self, c: u8) -> String {
        match c {
            0..=12 => self.texts.get(700 + u32::from(c), ""),
            18 => self.texts.get(713, "ocean"),
            c => self.texts.af("Terrain %1", &[c.to_string()]),
        }
    }

    /// Rows of the add list: the special entries, then the shapes by name.
    pub(super) fn add_items(&self) -> Vec<String> {
        let t = &self.texts;
        let mut v = vec![
            t.get(536, "** Special - Add City **"),
            t.get(537, "** Special - Add Region **"),
            t.get(538, "** Special - Add Label **"),
            t.get(539, "** Special - Lay River **"),
            t.get(540, "** Special - Reserve Cells **"),
            t.get(3321, "** Special - Lay Road **"),
        ];
        if let Some(e) = self.ed() {
            v.extend(e.shapes.iter().map(|(n, s)| format!("{n}  #{s}")));
        }
        v
    }

    // ---- the overlay over the map -----------------------------------------------------------

    fn build_editor_overlay(&self, ui: &mut Ui) {
        let Some(g) = self.game() else { return };
        let (Some(e), Some(save)) = (g.editor.as_deref(), g.world.save_ref()) else {
            return;
        };
        let map = &save.map;
        let root = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let tile = |ui: &mut Ui, x: i32, y: i32, c: Color, shrink: f32| {
            if x < 0 || y < 0 || x as usize >= map.width || y as usize >= map.height {
                return;
            }
            let (px, py, w, h) = self.tile_rect(g, map, x, y, shrink);
            let (cw, ch) = ui.logical_size();
            if px + w < 0.0
                || px > cw
                || py + h < 0.0
                || py + h > ch - bottom_height(ch, design_scale(ch))
            {
                return;
            }
            ui.add(
                root,
                Kind::Panel {
                    fill: Fill::Overlay(c),
                },
                at(px, py, w, h),
            );
        };
        // Places: a marker on each city, region and label, brighter when it is a city.
        if e.modal.is_none() {
            for r in save.places.ordered().take(900) {
                let (x, y) = (
                    i32::from(pool_u16(&r.data, 4)),
                    i32::from(pool_u16(&r.data, 6)),
                );
                let c = match r.data[0x10] {
                    0 => Color(0xff, 0xd0, 0x40, 0xd0),
                    1 => Color(0x60, 0xd0, 0xff, 0xb0),
                    _ => Color(0xd0, 0xd0, 0xd0, 0x90),
                };
                tile(ui, x, y, c, 0.3);
            }
        }
        // The brush footprint under the cursor.
        if let Some((hx, hy)) = e.hover {
            let r = match e.group {
                Group::Height => RAISE_RADIUS[e.brush],
                Group::Trees => TREE_RADIUS[e.brush],
                Group::Paint | Group::Territory => PAINT_RADIUS[e.brush],
                Group::Bulldoze => DOZER_RADIUS[e.brush],
                Group::Add if e.add_row == 4 => DOZER_RADIUS[e.brush],
                _ => 0,
            };
            let c = if e.group == Group::Bulldoze {
                Color(0xf0, 0x90, 0x30, 0x70)
            } else {
                Color(0xff, 0xff, 0xff, 0x50)
            };
            let square = e.group == Group::Paint
                || e.group == Group::Territory
                || (e.group == Group::Add && e.add_row == 4);
            for dx in -r..=r {
                for dy in -r..=r {
                    let edge = dx.abs() == r || dy.abs() == r;
                    let inside =
                        square || rt2_sim::track::tile_distance(hx + dx, hy + dy, hx, hy) <= r;
                    if (edge || r <= 3) && inside {
                        tile(ui, hx + dx, hy + dy, c, 0.1);
                    }
                }
            }
            tile(ui, hx, hy, Color(0xff, 0xff, 0xff, 0x80), 0.0);
        }
        // A building about to be placed.
        if e.group == Group::Add
            && e.add_row >= 6
            && let (Some((hx, hy)), Some((_, shape))) = (e.hover, e.shapes.get(e.add_row - 6))
            && let Some(data) = self.data.as_ref()
        {
            let ok = self.shape_fits(*shape, hx, hy);
            let (good, bad) = (Color(0x60, 0xe0, 0x60, 0x90), Color(0xf0, 0x50, 0x40, 0x90));
            for (lin, col) in rt2_sim::tilemark::cells() {
                if rt2_sim::tilemark::shape_cell(&data.image, *shape as usize, lin, col) & 0x3c3
                    != 0
                {
                    let ext = rt2_sim::tilemark::shape_extents(&data.image, *shape as usize);
                    let (ox, oy) = ((ext[0] >> 1) + hx, (ext[1] >> 1) + hy);
                    tile(ui, ox - col, oy - lin, if ok { good } else { bad }, 0.0);
                }
            }
        }
    }

    /// Whether a shape fits at a tile (the placement test of `0x4087a0`), on a copy of the world
    /// so the real one is not touched.
    fn shape_fits(&self, shape: i32, x: i32, y: i32) -> bool {
        let Some(g) = self.game() else { return false };
        let mut w = g.world.clone();
        w.editor_can_build(x, y, shape)
    }

    // ---- screenshots and tests --------------------------------------------------------------

    /// Opens a map file in the editor (screenshots, tests); errors are returned.
    pub fn open_editor(&mut self, path: &Path) -> anyhow::Result<()> {
        let raw = std::fs::read(path)?;
        let save = Save::parse(&raw)?;
        self.start_editor(save, None)
    }

    /// Opens a dialog of the editor by name (screenshots): `new`, `open`, `save-as`, `options`,
    /// `goals`, `restrictions`, `economy`, `industries`, `rules`, `place`, `territory`.
    pub fn editor_show_dialog(&mut self, name: &str) -> bool {
        if !self.editor_active() {
            return false;
        }
        match name {
            "new" => self.editor_new_dialog(),
            "open" => self.editor_open_dialog(),
            "save-as" => self.editor_save_as_dialog(),
            n if super::editor_modals::OPT_TABS.contains(&n) => {
                self.editor_options_dialog();
                let tab = super::editor_modals::OPT_TABS
                    .iter()
                    .position(|n| *n == name)
                    .unwrap_or(0);
                self.editor_modal_activated(A_MODAL + 80 + tab as u32);
            }
            "rules" => self.editor_rules_dialog(),
            // `image:<file>`: New Map > From Image with that PCX (screenshots).
            n if n.starts_with("image:") => {
                self.editor_new_dialog();
                if let Some(Modal::NewMap(m)) = self.ed_mut().and_then(|e| e.modal.as_mut()) {
                    m.image = n["image:".len()..].to_owned();
                    m.w = 120;
                    m.h = 100;
                }
                self.editor_modal_activated(A_MODAL + 66);
            }
            "place" => {
                let id = self
                    .editor_world()
                    .and_then(|w| w.editor_places().first().map(|p| p.id));
                match id {
                    Some(id) => self.editor_place_dialog(id, false),
                    None => return false,
                }
            }
            "territory" => self.editor_territory_edit(),
            _ => return false,
        }
        true
    }

    /// Selects a row of the add list (tests).
    #[cfg(test)]
    pub fn editor_select_row_for_test(&mut self, row: usize) {
        if let Some(e) = self.ed_mut() {
            e.add_row = row;
        }
    }

    /// The text typed into the focused field of a dialog (tests).
    #[cfg(test)]
    pub fn editor_type(&mut self, text: &str) {
        self.editor_modal_key(winit::keyboard::KeyCode::KeyA, Some(text));
    }

    /// Selects a tool group by name (screenshots): `select`, `height`, `paint`, `fill`, `trees`,
    /// `territory`, `add`, `bulldoze`, `overview`; `sub` picks the tool inside.
    pub fn editor_show_group(&mut self, name: &str, sub: usize, item: Option<usize>) -> bool {
        let g = match name {
            "select" => Group::Select,
            "height" => Group::Height,
            "paint" => Group::Paint,
            "fill" => Group::Fill,
            "trees" => Group::Trees,
            "territory" => Group::Territory,
            "add" => Group::Add,
            "bulldoze" => Group::Bulldoze,
            "overview" => Group::Overview,
            _ => return false,
        };
        let Some(e) = self.ed_mut() else {
            return false;
        };
        e.group = g;
        if g == Group::Add {
            e.add_row = sub;
        } else {
            e.sub = sub;
        }
        if let Some(i) = item {
            match (g, sub) {
                (Group::Paint, 1) => e.mix_slot = i.min(17),
                (Group::Trees, _) => e.tree_slot = i.min(7),
                _ => e.class_slot = i.min(15),
            }
        }
        self.rebuild_ui();
        true
    }

    /// Moves the hover to a tile (screenshots).
    pub fn editor_set_hover(&mut self, x: i32, y: i32) {
        if let Some(e) = self.ed_mut() {
            e.hover = Some((x, y));
        }
        self.rebuild_ui();
    }

    /// Applies the current tool at a tile as a click would (screenshots, tests).
    pub fn editor_apply(&mut self, x: i32, y: i32) {
        self.editor_click(x, y, true);
    }
}

fn pool_u16(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}

/// One colour per territory number, spread around the hue circle.
pub(super) fn region_color(region: u8) -> Color {
    let h = (u32::from(region) * 47 % 360) as f32;
    let (s, v) = (0.55f32, 0.85f32);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// "desert 70, wasteland 30": what a mixed-paint entry draws from.
pub(super) fn mix_text(i: usize) -> String {
    const NAMES: [&str; 8] = [
        "beach",
        "desert",
        "wasteland",
        "dirt",
        "dry grass",
        "grass",
        "lush grass",
        "swamp",
    ];
    editor::MIX_WEIGHTS[i]
        .iter()
        .enumerate()
        .filter(|(_, w)| **w > 0)
        .map(|(c, w)| format!("{} {}", NAMES.get(c).copied().unwrap_or("?"), w))
        .collect::<Vec<_>>()
        .join(", ")
}
