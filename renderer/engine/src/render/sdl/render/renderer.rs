use color::Color;
use std::f32::consts::PI;

use common::*;
use resources::Resources;
use simulation::{
    PhysicalComponent, RenderComponent, Renderer, Shape2d, TransformRenderDescription,
    UiElementComponent,
};
use unit::world::WorldPoint;

use crate::render::debug::DebugShape;
use crate::render::sdl::gl::{
    AttribType, BufferUsage, Capability, GlError, GlResult, Normalized, Pipeline, Primitive,
    Program, ScopedBindable,
};
use crate::render::sdl::render::entity::EntityRenderer;
use crate::render::sdl::render::terrain::TerrainRenderer;
use crate::render::sdl::render::text::TextRenderer;

pub struct GlRenderer {
    /// Populated between init() and deinit()
    frame_ctx: Option<GlFrameContext>,
    terrain_renderer: TerrainRenderer,
    entity_renderer: EntityRenderer,
    text_renderer: TextRenderer,

    debug_shapes: Vec<DebugShape>,
    debug_pipeline: Pipeline,
}

pub struct GlFrameContext {
    pub projection: Matrix4,
    pub view: Matrix4,
    /// proj*view for text
    pub text_transform: Matrix4,

    /// Amount to subtract from every entity's z pos, to normalize z around 0
    pub z_offset: f32,
    pub zoom: f32,
}

#[repr(C)]
pub struct DebugVertex {
    pos: [f32; 3],
    col: u32,
}

impl GlRenderer {
    pub fn new(resources: &Resources) -> GlResult<Self> {
        let shaders_res = resources.shaders().map_err(GlError::LoadingResource)?;
        let fonts_res = resources.fonts().map_err(GlError::LoadingResource)?;

        let terrain = TerrainRenderer::new(&shaders_res)?;
        let text_renderer = TextRenderer::new(&shaders_res, &fonts_res)?;

        // smooth lines look nice globally
        Capability::LineSmooth.enable();

        // init debug lines pipeline
        let debug_pipeline = {
            let pipeline = Pipeline::new(Program::load(&shaders_res, "debug", "rgb")?);

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
            text_renderer,
            debug_shapes: Vec::new(),
            debug_pipeline,
            entity_renderer: EntityRenderer::new(&shaders_res)?,
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

impl GlFrameContext {
    fn normalize_entity_z(&self, pos: &mut WorldPoint) {
        pos.modify_z(|mut z| {
            // tweak z position to keep normalized around 0
            z -= self.z_offset;

            // ...plus a tiny amount to always render above the terrain, not in it
            z += 0.001;

            z
        });
    }
}

impl Renderer for GlRenderer {
    type FrameContext = GlFrameContext;
    type Error = GlError;

    fn init(&mut self, target: Self::FrameContext) {
        self.frame_ctx = Some(target);
    }

    fn sim_start(&mut self) {}

    fn sim_entity(
        &mut self,
        transform: &TransformRenderDescription,
        render: &RenderComponent,
        physical: &PhysicalComponent,
    ) {
        let ctx = frame_ctx!(self);
        let mut position = transform.position;

        // TODO render head at head height, not the ground
        ctx.normalize_entity_z(&mut position);

        self.entity_renderer.add_entity(
            position,
            render.shape,
            render.color,
            physical.size.into(),
            transform.rotation,
        );
    }

    fn sim_selected(
        &mut self,
        transform: &TransformRenderDescription,
        physical: &PhysicalComponent,
    ) {
        // simple underline
        const PAD: f32 = 0.2;
        let radius = (physical.max_dimension().metres() / 2.0) + PAD;
        let from = transform.position + -Vector2::new(radius, radius);
        let to = from + Vector2::new(radius * 2.0, 0.0);
        self.debug_add_line(from, to, Color::rgb(250, 250, 250));
    }

    fn sim_ui_element(
        &mut self,
        transform: &TransformRenderDescription,
        ui: &UiElementComponent,
        selected: bool,
    ) {
        let color = if selected {
            Color::rgba(200, 200, 208, 150)
        } else {
            Color::rgba(170, 170, 185, 150)
        };

        let mut pos = transform.position;
        frame_ctx!(self).normalize_entity_z(&mut pos);

        self.entity_renderer.add_entity(
            pos,
            Shape2d::Rect,
            color,
            ui.size,
            Basis2::from_angle(rad(0.0)),
        );
    }

    fn sim_finish(&mut self) -> GlResult<()> {
        let ctx = self.frame_ctx.as_ref().unwrap();

        // entities
        self.entity_renderer.render_entities(ctx)?;

        // in-world text
        self.text_renderer.render_text(ctx)
    }

    fn debug_start(&mut self) {}

    fn debug_add_line(&mut self, mut from: WorldPoint, mut to: WorldPoint, color: Color) {
        // keep z normalized around 0
        let ctx = frame_ctx!(self);
        from.modify_z(|z| z - ctx.z_offset);
        to.modify_z(|z| z - ctx.z_offset);

        self.debug_shapes.push(DebugShape::Line {
            points: [from.into(), to.into()],
            color,
        });
    }

    fn debug_add_quad(&mut self, points: [WorldPoint; 4], color: Color) {
        // TODO add proper support for quads and other debug shapes
        self.debug_add_line(points[0], points[1], color);
        self.debug_add_line(points[1], points[2], color);
        self.debug_add_line(points[2], points[3], color);
        self.debug_add_line(points[3], points[0], color);
    }

    fn debug_add_circle(&mut self, centre: WorldPoint, radius: f32, color: Color) {
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

    fn debug_text(&mut self, centre: WorldPoint, text: &str) {
        self.text_renderer.queue_text(centre, text);
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
