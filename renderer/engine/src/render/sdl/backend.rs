use std::ops::{Deref, DerefMut};

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::video::{Window, WindowBuildError};
use sdl2::{EventPump, Sdl, VideoSubsystem};

use color::ColorRgb;
use common::input::{CameraDirection, GameKey, KeyAction, RendererKey};
use common::*;
use simulation::{
    Exit, InitializedSimulationBackend, PerfAvg, PersistentSimulationBackend, Simulation,
    WorldViewer,
};

use crate::render::sdl::camera::Camera;
use crate::render::sdl::gl::{Gl, GlError};
use crate::render::sdl::render::FrameTarget;
use crate::render::sdl::selection::Selection;
use crate::render::sdl::ui::{EventConsumed, Ui};
use crate::render::sdl::GlRenderer;
use resources::ResourceError;
use resources::Resources;
use sdl2::mouse::{MouseButton, MouseState};
use simulation::input::{InputEvent, SelectType, UiCommand, UiCommands, UiRequest, WorldColumn};
use std::hint::unreachable_unchecked;
use unit::world::{WorldPoint, WorldPosition};

pub struct SdlBackendPersistent {
    camera: Camera,
    is_first_init: bool,

    /// `take`n out and replaced each tick
    sdl_events: Option<EventPump>,
    #[allow(dead_code)]
    keep_alive: GraphicsKeepAlive,
    window: Window,

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

        let gl = Gl::new(&window, &video).map_err(SdlBackendError::Sdl)?;
        Gl::set_clear_color(ColorRgb::new(17, 17, 20));

        let ui = Ui::new(&window, &video);

        // enable vsync
        video
            .gl_set_swap_interval(1)
            .map_err(SdlBackendError::Sdl)?;

        let events = sdl.event_pump().map_err(SdlBackendError::Sdl)?;
        let renderer = {
            let shaders = resources.shaders().map_err(SdlBackendError::Resources)?;
            GlRenderer::new(&shaders)?
        };
        let camera = Camera::new(w as i32, h as i32);

        Ok(Self {
            camera,
            is_first_init: true,
            sdl_events: Some(events),
            keep_alive: GraphicsKeepAlive { sdl, video, gl },
            window,
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

    fn name() -> &'static str {
        "SDL2"
    }
}

impl InitializedSimulationBackend for SdlBackendInit {
    type Renderer = GlRenderer;
    type Persistent = SdlBackendPersistent;

    fn consume_events(&mut self, commands: &mut UiCommands) {
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
                } => match map_sdl_keycode(key) {
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
                    if let Some(action) = map_sdl_keycode(key) {
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
                        .and_then(|(select, col)| self.selection.mouse_up(select, col));

                    if let Some(evt) = evt {
                        self.sim_input_events.push(evt);
                    }
                }

                Event::MouseMotion {
                    mousestate, x, y, ..
                } => {
                    if let Some(mouse_btn) = mousestate.pressed_mouse_buttons().next() {
                        if let Some((select, col)) = self.parse_mouse_event(mouse_btn, x, y) {
                            self.selection.mouse_move(select, col);
                        }
                    }
                }

                _ => {}
            };
        }

        // put back event pump like we never took it
        let none = std::mem::replace(&mut self.sdl_events, Some(events));
        debug_assert!(none.is_none());
        std::mem::forget(none);
    }

    fn tick(&mut self) {
        let chunk_bounds = self.camera.tick();
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
        let projection = self.camera.projection_matrix();

        // position camera a fixed distance above the top of the terrain
        const CAMERA_Z_OFFSET: f32 = 20.0;
        let terrain_range = self.world_viewer.terrain_range();
        let view = self
            .camera
            .view_matrix(interpolation, terrain_range.size() as f32 + CAMERA_Z_OFFSET);

        // render world
        self.renderer
            .terrain()
            .render(&projection, &view, &self.world_viewer);

        // render simulation
        let lower_limit = terrain_range.bottom().slice() as f32;
        let frame_target = FrameTarget {
            proj: projection.as_ptr(),
            view: view.as_ptr(),
            z_offset: lower_limit,
        };

        let (_, mut blackboard) = simulation.render(
            &self.world_viewer,
            frame_target,
            &mut self.backend.renderer,
            interpolation,
            &self.backend.sim_input_events,
        );

        // input events were for this frame only
        self.sim_input_events.clear();

        // populate blackboard with backend info
        blackboard.world_view = Some(terrain_range);

        // render ui and collect input commands
        let backend = &mut self.backend;
        let mouse_state = backend.mouse_state();
        backend
            .ui
            .render(&backend.window, &mouse_state, perf, blackboard, commands);

        self.window.gl_swap_window();
    }

    fn world_viewer(&mut self) -> &mut WorldViewer {
        &mut self.world_viewer
    }

    fn end(mut self) -> Self::Persistent {
        self.sim_input_events.clear();
        self.renderer.reset();
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
                    }

                    (_, Camera(direction)) => {
                        self.camera.handle_move(direction, is_down);
                    }
                    _ => {
                        if is_down {
                            warn!("unhandled key down: {:?}", key);
                        }
                    }
                };

                // no ui command to return
                None
            }
            KeyAction::Game(key) => {
                if is_down {
                    let cmd = match key {
                        GameKey::Exit => UiRequest::ExitGame(Exit::Stop),
                        GameKey::Restart => UiRequest::ExitGame(Exit::Restart),
                    };

                    Some(cmd)
                } else {
                    // ignore key ups
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
        Selection::select_type(button).map(|select| {
            let (wx, wy) = self.camera.screen_to_world((x, y));
            let col = WorldColumn {
                x: wx,
                y: wy,
                slice_range: self.world_viewer.entity_range(),
            };
            (select, col)
        })
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
fn map_sdl_keycode(keycode: Keycode) -> Option<KeyAction> {
    use Keycode::*;
    use RendererKey::*;

    Some(match keycode {
        Escape => KeyAction::Game(GameKey::Exit),
        R => KeyAction::Game(GameKey::Restart),

        Up => KeyAction::Renderer(SliceUp),
        Down => KeyAction::Renderer(SliceDown),

        W => KeyAction::Renderer(Camera(CameraDirection::Up)),
        A => KeyAction::Renderer(Camera(CameraDirection::Left)),
        S => KeyAction::Renderer(Camera(CameraDirection::Down)),
        D => KeyAction::Renderer(Camera(CameraDirection::Right)),

        _ => return None,
    })
}
