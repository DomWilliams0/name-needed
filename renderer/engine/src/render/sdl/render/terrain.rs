use std::collections::HashMap;

use color::Color;
use common::*;
use simulation::{BaseVertex, WorldViewer};
use unit::space::view::ViewPoint;
use unit::world::{ChunkLocation, WorldPoint};

use crate::render::sdl::gl::{
    AttribType, BufferUsage, Capability, GlResult, Normalized, Primitive, Program, ScopedBindable,
    Vao, Vbo,
};
use cgmath::Matrix;
use resources::Shaders;

#[derive(Debug, Copy, Clone)]
pub struct WorldVertex {
    pos: [f32; 3],
    color: u32,
}

impl BaseVertex for WorldVertex {
    fn new(pos: (f32, f32, f32), color: Color) -> Self {
        Self {
            pos: [pos.0, pos.1, pos.2],
            color: color.into(),
        }
    }
}

pub struct ChunkMesh {
    vao: Vao,
    vbo: Vbo,
    chunk_pos: ChunkLocation,
}

pub struct TerrainRenderer {
    program: Program,
    chunk_meshes: HashMap<ChunkLocation, ChunkMesh>,
}

impl TerrainRenderer {
    pub fn new(shaders_res: &Shaders) -> GlResult<Self> {
        let program = Program::load(shaders_res, "terrain", "rgb")?;

        Ok(Self {
            program,
            chunk_meshes: HashMap::with_capacity(64),
        })
    }

    pub fn update_chunk_mesh(
        &mut self,
        chunk_pos: ChunkLocation,
        new_mesh: Vec<WorldVertex>,
    ) -> GlResult<()> {
        let mesh = self
            .chunk_meshes
            .entry(chunk_pos)
            .or_insert_with(|| ChunkMesh {
                vao: Vao::new(),
                vbo: Vbo::array_buffer(),
                chunk_pos,
            });

        // bind vao and vbo
        let bound_vao = mesh.vao.scoped_bind();
        let bound_vbo = mesh.vbo.scoped_bind();

        // setup vao attribs
        if !new_mesh.is_empty() {
            bound_vao
                .vertex_attribs()
                .add(3, AttribType::Float32, Normalized::FixedPoint) // pos
                .add(4, AttribType::UByte, Normalized::Normalized) // col
                .build()?;
        }

        // allocate mesh data
        bound_vbo.buffer_data(&new_mesh, BufferUsage::DynamicDraw)?;
        debug!("regenerated mesh"; chunk_pos);

        Ok(())
    }

    pub fn render(&self, proj: &Matrix4, view: &Matrix4, world_viewer: &WorldViewer) {
        // use program and setup common uniforms
        let prog = self.program.scoped_bind();
        prog.set_uniform_matrix("proj\0", proj.as_ptr());

        // enable face culling
        let _cull = Capability::CullFace.scoped_enable();

        let mut count = 0;
        for chunk_pos in world_viewer.visible_chunks() {
            if let Some(mesh) = self.chunk_meshes.get(&chunk_pos) {
                let _vao = mesh.vao.scoped_bind();
                let vbo = mesh.vbo.scoped_bind();

                // offset chunk
                let view = {
                    let world_point = WorldPoint::from(mesh.chunk_pos.get_block(0)); // z irrelevant
                    let view_point = ViewPoint::from(world_point);
                    let transform = Matrix4::from_translation(view_point.into());

                    view * transform
                };

                prog.set_uniform_matrix("view\0", view.as_ptr());

                vbo.draw_array(Primitive::Triangles);
                count += 1;
            }
        }

        trace!("rendered {count} visible chunks", count = count);
    }

    pub fn reset(&mut self) {
        self.chunk_meshes.clear();
    }
}
