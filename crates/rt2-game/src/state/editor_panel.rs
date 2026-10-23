//! The editor's tool panel drawn with the original art (`bp*.imb`, docs/ui.md "Editor"): the
//! 458x166 background of the active tool's window (`bpraisea`, `bppaintt`, `bpfillte`,
//! `bpaddtre`/`bpremtre`, `bpterrit`, `bpaddbui`, `bpbulldz`, `bpovervw`) over the 458x50
//! `bpmisc` strip (message field and button plate). The interactive parts of those windows are
//! the `Bar` controls (invisible click areas over the art) and two lists; here they are
//! [`Kind::Hotspot`]s and [`Kind::List`]s at the original design coordinates, scaled uniformly
//! to fit the area the palette gets. The original draws the terrain thumbnails at run time, so
//! each swatch is a flat colour with its name. Without the install art the clone-style palette
//! of `editor_ui.rs` stays.

use rt2_sim::editor::{self, FillMode, MIX_WEIGHTS, SOLID_CLASSES, TREE_CLASSES};
use rt2_ui::{Align, Anchor, Color, Fill, Fit, Kind, Layout, NodeId, Rect, SpriteId, Ui};

use super::AppState;
use super::build_ui::STYLES;
use super::editor_ui::{
    A_ADD_LIST, A_BRUSH, A_FILL_MODE, A_ITEM, A_PAL_DOWN, A_PAL_UP, A_STYLE, A_SUB, A_TERR_EDIT,
    A_TERR_LIST, A_TERR_NEW, A_UNDO, Group, MUTED, TEXT, at, fnt, lbl,
};
use crate::artwork::{MISC_SIZE, PANEL_SIZE};

/// The sprites of the panel of one tool: its background and the message strip.
#[derive(Clone, Copy, Debug)]
pub(super) struct PanelArt {
    bg: SpriteId,
    misc: SpriteId,
}

/// Design height of the whole composition: background over strip.
const COMP_H: f32 = PANEL_SIZE.1 + MISC_SIZE.1;

/// Where the composition goes inside `area`: the rectangle (centred, original proportions) and
/// the scale from design pixels.
pub(super) fn art_frame(area: Rect) -> (Rect, f32) {
    let k = (area.w / PANEL_SIZE.0).min(area.h / COMP_H).max(0.1);
    let (w, h) = (PANEL_SIZE.0 * k, COMP_H * k);
    (
        Rect::new(
            (area.x + (area.w - w) / 2.0).round(),
            (area.y + (area.h - h) / 2.0).round(),
            w,
            h,
        ),
        k,
    )
}

/// Approximate colour of a terrain class for the swatches that stand for the thumbnails the
/// original draws at run time (beach to snow, then the mountain and extra classes, ocean).
fn class_color(class: u8) -> [u8; 3] {
    match class {
        0 => [0xd8, 0xc6, 0x90],
        1 => [0xc8, 0xa0, 0x60],
        2 => [0x9a, 0x84, 0x68],
        3 => [0x7a, 0x5a, 0x3a],
        4 => [0x9a, 0x9a, 0x4a],
        5 => [0x4a, 0x8a, 0x3a],
        6 => [0x2f, 0x7a, 0x30],
        7 => [0x48, 0x6a, 0x50],
        8 => [0xee, 0xee, 0xf2],
        12 => [0x77, 0x70, 0x66],
        13 => [0x8a, 0x6a, 0x4a],
        14 => [0x6a, 0x6a, 0x6a],
        15 => [0x5a, 0x7a, 0x5a],
        16 => [0x9a, 0x8a, 0x7a],
        17 => [0x4a, 0x5a, 0x3a],
        18 => [0x1a, 0x3a, 0x6a],
        _ => [0x60, 0x60, 0x60],
    }
}

/// Colour of mixed-paint entry `i`: the average of its classes by weight.
fn mix_color(i: usize) -> [u8; 3] {
    let w = &MIX_WEIGHTS[i.min(MIX_WEIGHTS.len() - 1)];
    let total: u32 = w.iter().map(|&v| u32::from(v)).sum::<u32>().max(1);
    let mut out = [0u32; 3];
    for (c, &v) in w.iter().enumerate() {
        let col = class_color(c as u8);
        for (o, x) in out.iter_mut().zip(col) {
            *o += u32::from(v) * u32::from(x);
        }
    }
    [
        (out[0] / total) as u8,
        (out[1] / total) as u8,
        (out[2] / total) as u8,
    ]
}

/// Draws into one panel at design coordinates.
struct Pb<'a> {
    ui: &'a mut Ui,
    root: NodeId,
    k: f32,
    dz: f32,
    tips: &'a mut Vec<(NodeId, String)>,
}

impl Pb<'_> {
    /// Layout of a design rectangle.
    fn d(&self, x: f32, y: f32, w: f32, h: f32) -> Layout {
        at(x * self.k, y * self.k, w * self.k, h * self.k)
    }

    /// A click area over the art with its tooltip.
    fn hot(&mut self, r: [f32; 4], action: u32, on: bool, enabled: bool, tip: &str) -> NodeId {
        let l = self.d(r[0], r[1], r[2], r[3]);
        let n = self.ui.add(
            self.root,
            Kind::Hotspot {
                action,
                on,
                enabled,
            },
            l,
        );
        if !tip.is_empty() {
            self.tips.push((n, tip.to_owned()));
        }
        n
    }

    /// A short caption centred over a design rectangle.
    fn caption(&mut self, r: [f32; 4], text: &str, size: f32, color: Color, wrap: bool) {
        self.caption_at(r, text, size, color, wrap, Align::Center);
    }

    fn caption_at(
        &mut self,
        r: [f32; 4],
        text: &str,
        size: f32,
        color: Color,
        wrap: bool,
        align: Align,
    ) {
        let l = self.d(r[0], r[1], r[2], r[3]);
        self.ui.add(
            self.root,
            Kind::Label {
                text: text.to_owned(),
                font: fnt(1, self.dz * size),
                color,
                align,
                wrap,
            },
            l,
        );
    }

    /// Name over a swatch: dark on light colours, light on dark ones.
    fn swatch_name(&mut self, r: [f32; 4], c: [u8; 3], text: &str, wrap: bool) {
        let luma = 0.3 * f32::from(c[0]) + 0.59 * f32::from(c[1]) + 0.11 * f32::from(c[2]);
        let ink = if luma > 150.0 {
            Color::rgb(0x20, 0x18, 0x10)
        } else {
            TEXT
        };
        self.caption(r, text, 0.7, ink, wrap);
    }

    /// A flat colour square standing for a thumbnail.
    fn swatch(&mut self, r: [f32; 4], c: [u8; 3]) {
        let l = self.d(r[0] + 2.0, r[1] + 2.0, r[2] - 4.0, r[3] - 4.0);
        self.ui.add(
            self.root,
            Kind::Panel {
                fill: Fill::Solid(Color::rgb(c[0], c[1], c[2])),
            },
            l,
        );
    }

    /// The five brush sizes down the right edge of most panels (`Bar` ids 2008..2012).
    fn brushes(&mut self, brush: usize, tip: &str) {
        for i in 0..5 {
            let r = [426.0, 4.0 + 32.0 * i as f32, 27.0, 31.0];
            self.hot(r, A_BRUSH + i as u32, brush == i, true, tip);
            self.caption(r, &(i + 1).to_string(), 0.9, TEXT, false);
        }
    }
}

impl AppState {
    /// The art of the panel of a tool group, when the install has it all.
    pub(super) fn editor_panel_art(&self, group: Group, sub: usize) -> Option<PanelArt> {
        let e = self.dialog_art.editor;
        let i = match group {
            Group::Select => return None,
            Group::Height => 0,
            Group::Paint => 1,
            Group::Fill => 2,
            Group::Trees if sub == 0 => 3,
            Group::Trees => 4,
            Group::Territory => 5,
            Group::Add => 6,
            Group::Bulldoze => 7,
            Group::Overview => 8,
        };
        Some(PanelArt {
            bg: e.panels[i]?,
            misc: e.misc?,
        })
    }

    /// A message over the panel (the original shows these in the field of the strip, which is
    /// too small for the longer ones).
    pub(super) fn build_art_message(&self, ui: &mut Ui, text: &str, good: bool, ph: f32, dz: f32) {
        let (cw, _) = ui.logical_size();
        let h = (24.0 * dz).round();
        let root = ui.add_root(
            Kind::Panel {
                fill: Fill::Solid(Color(0x10, 0x20, 0x14, 0xe8)),
            },
            Layout::at(Anchor::Bottom, (cw * 0.5).min(640.0 * dz), h).offset(0.0, ph + 4.0),
        );
        let color = if good {
            super::editor_ui::GOOD
        } else {
            super::editor_ui::BAD
        };
        ui.add(root, lbl(text, fnt(2, dz * 0.9), color, Align::Center), {
            Layout::fill().offset(6.0 * dz, 0.0)
        });
    }

    /// Builds the panel inside `area` of `parent`; returns the label that shows the tooltip.
    pub(super) fn build_art_panel(
        &self,
        ui: &mut Ui,
        parent: NodeId,
        art: PanelArt,
        area: Rect,
        dz: f32,
        tips: &mut Vec<(NodeId, String)>,
    ) -> NodeId {
        let (comp, k) = art_frame(area);
        let t = &self.texts;
        let Some(ed) = self.ed() else {
            unreachable!("the editor is active")
        };
        let (group, sub, brush) = (ed.group, ed.sub, ed.brush);
        let root = ui.add(
            parent,
            Kind::Panel { fill: Fill::None },
            at(comp.x, comp.y, comp.w, comp.h),
        );
        ui.add(
            root,
            Kind::Image {
                sprite: art.bg,
                fit: Fit::Stretch,
            },
            at(0.0, 0.0, PANEL_SIZE.0 * k, PANEL_SIZE.1 * k),
        );
        ui.add(
            root,
            Kind::Image {
                sprite: art.misc,
                fit: Fit::Stretch,
            },
            at(0.0, PANEL_SIZE.1 * k, MISC_SIZE.0 * k, MISC_SIZE.1 * k),
        );
        // The message field of the strip (`bpmisc` label 2016) shows the tooltip, from the left
        // edge of the strip so the longer hints fit.
        let status = ui.add(
            root,
            Kind::Label {
                text: String::new(),
                font: fnt(1, dz * 0.72),
                color: TEXT,
                align: Align::Left,
                wrap: true,
            },
            at(
                116.0 * k,
                (PANEL_SIZE.1 + 9.0) * k,
                210.0 * k,
                MISC_SIZE.1 * k - 14.0 * k,
            ),
        );
        let mut p = Pb {
            ui,
            root,
            k,
            dz,
            tips,
        };
        // The plate of the strip (button 2015): undo.
        let undo = !ed.undo.is_empty();
        p.hot(
            [354.0, 175.0, 84.0, 29.0],
            A_UNDO,
            false,
            undo,
            &t.a("Undo the last change."),
        );
        p.caption(
            [354.0, 175.0, 84.0, 29.0],
            &t.a("Undo"),
            0.9,
            if undo { Color::BLACK } else { MUTED },
            false,
        );
        // Tool options in the blank left end of the strip.
        let side_toggle = |p: &mut Pb, row: usize, text: String, action: u32, on: bool| {
            let l = p.d(6.0, 168.0 + row as f32 * 23.0, 100.0, 22.0);
            p.ui.add(
                p.root,
                Kind::Toggle {
                    text,
                    font: fnt(1, p.dz * 0.8),
                    action,
                    on,
                    enabled: true,
                },
                l,
            );
        };
        match group {
            Group::Height => {
                let subs = [
                    t.a("Raise / lower (drag)"),
                    t.a("Smooth (Shift: flatten)"),
                    t.a("Lower to level 0"),
                ];
                let xs = [(5.0, 135.0), (144.0, 135.0), (282.0, 134.0)];
                for (i, (x, w)) in xs.into_iter().enumerate() {
                    p.hot(
                        [x, 3.0, w, 160.0],
                        A_SUB + i as u32,
                        sub == i,
                        true,
                        &subs[i],
                    );
                    p.caption([x + 4.0, 118.0, w - 8.0, 42.0], &subs[i], 0.75, TEXT, true);
                }
                p.brushes(brush, &t.a("The size of the brush."));
            }
            Group::Paint => {
                side_toggle(&mut p, 0, t.a("Solid"), A_SUB, sub == 0);
                side_toggle(&mut p, 1, t.a("Mixed"), A_SUB + 1, sub == 1);
                let mixed = sub == 1;
                let n = if mixed {
                    editor::MIX_COUNT
                } else {
                    SOLID_CLASSES.len()
                };
                let first = ed.pal_row.min(1) * 6;
                for j in 0..12 {
                    let i = first + j;
                    if i >= n {
                        break;
                    }
                    let r = [
                        5.0 + 63.0 * (j % 6) as f32,
                        3.0 + 81.0 * (j / 6) as f32,
                        59.0,
                        78.0,
                    ];
                    let (c, name, on, tip) = if mixed {
                        (
                            mix_color(i),
                            t.af("Mix %1", &[(i + 1).to_string()]),
                            ed.mix_slot == i,
                            super::editor_ui::mix_text(i),
                        )
                    } else {
                        let c = SOLID_CLASSES[i];
                        (
                            class_color(c),
                            self.class_name(c),
                            ed.class_slot == i,
                            String::new(),
                        )
                    };
                    p.swatch(r, c);
                    p.swatch_name([r[0], r[1] + 54.0, r[2], 22.0], c, &name, true);
                    p.hot(r, A_ITEM + i as u32, on, true, &tip);
                }
                p.hot(
                    [390.0, 4.0, 27.0, 78.0],
                    A_PAL_UP,
                    false,
                    ed.pal_row > 0,
                    "",
                );
                p.caption([390.0, 4.0, 27.0, 78.0], "^", 1.0, TEXT, false);
                p.hot(
                    [390.0, 83.0, 27.0, 80.0],
                    A_PAL_DOWN,
                    false,
                    ed.pal_row < 1,
                    "",
                );
                p.caption([390.0, 83.0, 27.0, 80.0], "v", 1.0, TEXT, false);
                p.brushes(brush, &t.a("The size of the brush."));
            }
            Group::Fill => {
                side_toggle(&mut p, 0, t.a("Adjacent"), A_SUB, sub == 0);
                side_toggle(&mut p, 1, t.a("All"), A_SUB + 1, sub == 1);
                for (i, c) in SOLID_CLASSES.iter().enumerate() {
                    let r = [
                        5.0 + 63.0 * (i % 6) as f32,
                        3.0 + 54.0 * (i / 6) as f32,
                        59.0,
                        52.0,
                    ];
                    p.swatch(r, class_color(*c));
                    p.swatch_name(
                        [r[0], r[1] + 30.0, r[2], 20.0],
                        class_color(*c),
                        &self.class_name(*c),
                        true,
                    );
                    p.hot(r, A_ITEM + i as u32, ed.class_slot == i, true, "");
                }
                p.hot(
                    [390.0, 4.0, 27.0, 78.0],
                    A_SUB,
                    sub == 0,
                    true,
                    &t.a("Adjacent"),
                );
                p.hot(
                    [390.0, 83.0, 27.0, 80.0],
                    A_SUB + 1,
                    sub == 1,
                    true,
                    &t.a("All"),
                );
                let modes = [
                    (
                        FillMode::Match,
                        "=",
                        t.a("Fill tiles of the same terrain as the one clicked."),
                    ),
                    (
                        FillMode::Above,
                        ">=",
                        t.a("Fill tiles at least as high as the one clicked."),
                    ),
                    (
                        FillMode::Below,
                        "<=",
                        t.a("Fill tiles at most as high as the one clicked."),
                    ),
                ];
                let rs = [
                    [426.0, 3.0, 27.0, 52.0],
                    [426.0, 56.0, 27.0, 52.0],
                    [426.0, 108.0, 27.0, 53.0],
                ];
                for (i, (m, sym, tip)) in modes.into_iter().enumerate() {
                    p.hot(rs[i], A_FILL_MODE + i as u32, ed.fill_mode == m, true, &tip);
                    p.caption(rs[i], sym, 0.9, TEXT, false);
                }
            }
            Group::Trees => {
                side_toggle(&mut p, 0, t.a("Add trees"), A_SUB, sub == 0);
                side_toggle(&mut p, 1, t.a("Cut trees"), A_SUB + 1, sub == 1);
                if sub == 0 {
                    for (i, c) in TREE_CLASSES.iter().enumerate() {
                        let r = [
                            5.0 + 95.0 * (i % 4) as f32,
                            3.0 + 82.0 * (i / 4) as f32,
                            91.0,
                            78.0,
                        ];
                        let g = class_color(*c);
                        p.swatch(r, [g[0] / 3, g[1] / 2 + 30, g[2] / 3]);
                        p.caption(
                            [r[0], r[1] + 56.0, r[2], 20.0],
                            &t.af("Trees %1", &[(i + 1).to_string()]),
                            0.75,
                            TEXT,
                            false,
                        );
                        p.hot(r, A_ITEM + i as u32, ed.tree_slot == i, true, "");
                    }
                    p.hot(
                        [390.0, 4.0, 27.0, 78.0],
                        A_SUB,
                        true,
                        true,
                        &t.a("Add trees"),
                    );
                    p.hot(
                        [390.0, 83.0, 27.0, 80.0],
                        A_SUB + 1,
                        false,
                        true,
                        &t.a("Cut trees"),
                    );
                }
                p.brushes(brush, &t.a("The size of the brush."));
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
                let sel = ed.terr_sel.min(items.len().saturating_sub(1));
                let l = p.d(5.0, 1.0, 416.0, 159.0);
                p.ui.add(
                    root,
                    Kind::List {
                        items,
                        selected: Some(sel),
                        scroll: sel.saturating_sub(3),
                        row_h: (18.0 * dz).round(),
                        font: fnt(1, dz * 0.85),
                        action: A_TERR_LIST,
                    },
                    l,
                );
                for (i, (text, action)) in [
                    (t.a("New territory..."), A_TERR_NEW),
                    (t.a("Edit territory..."), A_TERR_EDIT),
                ]
                .into_iter()
                .enumerate()
                {
                    let l = p.d(6.0, 168.0 + i as f32 * 23.0, 100.0, 22.0);
                    p.ui.add(
                        root,
                        Kind::Button {
                            text,
                            font: fnt(1, dz * 0.7),
                            action,
                            enabled: true,
                            hotkey: None,
                        },
                        l,
                    );
                }
                p.tips.push((
                    status,
                    t.a("Choose a territory, then paint its tiles on the map."),
                ));
                p.brushes(brush, &t.a("The size of the brush."));
            }
            Group::Add => {
                let items = self.add_items();
                let sel = ed.add_row.min(items.len().saturating_sub(1));
                let l = p.d(236.0, 2.0, 217.0, 162.0);
                p.ui.add(
                    root,
                    Kind::List {
                        items,
                        selected: Some(sel),
                        scroll: sel.saturating_sub(4),
                        row_h: (18.0 * dz).round(),
                        font: fnt(1, dz * 0.85),
                        action: A_ADD_LIST,
                    },
                    l,
                );
                if sel >= 6 {
                    for (i, (n, _)) in STYLES.iter().enumerate() {
                        let l = p.d(
                            9.0 + (i % 3) as f32 * 73.0,
                            110.0 + (i / 3) as f32 * 24.0,
                            72.0,
                            23.0,
                        );
                        p.ui.add(
                            root,
                            Kind::Toggle {
                                text: t.a(n),
                                font: fnt(1, dz * 0.7),
                                action: A_STYLE + i as u32,
                                on: ed.style == i,
                                enabled: true,
                            },
                            l,
                        );
                    }
                } else if sel == 4 {
                    for i in 0..5 {
                        let r = [9.0 + i as f32 * 44.0, 118.0, 42.0, 31.0];
                        p.hot(
                            r,
                            A_BRUSH + i as u32,
                            brush == i,
                            true,
                            &t.a("The size of the brush."),
                        );
                        p.caption(r, &(i + 1).to_string(), 0.9, TEXT, false);
                    }
                } else {
                    let hint = match sel {
                        0..=2 => t.a("Click the map to put a new place; its dialog opens next."),
                        3 => t.a("Click the map to lay a river tile."),
                        _ => t.a("Click the map to lay a road tile."),
                    };
                    p.caption([9.0, 110.0, 219.0, 48.0], &hint, 0.7, TEXT, true);
                }
            }
            Group::Bulldoze => {
                // The art is a picture: the hint shows in the strip under the cursor.
                let hint = t.a("Click to bulldoze the shaded area: track, trees, houses and industries. Stations cannot be bulldozed.");
                p.hot([4.0, 3.0, 418.0, 160.0], 0, false, false, &hint);
                p.brushes(brush, &t.a("The size of the brush."));
            }
            Group::Overview => {
                let hint = t.a("The radar shows each territory in its own colour.");
                p.caption([10.0, 60.0, 380.0, 40.0], &hint, 0.9, TEXT, true);
            }
            Group::Select => {}
        }
        status
    }
}
