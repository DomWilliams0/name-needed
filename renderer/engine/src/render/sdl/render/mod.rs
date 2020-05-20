use color::ColorRgb;
use common::*;
use simulation::{PhysicalComponent, Renderer, TransformComponent};
use unit::view::ViewPoint;
use unit::world::{WorldPoint, SCALE};

use crate::render::debug::DebugShape;
use crate::render::sdl::gl::{
    generate_circle_mesh, AttribType, BufferUsage, Capability, Divisor, GlError, GlResult,
    InstancedPipeline, Normalized, Pipeline, Primitive, Program, ScopedBindable,
};
use crate::render::sdl::render::terrain::TerrainRenderer;

pub mod terrain;

pub struct GlRenderer {
    frame_target: Option<FrameTarget>,
    terrain: TerrainRenderer,

    debug_shapes: Vec<DebugShape>,
    debug_pipeline: Pipeline,

    entities: Vec<(WorldPoint, PhysicalComponent)>,
    entity_pipeline: InstancedPipeline,
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

        // init entity pipeline
        let entity_pipeline = {
            let pipeline = InstancedPipeline::new(Program::load("entity", "rgb")?);
            let vao = pipeline.vao.scoped_bind();

            {
                // static circle mesh reused across all entities
                let circle_mesh = generate_circle_mesh(40);
                let vbo = pipeline.shared_vbo.scoped_bind();
                vbo.buffer_data(&circle_mesh, BufferUsage::StaticDraw)?;

                let shared_stride = AttribType::Float32.byte_size(3);
                vao.vertex_attribs_manual().attrib(
                    0,
                    3,
                    AttribType::Float32,
                    Normalized::FixedPoint,
                    Divisor::PerVertex,
                    shared_stride,
                    0,
                )?;
            }

            // instance attributes
            {
                let _vbo = pipeline.instanced_vbo.scoped_bind();
                let instance_stride =
                    AttribType::UByte.byte_size(4) + AttribType::Float32.byte_size(16);
                vao.vertex_attribs_manual()
                    // instance color
                    .attrib(
                        1,
                        4,
                        AttribType::UByte,
                        Normalized::Normalized,
                        Divisor::PerInstances(1),
                        instance_stride,
                        0,
                    )?
                    // instance model matrix
                    .attrib_matrix(
                        2,
                        Normalized::FixedPoint,
                        Divisor::PerInstances(1),
                        instance_stride,
                        AttribType::UByte.byte_size(4),
                    )?;
            }

            drop(vao);
            pipeline
        };

        Ok(Self {
            terrain,
            frame_target: None,
            debug_shapes: Vec::new(),
            debug_pipeline,
            entities: Vec::with_capacity(64),
            entity_pipeline,
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

#[repr(C)]
pub struct EntityInstance {
    color: u32,
    model: [[f32; 4]; 4],
}

impl Renderer for GlRenderer {
    type Target = FrameTarget;
    type Error = GlError;

    fn init(&mut self, target: Self::Target) {
        self.frame_target = Some(target);
    }

    fn sim_start(&mut self) {}

    fn sim_entity(&mut self, transform: &TransformComponent, physical: PhysicalComponent) {
        self.entities.push((transform.position, physical));
    }

    fn sim_finish(&mut self) -> GlResult<()> {
        let p = self.entity_pipeline.program.scoped_bind();
        let _vao = self.entity_pipeline.vao.scoped_bind();

        let vbo = self.entity_pipeline.instanced_vbo.scoped_bind();
        vbo.buffer_data_uninitialized::<EntityInstance>(
            self.entities.len(),
            BufferUsage::StreamDraw,
        )?;

        if let Some(mut mapped) = vbo.map_write_only::<EntityInstance>()? {
            // TODO cursor interface in ScopedMap

            for (i, (pos, physical)) in self.entities.iter().enumerate() {
                let pos = ViewPoint::from(*pos);
                mapped[i].color = physical.color().into();

                let radius = physical.radius() * SCALE;
                let model = Matrix4::from_translation(pos.into())
                    * Matrix4::from_nonuniform_scale(radius, radius, 1.0);
                mapped[i].model = model.into();
            }
        }

        let frame = self.frame_target.as_mut().unwrap();
        p.set_uniform_matrix("proj\0", frame.proj);
        p.set_uniform_matrix("view\0", frame.view);

        vbo.draw_array_instanced(Primitive::TriangleStrip, self.entities.len());

        self.entities.clear();
        Ok(())
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
