//! `rt2-viewer <file.MAP|file.GAM>`: look at an original map in the classic isometric view.
//!
//! Controls: drag with any mouse button or use WASD / arrows to pan, mouse wheel to zoom,
//! Q / E to rotate by 90 degrees, Home to re-frame the map, Esc to quit.
//! `--screenshot out.png` renders one frame headlessly instead of opening a window.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Parser;
use rt2_formats::map::Map;
use rt2_render::mesh::DEFAULT_HEIGHT_SCALE;
use rt2_render::{Camera, Renderer, TerrainMesh};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Keyboard pan speed in physical screen pixels per second.
const PAN_SPEED: f32 = 900.0;
/// Zoom factor per wheel notch.
const ZOOM_STEP: f32 = 1.15;

#[derive(Parser)]
#[command(about = "Isometric terrain viewer for Railroad Tycoon II maps and saves")]
struct Args {
    /// A .MAP or .GAM file
    path: PathBuf,
    /// Render one frame offscreen to this PNG and exit
    #[arg(long, value_name = "PNG")]
    screenshot: Option<PathBuf>,
    /// Screenshot size in pixels, WIDTHxHEIGHT
    #[arg(long, default_value = "1920x1080", value_parser = parse_size)]
    size: (u32, u32),
    /// Initial view rotation in quarter turns
    #[arg(long, default_value_t = 0)]
    rotation: u8,
    /// Initial zoom as a multiple of the fit-to-window zoom
    #[arg(long, default_value_t = 1.0)]
    zoom: f32,
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    let parse = |v: &str| v.parse::<u32>().map_err(|e| e.to_string());
    let size = (parse(w)?, parse(h)?);
    if size.0 == 0 || size.1 == 0 {
        return Err("size must be non-zero".into());
    }
    Ok(size)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let raw =
        std::fs::read(&args.path).with_context(|| format!("reading {}", args.path.display()))?;
    let map = Map::parse(&raw).with_context(|| format!("parsing {}", args.path.display()))?;
    eprintln!(
        "{}: {}x{} tiles, {:.0}% water",
        map.name,
        map.width,
        map.height,
        map.water_fraction() * 100.0
    );
    let mesh = TerrainMesh::build(&map, DEFAULT_HEIGHT_SCALE);
    let mut camera = Camera::fit(map.width, map.height, args.size);
    camera.rotation = args.rotation % 4;
    camera.zoom_by(args.zoom);

    if let Some(out) = &args.screenshot {
        let pixels = rt2_render::render_offscreen(&mesh, &camera, args.size)?;
        rt2_render::write_png(out, args.size, &pixels)?;
        eprintln!("wrote {}", out.display());
        return Ok(());
    }

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        map_size: (map.width, map.height),
        mesh,
        camera,
        args_zoom: args.zoom,
        gfx: None,
        keys: HashSet::new(),
        cursor: None,
        dragging: false,
        last_frame: Instant::now(),
        error: None,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => bail!(e),
        None => Ok(()),
    }
}

struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

impl Gfx {
    fn new(window: Arc<Window>, mesh: &TerrainMesh) -> Result<Gfx> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
        // Always the physical size: on HiDPI screens this is the real pixel count.
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("surface is not supported by the adapter")?;
        // Prefer an sRGB format: the shader outputs linear colour.
        let caps = surface.get_capabilities(&adapter);
        if let Some(&f) = caps.formats.iter().find(|f| f.is_srgb()) {
            config.format = f;
        }
        config.present_mode = wgpu::PresentMode::Fifo; // vsync
        surface.configure(&device, &config);
        let renderer = Renderer::new(&device, config.format, (config.width, config.height), mesh);
        Ok(Gfx {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // minimised
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.renderer.resize(&self.device, (width, height));
    }

    fn draw(&mut self, camera: &Camera) {
        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match self.surface.get_current_texture() {
            Cst::Success(frame) => frame,
            Cst::Suboptimal(frame) => {
                // Draw this frame, then reconfigure.
                self.render_frame(frame, camera);
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Cst::Outdated | Cst::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Cst::Timeout | Cst::Occluded => return,
            other => {
                eprintln!("surface error: {other:?}");
                return;
            }
        };
        self.render_frame(frame, camera);
    }

    fn render_frame(&self, frame: wgpu::SurfaceTexture, camera: &Camera) {
        let view = frame.texture.create_view(&Default::default());
        self.renderer
            .render(&self.device, &self.queue, &view, camera);
        self.window.pre_present_notify();
        self.queue.present(frame);
    }
}

struct App {
    map_size: (usize, usize),
    mesh: TerrainMesh,
    camera: Camera,
    args_zoom: f32,
    gfx: Option<Gfx>,
    keys: HashSet<KeyCode>,
    cursor: Option<PhysicalPosition<f64>>,
    dragging: bool,
    last_frame: Instant,
    error: Option<String>,
}

impl App {
    fn viewport(&self) -> (u32, u32) {
        self.gfx
            .as_ref()
            .map_or((1, 1), |g| (g.config.width, g.config.height))
    }

    fn held(&self, codes: &[KeyCode]) -> bool {
        codes.iter().any(|c| self.keys.contains(c))
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let axis = |neg: &[KeyCode], pos: &[KeyCode]| {
            self.held(pos) as i32 as f32 - self.held(neg) as i32 as f32
        };
        let dx = axis(
            &[KeyCode::KeyA, KeyCode::ArrowLeft],
            &[KeyCode::KeyD, KeyCode::ArrowRight],
        );
        let dy = axis(
            &[KeyCode::KeyS, KeyCode::ArrowDown],
            &[KeyCode::KeyW, KeyCode::ArrowUp],
        );
        if dx != 0.0 || dy != 0.0 {
            self.camera.pan(dx * PAN_SPEED * dt, dy * PAN_SPEED * dt);
        }
    }

    fn reframe(&mut self) {
        self.camera = Camera::fit(self.map_size.0, self.map_size.1, self.viewport());
        self.camera.zoom_by(self.args_zoom);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("rt2-viewer");
        let result = event_loop
            .create_window(attrs)
            .map_err(anyhow::Error::from)
            .and_then(|w| Gfx::new(Arc::new(w), &self.mesh));
        match result {
            Ok(gfx) => {
                self.gfx = Some(gfx);
                self.reframe();
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gfx) = &mut self.gfx {
                    gfx.resize(size.width, size.height);
                }
            }
            // The physical size changes with the scale factor; the Resized event that follows
            // carries the new size, so nothing to do here.
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                match event.state {
                    ElementState::Pressed => {
                        if !event.repeat {
                            match code {
                                KeyCode::Escape => event_loop.exit(),
                                KeyCode::KeyQ => self.camera.rotate(-1),
                                KeyCode::KeyE => self.camera.rotate(1),
                                KeyCode::Home => self.reframe(),
                                _ => {}
                            }
                        }
                        self.keys.insert(code);
                    }
                    ElementState::Released => {
                        self.keys.remove(&code);
                    }
                }
            }
            WindowEvent::MouseInput { state, .. } => {
                self.dragging = state == ElementState::Pressed;
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let (true, Some(prev)) = (self.dragging, self.cursor) {
                    self.camera
                        .drag((position.x - prev.x) as f32, (position.y - prev.y) as f32);
                }
                self.cursor = Some(position);
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
                self.dragging = false;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let notches = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
                self.camera.zoom_by(ZOOM_STEP.powf(notches));
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.dragging = false;
            }
            WindowEvent::RedrawRequested => {
                self.update();
                let camera = self.camera;
                if let Some(gfx) = &mut self.gfx {
                    gfx.draw(&camera);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }
}
