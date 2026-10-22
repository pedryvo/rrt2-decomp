//! The wgpu side: a textured terrain pass, a sprite pass (trees, houses, industries, stations) and
//! a headless helper for screenshots.

use std::path::Path;
use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};
use wgpu::util::DeviceExt;

use crate::atlas::{ATLAS_MIPS, Atlas, AtlasEntry};
use crate::camera::Camera;
use crate::live::LiveFrame;
use crate::mesh::surface_elevation;
use crate::mesh::{NATIVE_ZOOM, Vertex};
use crate::scene::{Instance, SMOKE_GROUPS, Scene};
use crate::textures::{LayerStack, MIP_LEVELS, TEXTURE_SIZE};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Below this many screen pixels per native sprite pixel sprites are not drawn (a 300x300 map
/// seen whole is a sea of sub-pixel trees).
pub const MIN_SPRITE_SCALE: f32 = 0.1;
const SKY: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.03,
    b: 0.05,
    a: 1.0,
};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: Mat4,
    light: [f32; 4],
    viewport: [f32; 2],
    sprite_scale: f32,
    water_frame: u32,
    /// Free-running animation counter (rivers use `% 5`, shores `& 31`, ocean `& 15`).
    anim: u32,
    _pad: [u32; 3],
}

/// Sprites of one view rotation on the GPU, already in back-to-front order.
struct RotationGpu {
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    count: u32,
    shadows: wgpu::Buffer,
    shadow_count: u32,
    /// Painter's keys of the (sorted, far to near) static instances.
    keys: Vec<f32>,
    /// Atlas entry of every record of every bank, for the live layer.
    bank_records: Vec<Vec<Option<AtlasEntry>>>,
}

/// Painter's key of a ground point: larger = farther from the viewer.
fn painter_key(fwd: Vec2, x: f32, z: f32) -> f32 {
    fwd.dot(Vec2::new(x, z))
}

pub struct Renderer {
    scene: Arc<Scene>,
    terrain_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    sprite_bind_layout: wgpu::BindGroupLayout,
    sprite_sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    texture_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    depth: wgpu::TextureView,
    size: (u32, u32),
    rotations: [Option<RotationGpu>; 4],
    water_frame: u32,
    anim: u32,
    /// Live layer: the car/smoke state of the previous and the latest simulation step.
    live_prev: LiveFrame,
    live_next: LiveFrame,
    /// Fraction of the way from the previous to the latest step (render-side only).
    live_alpha: f32,
    live_buffer: wgpu::Buffer,
    live_capacity: usize,
    live_scratch: Vec<(f32, Instance)>,
    live_instances: Vec<Instance>,
    /// Index into the static instances before which each live instance is drawn.
    live_split: Vec<u32>,
}

fn live_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("live instances"),
        size: (capacity * size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn depth_view(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

/// Upload a layer stack as a 2D texture array with a full mip chain. An empty stack becomes one
/// transparent layer so the shader always has something bound.
fn upload_stack(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    stack: &LayerStack,
) -> wgpu::TextureView {
    let layers = stack.len().max(1) as u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
            depth_or_array_layers: layers,
        },
        mip_level_count: MIP_LEVELS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, mips) in stack.layers.iter().enumerate() {
        for (level, data) in mips.iter().enumerate() {
            let side = (TEXTURE_SIZE >> level).max(1);
            if data.len() != (side * side * 4) as usize {
                continue; // padding layer of an incomplete bank
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(side * 4),
                    rows_per_image: Some(side),
                },
                wgpu::Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

fn upload_atlas(device: &wgpu::Device, queue: &wgpu::Queue, atlas: &Atlas) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite atlas"),
        size: wgpu::Extent3d {
            width: atlas.width as u32,
            height: atlas.height as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: ATLAS_MIPS as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, data) in atlas.mips.iter().enumerate() {
        let (w, h) = (
            (atlas.width >> level).max(1) as u32,
            (atlas.height >> level).max(1) as u32,
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn texture_entry(
    binding: u32,
    dimension: wgpu::TextureViewDimension,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dimension,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

impl Renderer {
    /// `format` is the colour format of the target the frames will be drawn into; `size` its size
    /// in physical pixels.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        scene: Arc<Scene>,
    ) -> Renderer {
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let terrain_tex_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("terrain textures"),
                entries: &[
                    texture_entry(0, wgpu::TextureViewDimension::D2Array),
                    texture_entry(1, wgpu::TextureViewDimension::D2Array),
                    texture_entry(2, wgpu::TextureViewDimension::D2Array),
                    sampler_entry(3),
                    texture_entry(4, wgpu::TextureViewDimension::D2Array),
                    texture_entry(5, wgpu::TextureViewDimension::D2Array),
                ],
            });
        let sprite_bind_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sprite atlas"),
                entries: &[
                    texture_entry(0, wgpu::TextureViewDimension::D2),
                    sampler_entry(1),
                ],
            });

        let color_target = |blend| {
            Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })
        };

        // Terrain.
        let terrain_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terrain.wgsl").into()),
        });
        let terrain_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&terrain_tex_layout)],
            immediate_size: 0,
        });
        let terrain_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain"),
            layout: Some(&terrain_layout),
            vertex: wgpu::VertexState {
                module: &terrain_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Uint32x4, 4 => Uint32x4
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &terrain_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[color_target(None)],
            }),
            multiview_mask: None,
            cache: None,
        });

        // Sprites: premultiplied alpha, depth-tested against the terrain but not written, drawn
        // back to front.
        let sprite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprites"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sprite.wgsl").into()),
        });
        let sprite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprites"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&sprite_bind_layout)],
            immediate_size: 0,
        });
        let sprite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprites"),
            layout: Some(&sprite_layout),
            vertex: wgpu::VertexState {
                module: &sprite_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32, 2 => Float32x4, 3 => Float32x4
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &sprite_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[color_target(Some(
                    wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
                ))],
            }),
            multiview_mask: None,
            cache: None,
        });

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });

        let terrain_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terrain sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let ground = upload_stack(device, queue, "ground", &scene.textures.ground);
        let ocean = upload_stack(device, queue, "ocean", &scene.textures.ocean);
        let track = upload_stack(device, queue, "track", &scene.textures.track);
        let extra = upload_stack(device, queue, "river and road", &scene.textures.extra);
        let beach = upload_stack(device, queue, "beach", &scene.textures.beach);
        let texture_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain textures"),
            layout: &terrain_tex_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ground),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&ocean),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&track),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&terrain_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&extra),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&beach),
                },
            ],
        });
        let sprite_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain vertices"),
            contents: bytemuck::cast_slice(&scene.mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain indices"),
            contents: bytemuck::cast_slice(&scene.mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Renderer {
            index_count: scene.mesh.indices.len() as u32,
            scene,
            terrain_pipeline,
            sprite_pipeline,
            sprite_bind_layout,
            sprite_sampler,
            uniforms,
            bind_group,
            texture_group,
            vertices,
            indices,
            depth: depth_view(device, size),
            size,
            rotations: [None, None, None, None],
            water_frame: 0,
            anim: 0,
            live_prev: LiveFrame::default(),
            live_next: LiveFrame::default(),
            live_alpha: 1.0,
            live_buffer: live_buffer(device, 1024),
            live_capacity: 1024,
            live_scratch: Vec::new(),
            live_instances: Vec::new(),
            live_split: Vec::new(),
        }
    }

    /// Swaps in a rebuilt scene (static layers changed: buildings, track, stations). The decoded
    /// textures of the old scene are kept on the GPU; the mesh and sprite atlases are rebuilt
    /// lazily.
    pub fn replace_scene(&mut self, device: &wgpu::Device, scene: Arc<Scene>) {
        self.vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain vertices"),
            contents: bytemuck::cast_slice(&scene.mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain indices"),
            contents: bytemuck::cast_slice(&scene.mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.index_count = scene.mesh.indices.len() as u32;
        self.scene = scene;
        self.rotations = [None, None, None, None];
    }

    /// Latest simulation step of the live layer; the previous one becomes the interpolation
    /// start. Call once per batch of simulation steps.
    pub fn push_live(&mut self, frame: LiveFrame) {
        self.live_prev = std::mem::replace(&mut self.live_next, frame);
        self.live_alpha = 0.0;
    }

    /// Position between the previous (0) and latest (1) step to draw the cars at.
    pub fn set_live_alpha(&mut self, alpha: f32) {
        self.live_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Replaces the live state without interpolation (a new game was opened).
    pub fn reset_live(&mut self, frame: LiveFrame) {
        self.live_prev = frame.clone();
        self.live_next = frame;
        self.live_alpha = 1.0;
    }

    /// Call when the target size (physical pixels) changes.
    pub fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if size != self.size && size.0 > 0 && size.1 > 0 {
            self.size = size;
            self.depth = depth_view(device, size);
        }
    }

    /// Ocean animation frame (0..16). Screenshots leave it at 0 so they stay deterministic.
    pub fn set_water_frame(&mut self, frame: u32) {
        self.water_frame = frame & 15;
        self.anim = frame;
    }

    /// Bake the sprite atlas of a view rotation and upload it (once per rotation).
    fn ensure_rotation(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rotation: u8) {
        let r = (rotation % 4) as usize;
        if self.rotations[r].is_some() {
            return;
        }
        let sprites =
            self.scene
                .plan
                .build_rotation(&self.scene.assets, r as u8, self.scene.height_scale);
        // Painter's order: farthest first (largest distance along the on-screen "up" direction).
        let fwd = Camera::ground_forward(r as u8);
        let mut instances = sprites.instances;
        let shadows = sprites.shadows;
        let key = |i: &Instance| painter_key(fwd, i.pos[0], i.pos[2]);
        instances.sort_by(|a, b| key(b).total_cmp(&key(a)));
        let keys: Vec<f32> = instances.iter().map(key).collect();
        let view = upload_atlas(device, queue, &sprites.atlas);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite atlas"),
            layout: &self.sprite_bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sprite_sampler),
                },
            ],
        });
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite instances"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let shadow_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shadow instances"),
            contents: bytemuck::cast_slice(&shadows),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.rotations[r] = Some(RotationGpu {
            bind_group,
            instances: buffer,
            count: instances.len() as u32,
            shadows: shadow_buffer,
            shadow_count: shadows.len() as u32,
            keys,
            bank_records: sprites.bank_records,
        });
    }

    /// Builds this frame's live instances (cars at their interpolated positions, smoke puffs),
    /// sorted far to near, and uploads them.
    fn build_live(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rotation: u8) {
        self.live_instances.clear();
        self.live_split.clear();
        let r = (rotation % 4) as usize;
        let Some(rot) = &self.rotations[r] else {
            return;
        };
        let scene = &self.scene;
        let fwd = Camera::ground_forward(r as u8);
        let hs = scene.height_scale;
        let (w, h) = (scene.map.width, scene.map.height);
        let ground = |x: f32, y: f32| -> f32 {
            let x = x.clamp(0.0, w as f32 - 0.001);
            let y = y.clamp(0.0, h as f32 - 0.001);
            let (tx, ty) = (x as usize, y as usize);
            surface_elevation(&scene.map, tx, ty, x - tx as f32, y - ty as f32) * hs
        };
        let scratch = &mut self.live_scratch;
        scratch.clear();
        let cars = self.live_prev.lerp(&self.live_next, self.live_alpha);
        for c in &cars {
            let Some(bank) = scene
                .plan
                .vehicle_banks
                .get(c.vehicle as usize)
                .copied()
                .flatten()
            else {
                continue;
            };
            let recs = &rot.bank_records[bank as usize];
            // Screen direction clockwise from "up" (`sprite-catalog.md` section 8: record
            // `(d + 1) & 7` at rotation 0); each view quarter turn moves it two steps back.
            let dir8 = (c.dir as usize + 1 + 8 - 2 * r) & 7;
            let rec = match recs.len() {
                0 => continue,
                1..=7 => dir8 & 3,
                _ => dir8,
            };
            let Some(Some(e)) = recs.get(rec) else {
                continue;
            };
            let pos = [c.x, ground(c.x, c.y), c.y];
            scratch.push((
                painter_key(fwd, c.x, c.y),
                Instance {
                    pos,
                    shade: 1.0,
                    rect: e.rect,
                    uv: e.uv,
                },
            ));
        }
        if let Some(bank) = scene.plan.smoke_bank {
            let recs = &rot.bank_records[bank as usize];
            for p in &self.live_next.smoke {
                let idx = p.seq as usize * SMOKE_GROUPS + p.frame as usize;
                let Some(Some(e)) = recs.get(idx) else {
                    continue;
                };
                let mut rect = e.rect;
                rect[1] -= p.height;
                scratch.push((
                    // Puffs rise above the cars of the same spot: drawn just after them.
                    painter_key(fwd, p.x, p.y) - 0.3,
                    Instance {
                        pos: [p.x, ground(p.x, p.y), p.y],
                        shade: -1.0,
                        rect,
                        uv: e.uv,
                    },
                ));
            }
        }
        scratch.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (k, inst) in scratch.iter() {
            self.live_split
                .push(rot.keys.partition_point(|s| s > k) as u32);
            self.live_instances.push(*inst);
        }
        if self.live_instances.len() > self.live_capacity {
            self.live_capacity = self.live_instances.len().next_power_of_two();
            self.live_buffer = live_buffer(device, self.live_capacity);
        }
        if !self.live_instances.is_empty() {
            queue.write_buffer(
                &self.live_buffer,
                0,
                bytemuck::cast_slice(&self.live_instances),
            );
        }
    }

    /// Draw one frame into `target`, which must have the size given to `new`/`resize`.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        camera: &Camera,
    ) {
        let sprite_scale = camera.zoom / NATIVE_ZOOM;
        let live = !self.live_next.cars.is_empty() || !self.live_next.smoke.is_empty();
        let draw_sprites =
            sprite_scale >= MIN_SPRITE_SCALE && (!self.scene.plan.placements.is_empty() || live);
        if draw_sprites {
            self.ensure_rotation(device, queue, camera.rotation);
            self.build_live(device, queue, camera.rotation);
        }
        // Direction towards the light: from the north, high in the sky (up-left on screen).
        let light = Vec3::new(-0.3, 0.7, -0.65).normalize();
        let data = Uniforms {
            view_proj: camera.view_proj(self.size),
            light: [light.x, light.y, light.z, 0.0],
            viewport: [self.size.0 as f32, self.size.1 as f32],
            sprite_scale,
            water_frame: self.water_frame,
            anim: self.anim,
            _pad: [0; 3],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&data));

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("map"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(SKY),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.terrain_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_bind_group(1, &self.texture_group, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.index_count, 0, 0..1);

            if draw_sprites && let Some(rot) = &self.rotations[(camera.rotation % 4) as usize] {
                pass.set_pipeline(&self.sprite_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_bind_group(1, &rot.bind_group, &[]);
                pass.set_vertex_buffer(0, rot.shadows.slice(..));
                pass.draw(0..4, 0..rot.shadow_count);
                // Static sprites (sorted far to near) with the live ones merged in by their
                // painter's keys: one draw per run, vertex buffer swapped between them.
                let mut s = 0u32;
                let mut i = 0usize;
                let n = self.live_instances.len();
                while i < n {
                    let ins = self.live_split[i];
                    if ins > s {
                        pass.set_vertex_buffer(0, rot.instances.slice(..));
                        pass.draw(0..4, s..ins);
                        s = ins;
                    }
                    let mut j = i + 1;
                    while j < n && self.live_split[j] == ins {
                        j += 1;
                    }
                    pass.set_vertex_buffer(0, self.live_buffer.slice(..));
                    pass.draw(0..4, i as u32..j as u32);
                    i = j;
                }
                if rot.count > s {
                    pass.set_vertex_buffer(0, rot.instances.slice(..));
                    pass.draw(0..4, s..rot.count);
                }
            }
        }
        queue.submit([encoder.finish()]);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no suitable GPU adapter: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("cannot create the GPU device: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("cannot read back the frame: {0}")]
    Readback(String),
}

/// Device features the renderer wants (none are required; anisotropy and mips are core).
pub fn device_descriptor() -> wgpu::DeviceDescriptor<'static> {
    wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }
}

/// Render one frame headlessly and return tightly packed RGBA8 (sRGB) pixels.
pub fn render_offscreen(
    scene: Arc<Scene>,
    camera: &Camera,
    size: (u32, u32),
) -> Result<Vec<u8>, RenderError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&device_descriptor()))?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("screenshot"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = Renderer::new(&device, &queue, format, size, scene);
    renderer.render(&device, &queue, &view, camera);

    let padded_row = (size.0 * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: padded_row as u64 * size.1 as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(size.1),
            },
        },
        wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| RenderError::Readback(e.to_string()))?;
    rx.recv()
        .map_err(|e| RenderError::Readback(e.to_string()))?
        .map_err(|e| RenderError::Readback(e.to_string()))?;
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|e| RenderError::Readback(e.to_string()))?;
    let mut pixels = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for row in mapped.chunks_exact(padded_row as usize) {
        pixels.extend_from_slice(&row[..(size.0 * 4) as usize]);
    }
    Ok(pixels)
}

/// Write tightly packed RGBA8 pixels as a PNG.
pub fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(())
}
