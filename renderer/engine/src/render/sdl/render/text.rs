use crate::render::sdl::gl::{
    AttribType, Bindable, BufferUsage, Capability, Divisor, Gl, GlError, GlResult, Normalized,
    Primitive, Program, ScopedBindable, Texture, Vao, Vbo,
};

use crate::render::sdl::render::GlFrameContext;
use common::*;
use glyph_brush::ab_glyph::{point, FontVec, Rect};
use glyph_brush::{
    BrushAction, BrushError, BuiltInLineBreaker, GlyphBrush, GlyphBrushBuilder, GlyphVertex,
    HorizontalAlign, Layout, Section, Text, VerticalAlign,
};
use resources::{ReadResource, ResourceContainer};
use unit::space::view::ViewPoint;
use unit::world::WorldPosition;

pub struct TextRenderer {
    glyph_brush: GlyphBrush<Vertex, VertexExtra, FontVec>,
    texture: Texture,

    program: Program,
    vao: Vao,
    vbo: Vbo,

    vertex_count: usize,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Vertex {
    left_top: [f32; 3],
    right_bottom: [f32; 2],

    tex_left_top: [f32; 2],
    tex_right_bottom: [f32; 2],

    color: [f32; 4],
}

#[derive(Copy, Clone, Hash, PartialEq, Default)]
struct VertexExtra {}

impl TextRenderer {
    pub fn new(shaders_res: &resources::Shaders, fonts_res: &resources::Fonts) -> GlResult<Self> {
        let font = {
            let path = fonts_res.get_file("ProggyClean.ttf")?;
            trace!("loading font from {}", path.resource_path().display());
            let bytes = Vec::<u8>::read_resource(path)?;
            FontVec::try_from_vec(bytes).map_err(|_| GlError::InvalidFont)?
        };

        let glyph_brush = GlyphBrushBuilder::using_font(font).build();

        let texture = {
            let (w, h) = glyph_brush.texture_dimensions();
            Texture::new_2d(w, h)?
        };

        let program = Program::load(shaders_res, "text", "tex")?;

        let vao = Vao::new();
        let vbo = Vbo::array_buffer();
        {
            let vao = vao.scoped_bind();
            let _vbo = vbo.scoped_bind();

            let program = program.scoped_bind();
            program.bind_frag_data_location(0, "out_color\0")?;

            vao.vertex_attribs()
                .add_instanced(
                    3,
                    AttribType::Float32,
                    Normalized::FixedPoint,
                    Divisor::PerInstances(1),
                ) // left_top
                .add_instanced(
                    2,
                    AttribType::Float32,
                    Normalized::FixedPoint,
                    Divisor::PerInstances(1),
                ) // right_bottom
                .add_instanced(
                    2,
                    AttribType::Float32,
                    Normalized::FixedPoint,
                    Divisor::PerInstances(1),
                ) // tex_left_top
                .add_instanced(
                    2,
                    AttribType::Float32,
                    Normalized::FixedPoint,
                    Divisor::PerInstances(1),
                ) // tex_right_bottom
                .add_instanced(
                    4,
                    AttribType::Float32,
                    Normalized::FixedPoint,
                    Divisor::PerInstances(1),
                ) // color
                .build()?;
            // TODO normalised color
        }

        Ok(Self {
            program,
            glyph_brush,
            texture,
            vao,
            vbo,
            vertex_count: 0,
        })
    }

    pub fn render_test(&mut self, ctx: &GlFrameContext) -> GlResult<()> {
        const RESOLUTION: f32 = 64.0;
        const FONT_SIZE: f32 = 4.0;

        let invert = Matrix4::from([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let scale = Matrix4::from_scale(1.0 / (FONT_SIZE * RESOLUTION));
        let transform = ctx.projection * ctx.view * invert * scale;

        for og_x in (0..=18).step_by(8) {
            for og_y in (-4..=18).step_by(2) {
                let pos = WorldPosition::from((og_x, og_y, 0));
                let view_point = ViewPoint::from(pos.centred().floored());
                let (x, y) = {
                    let (x, y, _) = view_point.xyz();
                    (x * FONT_SIZE * RESOLUTION, -y * FONT_SIZE * RESOLUTION)
                };

                self.glyph_brush.queue(
                    Section::<VertexExtra>::new()
                        .add_text(
                            Text::default()
                                .with_text(&format!("({}, {})", og_x, og_y))
                                .with_scale(RESOLUTION),
                        )
                        .with_screen_position((x, y))
                        .with_layout(Layout::SingleLine {
                            line_breaker: BuiltInLineBreaker::default(),
                            h_align: HorizontalAlign::Left,
                            v_align: VerticalAlign::Bottom,
                        }),
                );
            }
        }

        loop {
            let texture = self.texture.borrow();
            let action = self.glyph_brush.process_queued(
                |rect, tex_data| {
                    let texture = texture.bind();
                    if let Err(err) =
                        texture.sub_image(rect.min, [rect.width(), rect.height()], tex_data)
                    {
                        error!("failed to update font texture: {}", err);
                    }
                },
                to_vertex,
            );

            match action {
                Err(BrushError::TextureTooSmall { suggested, .. }) => {
                    let max_image_dimension = Gl::max_texture_size();

                    let (new_width, new_height) = if (suggested.0 > max_image_dimension
                        || suggested.1 > max_image_dimension)
                        && (self.glyph_brush.texture_dimensions().0 < max_image_dimension
                            || self.glyph_brush.texture_dimensions().1 < max_image_dimension)
                    {
                        (max_image_dimension, max_image_dimension)
                    } else {
                        suggested
                    };
                    trace!("resizing text glyph texture"; "size" => ?(new_width, new_height));

                    self.texture = Texture::new_2d(new_width, new_height)?;
                    self.glyph_brush.resize_texture(new_width, new_height);
                    continue; // try again
                }
                Ok(BrushAction::Draw(verts)) => {
                    debug!("draw {n} text verticies", n = verts.len());

                    let _vao = self.vao.scoped_bind();
                    let vbo = self.vbo.scoped_bind();
                    vbo.buffer_data(&verts, BufferUsage::StreamDraw)?;
                    self.vertex_count = verts.len();
                }

                Ok(BrushAction::ReDraw) => {}
            }

            // only loop on error
            break;
        }

        {
            let _vao = self.vao.scoped_bind();
            let prog = self.program.scoped_bind();
            let vbo = self.vbo.scoped_bind();

            let _no_depth = Capability::DepthTest.scoped_disable(); // TODO clear depth mask instead
            prog.set_uniform_matrix("transform\0", transform.as_ptr());
            vbo.draw_array_instanced(Primitive::TriangleStrip, 0, 4, self.vertex_count)?;
        }

        Ok(())
    }
}

fn to_vertex(
    GlyphVertex {
        mut tex_coords,
        pixel_coords,
        bounds,
        ..
    }: GlyphVertex<VertexExtra>,
) -> Vertex {
    let gl_bounds = bounds;

    let mut gl_rect = Rect {
        min: point(pixel_coords.min.x as f32, pixel_coords.min.y as f32),
        max: point(pixel_coords.max.x as f32, pixel_coords.max.y as f32),
    };

    // handle overlapping bounds, modify uv_rect to preserve texture aspect
    if gl_rect.max.x > gl_bounds.max.x {
        let old_width = gl_rect.width();
        gl_rect.max.x = gl_bounds.max.x;
        tex_coords.max.x = tex_coords.min.x + tex_coords.width() * gl_rect.width() / old_width;
    }
    if gl_rect.min.x < gl_bounds.min.x {
        let old_width = gl_rect.width();
        gl_rect.min.x = gl_bounds.min.x;
        tex_coords.min.x = tex_coords.max.x - tex_coords.width() * gl_rect.width() / old_width;
    }
    if gl_rect.max.y > gl_bounds.max.y {
        let old_height = gl_rect.height();
        gl_rect.max.y = gl_bounds.max.y;
        tex_coords.max.y = tex_coords.min.y + tex_coords.height() * gl_rect.height() / old_height;
    }
    if gl_rect.min.y < gl_bounds.min.y {
        let old_height = gl_rect.height();
        gl_rect.min.y = gl_bounds.min.y;
        tex_coords.min.y = tex_coords.max.y - tex_coords.height() * gl_rect.height() / old_height;
    }

    Vertex {
        left_top: [gl_rect.min.x, gl_rect.max.y, 0.0],
        right_bottom: [gl_rect.max.x, gl_rect.min.y],
        tex_left_top: [tex_coords.min.x, tex_coords.max.y],
        tex_right_bottom: [tex_coords.max.x, tex_coords.min.y],
        color: [1.0, 1.0, 1.0, 1.0],
    }
}
