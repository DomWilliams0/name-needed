use color::ColorRgb;
use common::*;
use simulation::{RenderComponent, Renderer, TransformComponent};
use unit::view::ViewPoint;
use unit::world::{WorldPoint, SCALE};

use crate::render::debug::DebugShape;
use crate::render::sdl::gl::{
    generate_circle_mesh, AttribType, BufferUsage, Capability, Divisor, GlError, GlResult,
    InstancedPipeline, Normalized, Pipeline, Primitive, Program, ScopedBindable,
};
use crate::render::sdl::render::entity::EntityPipeline;
use crate::render::sdl::render::terrain::TerrainRenderer;

mod entity;
pub mod terrain;

pub struct GlRenderer {
    frame_target: Option<FrameTarget>,
    terrain: TerrainRenderer,

    debug_shapes: Vec<DebugShape>,
    debug_pipeline: Pipeline,

    entity_pipeline: EntityPipeline,
}

impl GlRenderer {
    pub fn new() -> GlResult<Self> {
        let terrain = TerrainRenderer::new()?;

        // smooth lines look nice globally
        Capability::LineSmooth.enable();

        // init debug lines pipeline
        let debug_pipeline = {
            let pipeline = Pipeline::new(Program::load("debug", "rgb")?);

            let vao = pipeline.vao.scoped_bind();
            let _vbo = pipeline.vbo.scoped_bind();

            vao.vertex_attribs()
                .add(3, AttribType::Float32, Normalized::FixedPoint) // pos
                .add(4, AttribType::UByte, Normalized::Normalized) // col
                .build()?;

            drop(vao);
            drop(_vbo);
            pipeline
        };

        Ok(Self {
            terrain,
            frame_target: None,
            debug_shapes: Vec::new(),
            debug_pipeline,
            entity_pipeline: EntityPipeline::new()?,
        })
    }
    pub fn terrain(&self) -> &TerrainRenderer {
        &self.terrain
    }

    pub fn terrain_mut(&mut self) -> &mut TerrainRenderer {
        &mut self.terrain
    }
}

pub struct FrameTarget {
    pub proj: *const F,
    pub view: *const F,
}

#[repr(C)]
pub struct DebugVertex {
    pos: [f32; 3],
    col: u32,
}

impl Renderer for GlRenderer {
    type Target = FrameTarget;
    type Error = GlError;

    fn init(&mut self, target: Self::Target) {
        self.frame_target = Some(target);
    }

    fn sim_start(&mut self) {}

    fn sim_entity(&mut self, transform: &TransformComponent, render: &RenderComponent) {
        self.entity_pipeline
            .add_entity((transform.position, render.clone()));
    }

    fn sim_finish(&mut self) -> GlResult<()> {
        let frame = self.frame_target.as_mut().unwrap();
        self.entity_pipeline.render_entities(frame)
    }

    fn debug_start(&mut self) {
        self.debug_shapes.clear();
    }

    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: ColorRgb) {
        self.debug_shapes.push(DebugShape::Line {
            points: [from, to],
            color,
        });
    }

    fn debug_add_tri(&mut self, _points: [ViewPoint; 3], _color: ColorRgb) {
        unimplemented!()
    }

    fn debug_finish(&mut self) -> GlResult<()> {
        let frame_target = self.frame_target.as_ref().unwrap();
        let line_count = self.debug_shapes.len(); // assumes lines only

        let (program, _vao, vbo) = self.debug_pipeline.bind_all();

        // TODO use glBufferSubData to reuse the allocation if <= len
        vbo.buffer_data_uninitialized::<DebugVertex>(line_count * 2, BufferUsage::StreamDraw)?;

        if let Some(mut mapped) = vbo.map_write_only::<DebugVertex>()? {
            let mut i = 0;
            for shape in self.debug_shapes.drain(..) {
                for point in shape.points() {
                    let vertex = DebugVertex {
                        pos: (*point).into(),
                        col: shape.color().into(),
                    };

                    mapped[i] = vertex;
                    i += 1;
                }
            }
        }

        program.set_uniform_matrix("proj\0", frame_target.proj);
        program.set_uniform_matrix("view\0", frame_target.view);

        let _no_depth = Capability::DepthTest.scoped_disable();
        vbo.draw_array(Primitive::Lines);
        Ok(())
    }

    fn deinit(&mut self) -> Self::Target {
        self.frame_target.take().unwrap()
    }
}
