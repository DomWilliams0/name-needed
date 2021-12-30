use crate::render::sdl::gl::{
    AttribType, Bindable, BufferUsage, Capability, GlError, GlResult, Normalized, Primitive,
    Program, ScopedBindable, Texture, Vao, Vbo,
};

use crate::render::sdl::render::GlFrameContext;
use common::*;
use resources::{ReadResource, ResourceContainer};
use rusttype::gpu_cache::CacheBuilder;
use rusttype::{vector, Font, Point, PositionedGlyph, Rect, Scale};
use unit::space::view::ViewPoint;
use unit::world::WorldPoint;

const RESOLUTION: f32 = 64.0;
const FONT_SIZE: f32 = 4.0;
const WORD_WRAP: f32 = 1.6;

pub struct TextRenderer {
    font: rusttype::Font<'static>,
    cache: rusttype::gpu_cache::Cache<'static>,

    glyphs: Vec<PositionedGlyph<'static>>,
    word_boundaries: Vec<usize>,

    texture: Texture,

    program: Program,
    vao: Vao,
    vbo: Vbo,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct Vertex {
    pixel_pos: [f32; 2],
    tex_coords: [f32; 2],
    colour: [f32; 4],
}

impl TextRenderer {
    pub fn new(shaders_res: &resources::Shaders, fonts_res: &resources::Fonts) -> GlResult<Self> {
        let font = {
            let path = fonts_res.get_file("ProggyClean.ttf")?;
            trace!("loading font from {}", path.resource_path().display());
            let bytes = Vec::<u8>::read_resource(path)?;
            Font::try_from_vec(bytes).ok_or(GlError::InvalidFont)?
        };

        let cache_size = 512;
        let cache = CacheBuilder::default()
            .dimensions(cache_size, cache_size)
            .position_tolerance(2.0) // ignore positions for caching
            .build();

        let texture = Texture::new_2d(cache_size, cache_size)?;

        let program = Program::load(shaders_res, "text", "tex")?;

        let vao = Vao::new();
        let vbo = Vbo::array_buffer();
        {
            let vao = vao.scoped_bind();
            let _vbo = vbo.scoped_bind();

            let program = program.scoped_bind();
            program.bind_frag_data_location(0, "out_color\0")?;

            vao.vertex_attribs()
                .add(2, AttribType::Float32, Normalized::FixedPoint)
                .add(2, AttribType::Float32, Normalized::FixedPoint)
                .add(4, AttribType::Float32, Normalized::FixedPoint)
                .build()?;
            // TODO normalised color
        }

        Ok(Self {
            program,
            font,
            cache,
            glyphs: Vec::with_capacity(128),
            word_boundaries: Vec::with_capacity(32),
            texture,
            vao,
            vbo,
        })
    }

    pub fn queue_text(&mut self, centre: WorldPoint, text: &str) {
        let view_point = ViewPoint::from(centre);

        let scale = Scale::uniform(RESOLUTION);
        let (x, y) = {
            let (x, y, _) = view_point.xyz();
            (x * FONT_SIZE * RESOLUTION, -y * FONT_SIZE * RESOLUTION)
        };

        // TODO centre string

        let font: &'static Font<'static> = unsafe { std::mem::transmute(&self.font) };
        let start = Point { x, y };
        let glyphs = font.glyphs_for(text.chars()).scan((None, 0.0), |state, g| {
            let (last, x) = state;
            let g = g.scaled(scale);
            if let Some(last) = *last {
                *x += font.pair_kerning(scale, last, g.id());
            }
            let w = g.h_metrics().advance_width;
            let next = g.positioned(start + vector(*x, 0.0));
            *last = Some(next.id());
            *x += w;
            Some(next)
        });

        self.word_boundaries.push(self.glyphs.len());
        self.glyphs.extend(glyphs);
    }

    pub fn render_text(&mut self, ctx: &GlFrameContext) -> GlResult<()> {
        // TODO comptime this matrix multiplication
        let invert_y_axis = Matrix4::from([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let scale = Matrix4::from_scale(1.0 / (FONT_SIZE * RESOLUTION));
        let transform = ctx.text_transform * invert_y_axis * scale;

        let words = {
            self.word_boundaries.push(self.glyphs.len());
            self.word_boundaries.drain(..).tuple_windows()
        };
        let zoom = 1.0 / ctx.zoom;
        for (start, end) in words {
            // TODO ensure non zero len
            let word = &mut self.glyphs[start..end];

            let offset = {
                let first = &mut word[0];
                let pos_orig = first.position();
                let pos_new = Point {
                    x: pos_orig.x * zoom,
                    y: pos_orig.y * zoom,
                };

                first.set_position(pos_new);

                pos_new - pos_orig
            };

            for g in &mut word[1..] {
                g.set_position(g.position() + offset);
            }
        }
        for g in self.glyphs.iter_mut() {
            self.cache.queue_glyph(0, g.clone());
        }

        let texture = self.texture.borrow();
        let mut uploads = 0;
        let result = self.cache.cache_queued(|rect, data| {
            let texture = texture.bind();
            if let Err(err) = texture.sub_image(
                [rect.min.x, rect.min.y],
                [rect.width(), rect.height()],
                data,
            ) {
                error!("failed to update font texture: {}", err);
            } else {
                uploads += 1;
            }
        });

        if uploads > 0 {
            debug!("uploaded {n} glyphs to gpu", n = uploads)
        }

        match result {
            // TODO resize cache
            Err(err) => panic!("text fail {}", err),
            Ok(_) => {}
        }

        // TODO reuse alloc
        let mut vertices = vec![];

        for glyph in &self.glyphs {
            let (uv_rect, screen_rect) = match self.cache.rect_for(0, glyph) {
                Ok(Some(r)) => r,
                Ok(None) => continue,
                no => panic!("damn {:?}", no),
            };

            let pos_min = (screen_rect.min.x as f32, screen_rect.min.y as f32);
            let pos_max = (screen_rect.max.x as f32, screen_rect.max.y as f32);
            // TODO text colour
            // TODO use instances?
            let colour = [1.0, 1.0, 1.0, 1.0];
            vertices.extend([
                Vertex {
                    pixel_pos: [pos_min.0 as f32, pos_max.1 as f32],
                    tex_coords: [uv_rect.min.x, uv_rect.max.y],
                    colour,
                },
                Vertex {
                    pixel_pos: [pos_min.0 as f32, pos_min.1 as f32],
                    tex_coords: [uv_rect.min.x, uv_rect.min.y],
                    colour,
                },
                Vertex {
                    pixel_pos: [pos_max.0 as f32, pos_min.1 as f32],
                    tex_coords: [uv_rect.max.x, uv_rect.min.y],
                    colour,
                },
                Vertex {
                    pixel_pos: [pos_max.0 as f32, pos_min.1 as f32],
                    tex_coords: [uv_rect.max.x, uv_rect.min.y],
                    colour,
                },
                Vertex {
                    pixel_pos: [pos_max.0 as f32, pos_max.1 as f32],
                    tex_coords: [uv_rect.max.x, uv_rect.max.y],
                    colour,
                },
                Vertex {
                    pixel_pos: [pos_min.0 as f32, pos_max.1 as f32],
                    tex_coords: [uv_rect.min.x, uv_rect.max.y],
                    colour,
                },
            ]);
        }

        self.glyphs.clear();

        {
            let _vao = self.vao.scoped_bind();
            let prog = self.program.scoped_bind();
            let vbo = self.vbo.scoped_bind();

            let _no_depth = Capability::DepthTest.scoped_disable(); // TODO clear depth mask instead
            prog.set_uniform_matrix("transform\0", transform.as_ptr());
            vbo.buffer_data(&vertices, BufferUsage::StreamDraw)?;
            vbo.draw_array(Primitive::Triangles);
        }

        Ok(())
    }
}
