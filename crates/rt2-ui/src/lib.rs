//! A small retained UI layer for the Railroad Tycoon II clone (docs/ui.md).
//!
//! * [`ui`]: the widget tree (panels, labels, buttons, lists, images), anchor/stack layout in
//!   logical units, mouse and keyboard focus, hit testing. No GPU, no fonts: unit-testable.
//! * [`scale`]: logical units x UI scale (100-300 %) = physical pixels.
//! * [`font`]: the game's 10 bitmap fonts mapped to vector faces (OFL fonts in `assets/fonts/`).
//! * [`skin`]: sprite atlas + 9-slice skin built from images the application supplies (the
//!   original UI art, loaded from the install by `rt2-game`), with a flat fallback.
//! * [`render`]: the wgpu renderer (quads) and glyphon (text).

pub mod font;
pub mod geom;
pub mod render;
pub mod scale;
pub mod skin;
pub mod ui;

pub use font::{Face, FontStyle, game_font};
pub use geom::Rect;
pub use render::UiRenderer;
pub use scale::UiScale;
pub use skin::{ButtonSkin, NineSlice, Skin, SkinBuilder, SkinImage, SpriteId, Theme};
pub use ui::{
    Action, Align, Anchor, Color, Fill, Fit, Input, Key, Kind, Layout, MouseButton, NodeId,
    NodeState, Stack, Ui, UiEvent,
};
