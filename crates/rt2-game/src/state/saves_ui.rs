//! Save Game and Load Game dialogs for the clone's own saves, quick save / quick load and the
//! autosave rotation (docs/ui.md "Opcoes e saves"). The files and the sidecars are `saves.rs`;
//! this file is the dialogs and the glue to the running game.
//!
//! The Load dialog lists our saves (newest first, with date, scenario and company from the
//! sidecar, and a terrain thumbnail) and, below them, the original's own `.GAM` games from the
//! install (docs/formats/map-gam.md), which open through the same loader.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_ui::{Align, Anchor, Color, Fill, Input, Key, Kind, Layout, Ui, UiEvent, game_font};
use winit::keyboard::KeyCode;

use super::{AppState, Overlay, Screen, player_cash};
use crate::catalog;
use crate::saves::{self, SaveEntry, SaveKind, SaveMeta, Thumbnail};

pub(super) const A_SV_LIST: u32 = 9000;
pub(super) const A_SV_OK: u32 = 9001;
pub(super) const A_SV_DELETE: u32 = 9002;
pub(super) const A_SV_CANCEL: u32 = 9003;

pub(super) fn owns(a: u32) -> bool {
    (9000..9100).contains(&a)
}

const DIM: Color = Color(0, 0, 0, 170);
const CREAM: Color = Color::rgb(0xf2, 0xe9, 0xd0);
const GOLD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const WARN: Color = Color::rgb(0xff, 0xb0, 0x60);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavesMode {
    Save,
    Load,
}

struct SaveRow {
    entry: SaveEntry,
    /// One of the original game's `.GAM` files (listed after ours).
    original: bool,
}

/// What the right-hand side shows for the selected save.
#[derive(Default)]
struct Detail {
    meta: Option<SaveMeta>,
    thumb: Option<Thumbnail>,
    /// Scenario title read from the file when there is no sidecar.
    title: String,
}

pub(super) struct SavesState {
    mode: SavesMode,
    rows: Vec<SaveRow>,
    sel: Option<usize>,
    /// Save mode: the file name being typed.
    name: String,
    /// The Delete button was pressed once and waits for a second press.
    armed: bool,
    message: Option<(String, bool)>,
    detail: Detail,
}

#[cfg(test)]
impl SavesState {
    pub(super) fn rows_len(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn stem_at(&self, i: usize) -> &str {
        &self.rows[i].entry.stem
    }
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

/// The radar's colours as a thumbnail.
fn thumbnail_of(radar: &crate::radar::Radar) -> Thumbnail {
    Thumbnail {
        w: radar.cols as u32,
        h: radar.rows as u32,
        rgb: radar.colors().iter().map(|c| [c.0, c.1, c.2]).collect(),
    }
}

impl AppState {
    // ---- the game's side --------------------------------------------------------------------

    /// Our saves folder, unless it is inside the original's install (never written there).
    fn own_saves_dir(&self) -> Result<PathBuf, String> {
        let dir = self
            .saves_dir
            .clone()
            .ok_or_else(|| "no data folder".to_owned())?;
        if dir.starts_with(&self.install) {
            return Err("refusing to write inside the game install".to_owned());
        }
        Ok(dir)
    }

    /// The metadata of the game on screen.
    fn current_meta(&self, kind: SaveKind) -> Option<SaveMeta> {
        let Screen::Game(g) = &self.screen else {
            return None;
        };
        let d = g.world.date();
        let company = self
            .with_info(|i| {
                crate::buildtools::human_company(i.save).map(|c| i.company_name(c.max(0) as u32))
            })
            .flatten()
            .unwrap_or_default();
        Some(SaveMeta {
            version: saves::META_VERSION,
            kind,
            scenario: g
                .scenario
                .as_ref()
                .map_or_else(|| g.name.clone(), |s| s.title.clone()),
            company,
            year: d.year,
            month: d.month,
            day: d.day,
            cash: g
                .world
                .save_ref()
                .and_then(player_cash)
                .map(|c| c.round() as i64),
            saved_at: saves::now_unix(),
        })
    }

    /// Writes the running game as `stem` (manual or quick save).
    fn write_own_save(&self, stem: &str, kind: SaveKind) -> Result<PathBuf, String> {
        let Screen::Game(g) = &self.screen else {
            return Err("no game".to_owned());
        };
        let dir = self.own_saves_dir()?;
        let meta = self.current_meta(kind).ok_or("no game")?;
        let thumb = g.radar.as_ref().map(thumbnail_of);
        let bytes = g.world.to_save().to_bytes();
        saves::write_save(&dir, stem, &bytes, &meta, thumb.as_ref()).map_err(|e| format!("{e:#}"))
    }

    fn saved_toast(&mut self, r: Result<PathBuf, String>, quick: bool) {
        let m = match r {
            Ok(_) if quick => self.texts.a("Quick saved"),
            Ok(p) => self.texts.af("Saved %1", &[p.display().to_string()]),
            Err(e) => self.texts.af("Cannot save: %1", &[e]),
        };
        self.toast(m);
    }

    /// Writes the game as a manual save named `name` (screenshots, tests).
    pub fn save_as(&mut self, name: &str) -> Result<PathBuf, String> {
        self.write_own_save(&saves::sanitize_stem(name), SaveKind::Manual)
    }

    /// F5: the fixed quick-save slot.
    pub(super) fn quick_save(&mut self) {
        if !self.in_game() {
            return;
        }
        let r = self.write_own_save(saves::QUICK_STEM, SaveKind::Quick);
        self.saved_toast(r, true);
    }

    /// F9: loads the quick-save slot.
    pub(super) fn quick_load(&mut self) {
        let path = self
            .saves_dir
            .as_ref()
            .map(|d| saves::gam_path(d, saves::QUICK_STEM))
            .filter(|p| p.is_file());
        match path {
            Some(p) => {
                if let Err(e) = self.open_own_save(&p) {
                    let m = self.texts.af("Cannot load: %1", &[e]);
                    self.toast(m);
                }
            }
            None => {
                let m = self.texts.a("There is no quick save yet.");
                self.toast(m);
            }
        }
    }

    /// Opens one of our saves: it continues where it was, so no briefing is offered again.
    fn open_own_save(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.start(path).map_err(|e| format!("{e:#}"))?;
        self.overlay = None;
        self.rebuild_ui();
        Ok(())
    }

    /// Autosave when the configured number of game months has passed (called every frame).
    pub(super) fn autosave_tick(&mut self) {
        let (months, keep) = (
            self.config.gameplay.autosave_months,
            self.config.gameplay.autosave_keep,
        );
        if !self.persist || months == 0 {
            return;
        }
        let Screen::Game(g) = &mut self.screen else {
            return;
        };
        let d = g.world.date();
        let now = (d.year, d.month);
        if saves::months_between(g.last_autosave, now) < months as i32 {
            // A load into an earlier date restarts the count.
            if saves::months_between(g.last_autosave, now) < 0 {
                g.last_autosave = now;
            }
            return;
        }
        g.last_autosave = now;
        if g.over_shown || g.world.pending_choice().is_some() {
            return;
        }
        let Some(meta) = self.current_meta(SaveKind::Auto) else {
            return;
        };
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let thumb = g.radar.as_ref().map(thumbnail_of);
        let bytes = g.world.to_save().to_bytes();
        let result = self.own_saves_dir().and_then(|dir| {
            saves::autosave(&dir, keep.max(1) as usize, &bytes, &meta, thumb.as_ref())
                .map_err(|e| format!("{e:#}"))
        });
        match result {
            Ok(_) => {
                let m = self.texts.a("Autosaved");
                self.toast(m);
            }
            Err(e) => {
                let m = self.texts.af("Cannot save: %1", &[e]);
                self.toast(m);
            }
        }
    }

    // ---- opening the dialogs ----------------------------------------------------------------

    /// Opens Save Game (in a game) or Load Game.
    pub fn open_saves(&mut self, mode: SavesMode) {
        if mode == SavesMode::Save && !self.in_game() {
            return;
        }
        let mut rows: Vec<SaveRow> = self
            .saves_dir
            .as_deref()
            .map(saves::list)
            .unwrap_or_default()
            .into_iter()
            .map(|entry| SaveRow {
                entry,
                original: false,
            })
            .collect();
        if mode == SavesMode::Load {
            let mut originals = saves::list(&self.install.join("games"));
            originals.sort_by(|a, b| a.stem.cmp(&b.stem));
            rows.extend(originals.into_iter().map(|entry| SaveRow {
                entry,
                original: true,
            }));
        }
        let name = match (&self.screen, mode) {
            (Screen::Game(g), SavesMode::Save) => {
                let d = g.world.date();
                let base = g
                    .scenario
                    .as_ref()
                    .map_or_else(|| g.name.clone(), |s| s.title.clone());
                saves::sanitize_stem(&format!("{base} {}-{:02}", d.year, d.month))
            }
            _ => String::new(),
        };
        // Save: until a row is picked, the box shows the game being saved.
        let detail = match (mode, self.current_meta(SaveKind::Manual), &self.screen) {
            (SavesMode::Save, Some(meta), Screen::Game(g)) => Detail {
                meta: Some(meta),
                thumb: g.radar.as_ref().map(thumbnail_of),
                title: String::new(),
            },
            _ => Detail::default(),
        };
        let mut st = SavesState {
            mode,
            rows,
            sel: None,
            name,
            armed: false,
            message: None,
            detail,
        };
        if mode == SavesMode::Load && !st.rows.is_empty() {
            st.sel = Some(0);
            st.detail = self.detail_for(&st.rows[0]);
        }
        self.overlay = Some(Overlay::Saves(st));
        self.rebuild_ui();
    }

    /// Opens a dialog by name (screenshots): `save` or `load`.
    pub fn show_saves(&mut self, name: &str) -> bool {
        match name {
            "save" => self.open_saves(SavesMode::Save),
            "load" => self.open_saves(SavesMode::Load),
            _ => return false,
        }
        true
    }

    /// Metadata and thumbnail of a row: the sidecar's, else read from the file itself.
    fn detail_for(&self, row: &SaveRow) -> Detail {
        let path = &row.entry.path;
        let meta = row.entry.meta.clone();
        let mut thumb = saves::read_thumbnail(path);
        let mut title = String::new();
        if thumb.is_none() || meta.is_none() {
            // No sidecar (an original game, or a file copied by hand): read the map out of it.
            if let Ok(raw) = std::fs::read(path)
                && let Ok((map, _)) = catalog::load_any(&raw)
            {
                title = catalog::map_title(&map, path);
                if thumb.is_none() {
                    thumb = Some(thumbnail_of(&crate::radar::Radar::new(&map)));
                }
            }
        }
        Detail { meta, thumb, title }
    }

    // ---- input ------------------------------------------------------------------------------

    fn saves_mut(&mut self) -> Option<&mut SavesState> {
        match &mut self.overlay {
            Some(Overlay::Saves(s)) => Some(s),
            _ => None,
        }
    }

    pub(super) fn saves_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Saves(_)))
    }

    pub(super) fn saves_key(&mut self, code: KeyCode, repeat: bool, text: Option<&str>) {
        let ctrl = self.ctrl;
        let Some(st) = self.saves_mut() else {
            return;
        };
        let mode = st.mode;
        let k = match code {
            KeyCode::Escape if !repeat => {
                self.close_overlay();
                return;
            }
            KeyCode::Enter | KeyCode::NumpadEnter if !repeat => {
                self.saves_activated(A_SV_OK);
                return;
            }
            KeyCode::Delete if !repeat => {
                self.saves_activated(A_SV_DELETE);
                return;
            }
            KeyCode::Backspace if mode == SavesMode::Save => {
                if let Some(st) = self.saves_mut() {
                    st.name.pop();
                    st.armed = false;
                }
                self.rebuild_ui();
                return;
            }
            KeyCode::Tab => Some(if self.shift { Key::BackTab } else { Key::Tab }),
            KeyCode::ArrowUp => Some(Key::Up),
            KeyCode::ArrowDown => Some(Key::Down),
            KeyCode::PageUp => Some(Key::PageUp),
            KeyCode::PageDown => Some(Key::PageDown),
            KeyCode::Home => Some(Key::Home),
            KeyCode::End => Some(Key::End),
            _ => None,
        };
        if let Some(k) = k {
            let ev = self.ui.input(Input::KeyDown(k));
            self.handle_events(ev);
            return;
        }
        if mode == SavesMode::Save
            && !ctrl
            && let Some(t) = text
        {
            let typed: String = t.chars().filter(|c| !c.is_control()).collect();
            if !typed.is_empty()
                && let Some(st) = self.saves_mut()
                && st.name.chars().count() < 60
            {
                st.name.push_str(&typed);
                st.armed = false;
                self.rebuild_ui();
            }
        }
    }

    pub(super) fn saves_event(&mut self, e: &UiEvent) -> bool {
        let UiEvent::Selected(A_SV_LIST, row) = *e else {
            return false;
        };
        let Some(st) = self.saves_mut() else {
            return true;
        };
        st.sel = Some(row);
        st.armed = false;
        st.message = None;
        let mode = st.mode;
        let picked = st.rows.get(row).map(|r| (r.entry.clone(), r.original));
        if let Some((entry, original)) = picked {
            let d = self.detail_for(&SaveRow {
                entry: entry.clone(),
                original,
            });
            if let Some(st) = self.saves_mut() {
                st.detail = d;
                if mode == SavesMode::Save {
                    st.name = entry.stem;
                }
            }
        }
        self.rebuild_ui();
        true
    }

    pub(super) fn saves_activated(&mut self, a: u32) -> bool {
        let Some(st) = self.saves_mut() else {
            return false;
        };
        let mode = st.mode;
        match a {
            A_SV_CANCEL => self.close_overlay(),
            // A double click (or Enter) on a row of the Load dialog loads it.
            A_SV_LIST if mode == SavesMode::Load => return self.saves_activated(A_SV_OK),
            A_SV_LIST => {}
            A_SV_OK => match mode {
                SavesMode::Load => self.saves_load(),
                SavesMode::Save => self.saves_save(),
            },
            A_SV_DELETE => self.saves_delete(),
            _ => return false,
        }
        true
    }

    fn saves_load(&mut self) {
        let Some(st) = self.saves_mut() else { return };
        let Some(row) = st.sel.and_then(|i| st.rows.get(i)) else {
            return;
        };
        let (path, original) = (row.entry.path.clone(), row.original);
        match self.start(&path) {
            Ok(()) => {
                self.overlay = None;
                if original {
                    // An original game gets its briefing like the menu's Load Game does.
                    self.queue_opening_dialogs();
                } else {
                    self.rebuild_ui();
                }
            }
            Err(e) => {
                let m = self.texts.af("Cannot load: %1", &[format!("{e:#}")]);
                if let Some(st) = self.saves_mut() {
                    st.message = Some((m, true));
                }
                self.rebuild_ui();
            }
        }
    }

    fn saves_save(&mut self) {
        let Some(st) = self.saves_mut() else { return };
        let stem = saves::sanitize_stem(&st.name);
        if st.name.trim().is_empty() {
            return;
        }
        // Writing over `quicksave` or an autosave by name is allowed, it is the player's file.
        let kind = if stem == saves::QUICK_STEM {
            SaveKind::Quick
        } else {
            SaveKind::Manual
        };
        match self.write_own_save(&stem, kind) {
            Ok(p) => {
                self.overlay = None;
                self.saved_toast(Ok(p), false);
                self.rebuild_ui();
            }
            Err(e) => {
                let m = self.texts.af("Cannot save: %1", &[e]);
                if let Some(st) = self.saves_mut() {
                    st.message = Some((m, true));
                }
                self.rebuild_ui();
            }
        }
    }

    fn saves_delete(&mut self) {
        let Some(st) = self.saves_mut() else { return };
        let Some(i) = st.sel else { return };
        let Some(row) = st.rows.get(i) else { return };
        if row.original {
            return;
        }
        if !st.armed {
            st.armed = true;
            let m = self.texts.a("Press Delete again to delete this save.");
            if let Some(st) = self.saves_mut() {
                st.message = Some((m, false));
            }
            self.rebuild_ui();
            return;
        }
        let entry = row.entry.clone();
        let result = saves::delete(&entry);
        if let Some(st) = self.saves_mut() {
            st.armed = false;
            match result {
                Ok(()) => {
                    st.rows.remove(i);
                    st.sel = (!st.rows.is_empty()).then(|| i.min(st.rows.len() - 1));
                    st.detail = Detail::default();
                    st.message = None;
                }
                Err(e) => st.message = Some((format!("{e:#}"), true)),
            }
        }
        self.rebuild_ui();
    }

    // ---- building ---------------------------------------------------------------------------

    pub(super) fn build_saves(&self, ui: &mut Ui) {
        let Some(Overlay::Saves(st)) = &self.overlay else {
            return;
        };
        let t = &self.texts;
        let (cw, ch) = ui.logical_size();
        ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(DIM),
            },
            Layout::fill(),
        );
        let (pw, ph) = ((cw - 16.0).min(860.0), (ch - 16.0).min(600.0));
        let panel = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, pw, ph),
        );
        let u = (ph / 560.0).clamp(0.62, 1.15);
        let pad = 18.0;
        let title = if st.mode == SavesMode::Save {
            t.a("Save Game")
        } else {
            t.a("Load Game")
        };
        ui.add(
            panel,
            Kind::Label {
                text: title,
                font: game_font(3)
                    .with_size((game_font(3).size * u).round().max(12.0))
                    .with_shadow(),
                color: CREAM,
                align: Align::Center,
                wrap: false,
            },
            at(pad, pad * 0.7, pw - 2.0 * pad, 34.0 * u),
        );
        let top = pad * 0.7 + 34.0 * u + 8.0;
        let foot_h = 36.0 * u;
        let foot_y = ph - pad * 0.8 - foot_h;
        let name_h = if st.mode == SavesMode::Save {
            34.0 * u
        } else {
            0.0
        };
        let msg_h = 24.0 * u;
        let list_h = foot_y - top - name_h - msg_h - 14.0;
        let two_col = pw > 640.0;
        let list_w = if two_col {
            (pw - 2.0 * pad) * 0.52
        } else {
            pw - 2.0 * pad
        };
        let fs = (17.0 * u).round().max(11.0);
        let items: Vec<String> = st
            .rows
            .iter()
            .map(|r| {
                let tag = match (r.original, r.entry.kind()) {
                    (true, _) => t.a("(original)"),
                    (_, SaveKind::Auto) => t.a("(auto)"),
                    (_, SaveKind::Quick) => t.a("(quick)"),
                    _ => String::new(),
                };
                if tag.is_empty() {
                    r.entry.stem.clone()
                } else {
                    format!("{}  {tag}", r.entry.stem)
                }
            })
            .collect();
        let empty = items.is_empty();
        let list = ui.add(
            panel,
            Kind::List {
                items,
                selected: st.sel,
                scroll: st.sel.unwrap_or(0).saturating_sub(5),
                row_h: (fs * 1.55).round(),
                font: game_font(8).with_size(fs),
                action: A_SV_LIST,
            },
            at(pad, top, list_w, list_h),
        );
        if st.mode == SavesMode::Load {
            ui.set_focus(Some(list));
        }
        if empty {
            ui.add(
                panel,
                Kind::Label {
                    text: t.a("No saved games yet."),
                    font: game_font(8).with_size(fs).with_shadow(),
                    color: MUTED,
                    align: Align::Center,
                    wrap: false,
                },
                at(pad, top + list_h / 2.0 - 12.0, list_w, 24.0),
            );
        }
        if two_col {
            self.build_save_detail(
                ui,
                panel,
                st,
                pad + list_w + 14.0,
                top,
                pw - pad - list_w - 14.0 - pad,
                list_h,
                fs,
            );
        }
        let mut y = top + list_h + 6.0;
        if st.mode == SavesMode::Save {
            ui.add(
                panel,
                Kind::Label {
                    text: t.a("Name"),
                    font: game_font(8).with_size(fs).with_shadow(),
                    color: CREAM,
                    align: Align::Left,
                    wrap: false,
                },
                at(pad, y, 70.0, name_h),
            );
            let field = ui.add(
                panel,
                Kind::Panel {
                    fill: Fill::Solid(INSET),
                },
                at(pad + 76.0, y, pw - 2.0 * pad - 76.0, name_h),
            );
            ui.add(
                field,
                Kind::Label {
                    text: format!("{}_", st.name),
                    font: game_font(8).with_size(fs),
                    color: GOLD,
                    align: Align::Left,
                    wrap: false,
                },
                at(8.0, 0.0, pw - 2.0 * pad - 90.0, name_h),
            );
            y += name_h + 4.0;
        }
        if let Some((m, bad)) = &st.message {
            ui.add(
                panel,
                Kind::Label {
                    text: m.clone(),
                    font: game_font(8).with_size((fs * 0.9).round()).with_shadow(),
                    color: if *bad { WARN } else { GOLD },
                    align: Align::Left,
                    wrap: false,
                },
                at(pad, y, pw - 2.0 * pad, msg_h),
            );
        }
        // Buttons.
        let bf = game_font(2).with_size((game_font(2).size * u.max(0.8)).round().max(10.0));
        let bw = 150.0 * u.max(0.85);
        let sel_own = st
            .sel
            .and_then(|i| st.rows.get(i))
            .is_some_and(|r| !r.original);
        let overwrite = st.mode == SavesMode::Save
            && st
                .rows
                .iter()
                .any(|r| !r.original && r.entry.stem == saves::sanitize_stem(&st.name));
        let ok_text = match (st.mode, overwrite) {
            (SavesMode::Load, _) => t.a("Load"),
            (SavesMode::Save, true) => t.a("Overwrite"),
            (SavesMode::Save, false) => t.a("Save"),
        };
        ui.add(
            panel,
            Kind::Button {
                text: ok_text,
                font: bf,
                action: A_SV_OK,
                enabled: match st.mode {
                    SavesMode::Load => st.sel.is_some(),
                    SavesMode::Save => !st.name.trim().is_empty(),
                },
                hotkey: None,
            },
            at(pad, foot_y, bw, foot_h),
        );
        ui.add(
            panel,
            Kind::Button {
                text: if st.armed {
                    t.a("Really delete?")
                } else {
                    t.a("Delete")
                },
                font: bf,
                action: A_SV_DELETE,
                enabled: sel_own,
                hotkey: None,
            },
            at(pad + bw + 8.0, foot_y, bw, foot_h),
        );
        ui.add(
            panel,
            Kind::Button {
                text: t.a("Cancel"),
                font: bf,
                action: A_SV_CANCEL,
                enabled: true,
                hotkey: None,
            },
            at(pw - pad - bw, foot_y, bw, foot_h),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn build_save_detail(
        &self,
        ui: &mut Ui,
        panel: rt2_ui::NodeId,
        st: &SavesState,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fs: f32,
    ) {
        let t = &self.texts;
        let row = st.sel.and_then(|i| st.rows.get(i));
        let box_ = ui.add(
            panel,
            Kind::Panel {
                fill: Fill::Solid(INSET),
            },
            at(x, y, w, h),
        );
        // Thumbnail, aspect kept, in the top part.
        let mut ty = 8.0;
        if let Some(th) = &st.detail.thumb {
            let max_w = w - 16.0;
            let max_h = (h * 0.45).max(60.0);
            let k = (max_w / th.w as f32).min(max_h / th.h as f32);
            let (tw, thh) = (th.w as f32 * k, th.h as f32 * k);
            let colors: Vec<Color> = th
                .rgb
                .iter()
                .map(|p| Color::rgb(p[0], p[1], p[2]))
                .collect();
            ui.add(
                box_,
                Kind::Cells {
                    cols: th.w as usize,
                    rows: th.h as usize,
                    colors: Arc::new(colors),
                    action: 0,
                },
                at((w - tw) / 2.0, ty, tw, thh),
            );
            ty += thh + 10.0;
        }
        let m = st.detail.meta.as_ref();
        let mut lines: Vec<(String, String)> = Vec::new();
        let scenario = m
            .map(|m| m.scenario.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| st.detail.title.clone());
        if !scenario.is_empty() {
            lines.push((t.a("Scenario"), scenario));
        }
        if let Some(m) = m {
            if !m.company.is_empty() {
                lines.push((t.a("Company"), m.company.clone()));
            }
            if m.year > 0 {
                lines.push((t.a("Game date"), t.date(m.month, m.day, m.year)));
            }
            if let Some(c) = m.cash {
                lines.push((t.a("Cash"), crate::info::money(c as f64)));
            }
            if m.saved_at > 0 {
                lines.push((t.a("Saved"), saves::format_time(m.saved_at)));
            }
        } else if let Some(row) = row
            && row.entry.modified > 0
        {
            lines.push((t.a("Saved"), saves::format_time(row.entry.modified)));
        }
        if row.is_some_and(|r| r.original) {
            lines.push((t.a("Source"), t.a("Original game")));
        }
        let f = game_font(8).with_size((fs * 0.92).round());
        let lh = (fs * 1.45).round();
        for (label, value) in lines {
            if ty + lh > h {
                break;
            }
            ui.add(
                box_,
                Kind::Label {
                    text: format!("{label}: {value}"),
                    font: f.with_shadow(),
                    color: CREAM,
                    align: Align::Left,
                    wrap: false,
                },
                at(10.0, ty, w - 16.0, lh),
            );
            ty += lh;
        }
    }
}
