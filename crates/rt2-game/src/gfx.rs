//! GPU side of the game: the map renderer (when a game is open) and the UI renderer on top.

use anyhow::{Context, Result};
use rt2_render::Renderer;
use rt2_ui::{Skin, UiRenderer};

use crate::state::AppState;

pub struct Gfx {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    ui: UiRenderer,
    map: Option<Renderer>,
}

/// Menu background while no art is loaded behind it (linear colour).
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.008,
    g: 0.010,
    b: 0.008,
    a: 1.0,
};

impl Gfx {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        skin: Skin,
    ) -> Gfx {
        let ui = UiRenderer::new(&device, &queue, format, skin);
        Gfx {
            device,
            queue,
            format,
            size,
            ui,
            map: None,
        }
    }

    pub fn resize(&mut self, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        self.size = size;
        if let Some(m) = &mut self.map {
            m.resize(&self.device, size);
        }
    }

    pub fn draw(&mut self, state: &mut AppState, target: &wgpu::TextureView, water_frame: u32) {
        self.ui.set_script(state.script());
        for (id, rgba) in state.take_sprite_uploads() {
            self.ui.upload_sprite(&self.queue, id, &rgba);
        }
        if let Some(scene) = state.take_scene() {
            match &mut self.map {
                Some(m) => m.replace_scene(&self.device, scene),
                None => {
                    self.map = Some(Renderer::new(
                        &self.device,
                        &self.queue,
                        self.format,
                        self.size,
                        scene,
                    ));
                }
            }
        }
        match (state.camera(), &mut self.map) {
            (Some(camera), Some(map)) => {
                if let Some((frame, reset)) = state.take_live() {
                    if reset {
                        map.reset_live(frame);
                    } else {
                        map.push_live(frame);
                    }
                }
                map.set_live_alpha(state.live_alpha());
                map.set_water_frame(water_frame);
                map.render(&self.device, &self.queue, target, &camera);
                self.ui.render(
                    &self.device,
                    &self.queue,
                    target,
                    self.size,
                    &mut state.ui,
                    None,
                );
            }
            _ => {
                self.ui.render(
                    &self.device,
                    &self.queue,
                    target,
                    self.size,
                    &mut state.ui,
                    Some(CLEAR),
                );
            }
        }
    }
}

/// A headless device (software or hardware adapter).
pub fn headless_device() -> Result<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .context("no suitable GPU adapter")?;
    pollster::block_on(adapter.request_device(&rt2_render::renderer::device_descriptor()))
        .context("cannot create the GPU device")
}

/// Renders one frame of `state` offscreen and returns tightly packed RGBA8 (sRGB) pixels.
pub fn render_offscreen(gfx: &mut Gfx, state: &mut AppState, size: (u32, u32)) -> Result<Vec<u8>> {
    gfx.resize(size);
    let texture = gfx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("screenshot"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gfx.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    gfx.draw(state, &view, 0);

    let padded_row = (size.0 * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = gfx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded_row) * u64::from(size.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gfx.device.create_command_encoder(&Default::default());
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
    gfx.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gfx.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    rx.recv()??;
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut pixels = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for row in mapped.chunks_exact(padded_row as usize) {
        pixels.extend_from_slice(&row[..(size.0 * 4) as usize]);
    }
    Ok(pixels)
}

pub const SCREENSHOT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
