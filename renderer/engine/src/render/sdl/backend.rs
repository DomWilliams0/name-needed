use std::hint::unreachable_unchecked;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::{MouseButton, MouseState, MouseWheelDirection};
use sdl2::video::{Window, WindowBuildError};
use sdl2::{EventPump, Sdl, VideoSubsystem};

use color::Color;
use common::input::{CameraDirection, EngineKey, GameKey, KeyAction, RendererKey};
use common::*;
use config::Config;
use resources::ResourceError;
use resources::Resources;
use simulation::input::{
    InputEvent, SelectType, UiCommand, UiCommands, UiPopup, UiRequest, WorldColumn,
};
use simulation::{
    BackendData, ComponentWorld, Exit, GameSpeedChange, InitializedSimulationBackend,
    MainMenuAction, MainMenuConfig, MainMenuOutput, PerfAvg, PersistentSimulationBackend, Scenario,
    Simulation, WorldViewer,
};
use unit::world::{WorldPoint, WorldPoint2d, WorldPosition};

use crate::render::sdl::camera::Camera;
use crate::render::sdl::gl::{Gl, GlError};
use crate::render::sdl::render::GlFrameContext;
use crate::render::sdl::selection::Selection;
use crate::render::sdl::ui::{EventConsumed, Ui};
use crate::render::sdl::GlRenderer;

pub struct SdlBackendPersistent {
    camera: Camera,
    is_first_init: bool,

    /// `take`n out and replaced each tick
    sdl_events: Option<EventPump>,
    keep_alive: GraphicsKeepAlive,
    window: Window,
    window_id: u32,

    renderer: GlRenderer,
    ui: Ui,
    /// Events from game -> UI, queued up and passed to sim on each frame
    sim_input_events: Vec<InputEvent>,
    selection: Selection,
}

pub struct SdlBackendInit {
    backend: SdlBackendPersistent,
    world_viewer: WorldViewer,
}

// TODO per-world save directory abstraction
const PERSISTED_UI_PATH: &str = "uistate.bin";

/// Unused fields but need to be kept alive
#[allow(dead_code)]
struct GraphicsKeepAlive {
    sdl: Sdl,
    video: VideoSubsystem,
    gl: Gl,
}

#[derive(Debug, Error)]
pub enum SdlBackendError {
    #[error("SDL error: {0}")]
    Sdl(String),

    #[error("Failed to create window: {0}")]
    WindowCreation(#[from] WindowBuildError),

    #[error("OpenGL error: {0}")]
    Gl(#[from] GlError),

    #[error("Failed to load resources: {0}")]
    Resources(#[from] ResourceError),
}

impl PersistentSimulationBackend for SdlBackendPersistent {
    type Error = SdlBackendError;
    type Initialized = SdlBackendInit;

    fn new(resources: &Resources) -> Result<Self, Self::Error> {
        let sdl = sdl2::init().map_err(SdlBackendError::Sdl)?;
        let video = sdl.video().map_err(SdlBackendError::Sdl)?;
        video.gl_attr().set_context_version(3, 0);
        video.gl_attr().set_depth_size(24);
        info!(
            "opengl version {major}.{minor}",
            major = video.gl_attr().context_major_version(),
            minor = video.gl_attr().context_minor_version()
        );

        let (w, h) = config::get().display.resolution;
        info!("window size {width}x{height}", width = w, height = h);

        let window = {
            let mut builder = video.window("Name Needed", w, h);

            builder.position_centered().allow_highdpi().opengl();

            if config::get().display.resizable {
                builder.resizable();
            }
            builder.build()?
        };
        let window_id = window.id();

        let gl = Gl::new(&window, &video).map_err(SdlBackendError::Sdl)?;
        Gl::set_clear_color(Color::rgb(17, 17, 20));

        let ui = Ui::new(&window, &video, PERSISTED_UI_PATH.as_ref());

        // enable vsync
        video
            .gl_set_swap_interval(1)
            .map_err(SdlBackendError::Sdl)?;

        let events = sdl.event_pump().map_err(SdlBackendError::Sdl)?;
        let renderer = GlRenderer::new(resources)?;
        let camera = Camera::new(w as i32, h as i32);

        Ok(Self {
            camera,
            is_first_init: true,
            sdl_events: Some(events),
            keep_alive: GraphicsKeepAlive { sdl, video, gl },
            window,
            window_id,
            renderer,
            ui,
            sim_input_events: Vec::with_capacity(32),
            selection: Selection::default(),
        })
    }

    fn start(self, world: WorldViewer, initial_block: WorldPosition) -> Self::Initialized {
        let mut backend = SdlBackendInit {
            backend: self,
            world_viewer: world,
        };

        // move camera to focus on the initial block
        // only do on first instance, preserve player's camera position across other restarts
        if std::mem::take(&mut backend.is_first_init) {
            backend.camera.set_centre(WorldPoint::from(initial_block));
        }

        backend
    }

    fn show_main_menu(
        &mut self,
        scenarios: &[Scenario],
        initial_config: &Config,
    ) -> Result<MainMenuOutput, Self::Error> {
        let mut main_menu = self.ui.main_menu(scenarios);
        let mut config = MainMenuConfig {
            config: initial_config.clone(),
        };

        Gl::set_clear_color(Color::rgb(144, 151, 163));

        let action = 'outer: loop {
            // take events out temporarily
            let mut event_pump = match self.sdl_events.take() {
                Some(e) => e,
                _ => {
                    debug_assert!(false, "bad event pump state");
                    unsafe { unreachable_unchecked() }
                }
            };

            for event in event_pump.poll_iter() {
                if let EventConsumed::Consumed = main_menu.handle_event(&event) {
                    continue;
                }

                match &event {
                    Event::Quit { .. } => {
                        break 'outer MainMenuAction::Exit;
                    }
                    Event::Window {
                        win_event: WindowEvent::Close,
                        window_id,
                        ..
                    } if *window_id == self.window_id => {
                        break 'outer MainMenuAction::Exit;
                    }

                    _ => {}
                }
            }

            Gl::clear();

            let mouse_state = MouseState::new(&event_pump);
            let action = main_menu.render_main_menu(&self.window, &mouse_state, &mut config);

            // put back event pump like we never took it
            let none = ManuallyDrop::new(std::mem::replace(&mut self.sdl_events, Some(event_pump)));
            debug_assert!(none.is_none());

            self.window.gl_swap_window();

            if let Some(action) = action {
                break action;
            }
        };

        Ok(MainMenuOutput { action, config })
    }

    fn name() -> &'static str {
        "SDL2"
    }
}

impl InitializedSimulationBackend for SdlBackendInit {
    type Renderer = GlRenderer;
    type Persistent = SdlBackendPersistent;

    fn start(&mut self, commands_out: &mut UiCommands) {
        // emit commands to game from persisted ui state
        self.ui.on_start(commands_out);
    }

    fn consume_events(&mut self, commands: &mut UiCommands) -> BackendData {
        // take event pump out of self, to be replaced at the end of the tick
        let mut events = match self.sdl_events.take() {
            Some(e) => e,
            _ => {
                debug_assert!(false, "bad event pump state");
                unsafe { unreachable_unchecked() }
            }
        };

        for event in events.poll_iter() {
            if let EventConsumed::Consumed = self.ui.handle_event(&event) {
                continue;
            }

            match event {
                Event::Quit { .. } => {
                    commands.push(UiCommand::new(UiRequest::ExitGame(Exit::Stop)));
                    break;
                }
                Event::Window {
                    win_event: WindowEvent::Close,
                    window_id,
                    ..
                } if window_id == self.window_id => {
                    commands.push(UiCommand::new(UiRequest::ExitGame(Exit::Stop)));
                    break;
                }
                Event::Window {
                    win_event: WindowEvent::Resized(width, height),
                    ..
                } => {
                    debug!("resized window"; "width" => width, "height" => height);
                    Gl::set_viewport(width, height);
                    self.camera.on_resize(width, height);
                }

                Event::KeyDown {
                    keycode: Some(key),
                    keymod,
                    ..
                } => match map_sdl_keycode(key, keymod) {
                    Some(action) => {
                        let ui_req = self.handle_key(action, keymod, true);
                        commands.extend(ui_req.map(UiCommand::new).into_iter());
                    }

                    None => debug!("ignoring unknown key"; "key" => %key),
                },
                Event::KeyUp {
                    keycode: Some(key),
                    keymod,
                    ..
                } => {
                    if let Some(action) = map_sdl_keycode(key, keymod) {
                        let ui_req = self.handle_key(action, keymod, false);
                        commands.extend(ui_req.map(UiCommand::new).into_iter());
                    }
                }

                Event::MouseButtonDown {
                    mouse_btn, x, y, ..
                } => {
                    if let Some((sel, col)) = self.parse_mouse_event(mouse_btn, x, y) {
                        self.selection.mouse_down(sel, col);
                    }
                }

                Event::MouseButtonUp {
                    mouse_btn, x, y, ..
                } => {
                    let evt = self
                        .parse_mouse_event(mouse_btn, x, y)
                        .and_then(|(select, col)| {
                            let modifiers = self.mod_state();
                            self.selection.mouse_up(select, col, modifiers)
                        });

                    if let Some(evt) = evt {
                        self.sim_input_events.push(evt);
                    }
                }

                Event::MouseMotion {
                    mousestate, x, y, ..
                } => {
                    if let Some(mouse_btn) = mousestate.pressed_mouse_buttons().next() {
                        if let Some((select, col)) = self.parse_mouse_event(mouse_btn, x, y) {
                            let modifiers = self.mod_state();
                            let evt = self.selection.mouse_move(select, col, modifiers);
                            if let Some(evt) = evt {
                                self.sim_input_events.push(evt);
                            }
                        }
                    }
                }
                Event::MouseWheel {
                    mut y, direction, ..
                } => {
                    if let MouseWheelDirection::Flipped = direction {
                        y *= -1;
                    }

                    // TODO if mouse wheel is reused for anything else, add an input event for it
                    self.camera.handle_zoom(y);
                }

                _ => {}
            };
        }

        let mouse_state = MouseState::new(&events);

        // put back event pump like we never took it
        let none = std::mem::replace(&mut self.sdl_events, Some(events));
        debug_assert!(none.is_none());
        std::mem::forget(none);

        let mouse_position = {
            let (x, y) = self
                .camera
                .screen_to_world((mouse_state.x(), mouse_state.y()));

            let x = NotNan::new(x).ok();
            let y = NotNan::new(y).ok();
            x.zip(y).map(|(x, y)| WorldPoint2d::new(x, y))
        };

        BackendData { mouse_position }
    }

    fn tick(&mut self) {
        let chunk_bounds = self.camera.bounds();
        self.world_viewer.set_chunk_bounds(chunk_bounds);

        let renderer = self.backend.renderer.terrain_mut();
        self.world_viewer
            .regenerate_dirty_chunk_meshes(|chunk_pos, mesh| {
                if let Err(e) = renderer.update_chunk_mesh(chunk_pos, mesh) {
                    error!(
                        "failed to regenerate mesh for chunk";
                        chunk_pos, "error" => %e
                    );
                }
            });
    }

    fn render(
        &mut self,
        simulation: &mut Simulation<Self::Renderer>,
        interpolation: f64,
        perf: PerfAvg,
        commands: &mut UiCommands,
    ) {
        // clear window
        Gl::clear();

        // calculate projection and view matrices
        self.camera.tick(interpolation);
        let projection = self.camera.projection_matrix();

        let terrain_range = self.world_viewer.terrain_range();
        let camera_z = {
            // position camera a fixed distance above the top of the terrain
            const CAMERA_Z_OFFSET: f32 = 20.0;
            terrain_range.size() as f32 + CAMERA_Z_OFFSET
        };
        let view = self.camera.view_matrix(camera_z);

        // render world
        self.renderer
            .terrain()
            .render(&projection, &view, &self.world_viewer);

        // render simulation
        let lower_limit = terrain_range.bottom().slice() as f32;
        let frame_ctx = GlFrameContext {
            projection,
            text_transform: self.camera.scaled_text_transform_matrix(camera_z),
            zoom: self.camera.zoom(),
            view,
            z_offset: lower_limit,
        };
        let _ = simulation.render(
            &self.world_viewer,
            frame_ctx,
            &mut self.backend.renderer,
            interpolation,
            &self.backend.sim_input_events,
        );

        // input events were for this frame only
        self.sim_input_events.clear();

        // render ui and collect input commands
        let mouse_state = self.backend.mouse_state();
        let popups = simulation.world().resource_mut::<UiPopup>().prepare();
        self.backend.ui.render(
            &self.backend.window,
            &mouse_state,
            perf,
            simulation.as_ref(&self.world_viewer),
            commands,
            popups,
        );

        self.window.gl_swap_window();
    }

    fn world_viewer(&mut self) -> &mut WorldViewer {
        &mut self.world_viewer
    }

    fn end(mut self) -> Self::Persistent {
        self.sim_input_events.clear();
        self.renderer.reset();

        if let Err(err) = self.ui.on_exit(PERSISTED_UI_PATH.as_ref()) {
            warn!("failed to persist ui to {}: {}", PERSISTED_UI_PATH, err);
        }
        self.backend
    }
}

/// Helper to ease accessing self.backend
impl Deref for SdlBackendInit {
    type Target = SdlBackendPersistent;

    fn deref(&self) -> &Self::Target {
        &self.backend
    }
}

/// Helper to ease accessing self.backend
impl DerefMut for SdlBackendInit {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.backend
    }
}

impl SdlBackendInit {
    fn handle_key(
        &mut self,
        action: KeyAction,
        modifiers: Mod,
        is_down: bool,
    ) -> Option<UiRequest> {
        use RendererKey::*;

        match action {
            KeyAction::Renderer(key) => {
                match (is_down, key) {
                    (true, SliceDown) | (true, SliceUp) => {
                        let delta = if let SliceDown = key { -1 } else { 1 };

                        if modifiers & (Mod::LCTRLMOD | Mod::RCTRLMOD) != Mod::NOMOD {
                            // stretch world viewer
                            self.world_viewer.stretch_by(delta);
                        } else if modifiers & (Mod::LSHIFTMOD | Mod::RSHIFTMOD) != Mod::NOMOD {
                            // move by larger amount
                            self.world_viewer.move_by_multiple(delta);
                        } else {
                            // move by 1 slice
                            self.world_viewer.move_by(delta);
                        }

                        None
                    }

                    (_, Camera(direction)) => {
                        self.camera.handle_move(direction, is_down);
                        Some(UiRequest::CancelPopup)
                    }
                    _ => {
                        if is_down {
                            warn!("unhandled key down: {:?}", key);
                        }
                        None
                    }
                }
            }
            KeyAction::Engine(key) => {
                if is_down {
                    let cmd = match key {
                        EngineKey::Exit => UiRequest::ExitGame(Exit::Stop),
                        EngineKey::Restart => UiRequest::ExitGame(Exit::Restart),
                        EngineKey::SpeedUp => UiRequest::ChangeGameSpeed(GameSpeedChange::Faster),
                        EngineKey::SlowDown => UiRequest::ChangeGameSpeed(GameSpeedChange::Slower),
                    };

                    Some(cmd)
                } else {
                    // ignore key ups
                    None
                }
            }

            KeyAction::Game(key) => {
                // send input to game on down only
                if is_down {
                    Some(match key {
                        GameKey::CancelSelection => UiRequest::CancelSelection,
                        GameKey::TogglePaused => UiRequest::TogglePaused,
                    })
                } else {
                    None
                }
            }
        }
    }

    fn parse_mouse_event(
        &self,
        button: MouseButton,
        x: i32,
        y: i32,
    ) -> Option<(SelectType, WorldColumn)> {
        Selection::select_type(button).and_then(|select| {
            let (wx, wy) = self.camera.screen_to_world((x, y));
            let col = WorldColumn {
                x: NotNan::new(wx).ok()?,
                y: NotNan::new(wy).ok()?,
                slice_range: self.world_viewer.entity_range(),
            };
            Some((select, col))
        })
    }

    fn mod_state(&self) -> Mod {
        self.keep_alive.sdl.keyboard().mod_state()
    }
}

impl SdlBackendPersistent {
    fn mouse_state(&self) -> MouseState {
        self.sdl_events
            .as_ref()
            .unwrap() // always Some outside of consume_events
            .mouse_state()
    }
}

/// can't use TryInto/TryFrom for now
// -----
// error[E0119]: conflicting implementations of trait `std::convert::TryInto<common::input::Key>` for type `sdl2::keyboard::keycode::Keycode`:
//   --> engine/src/render/renderer.rs:306:1
//    |
//306 | impl TryInto<Key> for Keycode {
//    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//    |
//    = note: conflicting implementation in crate `core`:
//            - impl<T, U> std::convert::TryInto<U> for T
//              where U: std::convert::TryFrom<T>;
//    = note: upstream crates may add a new impl of trait `std::convert::From<sdl2::keyboard::keycode::Keycode>` for type `common::input::Key` in future versions
fn map_sdl_keycode(keycode: Keycode, keymod: Mod) -> Option<KeyAction> {
    use Keycode::*;
    use RendererKey::*;

    let alt_down = || keymod.intersects(Mod::LALTMOD | Mod::RALTMOD);

    Some(match keycode {
        Q if alt_down() => KeyAction::Engine(EngineKey::Exit),
        R if alt_down() => KeyAction::Engine(EngineKey::Restart),
        RightBracket => KeyAction::Engine(EngineKey::SpeedUp),
        LeftBracket => KeyAction::Engine(EngineKey::SlowDown),

        Up => KeyAction::Renderer(SliceUp),
        Down => KeyAction::Renderer(SliceDown),

        W => KeyAction::Renderer(Camera(CameraDirection::Up)),
        A => KeyAction::Renderer(Camera(CameraDirection::Left)),
        S => KeyAction::Renderer(Camera(CameraDirection::Down)),
        D => KeyAction::Renderer(Camera(CameraDirection::Right)),

        Escape => KeyAction::Game(GameKey::CancelSelection),
        P => KeyAction::Game(GameKey::TogglePaused),
        _ => return None,
    })
}
