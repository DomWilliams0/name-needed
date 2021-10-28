use std::f32::consts::PI;

use color::ColorRgb;
use common::*;
use resources::Shaders;
use simulation::{PhysicalComponent, RenderComponent, Renderer, TransformComponent};
use unit::world::WorldPoint;

use crate::render::debug::DebugShape;
use crate::render::sdl::gl::{
    AttribType, BufferUsage, Capability, GlError, GlResult, Normalized, Pipeline, Primitive,
    Program, ScopedBindable,
};
use crate::render::sdl::render::entity::EntityRenderer;
use crate::render::sdl::render::terrain::TerrainRenderer;

pub struct GlRenderer {
    /// Populated between init() and deinit()
    frame_ctx: Option<GlfFrameContext>,
    terrain_renderer: TerrainRenderer,
    entity_renderer: EntityRenderer,

    debug_shapes: Vec<DebugShape>,
    debug_pipeline: Pipeline,
}

pub struct GlfFrameContext {
    pub projection: Matrix4,
    pub view: Matrix4,

    /// Amount to subtract from every entity's z pos, to normalize z around 0
    pub z_offset: f32,
}

#[repr(C)]
pub struct DebugVertex {
    pos: [f32; 3],
    col: u32,
}

impl GlRenderer {
    pub fn new(shaders_res: &Shaders) -> GlResult<Self> {
        let terrain = TerrainRenderer::new(shaders_res)?;

        // smooth lines look nice globally
        Capability::LineSmooth.enable();

        // init debug lines pipeline
        let debug_pipeline = {
            let pipeline = Pipeline::new(Program::load(shaders_res, "debug", "rgb")?);

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
            terrain_renderer: terrain,
            frame_ctx: None,
            debug_shapes: Vec::new(),
            debug_pipeline,
            entity_renderer: EntityRenderer::new(shaders_res)?,
        })
    }
    pub fn terrain(&self) -> &TerrainRenderer {
        &self.terrain_renderer
    }

    pub fn terrain_mut(&mut self) -> &mut TerrainRenderer {
        &mut self.terrain_renderer
    }

    pub fn reset(&mut self) {
        self.terrain_renderer.reset();
    }
}

macro_rules! frame_ctx {
    ($this:expr) => {
        $this
            .frame_ctx
            .as_ref()
            .expect("frame target not initialised")
    };
}

impl Renderer for GlRenderer {
    type FrameContext = GlfFrameContext;
    type Error = GlError;

    fn init(&mut self, target: Self::FrameContext) {
        self.frame_ctx = Some(target);
    }

    fn sim_start(&mut self) {}

    fn sim_entity(
        &mut self,
        transform: &TransformComponent,
        render: &RenderComponent,
        physical: &PhysicalComponent,
    ) {
        let ctx = frame_ctx!(self);
        let mut position = transform.position;

        // TODO render head at head height, not the ground

        position.modify_z(|mut z| {
            // tweak z position to keep normalized around 0
            z -= ctx.z_offset;

            // ...plus a tiny amount to always render above the terrain, not in it
            z += 0.001;

            z
        });

        self.entity_renderer.add_entity((
            position,
            render.shape,
            render.color,
            physical.size.into(),
            transform.rotation,
        ));
    }

    fn sim_selected(&mut self, transform: &TransformComponent, physical: &PhysicalComponent) {
        // simple underline
        const PAD: f32 = 0.2;
        let radius = (physical.max_dimension().metres() / 2.0) + PAD;
        let from = transform.position + -Vector2::new(radius, radius);
        let to = from + Vector2::new(radius * 2.0, 0.0);
        self.debug_add_line(from, to, ColorRgb::new(250, 250, 250));
    }

    fn sim_finish(&mut self) -> GlResult<()> {
        let ctx = self.frame_ctx.as_ref().unwrap();
        self.entity_renderer.render_entities(ctx)
    }

    fn debug_start(&mut self) {}

    fn debug_add_line(&mut self, mut from: WorldPoint, mut to: WorldPoint, color: ColorRgb) {
        // keep z normalized around 0
        let ctx = frame_ctx!(self);
        from.modify_z(|z| z - ctx.z_offset);
        to.modify_z(|z| z - ctx.z_offset);

        self.debug_shapes.push(DebugShape::Line {
            points: [from.into(), to.into()],
            color,
        });
    }

    fn debug_add_quad(&mut self, points: [WorldPoint; 4], color: ColorRgb) {
        // TODO add proper support for quads and other debug shapes
        self.debug_add_line(points[0], points[1], color);
        self.debug_add_line(points[1], points[2], color);
        self.debug_add_line(points[2], points[3], color);
        self.debug_add_line(points[3], points[0], color);
    }

    fn debug_add_circle(&mut self, centre: WorldPoint, radius: f32, color: ColorRgb) {
        const SEGMENTS: usize = 30;

        (0..SEGMENTS)
            .map(|i| {
                let theta = 2.0 * PI * (i as f32 / SEGMENTS as f32);
                let x = radius * theta.cos();
                let y = radius * theta.sin();
                (x, y)
            })
            .cycle()
            .take(SEGMENTS + 1)
            .tuple_windows()
            .map(|((ax, ay), (bx, by))| (centre + (ax, ay, 0.0), centre + (bx, by, 0.0)))
            .for_each(|(from, to)| {
                self.debug_add_line(from, to, color);
            });
    }

    fn debug_finish(&mut self) -> GlResult<()> {
        let ctx = frame_ctx!(self);
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

        let proj_view = ctx.projection * ctx.view;
        program.set_uniform_matrix("proj_view\0", proj_view.as_ptr());

        let _no_depth = Capability::DepthTest.scoped_disable();
        vbo.draw_array(Primitive::Lines);

        self.debug_shapes.clear();

        Ok(())
    }

    fn deinit(&mut self) -> Self::FrameContext {
        self.frame_ctx.take().unwrap()
    }
}
