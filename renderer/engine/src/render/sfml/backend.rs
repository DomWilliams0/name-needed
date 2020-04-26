use std::collections::HashMap;

use sfml::graphics::{
    Color, PrimitiveType, RenderStates, RenderTarget, RenderWindow, Vertex, VertexBuffer,
    VertexBufferUsage,
};
use sfml::window::{ContextSettings, Event, Style};

use color::ColorRgb;
use common::input::{CameraDirection, Key, KeyEvent};
use common::*;
use simulation::{BaseVertex, EventsOutcome, ExitType, Simulation, SimulationBackend, WorldViewer};
use unit::view::ViewPoint;
use unit::world::{ChunkPosition, WorldPoint};

use crate::render::sfml::camera::Camera;
use crate::render::sfml::renderer::FrameTarget;
use crate::render::sfml::SfmlRenderer;

pub struct SfmlBackend {
    window: RenderWindow,
    world_viewer: WorldViewer,
    camera: Camera,

    chunk_meshes: HashMap<ChunkPosition, ChunkMesh>,
    renderer: SfmlRenderer,
}

struct ChunkMesh {
    vertex_buffer: VertexBuffer,
    chunk_pos: ChunkPosition,
}

impl SimulationBackend for SfmlBackend {
    type Renderer = SfmlRenderer;

    fn new(world_viewer: WorldViewer) -> Self {
        let ctx_settings = ContextSettings {
            major_version: 3,
            ..ContextSettings::default()
        };

        let (w, h) = config::get().display.resolution;
        info!("window size is {}x{}", w, h);
        let mut window = RenderWindow::new((w, h), "Name Needed", Style::NONE, &ctx_settings);

        info!(
            "using opengl {}.{}",
            window.settings().major_version,
            window.settings().minor_version
        );

        window.set_vertical_sync_enabled(true);

        let renderer = SfmlRenderer::default();

        Self {
            window,
            world_viewer,
            camera: Camera::new(w, h),
            chunk_meshes: HashMap::with_capacity(512),
            renderer,
        }
    }

    fn consume_events(&mut self) -> EventsOutcome {
        let mut outcome = EventsOutcome::Continue;

        while let Some(event) = self.window.poll_event() {
            match event {
                Event::Closed => {
                    outcome = EventsOutcome::Exit(ExitType::Stop);
                    break;
                }
                Event::Resized { width, height } => {
                    debug!("resized to {}x{}", width, height);
                    self.camera.on_resize(width, height);
                }

                Event::KeyPressed { code, .. } => match map_sfml_keycode(code) {
                    Some(Key::Exit) => {
                        outcome = EventsOutcome::Exit(ExitType::Stop);
                        break;
                    }
                    Some(Key::Restart) => {
                        outcome = EventsOutcome::Exit(ExitType::Restart);
                        break;
                    }
                    Some(key) => self.handle_key(KeyEvent::Down(key)),
                    None => debug!("ignoring unknown key {:?}", code),
                },
                Event::KeyReleased { code, .. } => {
                    if let Some(key) = map_sfml_keycode(code) {
                        self.handle_key(KeyEvent::Up(key))
                    }
                }

                _ => {}
            };
        }
        outcome
    }

    fn tick(&mut self) {
        self.camera.tick();

        let viewer = self.world_viewer.clone();

        let use_mesh = |chunk_pos, mesh: Vec<SfmlVertex>| {
            let vertices: Vec<Vertex> = unsafe {
                // safety: SfmlVertex is repr(transparent) to Vertex
                std::mem::transmute(mesh)
            };

            // TODO dynamic and reuse buffer when chunk changes?
            let mut vbo = VertexBuffer::new(
                PrimitiveType::Triangles,
                vertices.len() as u32,
                VertexBufferUsage::Static,
            );
            vbo.update(&vertices, 0);

            let mesh = ChunkMesh {
                vertex_buffer: vbo,
                chunk_pos,
            };

            self.chunk_meshes.insert(chunk_pos, mesh);
            debug!("regenerated mesh for chunk {:?}", chunk_pos);
        };

        viewer.regenerate_dirty_chunk_meshes(use_mesh);
    }

    fn render(&mut self, simulation: &mut Simulation<Self::Renderer>, interpolation: f64) {
        self.window.clear(Color::rgb(17, 17, 20));

        self.window.set_view(&self.camera.view(interpolation));

        // render world
        for mesh in self.chunk_meshes.values() {
            let r = {
                let world_point = WorldPoint::from(mesh.chunk_pos);
                let ViewPoint(x, y, _) = world_point.into();

                let mut states = RenderStates::default();
                states.transform.translate(x, y);
                states
            };

            self.window.draw_with_renderstates(&mesh.vertex_buffer, r);
        }

        // render simulation on top
        let renderer = &mut self.renderer;
        let range = self.world_viewer.range();
        take_mut::take(&mut self.window, |window| {
            let mut frame_target = FrameTarget { target: window };

            frame_target = simulation.render(range, frame_target, renderer, interpolation);

            frame_target.target
        });

        self.window.display();
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
struct SfmlVertex(Vertex);

impl BaseVertex for SfmlVertex {
    fn new(pos: (f32, f32), color: ColorRgb) -> Self {
        Self(Vertex::with_pos_color(
            pos.into(),
            Color::from(u32::from(color)),
        ))
    }
}

impl SfmlBackend {
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

/// can't use TryInto/TryFrom for now, see map_sdl_keycode
fn map_sfml_keycode(key: sfml::window::Key) -> Option<Key> {
    match key {
        sfml::window::Key::Escape => Some(Key::Exit),
        sfml::window::Key::R => Some(Key::Restart),
        sfml::window::Key::Up => Some(Key::SliceUp),
        sfml::window::Key::Down => Some(Key::SliceDown),
        sfml::window::Key::Y => Some(Key::ToggleWireframe),
        sfml::window::Key::W => Some(Key::Camera(CameraDirection::Up)),
        sfml::window::Key::A => Some(Key::Camera(CameraDirection::Left)),
        sfml::window::Key::S => Some(Key::Camera(CameraDirection::Down)),
        sfml::window::Key::D => Some(Key::Camera(CameraDirection::Right)),
        _ => None,
    }
}
