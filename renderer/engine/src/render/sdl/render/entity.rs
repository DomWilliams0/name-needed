use crate::render::sdl::gl::{
    generate_circle_mesh, generate_quad_mesh, AttribType, BufferUsage, Divisor, GlResult,
    InstancedPipeline, Normalized, Primitive, Program, ScopedBindable, Vbo,
};
use crate::render::sdl::render::renderer::GlfFrameContext;
use color::ColorRgb;
use common::*;
use resources::Shaders;
use simulation::Shape2d;
use unit::space::length::{Length, Length2};
use unit::space::view::ViewPoint;
use unit::world::WorldPoint;

pub(crate) struct EntityRenderer {
    pipeline: InstancedPipeline,
    indices_vbo: Vbo,
    entities: Vec<(WorldPoint, Shape2d, ColorRgb, Length2)>,
}

#[repr(C)]
struct EntityInstance {
    color: u32,
    model: [[f32; 4]; 4],
}

const CIRCLE_VERTEX_COUNT: usize = 40;
const RECT_VERTEX_COUNT: usize = 6;

impl EntityRenderer {
    pub fn new(shaders_res: &Shaders) -> GlResult<Self> {
        let pipeline = InstancedPipeline::new(Program::load(shaders_res, "entity", "rgb")?);
        let indices = Vbo::index_buffer();
        let vao = pipeline.vao.scoped_bind();

        {
            let circle_mesh = generate_circle_mesh(CIRCLE_VERTEX_COUNT);
            let quad_mesh = generate_quad_mesh();

            let vbo = pipeline.shared_vbo.scoped_bind();
            vbo.buffer_data_uninitialized::<[f32; 3]>(
                CIRCLE_VERTEX_COUNT + quad_mesh.len(),
                BufferUsage::StaticDraw,
            )?;

            // circle mesh is immediately followed by quad mesh
            vbo.buffer_sub_data(0, &circle_mesh)?;
            vbo.buffer_sub_data(CIRCLE_VERTEX_COUNT, &quad_mesh)?;

            // indices for rendering
            let indices = indices.scoped_bind();
            indices.buffer_data_uninitialized::<u16>(
                CIRCLE_VERTEX_COUNT + RECT_VERTEX_COUNT,
                BufferUsage::StaticDraw,
            )?;
            if let Some(mut indices) = indices.map_write_only::<u16>()? {
                // circle mesh, no special indices
                for i in 0..circle_mesh.len() {
                    indices[i] = i as u16;
                }

                // quad
                let quad_base = circle_mesh.len();
                let quad_indices = [0, 1, 2, 2, 3, 0];
                for (i, v) in quad_indices.iter().enumerate() {
                    indices[quad_base + i] = quad_base as u16 + v;
                }
            }

            // shared vertex position attribute
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

        Ok(Self {
            pipeline,
            indices_vbo: indices,
            entities: Vec::with_capacity(64),
        })
    }

    pub fn add_entity(&mut self, entity: (WorldPoint, Shape2d, ColorRgb, Length2)) {
        self.entities.push(entity);
    }

    pub fn render_entities(&mut self, frame_ctx: &GlfFrameContext) -> GlResult<()> {
        // sort by shape
        self.entities
            .sort_unstable_by_key(|(_, shape, _, _)| shape.ord());

        let p = self.pipeline.program.scoped_bind();
        let _vao = self.pipeline.vao.scoped_bind();

        let vbo = self.pipeline.instanced_vbo.scoped_bind();
        let _indices = self.indices_vbo.scoped_bind();

        // TODO use buffersubdata to reuse allocation if len <=
        vbo.buffer_data_uninitialized::<EntityInstance>(
            self.entities.len(),
            BufferUsage::StreamDraw,
        )?;

        if let Some(mut mapped) = vbo.map_write_only::<EntityInstance>()? {
            // TODO cursor interface in ScopedMap

            for (i, (pos, _, color, size)) in self.entities.iter().enumerate() {
                let pos = ViewPoint::from(*pos);
                mapped[i].color = (*color).into();

                let scale = {
                    let (x, y) = size.xy();
                    let scale = |len: Length| len.metres() / 2.0;
                    Matrix4::from_nonuniform_scale(scale(x), scale(y), 1.0)
                };

                let model = Matrix4::from_translation(pos.into()) * scale;
                mapped[i].model = model.into();
            }
        }

        // these are the same for all entities, so multiply them once on the cpu
        let proj_view = frame_ctx.projection * frame_ctx.view;
        p.set_uniform_matrix("proj_view\0", proj_view.as_ptr());

        let render_data = [
            (Primitive::TriangleStrip, 0, CIRCLE_VERTEX_COUNT),
            (Primitive::Triangles, CIRCLE_VERTEX_COUNT, RECT_VERTEX_COUNT), // rectangle
        ];
        let mut first_instance = 0;
        for (i, grouped) in self
            .entities
            .iter()
            .group_by(|(_, shape, _, _)| shape.ord())
            .into_iter()
        {
            let (primitive, start_ptr, element_count) = render_data[i];
            let instance_count = grouped.count();
            vbo.draw_elements_instanced(
                primitive,
                start_ptr,
                element_count,
                first_instance,
                instance_count,
            )?;

            first_instance += instance_count;
        }

        self.entities.clear();
        Ok(())
    }
}
