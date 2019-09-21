use std::collections::HashMap;

use cgmath::{ortho, Point3};
use glium::index::PrimitiveType;
use glium::uniform;
use glium::{implement_vertex, Surface};
use glium_sdl2::SDL2Facade;
use num_traits::ToPrimitive;

use tweaker::Tweak;
use world::{ChunkPosition, Vertex as WorldVertex, WorldViewer, BLOCK_RENDER_SIZE, CHUNK_SIZE};

use crate::camera::FreeRangeCamera;

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

type ChunkMesh = glium::VertexBuffer<Vertex>;

pub struct GliumRenderer<'a> {
    display: SDL2Facade,
    window_size: (i32, i32),

    chunk_meshes: HashMap<ChunkPosition, ChunkMesh>,
    program: glium::Program,

    world_viewer: WorldViewer<'a>,
    camera: FreeRangeCamera,
}

impl<'a> GliumRenderer<'a> {
    pub fn new(display: SDL2Facade, world_viewer: WorldViewer<'a>) -> Self {
        let program = glium::Program::from_source(
            &display,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/world.glslv")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/world.glslf")),
            None,
        ).unwrap();

        let window_size = {
            let (w, h) = display.window().size();
            (w as i32, h as i32)
        };

        let camera = {
            let block_count = CHUNK_SIZE.to_f32().unwrap();
            let pos = Point3::new(
                BLOCK_RENDER_SIZE * block_count, // mid chunk
                BLOCK_RENDER_SIZE * block_count, // mid chunk
                4.5,
            );

            FreeRangeCamera::new(pos)
        };

        Self {
            display,
            window_size,
            chunk_meshes: HashMap::new(),
            program,
            world_viewer,
            camera,
        }
    }

    pub fn on_resize(&mut self, w: i32, h: i32) {
        self.window_size = (w, h);
    }

    pub fn world_viewer(&mut self) -> &mut WorldViewer<'a> {
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
            self.chunk_meshes.insert(chunk_pos, vertex_buffer);
            println!("[mesh] regenerated for {:?}", chunk_pos);
        }
    }

    pub fn render(&mut self, _interpolation: f64) {
        let mut target = self.display.draw();

        // clear
        target.clear_color(0.06, 0.06, 0.075, 1.0);

        // uniforms
        let uniforms = {
            let zoom: f32 = Tweak::<f64>::lookup("zoom") as f32; // TODO move to camera
            let (w, h) = (self.window_size.0 as f32, self.window_size.1 as f32);

            // scale to window size to prevent stretching
            let scale_y = h / w;
            let base_size = zoom + (CHUNK_SIZE as f32) * BLOCK_RENDER_SIZE;

            let projection: [[f32; 4]; 4] = ortho(
                -base_size,
                base_size,
                -(base_size * scale_y),
                base_size * scale_y,
                0.1,
                100.0,
            ).into();
            let view: [[f32; 4]; 4] = self.camera.world_to_view().into();

            uniform! {
                proj: projection,
                view: view,
            }
        };

        // draw chunks
        // TODO chunk offset in view
        for mesh in self.chunk_meshes.values() {
            target
                .draw(
                    mesh,
                    &glium::index::NoIndices(PrimitiveType::TrianglesList),
                    &self.program,
                    &uniforms,
                    &Default::default(),
                )
                .unwrap();
        }

        // done
        target.finish().unwrap();
    }
}
