use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cgmath::perspective;
use glium::index::PrimitiveType;
use glium::uniform;
use glium::{implement_vertex, Surface};
use glium_sdl2::SDL2Facade;

use common::*;
use simulation::Simulation;
use unit;
use world::{ChunkPosition, Vertex as WorldVertex, ViewPoint, WorldPoint, WorldViewer, CHUNK_SIZE};

use crate::camera::FreeRangeCamera;
use crate::render::{draw_params, load_program, DrawParamType, FrameTarget, SimulationRenderer};

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

pub struct GliumRenderer {
    display: SDL2Facade,
    window_size: (i32, i32),

    // world rendering
    chunk_meshes: HashMap<ChunkPosition, ChunkMesh>,
    program: glium::Program,

    world_viewer: WorldViewer,
    camera: FreeRangeCamera,

    // simulation rendering
    simulation_renderer: SimulationRenderer,
}

impl GliumRenderer {
    pub fn new(display: SDL2Facade, world_viewer: WorldViewer) -> Self {
        // world program
        let program = load_program(&display, "world").expect("Failed to load world program");

        let window_size = {
            let (w, h) = display.window().size();
            (w as i32, h as i32)
        };
        info!("window size is {}x{}", window_size.0, window_size.1);

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

        let simulation_renderer = SimulationRenderer::new(&display);

        Self {
            display,
            window_size,
            chunk_meshes: HashMap::new(),
            program,
            world_viewer,
            camera,
            simulation_renderer,
        }
    }

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

    pub fn tick(&mut self) {
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
    pub fn render(&mut self, simulation: &mut Simulation<SimulationRenderer>, interpolation: f64) {
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
