use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::video::{Window, WindowBuildError};
use sdl2::{EventPump, Sdl, VideoSubsystem};

use color::ColorRgb;
use common::input::{CameraDirection, Key, KeyEvent};
use common::*;
use simulation::{EventsOutcome, ExitType, PerfAvg, Simulation, SimulationBackend, WorldViewer};

use crate::render::sdl::camera::Camera;
use crate::render::sdl::gl::{Gl, GlError};
use crate::render::sdl::render::FrameTarget;
use crate::render::sdl::ui::{EventConsumed, Ui};
use crate::render::sdl::GlRenderer;
use sdl2::mouse::MouseButton;
use simulation::input::{InputCommand, InputEvent, WorldColumn};

pub struct SdlBackend {
    world_viewer: WorldViewer,
    camera: Camera,

    sdl_events: EventPump,
    #[allow(dead_code)]
    keep_alive: GraphicsKeepAlive,
    window: Window,

    renderer: GlRenderer,
    ui: Ui,
    /// Events from game -> UI, queued up and passed to sim on each frame
    sim_input_events: Vec<InputEvent>,
}

/// Unused fields but need to be kept alive
#[allow(dead_code)]
struct GraphicsKeepAlive {
    sdl: Sdl,
    video: VideoSubsystem,
    gl: Gl,
}

#[derive(Debug)]
pub enum SdlBackendError {
    Sdl(String),
    WindowCreation(WindowBuildError),
    Gl(GlError),
}

impl SimulationBackend for SdlBackend {
    type Renderer = GlRenderer;
    type Error = SdlBackendError;

    fn new(world_viewer: WorldViewer) -> Result<Self, SdlBackendError> {
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        video.gl_attr().set_context_version(3, 0);
        video.gl_attr().set_depth_size(24);
        info!(
            "opengl {}.{}",
            video.gl_attr().context_major_version(),
            video.gl_attr().context_minor_version()
        );

        let (w, h) = config::get().display.resolution;
        info!("window size is {}x{}", w, h);

        let window = {
            let mut builder = video.window("Name Needed", w, h);

            builder.position_centered().allow_highdpi().opengl();

            if config::get().display.resizable {
                builder.resizable();
            }
            builder.build()?
        };

        let gl = Gl::new(&window, &video)?;
        Gl::set_clear_color(ColorRgb::new(17, 17, 20));

        let ui = Ui::new(&window, &video);

        // enable vsync
        video.gl_set_swap_interval(1)?;

        let events = sdl.event_pump()?;
        let renderer = GlRenderer::new()?;
        let camera = Camera::new(w as i32, h as i32);

        Ok(Self {
            world_viewer,
            camera,
            sdl_events: events,
            keep_alive: GraphicsKeepAlive { sdl, video, gl },
            window,
            renderer,
            ui,
            sim_input_events: Vec::with_capacity(32),
        })
    }

    fn consume_events(&mut self) -> EventsOutcome {
        let mut outcome = EventsOutcome::Continue;

        let mut events = StealysEventPump::steal(&mut self.sdl_events);
        for event in events.poll_iter() {
            if let EventConsumed::Consumed = self.ui.handle_event(&event) {
                continue;
            }

            match event {
                Event::Quit { .. } => {
                    outcome = EventsOutcome::Exit(ExitType::Stop);
                    break;
                }
                Event::Window {
                    win_event: WindowEvent::Resized(width, height),
                    ..
                } => {
                    debug!("resized to {}x{}", width, height);
                    Gl::set_viewport(width, height);
                    self.camera.on_resize(width, height);
                }

                Event::KeyDown {
                    keycode: Some(key), ..
                } => match map_sdl_keycode(key) {
                    Some(Key::Exit) => {
                        outcome = EventsOutcome::Exit(ExitType::Stop);
                        break;
                    }
                    Some(Key::Restart) => {
                        outcome = EventsOutcome::Exit(ExitType::Restart);
                        break;
                    }
                    Some(key) => self.handle_key(KeyEvent::Down(key)),
                    None => debug!("ignoring unknown key {:?}", key),
                },
                Event::KeyUp {
                    keycode: Some(key), ..
                } => {
                    if let Some(key) = map_sdl_keycode(key) {
                        self.handle_key(KeyEvent::Up(key))
                    }
                }

                Event::MouseButtonDown {
                    mouse_btn, x, y, ..
                } => {
                    let (wx, wy) = match mouse_btn {
                        MouseButton::Left | MouseButton::Right => {
                            self.camera.screen_to_world((x, y))
                        }
                        _ => continue,
                    };

                    let selected = WorldColumn {
                        x: wx,
                        y: wy,
                        slice_range: self.world_viewer.range(),
                    };

                    let event = match mouse_btn {
                        MouseButton::Left => InputEvent::LeftClick(selected),
                        MouseButton::Right => InputEvent::RightClick(selected),
                        _ => unreachable!(),
                    };
                    self.sim_input_events.push(event);
                }

                _ => {}
            };
        }

        // put back event pump like we never took it
        events.steal_back(&mut self.sdl_events);

        outcome
    }

    fn tick(&mut self) {
        let chunk_bounds = self.camera.tick();
        self.world_viewer.set_chunk_bounds(chunk_bounds);

        let renderer = self.renderer.terrain_mut();
        self.world_viewer
            .regenerate_dirty_chunk_meshes(|chunk_pos, mesh| {
                if let Err(e) = renderer.update_chunk_mesh(chunk_pos, mesh) {
                    error!(
                        "failed to regenerate mesh for chunk {:?}: {:?}",
                        chunk_pos, e
                    );
                }
            });
    }

    fn render(
        &mut self,
        simulation: &mut Simulation<Self::Renderer>,
        interpolation: f64,
        perf: &PerfAvg,
        commands: &mut Vec<InputCommand>,
    ) {
        // clear window
        Gl::clear();

        // calculate projection and view matrices
        let projection = self.camera.projection_matrix();
        let view = self.camera.view_matrix(interpolation);

        // render world
        self.renderer
            .terrain()
            .render(&projection, &view, &self.world_viewer);

        // render simulation
        let frame_target = FrameTarget {
            proj: projection.as_ptr(),
            view: view.as_ptr(),
        };

        let (_, blackboard) = simulation.render(
            self.world_viewer.range(),
            frame_target,
            &mut self.renderer,
            interpolation,
            &self.sim_input_events,
        );

        // input events were for this frame only
        self.sim_input_events.clear();

        // render ui and collect input commands
        self.ui.render(
            &self.window,
            &self.sdl_events.mouse_state(),
            perf,
            blackboard,
            commands,
        );

        self.window.gl_swap_window();
    }
}

impl SdlBackend {
    fn handle_key(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Down(Key::SliceDown) => self.world_viewer.move_by(-1),
            KeyEvent::Down(Key::SliceUp) => self.world_viewer.move_by(1),
            other => {
                let _handled = self.camera.handle_key(other);
                // TODO cascade through other handlers
            }
        }
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
fn map_sdl_keycode(keycode: Keycode) -> Option<Key> {
    match keycode {
        Keycode::Escape => Some(Key::Exit),
        Keycode::R => Some(Key::Restart),
        Keycode::Up => Some(Key::SliceUp),
        Keycode::Down => Some(Key::SliceDown),
        Keycode::Y => Some(Key::ToggleWireframe),
        Keycode::W => Some(Key::Camera(CameraDirection::Up)),
        Keycode::A => Some(Key::Camera(CameraDirection::Left)),
        Keycode::S => Some(Key::Camera(CameraDirection::Down)),
        Keycode::D => Some(Key::Camera(CameraDirection::Right)),
        _ => None,
    }
}

struct StealysEventPump {
    pump: EventPump,
}

impl StealysEventPump {
    fn steal(pump: &mut EventPump) -> Self {
        let dummy = MaybeUninit::uninit();
        let pump = std::mem::replace(pump, unsafe { dummy.assume_init() });

        // the given reference is now uninitialized!

        Self { pump }
    }

    fn steal_back(self, original: &mut EventPump) {
        // steal back real pump, forgetting about the uninitialized dummy
        let dummy = std::mem::replace(original, self.pump);
        std::mem::forget(dummy);
    }
}

impl Deref for StealysEventPump {
    type Target = EventPump;

    fn deref(&self) -> &Self::Target {
        &self.pump
    }
}

impl DerefMut for StealysEventPump {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.pump
    }
}

impl From<String> for SdlBackendError {
    fn from(s: String) -> Self {
        SdlBackendError::Sdl(s)
    }
}

impl From<WindowBuildError> for SdlBackendError {
    fn from(e: WindowBuildError) -> Self {
        SdlBackendError::WindowCreation(e)
    }
}

impl From<GlError> for SdlBackendError {
    fn from(e: GlError) -> Self {
        SdlBackendError::Gl(e)
    }
}
