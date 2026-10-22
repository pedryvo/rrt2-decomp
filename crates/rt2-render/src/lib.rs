//! Terrain rendering for the Railroad Tycoon II clone, on wgpu. Pure presentation: it turns a
//! parsed [`rt2_formats::map::Map`] into a mesh and draws it; there is no game logic in here.

pub mod camera;
pub mod mesh;
pub mod renderer;

pub use camera::Camera;
pub use mesh::{TerrainMesh, Vertex};
pub use renderer::{DEPTH_FORMAT, RenderError, Renderer, render_offscreen, write_png};
