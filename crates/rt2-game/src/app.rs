//! The window loop (winit): creates the window and surface, feeds input to the [`AppState`] and
//! draws it every frame.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use rt2_ui::Skin;

use crate::gfx::Gfx;
use crate::sound::SoundSystem;
use crate::state::{AppState, Effect, Init};

struct Surface {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gfx: Gfx,
}

pub struct App {
    state: Option<AppState>,
    init: Option<(Init, Skin)>,
    open: Option<std::path::PathBuf>,
    surface: Option<Surface>,
    started: Instant,
    last: Instant,
    error: Option<String>,
    sound: SoundSystem,
}

impl Surface {
    fn new(window: Arc<Window>, skin: Skin) -> Result<Surface> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .context("no suitable GPU adapter")?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&rt2_render::renderer::device_descriptor()))?;
        // Always the physical size: on HiDPI screens this is the real pixel count.
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("surface is not supported by the adapter")?;
        let caps = surface.get_capabilities(&adapter);
        if let Some(&f) = caps.formats.iter().find(|f| f.is_srgb()) {
            config.format = f;
        }
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(&device, &config);
        let gfx = Gfx::new(
            device,
            queue,
            config.format,
            (config.width, config.height),
            skin,
        );
        Ok(Surface {
            window,
            surface,
            config,
            gfx,
        })
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.gfx.device, &self.config);
        self.gfx.resize((w, h));
    }

    fn draw(&mut self, state: &mut AppState, water_frame: u32) {
        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match self.surface.get_current_texture() {
            Cst::Success(f) => f,
            Cst::Suboptimal(f) => {
                self.present(f, state, water_frame);
                self.surface.configure(&self.gfx.device, &self.config);
                return;
            }
            Cst::Outdated | Cst::Lost => {
                self.surface.configure(&self.gfx.device, &self.config);
                return;
            }
            Cst::Timeout | Cst::Occluded => return,
            other => {
                eprintln!("surface error: {other:?}");
                return;
            }
        };
        self.present(frame, state, water_frame);
    }

    fn present(&mut self, frame: wgpu::SurfaceTexture, state: &mut AppState, water_frame: u32) {
        let view = frame.texture.create_view(&Default::default());
        self.gfx.draw(state, &view, water_frame);
        self.window.pre_present_notify();
        self.gfx.queue.present(frame);
    }
}

/// Runs the game window until it is closed.
pub fn run(init: Init, skin: Skin, open: Option<std::path::PathBuf>) -> Result<()> {
    // Audio is optional: without a device this logs once and the game runs silent.
    let sound = SoundSystem::open(&init.install, init.config.audio.clone());
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        state: None,
        init: Some((init, skin)),
        open,
        surface: None,
        started: Instant::now(),
        last: Instant::now(),
        error: None,
        sound,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => bail!(e),
        None => Ok(()),
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        let Some((mut init, skin)) = self.init.take() else {
            return;
        };
        let attrs = Window::default_attributes()
            .with_title("Railroad Tycoon II (rrt2)")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        init.viewport = (size.width.max(1), size.height.max(1));
        if !init.scale_pinned {
            init.scale = rt2_ui::UiScale::from_dpi(window.scale_factor());
        }
        match Surface::new(window, skin) {
            Ok(s) => {
                let mut state = AppState::new(init);
                // Each real game gets its own seed (the original seeds from the clock); the
                // simulation never sees the clock, only this number.
                state.set_seed(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(1, |d| (d.as_millis() % 0xffff_ffff) as u32),
                );
                if !state.invalid_keys().is_empty() {
                    eprintln!(
                        "unknown key names in config.toml: {:?}",
                        state.invalid_keys()
                    );
                }
                if let Some(path) = self.open.take()
                    && let Err(e) = state.start(&path)
                {
                    self.error = Some(format!("{e:#}"));
                    event_loop.exit();
                    return;
                }
                self.state = Some(state);
                self.surface = Some(s);
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let (Some(state), Some(surface)) = (&mut self.state, &mut self.surface) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                surface.resize(size.width, size.height);
                state.resize((size.width.max(1), size.height.max(1)));
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => state.dpi_changed(scale_factor),
            WindowEvent::ModifiersChanged(m) => {
                let s: ModifiersState = m.state();
                state.modifiers(s.control_key(), s.shift_key());
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                let text = event.text.as_ref().map(|t| t.as_str());
                state.key(
                    code,
                    event.state == ElementState::Pressed,
                    event.repeat,
                    text,
                );
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_moved(position.x as f32, position.y as f32);
            }
            WindowEvent::CursorLeft { .. } => state.cursor_left(),
            WindowEvent::MouseInput {
                state: s, button, ..
            } => {
                state.mouse_button(button == MouseButton::Left, s == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let notches = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
                state.wheel(notches);
            }
            WindowEvent::Focused(false) => state.focus_lost(),
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last).as_secs_f32() * 1000.0;
                self.last = now;
                state.update(dt);
                self.sound.update();
                // The ocean animates at about 8 frames per second.
                let frame = (self.started.elapsed().as_millis() / 125) as u32;
                surface.draw(state, frame);
            }
            _ => {}
        }
        for cue in state.take_cues() {
            self.sound.apply(cue);
        }
        for e in state.take_effects() {
            match e {
                Effect::Quit => event_loop.exit(),
                Effect::ToggleFullscreen => {
                    let fs = surface.window.fullscreen().is_none();
                    surface
                        .window
                        .set_fullscreen(fs.then_some(Fullscreen::Borderless(None)));
                }
                Effect::SaveLanguage(locale) => save_language(&locale),
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.surface {
            s.window.request_redraw();
        }
    }
}

/// Remembers the chosen language in config.toml (a failure is only logged).
fn save_language(locale: &str) {
    let Some(path) = crate::paths::config_file() else {
        return;
    };
    let result = crate::config::Config::load(&path).and_then(|mut c| {
        c.language = Some(locale.to_owned());
        c.save(&path)
    });
    if let Err(e) = result {
        eprintln!("cannot save the language: {e:#}");
    }
}
