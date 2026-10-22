//! Rendering for the Railroad Tycoon II clone, on wgpu: textured terrain from the original's own
//! ground/track/ocean textures, sprites (trees, buildings, stations) with a baked upscale, and a
//! headless screenshot path. Pure presentation: there is no game logic in here.

pub mod assets;
pub mod atlas;
pub mod camera;
pub mod ground_map;
pub mod mesh;
pub mod renderer;
pub mod scene;
pub mod textures;

pub use assets::{AssetError, GameAssets};
pub use camera::Camera;
pub use mesh::{NATIVE_ZOOM, TerrainMesh, Vertex, height_scale};
pub use renderer::{DEPTH_FORMAT, RenderError, Renderer, render_offscreen, write_png};
pub use scene::{Scene, SceneOptions, build_grid};
