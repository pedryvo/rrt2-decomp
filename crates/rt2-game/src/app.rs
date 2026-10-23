//! The window loop (winit): creates the window and surface, feeds input to the [`AppState`] and
//! draws it every frame.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use rt2_ui::Skin;

use crate::config::Config;
use crate::gamedir::{self, GameDirScreen};
use crate::gfx::Gfx;
use crate::sound::SoundSystem;
use crate::state::{AppState, Effect, Init};

/// How the window starts: straight into the game, or on the game-folder screen (no usable install
/// is configured).
pub enum Boot {
    Game(Box<Init>, Box<Skin>),
    Setup(Box<gamedir::Params>),
}

struct Surface {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gfx: Gfx,
}

pub struct App {
    state: Option<AppState>,
    boot: Option<Boot>,
    /// The game-folder screen: on top of everything while it is `Some`.
    setup: Option<GameDirScreen>,
    /// The folder chooser running on its own thread.
    pick: Option<mpsc::Receiver<Option<PathBuf>>>,
    /// The install of the running game (a new folder from the setup screen reloads the game).
    installed: Option<PathBuf>,
    /// `--ui-scale`, for a game loaded after the window exists.
    ui_scale_arg: Option<u32>,
    shift: bool,
    open: Option<PathBuf>,
    surface: Option<Surface>,
    started: Instant,
    last: Instant,
    error: Option<String>,
    sound: Option<SoundSystem>,
}

impl Surface {
    fn new(window: Arc<Window>, skin: Skin, vsync: bool) -> Result<Surface> {
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
        config.present_mode = present_mode(vsync);
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

    /// Vertical sync on (`Fifo`) or off (the best mode without waiting).
    fn set_vsync(&mut self, vsync: bool) {
        let mode = present_mode(vsync);
        if self.config.present_mode != mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.gfx.device, &self.config);
        }
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
        self.draw_with(|gfx, view| gfx.draw(state, view, water_frame));
    }

    fn draw_setup(&mut self, screen: &mut GameDirScreen) {
        self.draw_with(|gfx, view| gfx.draw_setup(screen, view));
    }

    fn draw_with(&mut self, f: impl FnOnce(&mut Gfx, &wgpu::TextureView)) {
        use wgpu::CurrentSurfaceTexture as Cst;
        let frame = match self.surface.get_current_texture() {
            Cst::Success(frame) => frame,
            Cst::Suboptimal(frame) => {
                self.present(frame, f);
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
        self.present(frame, f);
    }

    fn present(
        &mut self,
        frame: wgpu::SurfaceTexture,
        f: impl FnOnce(&mut Gfx, &wgpu::TextureView),
    ) {
        let view = frame.texture.create_view(&Default::default());
        f(&mut self.gfx, &view);
        self.window.pre_present_notify();
        self.gfx.queue.present(frame);
    }
}

fn present_mode(vsync: bool) -> wgpu::PresentMode {
    if vsync {
        wgpu::PresentMode::Fifo
    } else {
        wgpu::PresentMode::AutoNoVsync
    }
}

/// The monitors' names for the Options screen, in the order `available_monitors` gives them.
fn monitor_names(event_loop: &ActiveEventLoop) -> Vec<String> {
    event_loop
        .available_monitors()
        .enumerate()
        .map(|(i, m)| {
            let size = m.size();
            let name = m.name().unwrap_or_else(|| format!("Monitor {}", i + 1));
            format!("{name} ({}x{})", size.width, size.height)
        })
        .collect()
}

/// Window mode, size and monitor from the `[display]` settings.
fn apply_display(window: &Window, d: &crate::config::Display, event_loop: &ActiveEventLoop) {
    use crate::config::WindowMode;
    let monitor = d
        .monitor
        .and_then(|i| event_loop.available_monitors().nth(i));
    match d.mode {
        WindowMode::Windowed => {
            window.set_fullscreen(None);
            if let Some(m) = &monitor {
                window.set_outer_position(m.position());
            }
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(d.width, d.height));
        }
        WindowMode::Borderless => window.set_fullscreen(Some(Fullscreen::Borderless(monitor))),
    }
}

/// Runs the game window until it is closed.
pub fn run(boot: Boot, open: Option<PathBuf>, ui_scale_arg: Option<u32>) -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        state: None,
        boot: Some(boot),
        setup: None,
        pick: None,
        installed: None,
        ui_scale_arg,
        shift: false,
        open,
        surface: None,
        started: Instant::now(),
        last: Instant::now(),
        error: None,
        sound: None,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => bail!(e),
        None => Ok(()),
    }
}

impl App {
    /// A game over the current surface: the state, the sound and the first screen.
    fn start_game(&mut self, event_loop: &ActiveEventLoop, mut init: Init) {
        let Some(surface) = &self.surface else {
            return;
        };
        let size = surface.window.inner_size();
        init.viewport = (size.width.max(1), size.height.max(1));
        let dpi = surface.window.scale_factor();
        if !init.scale_pinned {
            init.scale = rt2_ui::UiScale::from_dpi(dpi);
        }
        self.installed = Some(init.install.clone());
        // Audio is optional: without a device this logs once and the game runs silent.
        self.sound = Some(SoundSystem::open(&init.install, init.config.audio.clone()));
        let mut state = AppState::new(init);
        state.dpi_changed(dpi);
        state.set_monitors(monitor_names(event_loop));
        // The real game: choices wait for the player, autosaves are written.
        state.set_interactive_choices(true);
        state.set_persistence(true);
        // Clips of the original (intro, end of a scenario) play in the real game only.
        state.set_videos(true);
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
        if let Some(path) = self.open.take() {
            if let Err(e) = state.start(&path) {
                self.error = Some(format!("{e:#}"));
                event_loop.exit();
                return;
            }
        } else {
            state.play_intro();
        }
        self.state = Some(state);
        self.last = Instant::now();
    }

    /// Opens the folder chooser on its own thread (the portal or the system dialog blocks).
    fn spawn_pick(&mut self, start: Option<PathBuf>, title: String) {
        let (tx, rx) = mpsc::channel();
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if let Some(dir) = start.filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(surface) = &self.surface {
            dialog = dialog.set_parent(&*surface.window);
        }
        std::thread::spawn(move || {
            let _ = tx.send(dialog.pick_folder());
        });
        self.pick = Some(rx);
    }

    /// The game-folder screen over the running game (Options, "Game folder...").
    fn open_setup(&mut self, config: Config) {
        let Some(surface) = &self.surface else {
            return;
        };
        let size = surface.window.inner_size();
        self.setup = Some(GameDirScreen::new(
            gamedir::Params {
                config,
                config_path: crate::paths::config_file(),
                can_cancel: true,
                roots: None,
            },
            (size.width, size.height),
            surface.window.scale_factor(),
        ));
        if let Some(state) = &mut self.state {
            state.focus_lost();
        }
    }

    /// What the game-folder screen asked for.
    fn setup_effects(&mut self, event_loop: &ActiveEventLoop) {
        let Some(setup) = &mut self.setup else {
            return;
        };
        for e in setup.take_effects() {
            match e {
                gamedir::Effect::Browse { start, title } => self.spawn_pick(start, title),
                gamedir::Effect::Leave => {
                    if self.state.is_some() {
                        self.setup = None;
                        self.last = Instant::now();
                    } else {
                        event_loop.exit();
                    }
                    return;
                }
                gamedir::Effect::Play { path, config } => {
                    if self.state.is_some() && self.installed.as_deref() == Some(path.as_path()) {
                        self.setup = None;
                        self.last = Instant::now();
                        return;
                    }
                    let Some(surface) = &self.surface else {
                        return;
                    };
                    let size = surface.window.inner_size();
                    match crate::build_init(
                        path,
                        *config,
                        self.ui_scale_arg,
                        (size.width, size.height),
                    ) {
                        Ok((init, skin)) => {
                            if let Some(s) = &mut self.surface {
                                s.gfx.replace_skin(skin);
                            }
                            self.state = None;
                            self.setup = None;
                            self.start_game(event_loop, init);
                        }
                        Err(e) => {
                            if let Some(setup) = &mut self.setup {
                                setup.failed(&format!("{e:#}"));
                            }
                        }
                    }
                    return;
                }
            }
        }
    }

    fn setup_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let (Some(setup), Some(surface)) = (&mut self.setup, &mut self.surface) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                surface.resize(size.width, size.height);
                setup.resize((size.width, size.height));
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => setup.dpi_changed(scale_factor),
            WindowEvent::ModifiersChanged(m) => self.shift = m.state().shift_key(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    setup.key(
                        code,
                        event.state == ElementState::Pressed,
                        event.repeat,
                        self.shift,
                    );
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                setup.cursor_moved(position.x as f32, position.y as f32);
            }
            WindowEvent::CursorLeft { .. } => setup.cursor_left(),
            WindowEvent::MouseInput {
                state: s, button, ..
            } => setup.mouse_button(button == MouseButton::Left, s == ElementState::Pressed),
            WindowEvent::MouseWheel { delta, .. } => {
                setup.wheel(match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                });
            }
            WindowEvent::RedrawRequested => {
                setup.update(0.0);
                surface.draw_setup(setup);
            }
            _ => {}
        }
        self.setup_effects(event_loop);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        let Some(boot) = self.boot.take() else {
            return;
        };
        let (display, skin, init, params) = match boot {
            Boot::Game(init, skin) => (init.config.display.clone(), *skin, Some(*init), None),
            Boot::Setup(p) => (p.config.display.clone(), Skin::flat(), None, Some(*p)),
        };
        let mut attrs = Window::default_attributes()
            .with_title("Railroad Tycoon II (rrt2)")
            .with_inner_size(winit::dpi::PhysicalSize::new(
                display.width.max(320),
                display.height.max(240),
            ));
        if display.mode == crate::config::WindowMode::Borderless {
            let monitor = display
                .monitor
                .and_then(|i| event_loop.available_monitors().nth(i));
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(monitor)));
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        let dpi = window.scale_factor();
        match Surface::new(window, skin, display.vsync) {
            Ok(s) => {
                self.surface = Some(s);
                if let Some(init) = init {
                    self.start_game(event_loop, init);
                }
                if let Some(params) = params {
                    self.setup = Some(GameDirScreen::new(params, (size.width, size.height), dpi));
                }
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.setup.is_some() {
            self.setup_event(event_loop, event);
            return;
        }
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
                if let Some(sound) = &mut self.sound {
                    sound.update();
                }
                // The ocean animates at about 8 frames per second.
                let frame = (self.started.elapsed().as_millis() / 125) as u32;
                surface.draw(state, frame);
            }
            _ => {}
        }
        for cue in state.take_cues() {
            if let Some(sound) = &mut self.sound {
                sound.apply(cue);
            }
        }
        let mut open_setup = None;
        for e in state.take_effects() {
            match e {
                Effect::Quit => event_loop.exit(),
                Effect::ApplyDisplay(d) => {
                    apply_display(&surface.window, &d, event_loop);
                    surface.set_vsync(d.vsync);
                }
                Effect::SaveConfig(c) => save_config(&c),
                Effect::ChooseGameFolder(c) => open_setup = Some(*c),
            }
        }
        if let Some(config) = open_setup {
            self.open_setup(config);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(rx) = &self.pick {
            let answer = match rx.try_recv() {
                Ok(p) => Some(p),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(None),
            };
            if let Some(p) = answer {
                self.pick = None;
                if let Some(setup) = &mut self.setup {
                    setup.picked(p);
                }
            }
        }
        if let Some(s) = &self.surface {
            s.window.request_redraw();
        }
    }
}

/// Writes `config.toml` (a failure is only logged).
fn save_config(config: &crate::config::Config) {
    let Some(path) = crate::paths::config_file() else {
        return;
    };
    if let Err(e) = config.save(&path) {
        eprintln!("cannot save the configuration: {e:#}");
    }
}
