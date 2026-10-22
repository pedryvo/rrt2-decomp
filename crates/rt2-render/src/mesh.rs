//! CPU-side terrain mesh built from the corner elevation grid.
//!
//! World space: `x` east (tile columns), `z` south (tile rows), `y` up. One tile is one world unit
//! wide. Every tile gets its own four vertices so it can carry a flat colour, while the lighting
//! normals are smooth (central differences of the corner grid).

use rt2_formats::map::{Map, Tile};

/// World units of height per elevation unit. The original's scale is unknown, this only makes the
/// relief readable.
pub const DEFAULT_HEIGHT_SCALE: f32 = 1.0 / 12.0;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Linear RGB.
    pub color: [f32; 3],
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

fn srgb_to_linear(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| v.powf(2.2))
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// Land colour ramp (sRGB 0..1) over `t` in 0..=1 of the height above sea level.
fn land_ramp(t: f32) -> [f32; 3] {
    const STOPS: [(f32, [f32; 3]); 5] = [
        (0.0, [0.45, 0.62, 0.30]),
        (0.25, [0.52, 0.64, 0.30]),
        (0.5, [0.62, 0.55, 0.32]),
        (0.75, [0.52, 0.46, 0.40]),
        (1.0, [0.95, 0.95, 0.95]),
    ];
    let t = t.clamp(0.0, 1.0);
    for pair in STOPS.windows(2) {
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if t <= t1 {
            return lerp3(c0, c1, (t - t0) / (t1 - t0));
        }
    }
    STOPS[STOPS.len() - 1].1
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

fn tile_color(tile: &Tile, height_above_sea: f32, range: f32) -> [f32; 3] {
    let srgb = if tile.is_water() {
        [0.16, 0.36, 0.58]
    } else {
        let t = (height_above_sea / range).clamp(0.0, 1.0).sqrt();
        let base = land_ramp(t);
        // Forest: darker green, up to ~35% at the densest.
        let forest = tile.trees as f32 / 28.0 * 0.35;
        lerp3(base, [0.12, 0.28, 0.10], forest)
    };
    srgb_to_linear(srgb)
}

impl TerrainMesh {
    pub fn build(map: &Map, height_scale: f32) -> TerrainMesh {
        let (w, h) = (map.width, map.height);
        let sea = sea_level(map);
        let top = map.elevation.iter().copied().max().unwrap_or(0) as f32;
        let range = (top - sea).max(1.0);
        let world_y = |x: usize, y: usize| map.corner(x, y) as f32 * height_scale;

        // Smooth normal at corner (x, y) from central differences.
        let normal = |x: usize, y: usize| -> [f32; 3] {
            let (x0, x1) = (x.saturating_sub(1), (x + 1).min(w));
            let (y0, y1) = (y.saturating_sub(1), (y + 1).min(h));
            let dx = (world_y(x1, y) - world_y(x0, y)) / (x1 - x0) as f32;
            let dz = (world_y(x, y1) - world_y(x, y0)) / (y1 - y0) as f32;
            let len = (dx * dx + 1.0 + dz * dz).sqrt();
            [-dx / len, 1.0 / len, -dz / len]
        };

        let mut vertices = Vec::with_capacity(w * h * 4);
        let mut indices = Vec::with_capacity(w * h * 6);
        let mut max_y = 0.0f32;
        for ty in 0..h {
            for tx in 0..w {
                let tile = map.tile(tx, ty);
                let corners = [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)];
                let avg = corners
                    .iter()
                    .map(|&(x, y)| map.corner(x, y) as f32)
                    .sum::<f32>()
                    / 4.0;
                let color = tile_color(tile, avg - sea, range);
                let base = vertices.len() as u32;
                for (x, y) in corners {
                    let py = world_y(x, y);
                    max_y = max_y.max(py);
                    vertices.push(Vertex {
                        position: [x as f32, py, y as f32],
                        normal: normal(x, y),
                        color,
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

    #[test]
    fn mesh_has_four_vertices_and_two_triangles_per_tile() {
        let mesh = TerrainMesh::build(&flat_map(4, 3, 10), 0.5);
        assert_eq!(mesh.vertices.len(), 48);
        assert_eq!(mesh.indices.len(), 72);
        assert!(
            mesh.indices
                .iter()
                .all(|&i| (i as usize) < mesh.vertices.len())
        );
        assert_eq!(mesh.max_y, 5.0);
    }

    #[test]
    fn flat_terrain_has_up_normals() {
        let mesh = TerrainMesh::build(&flat_map(2, 2, 7), 1.0);
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
        let mesh = TerrainMesh::build(&map, 1.0);
        assert!(mesh.vertices.iter().all(|v| v.normal[0] < 0.0));
    }

    #[test]
    fn sea_level_is_median_of_water_corners() {
        let mut map = flat_map(2, 1, 5);
        map.tiles[0].ground = 0x61;
        map.elevation[0] = -1;
        assert_eq!(sea_level(&map), -1.0);
    }
}
