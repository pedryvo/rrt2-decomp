//! CPU-side terrain mesh built from the corner elevation grid.
//!
//! World space: `x` east (tile columns), `z` south (tile rows), `y` up. One tile is one world unit
//! wide. Every tile gets its own four vertices so it can carry its own texture layers, while the
//! lighting normals are smooth (central differences of the corner grid).

use rt2_formats::map::Map;

/// Screen pixels per world unit along the screen x axis at the original's maximum zoom: a tile is
/// a 128x64 diamond, so one tile edge spans `64 * sqrt(2)` px horizontally
/// (`docs/formats/sprite-catalog.md` section 2.1).
pub const NATIVE_ZOOM: f32 = 64.0 * std::f32::consts::SQRT_2;

/// World units of height per elevation unit: the original draws 2 px per elevation unit at native
/// zoom (`tela_y = 32 (X + Y) - 2 elevation`, section 2.1). In the 30 degree camera a world unit
/// of height covers `cos(30 deg) * NATIVE_ZOOM` px, hence the division.
pub fn height_scale() -> f32 {
    2.0 / (NATIVE_ZOOM * (30.0f32).to_radians().cos())
}

/// Marks "no layer" in the per-vertex layer fields.
pub const NO_LAYER: u32 = u32::MAX;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Texture coordinate over the tile, `(0, 0)` at the tile's north corner `(x, y)`, `u` along
    /// `+x` and `v` along `+y` (the 64x64 texture is mapped onto the diamond as-is).
    pub uv: [f32; 2],
    /// `[ground layer, ocean layer, track layer, quarter turns]`; a layer is [`NO_LAYER`] when the
    /// tile has none of that kind.
    pub layers: [u32; 4],
}

/// What a tile is drawn with (decided by the scene builder, the mesh only carries it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileLook {
    /// Layer in the ground stack, `None` for open water.
    pub ground: Option<u16>,
    /// Layer in the ocean stack (variant 0..4); the shader adds the animation frame.
    pub water: Option<u16>,
    /// Layer in the track stack.
    pub track: Option<u16>,
    /// Quarter turns applied to the ground texture.
    pub turns: u8,
}

#[derive(Debug, Clone)]
pub struct TerrainMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Map size in tiles.
    pub width: usize,
    pub height: usize,
    /// Highest vertex, in world units (for camera framing).
    pub max_y: f32,
}

/// Sea level of the map: the median elevation of the corners of water tiles (the original has no
/// single known field for it; it varies per map, e.g. 0 for Britain and ~41 for the B&O).
pub fn sea_level(map: &Map) -> f32 {
    let mut levels: Vec<i16> = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            if map.tile(x, y).is_water() {
                levels.push(map.corner(x, y));
            }
        }
    }
    if levels.is_empty() {
        return map.elevation.iter().copied().min().unwrap_or(0) as f32;
    }
    levels.sort_unstable();
    levels[levels.len() / 2] as f32
}

/// Elevation of the surface at tile-local coordinates `(fx, fy)` in `0..=1` (bilinear over the four
/// corners; the mesh splits tiles in triangles, this is the close planar approximation).
pub fn surface_elevation(map: &Map, tx: usize, ty: usize, fx: f32, fy: f32) -> f32 {
    let c = |dx: usize, dy: usize| map.corner(tx + dx, ty + dy) as f32;
    let top = c(0, 0) * (1.0 - fx) + c(1, 0) * fx;
    let bottom = c(0, 1) * (1.0 - fx) + c(1, 1) * fx;
    top * (1.0 - fy) + bottom * fy
}

impl TerrainMesh {
    /// `looks` has one entry per tile, row-major. `scale` is the world height per elevation unit
    /// (normally [`height_scale`]).
    pub fn build(map: &Map, scale: f32, looks: &[TileLook]) -> TerrainMesh {
        let (w, h) = (map.width, map.height);
        assert_eq!(looks.len(), w * h, "one look per tile");
        let world_y = |x: usize, y: usize| map.corner(x, y) as f32 * scale;

        // Smooth normal at corner (x, y) from central differences.
        let normal = |x: usize, y: usize| -> [f32; 3] {
            let (x0, x1) = (x.saturating_sub(1), (x + 1).min(w));
            let (y0, y1) = (y.saturating_sub(1), (y + 1).min(h));
            let dx = (world_y(x1, y) - world_y(x0, y)) / (x1 - x0) as f32;
            let dz = (world_y(x, y1) - world_y(x, y0)) / (y1 - y0) as f32;
            let len = (dx * dx + 1.0 + dz * dz).sqrt();
            [-dx / len, 1.0 / len, -dz / len]
        };

        let layer = |l: Option<u16>| l.map_or(NO_LAYER, u32::from);
        let mut vertices = Vec::with_capacity(w * h * 4);
        let mut indices = Vec::with_capacity(w * h * 6);
        let mut max_y = 0.0f32;
        for ty in 0..h {
            for tx in 0..w {
                let look = looks[ty * w + tx];
                let layers = [
                    layer(look.ground),
                    layer(look.water),
                    layer(look.track),
                    look.turns as u32 & 3,
                ];
                let corners = [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)];
                let uvs = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
                let base = vertices.len() as u32;
                for (&(x, y), uv) in corners.iter().zip(uvs) {
                    let py = world_y(x, y);
                    max_y = max_y.max(py);
                    vertices.push(Vertex {
                        position: [x as f32, py, y as f32],
                        normal: normal(x, y),
                        uv,
                        layers,
                    });
                }
                indices.extend_from_slice(&[
                    base,
                    base + 2,
                    base + 1,
                    base + 1,
                    base + 2,
                    base + 3,
                ]);
            }
        }
        TerrainMesh {
            vertices,
            indices,
            width: w,
            height: h,
            max_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rt2_formats::map::Tile;

    fn flat_map(w: usize, h: usize, elev: i16) -> Map {
        let tile = Tile {
            var: 0,
            ground: 0x2d,
            flags2: 0,
            trees: 0,
            struct_id: 0,
            reserved5: 0,
            flags6: 0,
            track_id: 0,
            reserved8: 0,
            region: 1,
        };
        Map {
            version: rt2_formats::map::MAGIC,
            name: String::new(),
            title: String::new(),
            width: w,
            height: h,
            tiles: vec![tile; w * h],
            elevation: vec![elev; (w + 1) * (h + 1)],
        }
    }

    fn looks(n: usize) -> Vec<TileLook> {
        vec![
            TileLook {
                ground: Some(5),
                water: None,
                track: None,
                turns: 1,
            };
            n
        ]
    }

    #[test]
    fn two_pixels_per_elevation_unit_at_native_zoom() {
        // A world unit of height is cos(30 deg) * NATIVE_ZOOM px on screen.
        let px = height_scale() * (30.0f32).to_radians().cos() * NATIVE_ZOOM;
        assert!((px - 2.0).abs() < 1e-4);
        // The tile diamond is 128 px wide at native zoom: east and south edges each cover
        // cos(45 deg) * NATIVE_ZOOM = 64 px horizontally.
        let half = (45.0f32).to_radians().cos() * NATIVE_ZOOM;
        assert!((half - 64.0).abs() < 1e-3);
    }

    #[test]
    fn mesh_has_four_vertices_and_two_triangles_per_tile() {
        let mesh = TerrainMesh::build(&flat_map(4, 3, 10), 0.5, &looks(12));
        assert_eq!(mesh.vertices.len(), 48);
        assert_eq!(mesh.indices.len(), 72);
        assert!(
            mesh.indices
                .iter()
                .all(|&i| (i as usize) < mesh.vertices.len())
        );
        assert_eq!(mesh.max_y, 5.0);
        assert_eq!(mesh.vertices[0].layers, [5, NO_LAYER, NO_LAYER, 1]);
    }

    #[test]
    fn flat_terrain_has_up_normals() {
        let mesh = TerrainMesh::build(&flat_map(2, 2, 7), 1.0, &looks(4));
        for v in &mesh.vertices {
            assert_eq!(v.normal, [0.0, 1.0, 0.0]);
        }
    }

    #[test]
    fn slope_tilts_normals_away_from_the_rise() {
        let mut map = flat_map(2, 2, 0);
        for y in 0..3 {
            for x in 0..3 {
                map.elevation[y * 3 + x] = (x * 4) as i16; // rises towards +x
            }
        }
        let mesh = TerrainMesh::build(&map, 1.0, &looks(4));
        assert!(mesh.vertices.iter().all(|v| v.normal[0] < 0.0));
    }

    #[test]
    fn sea_level_is_median_of_water_corners() {
        let mut map = flat_map(2, 1, 5);
        map.tiles[0].ground = 0x61;
        map.elevation[0] = -1;
        assert_eq!(sea_level(&map), -1.0);
    }

    #[test]
    fn surface_elevation_interpolates_the_corners() {
        let mut map = flat_map(1, 1, 0);
        map.elevation = vec![0, 10, 20, 30];
        assert_eq!(surface_elevation(&map, 0, 0, 0.0, 0.0), 0.0);
        assert_eq!(surface_elevation(&map, 0, 0, 1.0, 1.0), 30.0);
        assert_eq!(surface_elevation(&map, 0, 0, 0.5, 0.5), 15.0);
    }
}
