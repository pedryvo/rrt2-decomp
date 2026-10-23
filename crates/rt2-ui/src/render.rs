//! wgpu renderer of a [`Ui`]: coloured/textured quads from the skin atlas, then text through
//! glyphon, layer by layer (each root of the tree is one layer, so an overlay's panel hides the
//! text below it).

use std::collections::BTreeMap;

use bytemuck::{Pod, Zeroable};
use glyphon::{
    Attrs, Buffer, Cache, Color as GColor, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Wrap, cosmic_text,
};
use wgpu::util::DeviceExt;

use crate::font::{self, FontStyle};
use crate::geom::Rect;
use crate::skin::{NineSlice, Skin, Sprite};
use crate::ui::{Align, Color, Fill, Fit, Kind, NodeId, Ui};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

/// Smallest share of the font size a one-line text shrinks to when it does not fit its box.
const MIN_SHRINK: f32 = 0.55;

#[derive(Clone, PartialEq)]
struct TextKey {
    text: String,
    family: &'static str,
    size_px: f32,
    weight: u16,
    w: f32,
    h: f32,
    align: Align,
    wrap: bool,
}

struct CachedText {
    key: TextKey,
    buffer: Buffer,
    /// Height of the shaped text in physical pixels.
    height: f32,
    used: u64,
}

/// Text to draw this frame (physical pixel geometry).
struct TextJob {
    slot: (usize, u32),
    text: String,
    font: FontStyle,
    rect: Rect,
    clip: Rect,
    color: Color,
    align: Align,
    wrap: bool,
    shadow: bool,
}

#[derive(Default)]
struct Layer {
    quads: Vec<Vertex>,
    texts: Vec<TextJob>,
}

pub struct UiRenderer {
    skin: Skin,
    atlas_texture: wgpu::Texture,
    linear: bool,
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    vertex_capacity: u64,
    font_system: FontSystem,
    script: font::Script,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderers: Vec<TextRenderer>,
    texts: BTreeMap<(usize, u32), CachedText>,
    frame: u64,
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

impl UiRenderer {
    /// `format` is the format of the render targets (sRGB formats are handled correctly).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        skin: Skin,
    ) -> UiRenderer {
        let linear = format.is_srgb();
        let (aw, ah) = skin.atlas_size;
        let atlas_format = if linear {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("ui atlas"),
                size: wgpu::Extent3d {
                    width: aw,
                    height: ah,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: atlas_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &skin.atlas,
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui bind layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui bind group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertex_capacity = 4096 * std::mem::size_of::<Vertex>() as u64;
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui vertices"),
            size: vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let font_system = FontSystem::new_with_fonts(font::font_sources());
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mode = if linear {
            glyphon::ColorMode::Accurate
        } else {
            glyphon::ColorMode::Web
        };
        let atlas = TextAtlas::with_color_mode(device, queue, &cache, format, mode);
        UiRenderer {
            atlas_texture: texture,
            skin,
            linear,
            pipeline,
            globals,
            bind_group,
            vertices,
            vertex_capacity,
            font_system,
            script: font::Script::Latin,
            swash: SwashCache::new(),
            viewport,
            atlas,
            text_renderers: Vec::new(),
            texts: BTreeMap::new(),
            frame: 0,
        }
    }

    /// Selects the writing system whose faces draw the text (a language change). Cached text
    /// layouts are keyed by family, so they rebuild by themselves.
    pub fn set_script(&mut self, script: font::Script) {
        self.script = script;
    }

    /// Replaces the pixels of a sprite reserved with `SkinBuilder::reserve` (straight-alpha RGBA8,
    /// exactly the sprite's size). Does nothing when the size does not match.
    pub fn upload_sprite(&self, queue: &wgpu::Queue, id: crate::SpriteId, rgba: &[u8]) {
        let Some((x, y, w, h)) = self.skin.sprite_rect(id) else {
            return;
        };
        if rgba.len() != (w * h * 4) as usize {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
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

    pub fn skin(&self) -> &Skin {
        &self.skin
    }

    fn col(&self, c: Color, tint: f32) -> [f32; 4] {
        let f = |v: u8| {
            let x = if self.linear {
                srgb_to_linear(v)
            } else {
                v as f32 / 255.0
            };
            x * tint
        };
        [f(c.0), f(c.1), f(c.2), c.3 as f32 / 255.0]
    }

    /// Lays the tree out for `size` (physical pixels) and draws it into `target`. With
    /// `clear = None` the existing contents are kept (UI over the map).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: (u32, u32),
        ui: &mut Ui,
        clear: Option<wgpu::Color>,
    ) {
        self.frame += 1;
        ui.layout(size);
        let layers = self.build_layers(ui);

        // Text: shape, then prepare one TextRenderer per layer.
        while self.text_renderers.len() < layers.len() {
            let r = TextRenderer::new(
                &mut self.atlas,
                device,
                wgpu::MultisampleState::default(),
                None,
            );
            self.text_renderers.push(r);
        }
        self.viewport.update(
            queue,
            Resolution {
                width: size.0,
                height: size.1,
            },
        );
        for layer in &layers {
            for job in &layer.texts {
                self.ensure_text(job);
            }
        }
        self.texts.retain(|_, t| t.used == self.frame);
        for (i, layer) in layers.iter().enumerate() {
            let mut areas = Vec::new();
            for job in &layer.texts {
                let Some(cached) = self.texts.get(&job.slot) else {
                    continue;
                };
                let top = if job.wrap {
                    job.rect.y
                } else {
                    job.rect.y + (job.rect.h - cached.height) / 2.0
                }
                .round();
                let bounds = TextBounds {
                    left: job.clip.x.floor() as i32,
                    top: job.clip.y.floor() as i32,
                    right: job.clip.right().ceil() as i32,
                    bottom: job.clip.bottom().ceil() as i32,
                };
                let c = job.color;
                if job.shadow {
                    let d = (self.shadow_offset(ui)).max(1.0);
                    let s = self.skin.theme.shadow;
                    areas.push(TextArea {
                        buffer: &cached.buffer,
                        left: job.rect.x.round() + d,
                        top: top + d,
                        scale: 1.0,
                        bounds,
                        default_color: GColor::rgba(
                            s.0,
                            s.1,
                            s.2,
                            ((s.3 as u32 * c.3 as u32) / 255) as u8,
                        ),
                        custom_glyphs: &[],
                    });
                }
                areas.push(TextArea {
                    buffer: &cached.buffer,
                    left: job.rect.x.round(),
                    top,
                    scale: 1.0,
                    bounds,
                    default_color: GColor::rgba(c.0, c.1, c.2, c.3),
                    custom_glyphs: &[],
                });
            }
            let _ = self.text_renderers[i].prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash,
            );
        }

        // Quads of all layers into one vertex buffer.
        let mut all: Vec<Vertex> = Vec::new();
        let mut ranges = Vec::new();
        for layer in &layers {
            let start = all.len() as u32;
            all.extend_from_slice(&layer.quads);
            ranges.push(start..all.len() as u32);
        }
        let need = (all.len() * std::mem::size_of::<Vertex>()) as u64;
        if need > self.vertex_capacity {
            self.vertex_capacity = need.next_power_of_two();
            self.vertices = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ui vertices"),
                size: self.vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !all.is_empty() {
            queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&all));
        }
        queue.write_buffer(
            &self.globals,
            0,
            bytemuck::cast_slice(&[size.0 as f32, size.1 as f32, 0.0, 0.0]),
        );

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match clear {
                            Some(c) => wgpu::LoadOp::Clear(c),
                            None => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for (i, range) in ranges.iter().enumerate() {
                if !range.is_empty() {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.vertices.slice(..));
                    pass.draw(range.clone(), 0..1);
                }
                let _ = self.text_renderers[i].render(&self.atlas, &self.viewport, &mut pass);
            }
        }
        queue.submit([encoder.finish()]);
        self.atlas.trim();
    }

    fn shadow_offset(&self, ui: &Ui) -> f32 {
        ui.scale().factor().round()
    }

    fn ensure_text(&mut self, job: &TextJob) {
        let key = TextKey {
            text: job.text.clone(),
            family: job.font.face.family_for(self.script),
            size_px: job.font.size,
            weight: job.font.weight,
            w: job.rect.w,
            h: job.rect.h,
            align: job.align,
            wrap: job.wrap,
        };
        let frame = self.frame;
        if let Some(c) = self.texts.get_mut(&job.slot)
            && c.key == key
        {
            c.used = frame;
            return;
        }
        let attrs = Attrs::new()
            .family(Family::Name(job.font.face.family_for(self.script)))
            .weight(glyphon::Weight(job.font.weight));
        let align = match job.align {
            Align::Left => cosmic_text::Align::Left,
            Align::Center => cosmic_text::Align::Center,
            Align::Right => cosmic_text::Align::End,
        };
        let full_em = job.font.size * job.font.face.size_factor(self.script);
        // One line that is too wide for its box shrinks to fit (down to `MIN_SHRINK` of the size)
        // instead of being clipped: column headers and buttons in long translations. What still
        // does not fit at that size is cut with an ellipsis.
        let mut text = job.text.clone();
        let mut factor = 1.0_f32;
        let shape = |this: &mut Self, text: &str, factor: f32| {
            let em = full_em * factor;
            let line = (job.font.size * factor * 1.25).round();
            let mut buffer = Buffer::new(&mut this.font_system, Metrics::new(em, line));
            buffer.set_wrap(if job.wrap {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            });
            buffer.set_size(Some(job.rect.w), Some(job.rect.h.max(line)));
            buffer.set_text(text, &attrs, Shaping::Advanced, Some(align));
            buffer.shape_until_scroll(&mut this.font_system, false);
            let width = buffer
                .layout_runs()
                .map(|r| r.line_w)
                .fold(0.0_f32, f32::max);
            (buffer, line, width)
        };
        let fits = |width: f32| job.wrap || width <= job.rect.w || job.rect.w <= 0.0;
        let (mut buffer, mut line, width) = shape(self, &text, factor);
        if !fits(width) {
            factor = (job.rect.w / width * 0.98).clamp(MIN_SHRINK, 1.0);
            let (b, l, mut width) = shape(self, &text, factor);
            (buffer, line) = (b, l);
            let mut chars: Vec<char> = text.chars().collect();
            while !fits(width) && chars.len() > 1 {
                chars.pop();
                while chars.last() == Some(&' ') {
                    chars.pop();
                }
                text = chars.iter().collect::<String>() + "\u{2026}";
                let (b, l, w) = shape(self, &text, factor);
                (buffer, line, width) = (b, l, w);
            }
        }
        let lines = buffer.layout_runs().count().max(1);
        self.texts.insert(
            job.slot,
            CachedText {
                key,
                buffer,
                height: lines as f32 * line,
                used: frame,
            },
        );
    }

    // ---- quad building --------------------------------------------------------------------

    fn build_layers(&self, ui: &Ui) -> Vec<Layer> {
        let k = ui.scale().factor();
        let theme = self.skin.theme;
        let mut layers: Vec<Layer> = (0..ui.layer_count()).map(|_| Layer::default()).collect();
        let (cx, cy) = ui.cursor();
        for (li, id) in ui.draw_order() {
            if !ui.is_shown(id) {
                continue;
            }
            let r = ui.rect(id).scaled(k);
            let st = ui.state(id);
            let layer = &mut layers[li];
            match &ui.node(id).kind {
                Kind::Panel { fill } => match fill {
                    Fill::None => {}
                    Fill::Solid(c) | Fill::Overlay(c) => self.solid(&mut layer.quads, r, *c),
                    Fill::Frame(c) => self.outline(&mut layer.quads, r, *c, k),
                    Fill::Panel => self.panel(&mut layer.quads, r, k),
                },
                Kind::Image { sprite, fit } => {
                    self.image(&mut layer.quads, r, *sprite, *fit, k);
                }
                Kind::Label {
                    text,
                    font,
                    color,
                    align,
                    wrap,
                } => layer.texts.push(self.text_job(
                    id,
                    u32::MAX,
                    text,
                    *font,
                    r,
                    r,
                    *color,
                    *align,
                    *wrap,
                    font.shadow,
                    k,
                )),
                Kind::Button {
                    text,
                    font,
                    enabled,
                    ..
                } => self.button_like(layer, id, r, st, text, *font, *enabled, false, k),
                Kind::Toggle {
                    text,
                    font,
                    enabled,
                    on,
                    ..
                } => self.button_like(layer, id, r, st, text, *font, *enabled, *on, k),
                Kind::Icon {
                    up,
                    down,
                    fallback,
                    font,
                    on,
                    ..
                } => {
                    let art = if st.pressed || *on { down } else { up }
                        .filter(|s| self.skin.sprite(*s).is_some());
                    match art {
                        Some(sprite) => {
                            let tint = if st.hover || st.focus { 1.18 } else { 1.0 };
                            self.image_tinted(&mut layer.quads, r, sprite, tint);
                            if st.focus {
                                self.outline(
                                    &mut layer.quads,
                                    r.inset(1.0 * k, 1.0 * k),
                                    theme.accent,
                                    k,
                                );
                            }
                        }
                        None => self.button_like(layer, id, r, st, fallback, *font, true, *on, k),
                    }
                }
                Kind::Hotspot { on, enabled, .. } => {
                    if *enabled && st.hover {
                        self.solid(&mut layer.quads, r, Color(255, 255, 255, 0x24));
                    }
                    if *on {
                        self.outline(&mut layer.quads, r, theme.accent, k);
                    }
                    if st.focus {
                        self.outline(&mut layer.quads, r.inset(1.0 * k, 1.0 * k), theme.accent, k);
                    }
                }
                Kind::Cells {
                    cols, rows, colors, ..
                } => self.cells(&mut layer.quads, r, *cols, *rows, colors),
                Kind::Text {
                    lines,
                    scroll,
                    row_h,
                    font,
                    color,
                } => {
                    let rh = row_h * k;
                    let vis = ((r.h / rh).floor() as usize).max(1);
                    let sb = if lines.len() > vis { 6.0 * k } else { 0.0 };
                    for (n, row) in (*scroll..lines.len()).take(vis).enumerate() {
                        let tr = Rect::new(r.x, r.y + n as f32 * rh, r.w - sb, rh);
                        layer.texts.push(self.text_job(
                            id,
                            row as u32,
                            &lines[row],
                            *font,
                            tr,
                            tr.intersect(&r),
                            *color,
                            Align::Left,
                            false,
                            font.shadow,
                            k,
                        ));
                    }
                    if sb > 0.0 {
                        let track = Rect::new(r.right() - sb, r.y, sb, r.h);
                        self.solid(&mut layer.quads, track, Color(255, 255, 255, 0x18));
                        let frac = vis as f32 / lines.len() as f32;
                        let th = (track.h * frac).max(12.0 * k);
                        let max_scroll = (lines.len() - vis).max(1) as f32;
                        let ty = track.y + (track.h - th) * (*scroll as f32 / max_scroll);
                        self.solid(
                            &mut layer.quads,
                            Rect::new(track.x, ty, sb, th),
                            theme.panel_border,
                        );
                    }
                }
                Kind::List {
                    items,
                    selected,
                    scroll,
                    row_h,
                    font,
                    ..
                } => {
                    self.bordered(
                        &mut layer.quads,
                        r,
                        Color(0x10, 0x13, 0x10, 0xd8),
                        theme.panel_border,
                        1.0,
                        k,
                    );
                    if st.focus {
                        self.outline(&mut layer.quads, r.inset(2.0 * k, 2.0 * k), theme.accent, k);
                    }
                    let rh = row_h * k;
                    let pad = 6.0 * k;
                    let vis = ((r.h / rh).floor() as usize).max(1);
                    let sb = if items.len() > vis { 8.0 * k } else { 0.0 };
                    for (n, row) in (*scroll..items.len()).take(vis).enumerate() {
                        let rr =
                            Rect::new(r.x + k, r.y + k + n as f32 * rh, r.w - 2.0 * k - sb, rh);
                        let hovered =
                            st.hover && Rect::new(rr.x, rr.y, rr.w, rr.h).contains(cx * k, cy * k);
                        if *selected == Some(row) {
                            self.solid(&mut layer.quads, rr, theme.row_selected);
                        } else if hovered {
                            self.solid(&mut layer.quads, rr, theme.row_hover);
                        }
                        let tr = Rect::new(rr.x + pad, rr.y, rr.w - 2.0 * pad, rr.h);
                        layer.texts.push(self.text_job(
                            id,
                            row as u32,
                            &items[row],
                            *font,
                            tr,
                            rr.intersect(&r),
                            theme.text,
                            Align::Left,
                            false,
                            false,
                            k,
                        ));
                    }
                    if sb > 0.0 {
                        let track = Rect::new(r.right() - sb - k, r.y + k, sb, r.h - 2.0 * k);
                        self.solid(&mut layer.quads, track, Color(255, 255, 255, 0x18));
                        let frac = vis as f32 / items.len() as f32;
                        let th = (track.h * frac).max(12.0 * k);
                        let max_scroll = (items.len() - vis).max(1) as f32;
                        let ty = track.y + (track.h - th) * (*scroll as f32 / max_scroll);
                        self.solid(
                            &mut layer.quads,
                            Rect::new(track.x, ty, sb, th),
                            theme.panel_border,
                        );
                    }
                }
            }
        }
        layers
    }

    /// A plate button (skin art or flat), pressed look while `on`, caption centred.
    #[allow(clippy::too_many_arguments)]
    fn button_like(
        &self,
        layer: &mut Layer,
        id: NodeId,
        r: Rect,
        st: crate::ui::NodeState,
        text: &str,
        font: FontStyle,
        enabled: bool,
        on: bool,
        k: f32,
    ) {
        let theme = self.skin.theme;
        let down = st.pressed || on;
        let tint = if !enabled {
            0.5
        } else if st.pressed {
            0.8
        } else if st.hover || st.focus {
            1.2
        } else {
            1.0
        };
        let on_art = if let Some(b) = self.skin.button {
            let n = if down { b.down } else { b.up };
            self.nine(&mut layer.quads, r, &n, k, [tint, tint, tint, 1.0]);
            true
        } else {
            let base = if down {
                Color::rgb(0x2a, 0x2c, 0x22)
            } else {
                theme.button_fill
            };
            self.bordered(&mut layer.quads, r, base, theme.button_border, tint, k);
            false
        };
        if st.focus || on {
            self.outline(&mut layer.quads, r.inset(2.0 * k, 2.0 * k), theme.accent, k);
        }
        let color = if !enabled {
            theme.text_disabled
        } else if on_art {
            theme.text_on_button
        } else {
            theme.text
        };
        layer.texts.push(self.text_job(
            id,
            u32::MAX,
            text,
            font,
            r.inset(6.0 * k, 0.0),
            r,
            color,
            Align::Center,
            false,
            !on_art && font.shadow,
            k,
        ));
    }

    /// A grid of coloured cells, each run of equal colours in a row drawn as one quad.
    fn cells(&self, out: &mut Vec<Vertex>, r: Rect, cols: usize, rows: usize, colors: &[Color]) {
        if cols == 0 || rows == 0 || colors.len() < cols * rows {
            return;
        }
        let xs = |i: usize| (r.x + r.w * i as f32 / cols as f32).round();
        let ys = |j: usize| (r.y + r.h * j as f32 / rows as f32).round();
        for j in 0..rows {
            let mut i = 0;
            while i < cols {
                let c = colors[j * cols + i];
                let mut e = i + 1;
                while e < cols && colors[j * cols + e] == c {
                    e += 1;
                }
                self.solid(
                    out,
                    Rect::new(xs(i), ys(j), xs(e) - xs(i), ys(j + 1) - ys(j)),
                    c,
                );
                i = e;
            }
        }
    }

    fn image_tinted(&self, out: &mut Vec<Vertex>, r: Rect, id: crate::skin::SpriteId, tint: f32) {
        if let Some(s) = self.skin.sprite(id) {
            self.quad(out, r, self.sprite_uv(s), [tint, tint, tint, 1.0]);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn text_job(
        &self,
        id: NodeId,
        row: u32,
        text: &str,
        font: FontStyle,
        rect: Rect,
        clip: Rect,
        color: Color,
        align: Align,
        wrap: bool,
        shadow: bool,
        k: f32,
    ) -> TextJob {
        let mut font = font;
        font.size = (font.size * k).round();
        TextJob {
            slot: (id.0, row),
            text: if font.caps {
                text.to_uppercase()
            } else {
                text.to_owned()
            },
            font,
            rect,
            clip,
            color,
            align,
            wrap,
            shadow,
        }
    }

    fn quad(&self, out: &mut Vec<Vertex>, r: Rect, uv: [f32; 4], color: [f32; 4]) {
        if r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let v = |x: f32, y: f32, u: f32, w: f32| Vertex {
            pos: [x, y],
            uv: [u, w],
            color,
        };
        let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
        let (u0, v0, u1, v1) = (uv[0], uv[1], uv[2], uv[3]);
        out.extend([
            v(x0, y0, u0, v0),
            v(x1, y0, u1, v0),
            v(x0, y1, u0, v1),
            v(x1, y0, u1, v0),
            v(x1, y1, u1, v1),
            v(x0, y1, u0, v1),
        ]);
    }

    fn solid(&self, out: &mut Vec<Vertex>, r: Rect, c: Color) {
        let [u, v] = self.skin.white;
        self.quad(out, r, [u, v, u, v], self.col(c, 1.0));
    }

    fn solid_tinted(&self, out: &mut Vec<Vertex>, r: Rect, c: Color, tint: f32) {
        let [u, v] = self.skin.white;
        self.quad(out, r, [u, v, u, v], self.col(c, tint));
    }

    fn outline(&self, out: &mut Vec<Vertex>, r: Rect, c: Color, k: f32) {
        let t = k.round().max(1.0);
        let r = Rect::new(r.x.round(), r.y.round(), r.w.round(), r.h.round());
        self.solid(out, Rect::new(r.x, r.y, r.w, t), c);
        self.solid(out, Rect::new(r.x, r.bottom() - t, r.w, t), c);
        self.solid(out, Rect::new(r.x, r.y + t, t, r.h - 2.0 * t), c);
        self.solid(out, Rect::new(r.right() - t, r.y + t, t, r.h - 2.0 * t), c);
    }

    fn bordered(
        &self,
        out: &mut Vec<Vertex>,
        r: Rect,
        fill: Color,
        border: Color,
        tint: f32,
        k: f32,
    ) {
        self.solid_tinted(out, r, fill, tint);
        // A light line along the top and left edges gives the plate some relief.
        self.outline(out, r, border, k);
    }

    fn panel(&self, out: &mut Vec<Vertex>, r: Rect, k: f32) {
        match self.skin.panel {
            Some(n) => self.nine(out, r, &n, k, [1.0; 4]),
            None => {
                let t = self.skin.theme;
                self.bordered(out, r, t.panel_fill, t.panel_border, 1.0, k);
                self.outline(
                    out,
                    r.inset(3.0 * k, 3.0 * k),
                    t.panel_border.with_alpha(0x50),
                    k,
                );
            }
        }
    }

    fn sprite_uv(&self, s: &Sprite) -> [f32; 4] {
        s.uv
    }

    fn image(&self, out: &mut Vec<Vertex>, r: Rect, id: crate::skin::SpriteId, fit: Fit, k: f32) {
        let Some(s) = self.skin.sprite(id) else {
            return;
        };
        let uv = self.sprite_uv(s);
        let (ow, oh) = s.size;
        match fit {
            Fit::Stretch => self.quad(out, r, uv, [1.0; 4]),
            Fit::Tile(n) => {
                let (tw, th) = (ow * f32::from(n) / 100.0 * k, oh * f32::from(n) / 100.0 * k);
                if tw < 1.0 || th < 1.0 {
                    return;
                }
                let (du, dv) = (uv[2] - uv[0], uv[3] - uv[1]);
                // A hair of tolerance so a rectangle sized to whole tiles gets no sliver.
                let eps = 0.01;
                let mut y = 0.0;
                while y < r.h - eps {
                    let h = th.min(r.h - y);
                    let mut x = 0.0;
                    while x < r.w - eps {
                        let w = tw.min(r.w - x);
                        self.quad(
                            out,
                            Rect::new(r.x + x, r.y + y, w, h),
                            [uv[0], uv[1], uv[0] + du * w / tw, uv[1] + dv * h / th],
                            [1.0; 4],
                        );
                        x += tw;
                    }
                    y += th;
                }
            }
            Fit::Contain => {
                let k = (r.w / ow).min(r.h / oh);
                let (w, h) = (ow * k, oh * k);
                self.quad(
                    out,
                    Rect::new(r.x + (r.w - w) / 2.0, r.y + (r.h - h) / 2.0, w, h),
                    uv,
                    [1.0; 4],
                );
            }
            Fit::Cover | Fit::CoverTop => {
                let k = (r.w / ow).max(r.h / oh);
                // Visible fraction of the sprite on each axis.
                let (fx, fy) = (r.w / (ow * k), r.h / (oh * k));
                let (du, dv) = (uv[2] - uv[0], uv[3] - uv[1]);
                let (mx, my) = ((1.0 - fx) / 2.0 * du, (1.0 - fy) / 2.0 * dv);
                let (top, bottom) = if fit == Fit::CoverTop {
                    (0.0, 2.0 * my)
                } else {
                    (my, my)
                };
                self.quad(
                    out,
                    r,
                    [uv[0] + mx, uv[1] + top, uv[2] - mx, uv[3] - bottom],
                    [1.0; 4],
                );
            }
        }
    }

    fn nine(&self, out: &mut Vec<Vertex>, r: Rect, n: &NineSlice, k: f32, color: [f32; 4]) {
        let Some(s) = self.skin.sprite(n.sprite) else {
            return;
        };
        let (aw, ah) = self.skin.atlas_size;
        let art = self.skin.art_scale * k;
        let [il, it, ir, ib] = n.insets;
        // Border sizes on screen, shrunk if the rectangle is smaller than the borders.
        let mut sx = 1.0f32;
        let mut sy = 1.0f32;
        if (il + ir) * art > r.w {
            sx = r.w / ((il + ir) * art);
        }
        if (it + ib) * art > r.h {
            sy = r.h / ((it + ib) * art);
        }
        let (l, t, rr, b) = (il * art * sx, it * art * sy, ir * art * sx, ib * art * sy);
        let xs = [r.x, r.x + l, r.right() - rr, r.right()];
        let ys = [r.y, r.y + t, r.bottom() - b, r.bottom()];
        let bake = s.bake;
        let (du, dv) = (bake / aw as f32, bake / ah as f32);
        let us = [
            s.uv[0],
            s.uv[0] + il * sx.min(1.0) * du,
            s.uv[2] - ir * sx.min(1.0) * du,
            s.uv[2],
        ];
        let vs = [
            s.uv[1],
            s.uv[1] + it * sy.min(1.0) * dv,
            s.uv[3] - ib * sy.min(1.0) * dv,
            s.uv[3],
        ];
        for j in 0..3 {
            for i in 0..3 {
                let cell = Rect::new(xs[i], ys[j], xs[i + 1] - xs[i], ys[j + 1] - ys[j]);
                self.quad(out, cell, [us[i], vs[j], us[i + 1], vs[j + 1]], color);
            }
        }
    }
}
