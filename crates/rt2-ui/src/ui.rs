//! The retained widget tree: layout in logical units, hit testing, focus and input.
//!
//! The tree is an arena of [`Node`]s addressed by [`NodeId`]. Roots are drawn in order (a later
//! root is on top and each root is its own draw layer). [`Ui::layout`] computes every rectangle
//! for a physical viewport and the UI scale; [`Ui::input`] turns mouse and keyboard input into
//! [`UiEvent`]s (action ids chosen by the application). Nothing here touches the GPU.

use std::sync::Arc;

use crate::font::FontStyle;
use crate::geom::Rect;
use crate::scale::UiScale;
use crate::skin::SpriteId;

/// Application-defined id carried by buttons and lists.
pub type Action = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(pub(crate) usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl Color {
    pub const WHITE: Color = Color(255, 255, 255, 255);
    pub const BLACK: Color = Color(0, 0, 0, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color(r, g, b, 255)
    }

    pub const fn with_alpha(self, a: u8) -> Color {
        Color(self.0, self.1, self.2, a)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    /// The whole parent rectangle, inset by `(dx, dy)` on each side.
    Fill,
}

/// Placement of a node in its parent. A `w` or `h` of 0 stretches to the parent minus the offset
/// on both sides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub anchor: Anchor,
    pub dx: f32,
    pub dy: f32,
    pub w: f32,
    pub h: f32,
}

impl Layout {
    pub const fn at(anchor: Anchor, w: f32, h: f32) -> Layout {
        Layout {
            anchor,
            dx: 0.0,
            dy: 0.0,
            w,
            h,
        }
    }

    pub const fn fill() -> Layout {
        Layout::at(Anchor::Fill, 0.0, 0.0)
    }

    pub const fn offset(mut self, dx: f32, dy: f32) -> Layout {
        self.dx = dx;
        self.dy = dy;
        self
    }

    fn place(&self, p: Rect) -> Rect {
        if self.anchor == Anchor::Fill {
            return p.inset(self.dx, self.dy);
        }
        use Anchor::*;
        // A size of 0 stretches: the rectangle spans the parent minus |offset| on both sides
        // (and the anchor no longer matters on that axis).
        let (x, w) = if self.w > 0.0 {
            let x = match self.anchor {
                TopLeft | Left | BottomLeft => p.x + self.dx,
                Top | Center | Bottom => p.x + (p.w - self.w) / 2.0 + self.dx,
                TopRight | Right | BottomRight => p.right() - self.w - self.dx,
                Fill => unreachable!(),
            };
            (x, self.w)
        } else {
            (p.x + self.dx.abs(), (p.w - 2.0 * self.dx.abs()).max(0.0))
        };
        let (y, h) = if self.h > 0.0 {
            let y = match self.anchor {
                TopLeft | Top | TopRight => p.y + self.dy,
                Left | Center | Right => p.y + (p.h - self.h) / 2.0 + self.dy,
                BottomLeft | Bottom | BottomRight => p.bottom() - self.h - self.dy,
                Fill => unreachable!(),
            };
            (y, self.h)
        } else {
            (p.y + self.dy.abs(), (p.h - 2.0 * self.dy.abs()).max(0.0))
        };
        Rect::new(x, y, w, h)
    }
}

/// Sequential layout of the children of a node (replaces the children's anchors).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stack {
    pub vertical: bool,
    pub gap: f32,
    pub padding: f32,
    /// Centre the group on the main axis and the fixed-size children on the cross axis.
    pub center: bool,
}

impl Stack {
    pub const fn column(gap: f32, padding: f32) -> Stack {
        Stack {
            vertical: true,
            gap,
            padding,
            center: false,
        }
    }

    pub const fn row(gap: f32, padding: f32) -> Stack {
        Stack {
            vertical: false,
            gap,
            padding,
            center: false,
        }
    }

    pub const fn centered(mut self) -> Stack {
        self.center = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    Stretch,
    /// Fill the rectangle, cropping the overflow equally on both sides.
    Cover,
    /// Fill the rectangle, cropping only the bottom (or the sides) so the top edge stays visible.
    CoverTop,
    Contain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    None,
    /// The skin's window panel (flat in the fallback theme).
    Panel,
    Solid(Color),
    /// A one-pixel outline only (does not take the mouse): a camera frame over a minimap.
    Frame(Color),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Tab,
    BackTab,
    Enter,
    Space,
    Escape,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    /// A printable character (hotkeys compare case-insensitively).
    Char(char),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Panel {
        fill: Fill,
    },
    Image {
        sprite: SpriteId,
        fit: Fit,
    },
    Label {
        text: String,
        font: FontStyle,
        color: Color,
        align: Align,
        /// Wrap long text onto several lines (top aligned); otherwise one line, centred
        /// vertically.
        wrap: bool,
    },
    Button {
        text: String,
        font: FontStyle,
        action: Action,
        enabled: bool,
        hotkey: Option<char>,
    },
    List {
        items: Vec<String>,
        selected: Option<usize>,
        /// First visible row.
        scroll: usize,
        row_h: f32,
        font: FontStyle,
        action: Action,
    },
    /// A button that can be switched on (tabs, radio buttons, check boxes, option horses); drawn
    /// pressed with an outline while `on`.
    Toggle {
        text: String,
        font: FontStyle,
        action: Action,
        on: bool,
        enabled: bool,
    },
    /// A button drawn with two sprites (up, down) such as the original's tool icons; `fallback`
    /// is the caption drawn on a flat button when the skin has no art.
    Icon {
        up: Option<SpriteId>,
        down: Option<SpriteId>,
        fallback: String,
        font: FontStyle,
        action: Action,
        on: bool,
    },
    /// A grid of coloured cells (a minimap). A press or drag over it sends
    /// [`UiEvent::Pointer`] with the position as a fraction of the grid.
    Cells {
        cols: usize,
        rows: usize,
        colors: Arc<Vec<Color>>,
        action: Action,
    },
    /// Read-only text lines (already wrapped by the application) with a scroll offset; the wheel
    /// scrolls it.
    Text {
        lines: Vec<String>,
        scroll: usize,
        row_h: f32,
        font: FontStyle,
        color: Color,
    },
}

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: Kind,
    pub layout: Layout,
    pub stack: Option<Stack>,
    pub visible: bool,
    pub children: Vec<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// Cursor position in physical pixels.
    CursorMoved {
        x: f32,
        y: f32,
    },
    CursorLeft,
    MouseDown(MouseButton),
    MouseUp(MouseButton),
    /// Wheel notches, positive = up.
    Wheel(f32),
    KeyDown(Key),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    /// A button was pressed, a hotkey fired, or a selected list row was activated.
    Activated(Action),
    /// The selection of a list changed to the row.
    Selected(Action, usize),
    /// A press or drag over a [`Kind::Cells`] grid: position as a fraction (0..=1, x then y).
    Pointer(Action, f32, f32),
}

/// Interaction state of a node, for drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeState {
    pub hover: bool,
    pub pressed: bool,
    pub focus: bool,
}

pub struct Ui {
    nodes: Vec<Node>,
    roots: Vec<NodeId>,
    parents: Vec<Option<NodeId>>,
    rects: Vec<Rect>,
    shown: Vec<bool>,
    scale: UiScale,
    viewport: (u32, u32),
    cursor: (f32, f32),
    hover: Option<NodeId>,
    pressed: Option<NodeId>,
    focus: Option<NodeId>,
}

impl Ui {
    pub fn new(scale: UiScale) -> Ui {
        Ui {
            nodes: Vec::new(),
            roots: Vec::new(),
            parents: Vec::new(),
            rects: Vec::new(),
            shown: Vec::new(),
            scale,
            viewport: (1, 1),
            cursor: (-1.0, -1.0),
            hover: None,
            pressed: None,
            focus: None,
        }
    }

    pub fn scale(&self) -> UiScale {
        self.scale
    }

    pub fn set_scale(&mut self, scale: UiScale) {
        self.scale = scale;
        self.layout(self.viewport);
    }

    /// Logical canvas size at the last layout.
    pub fn logical_size(&self) -> (f32, f32) {
        self.scale.logical_size(self.viewport)
    }

    pub fn add_root(&mut self, kind: Kind, layout: Layout) -> NodeId {
        let id = self.push(kind, layout, None);
        self.roots.push(id);
        id
    }

    pub fn add(&mut self, parent: NodeId, kind: Kind, layout: Layout) -> NodeId {
        let id = self.push(kind, layout, Some(parent));
        self.nodes[parent.0].children.push(id);
        id
    }

    fn push(&mut self, kind: Kind, layout: Layout, parent: Option<NodeId>) -> NodeId {
        self.nodes.push(Node {
            kind,
            layout,
            stack: None,
            visible: true,
            children: Vec::new(),
        });
        self.parents.push(parent);
        self.rects.push(Rect::default());
        self.shown.push(false);
        NodeId(self.nodes.len() - 1)
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.0]
    }

    pub fn set_stack(&mut self, id: NodeId, stack: Stack) {
        self.nodes[id.0].stack = Some(stack);
    }

    pub fn set_visible(&mut self, id: NodeId, visible: bool) {
        self.nodes[id.0].visible = visible;
    }

    /// New text of a label or button.
    pub fn set_text(&mut self, id: NodeId, new: impl Into<String>) {
        match &mut self.nodes[id.0].kind {
            Kind::Label { text, .. } | Kind::Button { text, .. } | Kind::Toggle { text, .. } => {
                *text = new.into();
            }
            _ => {}
        }
    }

    /// Switches a [`Kind::Toggle`] or [`Kind::Icon`] on or off.
    pub fn set_on(&mut self, id: NodeId, value: bool) {
        match &mut self.nodes[id.0].kind {
            Kind::Toggle { on, .. } | Kind::Icon { on, .. } => *on = value,
            _ => {}
        }
    }

    /// New lines of a [`Kind::Text`] (scroll back to the top).
    pub fn set_lines(&mut self, id: NodeId, new: Vec<String>) {
        if let Kind::Text { lines, scroll, .. } = &mut self.nodes[id.0].kind {
            *lines = new;
            *scroll = 0;
        }
    }

    /// New colours of a [`Kind::Cells`] grid.
    pub fn set_cells(&mut self, id: NodeId, new: Arc<Vec<Color>>) {
        if let Kind::Cells { colors, .. } = &mut self.nodes[id.0].kind {
            *colors = new;
        }
    }

    pub fn set_enabled(&mut self, id: NodeId, on: bool) {
        if let Kind::Button { enabled, .. } | Kind::Toggle { enabled, .. } =
            &mut self.nodes[id.0].kind
        {
            *enabled = on;
        }
    }

    pub fn set_items(&mut self, id: NodeId, new: Vec<String>) {
        if let Kind::List {
            items,
            selected,
            scroll,
            ..
        } = &mut self.nodes[id.0].kind
        {
            *items = new;
            *selected = None;
            *scroll = 0;
        }
    }

    pub fn selected(&self, id: NodeId) -> Option<usize> {
        match &self.nodes[id.0].kind {
            Kind::List { selected, .. } => *selected,
            _ => None,
        }
    }

    pub fn state(&self, id: NodeId) -> NodeState {
        NodeState {
            hover: self.hover == Some(id),
            pressed: self.pressed == Some(id) && self.hover == Some(id),
            focus: self.focus == Some(id),
        }
    }

    /// Logical rectangle of a node at the last layout.
    pub fn rect(&self, id: NodeId) -> Rect {
        self.rects[id.0]
    }

    pub fn is_shown(&self, id: NodeId) -> bool {
        self.shown[id.0]
    }

    // ---- layout ---------------------------------------------------------------------------

    pub fn layout(&mut self, physical: (u32, u32)) {
        self.viewport = (physical.0.max(1), physical.1.max(1));
        let (w, h) = self.scale.logical_size(self.viewport);
        let canvas = Rect::new(0.0, 0.0, w, h);
        for root in self.roots.clone() {
            let r = self.nodes[root.0].layout.place(canvas);
            self.layout_node(root, r, true);
        }
    }

    fn layout_node(&mut self, id: NodeId, rect: Rect, parent_shown: bool) {
        let shown = parent_shown && self.nodes[id.0].visible;
        self.rects[id.0] = rect;
        self.shown[id.0] = shown;
        let children = self.nodes[id.0].children.clone();
        match self.nodes[id.0].stack {
            None => {
                for c in children {
                    let r = self.nodes[c.0].layout.place(rect);
                    self.layout_node(c, r, shown);
                }
            }
            Some(st) => {
                let placed = self.stack_rects(&st, rect, &children);
                for (c, r) in children.into_iter().zip(placed) {
                    self.layout_node(c, r, shown);
                }
            }
        }
    }

    fn stack_rects(&self, st: &Stack, p: Rect, children: &[NodeId]) -> Vec<Rect> {
        let visible: Vec<NodeId> = children
            .iter()
            .copied()
            .filter(|c| self.nodes[c.0].visible)
            .collect();
        let main = |l: &Layout| if st.vertical { l.h } else { l.w };
        let cross = |l: &Layout| if st.vertical { l.w } else { l.h };
        let total: f32 = visible
            .iter()
            .map(|c| main(&self.nodes[c.0].layout))
            .sum::<f32>()
            + st.gap * visible.len().saturating_sub(1) as f32;
        let (p_main, p_cross) = if st.vertical { (p.h, p.w) } else { (p.w, p.h) };
        let mut cursor = if st.center {
            (p_main - total) / 2.0
        } else {
            st.padding
        };
        let mut out = Vec::with_capacity(children.len());
        for c in children {
            let l = self.nodes[c.0].layout;
            if !self.nodes[c.0].visible {
                out.push(Rect::default());
                continue;
            }
            let m = main(&l);
            let cr = cross(&l);
            let (cross_pos, cross_len) = if cr > 0.0 {
                let pos = if st.center {
                    (p_cross - cr) / 2.0
                } else {
                    st.padding
                };
                (pos, cr)
            } else {
                (st.padding, (p_cross - 2.0 * st.padding).max(0.0))
            };
            let r = if st.vertical {
                Rect::new(p.x + cross_pos + l.dx, p.y + cursor + l.dy, cross_len, m)
            } else {
                Rect::new(p.x + cursor + l.dx, p.y + cross_pos + l.dy, m, cross_len)
            };
            out.push(r);
            cursor += m + st.gap;
        }
        out
    }

    // ---- traversal for drawing ------------------------------------------------------------

    /// Visible nodes in draw order with their root index (draw layer) and logical rectangle.
    pub fn draw_order(&self) -> Vec<(usize, NodeId)> {
        let mut out = Vec::new();
        for (layer, &root) in self.roots.iter().enumerate() {
            self.collect(root, layer, &mut out);
        }
        out
    }

    fn collect(&self, id: NodeId, layer: usize, out: &mut Vec<(usize, NodeId)>) {
        if !self.nodes[id.0].visible {
            return;
        }
        out.push((layer, id));
        for &c in &self.nodes[id.0].children {
            self.collect(c, layer, out);
        }
    }

    pub fn layer_count(&self) -> usize {
        self.roots.len()
    }

    // ---- hit testing and focus ------------------------------------------------------------

    fn blocks(&self, id: NodeId) -> bool {
        match &self.nodes[id.0].kind {
            Kind::Panel { fill } => !matches!(fill, Fill::None | Fill::Frame(_)),
            Kind::Button { .. }
            | Kind::Toggle { .. }
            | Kind::Icon { .. }
            | Kind::Cells { .. }
            | Kind::Text { .. }
            | Kind::List { .. } => true,
            Kind::Image { .. } | Kind::Label { .. } => false,
        }
    }

    /// Topmost node under a logical point that takes mouse input.
    pub fn hit(&self, x: f32, y: f32) -> Option<NodeId> {
        self.roots.iter().rev().find_map(|&r| self.hit_in(r, x, y))
    }

    fn hit_in(&self, id: NodeId, x: f32, y: f32) -> Option<NodeId> {
        if !self.shown[id.0] {
            return None;
        }
        for &c in self.nodes[id.0].children.iter().rev() {
            if let Some(h) = self.hit_in(c, x, y) {
                return Some(h);
            }
        }
        (self.blocks(id) && self.rects[id.0].contains(x, y)).then_some(id)
    }

    /// Cursor position in logical units.
    pub fn cursor(&self) -> (f32, f32) {
        self.cursor
    }

    /// The node under the cursor that takes the mouse, if any.
    pub fn hovered(&self) -> Option<NodeId> {
        self.hover
    }

    /// The action of a button-like node ([`Kind::Button`], [`Kind::Toggle`], [`Kind::Icon`])
    /// that can be pressed.
    fn press_action(&self, id: NodeId) -> Option<Action> {
        match &self.nodes[id.0].kind {
            Kind::Button {
                action,
                enabled: true,
                ..
            }
            | Kind::Toggle {
                action,
                enabled: true,
                ..
            }
            | Kind::Icon { action, .. } => Some(*action),
            _ => None,
        }
    }

    /// Fraction of a grid under a logical point, clamped to the grid.
    fn cells_point(&self, id: NodeId, x: f32, y: f32) -> (f32, f32) {
        let r = self.rects[id.0];
        (
            ((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0),
            ((y - r.y) / r.h.max(1.0)).clamp(0.0, 1.0),
        )
    }

    /// True if the cursor is over a widget that takes the mouse (the map must not react).
    pub fn over_ui(&self) -> bool {
        self.hover.is_some()
    }

    fn focusable(&self, id: NodeId) -> bool {
        self.shown[id.0]
            && match &self.nodes[id.0].kind {
                Kind::Button { enabled, .. } | Kind::Toggle { enabled, .. } => *enabled,
                Kind::Icon { .. } | Kind::List { .. } => true,
                _ => false,
            }
    }

    /// Focusable nodes in tree order.
    pub fn focus_order(&self) -> Vec<NodeId> {
        self.draw_order()
            .into_iter()
            .map(|(_, id)| id)
            .filter(|&id| self.focusable(id))
            .collect()
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.focus.filter(|&f| self.focusable(f))
    }

    pub fn set_focus(&mut self, id: Option<NodeId>) {
        self.focus = id;
    }

    fn move_focus(&mut self, delta: i32) {
        let order = self.focus_order();
        if order.is_empty() {
            return;
        }
        let cur = self
            .focused()
            .and_then(|f| order.iter().position(|&o| o == f));
        let next = match cur {
            None if delta >= 0 => 0,
            None => order.len() - 1,
            Some(i) => (i as i32 + delta).rem_euclid(order.len() as i32) as usize,
        };
        self.focus = Some(order[next]);
    }

    // ---- input ----------------------------------------------------------------------------

    pub fn input(&mut self, input: Input) -> Vec<UiEvent> {
        let mut ev = Vec::new();
        match input {
            Input::CursorMoved { x, y } => {
                let k = self.scale.factor();
                self.cursor = (x / k, y / k);
                self.hover = self.hit(self.cursor.0, self.cursor.1);
                if let Some(p) = self.pressed
                    && let Kind::Cells { action, .. } = self.nodes[p.0].kind
                {
                    let (fx, fy) = self.cells_point(p, self.cursor.0, self.cursor.1);
                    ev.push(UiEvent::Pointer(action, fx, fy));
                }
            }
            Input::CursorLeft => {
                self.hover = None;
                self.pressed = None;
            }
            Input::MouseDown(MouseButton::Left) => {
                self.pressed = self.hover;
                if let Some(h) = self.hover {
                    if self.focusable(h) {
                        self.focus = Some(h);
                    }
                    if let Kind::List { .. } = self.nodes[h.0].kind {
                        self.click_list(h, &mut ev);
                    }
                    if let Kind::Cells { action, .. } = self.nodes[h.0].kind {
                        let (fx, fy) = self.cells_point(h, self.cursor.0, self.cursor.1);
                        ev.push(UiEvent::Pointer(action, fx, fy));
                    }
                }
            }
            Input::MouseDown(MouseButton::Other) | Input::MouseUp(MouseButton::Other) => {}
            Input::MouseUp(MouseButton::Left) => {
                let pressed = self.pressed.take();
                if let (Some(p), Some(h)) = (pressed, self.hover)
                    && p == h
                    && let Some(action) = self.press_action(h)
                {
                    ev.push(UiEvent::Activated(action));
                }
            }
            Input::Wheel(lines) => {
                if let Some(h) = self.hover {
                    self.scroll_list(h, -(lines * 3.0).round() as i32);
                }
            }
            Input::KeyDown(key) => self.key(key, &mut ev),
        }
        ev
    }

    fn visible_rows(&self, id: NodeId) -> usize {
        match &self.nodes[id.0].kind {
            Kind::List { row_h, .. } | Kind::Text { row_h, .. } => {
                ((self.rects[id.0].h / row_h).floor() as usize).max(1)
            }
            _ => 0,
        }
    }

    fn scroll_list(&mut self, id: NodeId, delta_rows: i32) {
        let vis = self.visible_rows(id);
        match &mut self.nodes[id.0].kind {
            Kind::List { items, scroll, .. } => {
                let max = items.len().saturating_sub(vis);
                *scroll = (*scroll as i32 + delta_rows).clamp(0, max as i32) as usize;
            }
            Kind::Text { lines, scroll, .. } => {
                let max = lines.len().saturating_sub(vis);
                *scroll = (*scroll as i32 + delta_rows).clamp(0, max as i32) as usize;
            }
            _ => {}
        }
    }

    fn click_list(&mut self, id: NodeId, ev: &mut Vec<UiEvent>) {
        let rect = self.rects[id.0];
        let (cx, cy) = self.cursor;
        if let Kind::List {
            items,
            selected,
            scroll,
            row_h,
            action,
            ..
        } = &mut self.nodes[id.0].kind
        {
            let _ = cx;
            let row = *scroll + ((cy - rect.y) / *row_h).floor().max(0.0) as usize;
            if row < items.len() {
                if *selected == Some(row) {
                    ev.push(UiEvent::Activated(*action));
                } else {
                    *selected = Some(row);
                    ev.push(UiEvent::Selected(*action, row));
                }
            }
        }
    }

    fn select_row(&mut self, id: NodeId, target: i64, ev: &mut Vec<UiEvent>) {
        let vis = self.visible_rows(id);
        if let Kind::List {
            items,
            selected,
            scroll,
            action,
            ..
        } = &mut self.nodes[id.0].kind
        {
            if items.is_empty() {
                return;
            }
            let row = target.clamp(0, items.len() as i64 - 1) as usize;
            *selected = Some(row);
            if row < *scroll {
                *scroll = row;
            } else if row >= *scroll + vis {
                *scroll = row + 1 - vis;
            }
            ev.push(UiEvent::Selected(*action, row));
        }
    }

    fn key(&mut self, key: Key, ev: &mut Vec<UiEvent>) {
        match key {
            Key::Tab => self.move_focus(1),
            Key::BackTab => self.move_focus(-1),
            Key::Enter | Key::Space => {
                if let Some(f) = self.focused() {
                    match &self.nodes[f.0].kind {
                        Kind::Button { .. } | Kind::Toggle { .. } | Kind::Icon { .. } => {
                            if let Some(a) = self.press_action(f) {
                                ev.push(UiEvent::Activated(a));
                            }
                        }
                        Kind::List {
                            selected: Some(_),
                            action,
                            ..
                        } if key == Key::Enter => ev.push(UiEvent::Activated(*action)),
                        _ => {}
                    }
                }
            }
            Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Home | Key::End => {
                let dir = match key {
                    Key::Up | Key::PageUp => -1i64,
                    _ => 1,
                };
                match self.focused() {
                    Some(f) if matches!(self.nodes[f.0].kind, Kind::List { .. }) => {
                        let vis = self.visible_rows(f) as i64;
                        let cur = self.selected(f).map_or(-1, |s| s as i64);
                        let last = match &self.nodes[f.0].kind {
                            Kind::List { items, .. } => items.len() as i64 - 1,
                            _ => 0,
                        };
                        let target = match key {
                            Key::Up | Key::Down => cur + dir,
                            Key::PageUp | Key::PageDown => cur + dir * vis,
                            Key::Home => 0,
                            _ => last,
                        };
                        self.select_row(f, target, ev);
                    }
                    _ => self.move_focus(if dir < 0 { -1 } else { 1 }),
                }
            }
            Key::Char(c) => {
                let lc = c.to_ascii_lowercase();
                let hit = self.draw_order().into_iter().find_map(|(_, id)| {
                    match &self.nodes[id.0].kind {
                        Kind::Button {
                            hotkey: Some(h),
                            action,
                            enabled: true,
                            ..
                        } if h.to_ascii_lowercase() == lc => Some(*action),
                        _ => None,
                    }
                });
                if let Some(a) = hit {
                    ev.push(UiEvent::Activated(a));
                }
            }
            Key::Escape => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::game_font;

    fn button(ui: &mut Ui, parent: NodeId, text: &str, action: Action, l: Layout) -> NodeId {
        ui.add(
            parent,
            Kind::Button {
                text: text.into(),
                font: game_font(2),
                action,
                enabled: true,
                hotkey: text.chars().next(),
            },
            l,
        )
    }

    fn menu() -> (Ui, NodeId, [NodeId; 3]) {
        let mut ui = Ui::new(UiScale::from_percent(100));
        let root = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, 200.0, 300.0),
        );
        ui.set_stack(root, Stack::column(10.0, 20.0));
        let a = button(
            &mut ui,
            root,
            "New",
            1,
            Layout::at(Anchor::TopLeft, 0.0, 40.0),
        );
        let b = button(
            &mut ui,
            root,
            "Load",
            2,
            Layout::at(Anchor::TopLeft, 0.0, 40.0),
        );
        let c = button(
            &mut ui,
            root,
            "Quit",
            3,
            Layout::at(Anchor::TopLeft, 0.0, 40.0),
        );
        ui.layout((1000, 800));
        (ui, root, [a, b, c])
    }

    #[test]
    fn anchors_place_and_stretch() {
        let mut ui = Ui::new(UiScale::from_percent(100));
        let r = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let tl = ui.add(
            r,
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::TopLeft, 10.0, 20.0).offset(5.0, 6.0),
        );
        let br = ui.add(
            r,
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::BottomRight, 10.0, 20.0).offset(1.0, 2.0),
        );
        let c = ui.add(
            r,
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Center, 100.0, 50.0),
        );
        let bar = ui.add(
            r,
            Kind::Panel { fill: Fill::None },
            Layout::at(Anchor::Top, 0.0, 30.0),
        );
        ui.layout((800, 600));
        assert_eq!(ui.rect(tl), Rect::new(5.0, 6.0, 10.0, 20.0));
        assert_eq!(ui.rect(br), Rect::new(789.0, 578.0, 10.0, 20.0));
        assert_eq!(ui.rect(c), Rect::new(350.0, 275.0, 100.0, 50.0));
        assert_eq!(ui.rect(bar), Rect::new(0.0, 0.0, 800.0, 30.0));
    }

    #[test]
    fn scale_changes_the_logical_canvas() {
        let mut ui = Ui::new(UiScale::from_percent(200));
        let r = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        ui.layout((1920, 1080));
        assert_eq!(ui.rect(r), Rect::new(0.0, 0.0, 960.0, 540.0));
        ui.set_scale(UiScale::from_percent(100));
        assert_eq!(ui.rect(r).w, 1920.0);
    }

    #[test]
    fn stack_lays_children_out_in_a_column() {
        let (ui, root, [a, b, c]) = menu();
        let p = ui.rect(root);
        assert_eq!(p, Rect::new(400.0, 250.0, 200.0, 300.0));
        assert_eq!(ui.rect(a), Rect::new(420.0, 270.0, 160.0, 40.0));
        assert_eq!(ui.rect(b).y, 320.0);
        assert_eq!(ui.rect(c).y, 370.0);
    }

    #[test]
    fn hit_testing_prefers_children_and_ignores_hidden() {
        let (mut ui, root, [a, ..]) = menu();
        assert_eq!(ui.hit(430.0, 280.0), Some(a));
        assert_eq!(ui.hit(405.0, 255.0), Some(root));
        assert_eq!(ui.hit(10.0, 10.0), None);
        ui.set_visible(a, false);
        ui.layout((1000, 800));
        // the hidden button takes no space: the next one moved up into its place
        assert_ne!(ui.hit(430.0, 280.0), Some(a));
    }

    #[test]
    fn click_activates_only_when_released_over_the_same_button() {
        let (mut ui, _, [a, b, _]) = menu();
        let at = |ui: &mut Ui, n: NodeId| {
            let r = ui.rect(n);
            ui.input(Input::CursorMoved {
                x: r.x + 5.0,
                y: r.y + 5.0,
            });
        };
        at(&mut ui, a);
        ui.input(Input::MouseDown(MouseButton::Left));
        at(&mut ui, b);
        assert!(ui.input(Input::MouseUp(MouseButton::Left)).is_empty());
        at(&mut ui, a);
        ui.input(Input::MouseDown(MouseButton::Left));
        assert_eq!(
            ui.input(Input::MouseUp(MouseButton::Left)),
            vec![UiEvent::Activated(1)]
        );
        assert!(ui.over_ui());
    }

    #[test]
    fn scaled_cursor_maps_to_logical_units() {
        let mut ui = Ui::new(UiScale::from_percent(200));
        let r = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let b = button(
            &mut ui,
            r,
            "OK",
            7,
            Layout::at(Anchor::TopLeft, 50.0, 20.0).offset(10.0, 10.0),
        );
        ui.layout((800, 600));
        // logical (10..60, 10..30) is physical (20..120, 20..60)
        ui.input(Input::CursorMoved { x: 100.0, y: 50.0 });
        assert_eq!(ui.hit(50.0, 25.0), Some(b));
        ui.input(Input::MouseDown(MouseButton::Left));
        assert_eq!(
            ui.input(Input::MouseUp(MouseButton::Left)),
            vec![UiEvent::Activated(7)]
        );
    }

    #[test]
    fn keyboard_focus_hotkeys_and_activation() {
        let (mut ui, _, [a, b, c]) = menu();
        assert!(ui.focused().is_none());
        ui.input(Input::KeyDown(Key::Tab));
        assert_eq!(ui.focused(), Some(a));
        ui.input(Input::KeyDown(Key::Down));
        assert_eq!(ui.focused(), Some(b));
        assert_eq!(
            ui.input(Input::KeyDown(Key::Enter)),
            vec![UiEvent::Activated(2)]
        );
        ui.input(Input::KeyDown(Key::BackTab));
        ui.input(Input::KeyDown(Key::BackTab));
        assert_eq!(ui.focused(), Some(c), "wraps around");
        assert_eq!(
            ui.input(Input::KeyDown(Key::Char('Q'))),
            vec![UiEvent::Activated(3)]
        );
        ui.set_enabled(a, false);
        assert!(!ui.focus_order().contains(&a));
    }

    #[test]
    fn list_selection_scroll_and_activate() {
        let mut ui = Ui::new(UiScale::from_percent(100));
        let r = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let l = ui.add(
            r,
            Kind::List {
                items: (0..20).map(|i| format!("row {i}")).collect(),
                selected: None,
                scroll: 0,
                row_h: 20.0,
                font: game_font(2),
                action: 9,
            },
            Layout::at(Anchor::TopLeft, 200.0, 100.0),
        );
        ui.layout((800, 600));
        ui.input(Input::CursorMoved { x: 10.0, y: 45.0 });
        assert_eq!(
            ui.input(Input::MouseDown(MouseButton::Left)),
            vec![UiEvent::Selected(9, 2)]
        );
        // second click on the selected row activates
        assert_eq!(
            ui.input(Input::MouseDown(MouseButton::Left)),
            vec![UiEvent::Activated(9)]
        );
        ui.input(Input::Wheel(-1.0));
        match ui.node(l).kind {
            Kind::List { scroll, .. } => assert_eq!(scroll, 3),
            _ => unreachable!(),
        }
        // keyboard: End selects the last row and scrolls it into view (5 rows visible)
        ui.input(Input::KeyDown(Key::End));
        assert_eq!(ui.selected(l), Some(19));
        match ui.node(l).kind {
            Kind::List { scroll, .. } => assert_eq!(scroll, 15),
            _ => unreachable!(),
        }
        assert_eq!(
            ui.input(Input::KeyDown(Key::Enter)),
            vec![UiEvent::Activated(9)]
        );
    }

    #[test]
    fn toggles_icons_and_cells_take_the_mouse() {
        let mut ui = Ui::new(UiScale::from_percent(100));
        let r = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let t = ui.add(
            r,
            Kind::Toggle {
                text: "Tab".into(),
                font: game_font(2),
                action: 7,
                on: false,
                enabled: true,
            },
            Layout::at(Anchor::TopLeft, 100.0, 40.0),
        );
        let cells = ui.add(
            r,
            Kind::Cells {
                cols: 2,
                rows: 2,
                colors: Arc::new(vec![Color::BLACK; 4]),
                action: 9,
            },
            Layout::at(Anchor::TopLeft, 100.0, 100.0).offset(0.0, 100.0),
        );
        let frame = ui.add(
            r,
            Kind::Panel {
                fill: Fill::Frame(Color::WHITE),
            },
            Layout::at(Anchor::TopLeft, 50.0, 50.0).offset(300.0, 0.0),
        );
        ui.layout((800, 600));
        ui.set_on(t, true);
        assert!(matches!(ui.node(t).kind, Kind::Toggle { on: true, .. }));
        ui.input(Input::CursorMoved { x: 10.0, y: 10.0 });
        ui.input(Input::MouseDown(MouseButton::Left));
        assert_eq!(
            ui.input(Input::MouseUp(MouseButton::Left)),
            vec![UiEvent::Activated(7)]
        );
        // Press on the grid, then drag past its edge: the fraction is clamped.
        ui.input(Input::CursorMoved { x: 25.0, y: 175.0 });
        let down = ui.input(Input::MouseDown(MouseButton::Left));
        assert_eq!(down, vec![UiEvent::Pointer(9, 0.25, 0.75)]);
        let drag = ui.input(Input::CursorMoved { x: 400.0, y: 20.0 });
        assert_eq!(drag, vec![UiEvent::Pointer(9, 1.0, 0.0)]);
        ui.input(Input::MouseUp(MouseButton::Left));
        // A frame does not block the mouse; the cells do.
        ui.input(Input::CursorMoved { x: 310.0, y: 10.0 });
        assert!(!ui.over_ui());
        let _ = (cells, frame);
        ui.input(Input::CursorMoved { x: 50.0, y: 150.0 });
        assert!(ui.over_ui());
    }

    #[test]
    fn text_lines_scroll_with_the_wheel() {
        let mut ui = Ui::new(UiScale::from_percent(100));
        let r = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        let t = ui.add(
            r,
            Kind::Text {
                lines: (0..30).map(|i| format!("line {i}")).collect(),
                scroll: 0,
                row_h: 20.0,
                font: game_font(2),
                color: Color::WHITE,
            },
            Layout::at(Anchor::TopLeft, 200.0, 100.0),
        );
        ui.layout((800, 600));
        ui.input(Input::CursorMoved { x: 10.0, y: 10.0 });
        ui.input(Input::Wheel(-1.0));
        match ui.node(t).kind {
            Kind::Text { scroll, .. } => assert_eq!(scroll, 3),
            _ => unreachable!(),
        }
        ui.input(Input::Wheel(-100.0));
        match ui.node(t).kind {
            Kind::Text { scroll, .. } => assert_eq!(scroll, 25),
            _ => unreachable!(),
        }
    }
}
