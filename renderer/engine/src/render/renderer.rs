use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::rc::Rc;

use cgmath::perspective;
use glium::index::PrimitiveType;
use glium::uniform;
use glium::{implement_vertex, Surface};
use glium_sdl2::{DisplayBuild, SDL2Facade};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::EventPump;

use common::input::{Key, KeyEvent};
use common::*;
use simulation::{
    EventsOutcome, ExitType, Simulation, SimulationBackend, WorldVertex, WorldViewer,
};

use crate::render;
use crate::render::camera::FreeRangeCamera;
use crate::render::{draw_params, load_program, DrawParamType, FrameTarget, GliumRenderer};
use unit::dim::CHUNK_SIZE;
use unit::view::ViewPoint;
use unit::world::{ChunkPosition, WorldPoint};

/// Copy of world::mesh::Vertex
#[derive(Copy, Clone)]
pub struct Vertex {
    v_pos: [f32; 3],
    v_color: [f32; 3],
}

implement_vertex!(Vertex, v_pos, v_color);

impl From<WorldVertex> for Vertex {
    fn from(v: WorldVertex) -> Self {
        Self {
            v_pos: v.v_pos,
            v_color: v.v_color,
        }
    }
}

struct ChunkMesh {
    vertex_buffer: glium::VertexBuffer<Vertex>,
    chunk_pos: ChunkPosition,
}

pub struct SdlGliumBackend {
    event_pump: EventPump,
    display: SDL2Facade,
    window_size: (i32, i32),

    // world rendering
    chunk_meshes: HashMap<ChunkPosition, ChunkMesh>,
    program: glium::Program,

    world_viewer: WorldViewer,
    camera: FreeRangeCamera,

    // simulation rendering
    simulation_renderer: GliumRenderer,
}

impl SimulationBackend for SdlGliumBackend {
    type Renderer = GliumRenderer;

    /// Panics if SDL or glium initialisation fails
    fn new(world_viewer: WorldViewer) -> Self {
        // init SDL
        let sdl = sdl2::init().expect("Failed to init SDL");
        let video = sdl.video().expect("Failed to init SDL video");
        video.gl_attr().set_context_version(3, 3);
        video.gl_attr().set_context_minor_version(3);
        debug!(
            "opengl {}.{}",
            video.gl_attr().context_major_version(),
            video.gl_attr().context_minor_version(),
        );
        let event_pump = sdl.event_pump().expect("Failed to create event pump");

        // create window
        let (w, h) = config::get().display.resolution;
        info!("window size is {}x{}", w, h);
        let display = video
            .window("Name Needed", w, h)
            .position_centered()
            .build_glium()
            .expect("Failed to create glium window");

        // configure opengl
        video.gl_attr().set_depth_size(24);

        // load world program
        let program = load_program(&display, "world").expect("Failed to load world program");

        // create camera
        let camera = {
            // mid chunk
            let pos = Point3::new(
                unit::BLOCK_DIAMETER * CHUNK_SIZE.as_f32() * 0.5,
                unit::BLOCK_DIAMETER * CHUNK_SIZE.as_f32() * 0.5,
                15.0,
            );

            info!("placing camera at {:?}", pos);

            FreeRangeCamera::new(pos)
        };

        let simulation_renderer = GliumRenderer::new(&display);

        Self {
            event_pump,
            display,
            window_size: (w as i32, h as i32),
            chunk_meshes: HashMap::new(),
            program,
            world_viewer,
            camera,
            simulation_renderer,
        }
    }

    fn consume_events(&mut self) -> EventsOutcome {
        // we need mutable access to self while consuming events, so temporarily move event pump
        // out of `self`
        #[allow(clippy::uninit_assumed_init, invalid_value)]
        let dummy = unsafe { MaybeUninit::uninit().assume_init() };
        let mut event_pump = std::mem::replace(&mut self.event_pump, dummy);

        let mut outcome = EventsOutcome::Continue;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    outcome = EventsOutcome::Exit(ExitType::Stop);
                    break;
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
                    Some(key) => self.handle_key_down(key),

                    None => debug!("ignoring unknown key {:?}", key),
                },
                Event::KeyUp {
                    keycode: Some(key), ..
                } => self.handle_key_up(key),
                Event::Window {
                    win_event: WindowEvent::Resized(w, h),
                    ..
                } => self.on_resize(w, h),

                Event::MouseButtonDown { .. } => self.camera.handle_click(true),
                Event::MouseButtonUp { .. } => self.camera.handle_click(false),
                Event::MouseMotion { xrel, yrel, .. } => self.camera.handle_cursor(xrel, yrel),
                _ => {}
            }
        }

        // move real event pump back into `self`, forgetting about the uninit'd dummy
        let dummy = std::mem::replace(&mut self.event_pump, event_pump);
        std::mem::forget(dummy);

        outcome
    }

    fn tick(&mut self) {
        // regenerate meshes for dirty chunks
        for (chunk_pos, new_mesh) in self.world_viewer.regen_dirty_chunk_meshes() {
            let converted_vertices: Vec<Vertex> = new_mesh.into_iter().map(|v| v.into()).collect();
            let vertex_buffer =
                glium::VertexBuffer::dynamic(&self.display, &converted_vertices).unwrap();

            let mesh = ChunkMesh {
                vertex_buffer,
                chunk_pos,
            };
            self.chunk_meshes.insert(chunk_pos, mesh);
            debug!("regenerated mesh for chunk {:?}", chunk_pos);
        }
    }

    /// Calculates camera projection, renders world then entities
    fn render(&mut self, simulation: &mut Simulation<GliumRenderer>, interpolation: f64) {
        let target = Rc::new(RefCell::new(FrameTarget {
            frame: self.display.draw(),
            projection: Default::default(),
            view: Default::default(),
        }));

        {
            let mut world_target = target.borrow_mut();

            // clear
            world_target
                .frame
                .clear_color_and_depth((0.06, 0.06, 0.075, 1.0), 1.0);

            // calculate projection and view matrices
            let (projection, view) = {
                let (w, h) = (self.window_size.0 as f32, self.window_size.1 as f32);
                let aspect = w / h;

                let fov = Deg(config::get().display.fov);
                let projection: [[f32; 4]; 4] = perspective(fov, aspect, 0.1, 100.0).into();

                let view = self.camera.world_to_view();

                world_target.projection = projection;
                world_target.view = view.into();
                (projection, view)
            };

            // draw world chunks
            for mesh in self.chunk_meshes.values() {
                let view: [[f32; 4]; 4] = {
                    // chunk offset
                    let world_point = WorldPoint::from(mesh.chunk_pos);
                    let ViewPoint(x, y, z) = ViewPoint::from(world_point);
                    let translate = Vector3::new(x, y, z);

                    (view * Matrix4::from_translation(translate)).into()
                };

                let uniforms = uniform! { proj: projection, view: view, };

                world_target
                    .frame
                    .draw(
                        &mesh.vertex_buffer,
                        &glium::index::NoIndices(PrimitiveType::TrianglesList),
                        &self.program,
                        &uniforms,
                        &draw_params(DrawParamType::World),
                    )
                    .unwrap();
            }
        }

        // draw simulation
        simulation.render(
            self.world_viewer.range(),
            target.clone(),
            &mut self.simulation_renderer,
            interpolation,
        );

        // done
        target
            .borrow_mut()
            .frame
            .set_finish()
            .expect("failed to swap buffers");

        assert_eq!(Rc::strong_count(&target), 1); // target should be dropped here
    }
}

impl SdlGliumBackend {
    pub fn on_resize(&mut self, w: i32, h: i32) {
        self.window_size = (w, h);
        debug!("window resized to {}x{}", w, h);
    }

    pub fn world_viewer(&mut self) -> &mut WorldViewer {
        &mut self.world_viewer
    }

    pub fn camera(&mut self) -> &mut FreeRangeCamera {
        &mut self.camera
    }

    #[inline(always)]
    fn handle_key_down(&mut self, key: Key) {
        self.handle_key(KeyEvent::Down(key))
    }

    #[inline(always)]
    fn handle_key_up(&mut self, key: Keycode) {
        if let Some(key) = map_sdl_keycode(key) {
            self.handle_key(KeyEvent::Up(key))
        }
    }

    fn handle_key(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Down(Key::SliceUp) => self.world_viewer.move_by(1),
            KeyEvent::Down(Key::SliceDown) => self.world_viewer.move_by(-1),
            KeyEvent::Down(Key::ToggleWireframe) => {
                let wireframe = unsafe { render::wireframe_world_toggle() };
                debug!(
                    "world is {} wireframe",
                    if wireframe { "now" } else { "no longer" }
                )
            }
            k if k.is_camera_key() => self.camera.handle_key(k),
            _ => {}
        };
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
        Keycode::W => Some(Key::CameraForward),
        Keycode::A => Some(Key::CameraLeft),
        Keycode::S => Some(Key::CameraBack),
        Keycode::D => Some(Key::CameraRight),
        _ => None,
    }
}
