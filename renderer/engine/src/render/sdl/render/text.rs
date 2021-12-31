use crate::render::sdl::gl::{
    AttribType, BufferUsage, Capability, GlError, GlResult, Normalized, Primitive, Program,
    ScopedBindable, Texture, Vao, Vbo,
};

use crate::render::sdl::render::GlFrameContext;
use color::Color;
use common::*;
use resources::{ReadResource, ResourceContainer};
use rusttype::gpu_cache::CacheBuilder;
use rusttype::{Font, Point, PositionedGlyph, Scale};
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
    colour: u32,
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
                .add(4, AttribType::UByte, Normalized::Normalized)
                .build()?;
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
        if text.is_empty() {
            return;
        }

        let view_point = ViewPoint::from(centre);
        let scale = Scale::uniform(RESOLUTION);
        let (x, y) = {
            let (x, y, _) = view_point.xyz();
            (x * FONT_SIZE * RESOLUTION, -y * FONT_SIZE * RESOLUTION)
        };

        // safety: font lives as long as we are rendering
        let font: &'static Font<'static> = unsafe { std::mem::transmute(&self.font) };
        self.word_boundaries.push(self.glyphs.len());
        self.glyphs.extend(font.layout(text, scale, Point { x, y }));
    }

    fn queue_all_glyphs(&mut self, ctx: &GlFrameContext) {
        let words = {
            self.word_boundaries.push(self.glyphs.len());
            self.word_boundaries.drain(..).tuple_windows()
        };
        let zoom = 1.0 / ctx.zoom;

        let half_height = {
            // for centring
            let scale = Scale::uniform(RESOLUTION);
            self.font.v_metrics(scale).line_gap / 2.0
        };

        for (start, end) in words {
            let word: &mut [PositionedGlyph] = &mut self.glyphs[start..end];
            debug_assert!(!word.is_empty(), "zero len string not allowed");

            let half_width = {
                // for centring
                let first = &word[0].position();
                let last = &word[word.len() - 1].position();
                (last.x - first.x) / 2.0
            };

            let offset = {
                let first = &mut word[0];
                let pos_orig = first.position();
                let pos_new = Point {
                    x: (pos_orig.x * zoom) - half_width,
                    y: (pos_orig.y * zoom) + half_height,
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
    }

    pub fn render_text(&mut self, ctx: &GlFrameContext) -> GlResult<()> {
        self.queue_all_glyphs(ctx);

        let transform = {
            const SCALE: f32 = 1.0 / (FONT_SIZE * RESOLUTION);
            let invert_y_axis_and_scale = Matrix4::from([
                [SCALE, 0.0, 0.0, 0.0],
                [0.0, -SCALE, 0.0, 0.0],
                [0.0, 0.0, SCALE, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]);
            ctx.text_transform * invert_y_axis_and_scale
        };

        let mut uploads = 0;
        loop {
            let texture = self.texture.borrow();
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

            match result {
                Err(err) => {
                    let (w, h) = {
                        let (w, h) = self.cache.dimensions();
                        (w * 2, h * 2)
                    };
                    warn!("font cache error, increasing size"; "err" => %err, "new_size" => ?(w,h));
                    self.cache
                        .to_builder()
                        .dimensions(w, h)
                        .rebuild(&mut self.cache);
                    self.texture = Texture::new_2d(w, h)?;
                }
                Ok(_) => break,
            }
        }

        if uploads > 0 {
            debug!("uploaded {n} glyphs to gpu", n = uploads)
        }

        if self.glyphs.is_empty() {
            // no glyphs, nothing to do
            return Ok(());
        }

        let _vao = self.vao.scoped_bind();
        let prog = self.program.scoped_bind();
        let vbo = self.vbo.scoped_bind();

        let vertex_count = self.glyphs.len() * 6;
        vbo.buffer_data_uninitialized::<Vertex>(vertex_count, BufferUsage::StreamDraw)?;
        let mut vertices = vbo.map_write_only()?.unwrap(); // checked to be not empty
        let mut i = 0;

        for glyph in &self.glyphs {
            let (uv_rect, screen_rect) = match self.cache.rect_for(0, glyph) {
                Ok(Some(r)) => r,
                Ok(None) => continue,
                no => panic!("damn {:?}", no),
            };

            // TODO customise text colour
            // TODO use instances or indices?
            let pos_min = (screen_rect.min.x as f32, screen_rect.min.y as f32);
            let pos_max = (screen_rect.max.x as f32, screen_rect.max.y as f32);
            let colour = u32::from(Color::rgba(255, 255, 255, 255));

            vertices[i] = Vertex {
                pixel_pos: [pos_min.0, pos_max.1],
                tex_coords: [uv_rect.min.x, uv_rect.max.y],
                colour,
            };
            vertices[i + 1] = Vertex {
                pixel_pos: [pos_min.0, pos_min.1],
                tex_coords: [uv_rect.min.x, uv_rect.min.y],
                colour,
            };
            vertices[i + 2] = Vertex {
                pixel_pos: [pos_max.0, pos_min.1],
                tex_coords: [uv_rect.max.x, uv_rect.min.y],
                colour,
            };
            vertices[i + 3] = Vertex {
                pixel_pos: [pos_max.0, pos_min.1],
                tex_coords: [uv_rect.max.x, uv_rect.min.y],
                colour,
            };
            vertices[i + 4] = Vertex {
                pixel_pos: [pos_max.0, pos_max.1],
                tex_coords: [uv_rect.max.x, uv_rect.max.y],
                colour,
            };
            vertices[i + 5] = Vertex {
                pixel_pos: [pos_min.0, pos_max.1],
                tex_coords: [uv_rect.min.x, uv_rect.max.y],
                colour,
            };
            i += 6;
        }

        debug_assert!(i <= vertex_count);

        self.glyphs.clear();

        let _no_depth = Capability::DepthTest.scoped_disable(); // TODO clear depth mask instead
        prog.set_uniform_matrix("transform\0", transform.as_ptr());
        vbo.draw_array(Primitive::Triangles);
        Ok(())
    }
}
