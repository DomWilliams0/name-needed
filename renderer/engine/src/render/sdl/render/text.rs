use crate::render::sdl::gl::{GlError, GlResult, Program};
use crate::render::sdl::render::text::example::{GlGlyphTexture, GlTextPipe, Vertex};
use common::*;
use glyph_brush::ab_glyph::FontVec;
use glyph_brush::{
    BrushAction, BrushError, Color, Extra, GlyphBrush, GlyphBrushBuilder, Section, Text,
};
use resources::{ReadResource, ResourceContainer};

pub struct TextRenderer {
    // program: Program,
    glyph_brush: GlyphBrush<example::Vertex, Extra, FontVec>,
    text_pipe: GlTextPipe,
    texture: GlGlyphTexture,
}

impl TextRenderer {
    pub fn new(shaders_res: &resources::Shaders, fonts_res: &resources::Fonts) -> GlResult<Self> {
        let font = {
            let path = fonts_res.get_file("PrStart.ttf")?;
            trace!("loading font from {}", path.resource_path().display());
            let bytes = Vec::<u8>::read_resource(path)?;
            FontVec::try_from_vec(bytes).map_err(|_| GlError::InvalidFont)?
        };

        // let program = Program::load(shaders_res, "text", "tex")?;
        let glyph_brush = GlyphBrushBuilder::using_font(font).build();

        let mut texture = GlGlyphTexture::new(glyph_brush.texture_dimensions());

        let mut text_pipe = GlTextPipe::new()?;

        Ok(Self {
            text_pipe,
            glyph_brush,
            texture,
        })
    }

    pub fn render_test(&mut self) -> GlResult<()> {
        self.glyph_brush.queue(
            Section::default()
                .add_text(Text::new("NICE").with_color([1.0, 1.0, 1.0, 1.0]))
                .with_screen_position((8.0, 0.0)),
        );

        let texture = self.texture.name;
        let action = self.glyph_brush.process_queued(
            |rect, tex_data| unsafe {
                // Update part of gpu texture with new glyph alpha values
                gl::BindTexture(gl::TEXTURE_2D, texture);
                gl::TexSubImage2D(
                    gl::TEXTURE_2D,
                    0,
                    rect.min[0] as _,
                    rect.min[1] as _,
                    rect.width() as _,
                    rect.height() as _,
                    gl::RED,
                    gl::UNSIGNED_BYTE,
                    tex_data.as_ptr() as _,
                );
            },
            example::to_vertex,
        );

        match action {
            Err(BrushError::TextureTooSmall { suggested, .. }) => {
                let max_image_dimension = {
                    let mut value = 0;
                    unsafe { gl::GetIntegerv(gl::MAX_TEXTURE_SIZE, &mut value) };
                    value as u32
                };

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

                // Recreate texture as a larger size to fit more
                self.texture = GlGlyphTexture::new((new_width, new_height));
                self.glyph_brush.resize_texture(new_width, new_height);
            }
            Ok(BrushAction::Draw(verts)) => {
                debug!("draw {} verts", verts.len());
                self.text_pipe.upload_vertices(&verts);
            }

            Ok(BrushAction::ReDraw) => {}
        }

        self.text_pipe.draw();

        Ok(())
    }
}

mod example {
    use gl::types::*;
    use glyph_brush::ab_glyph::{point, Rect};
    use std::ffi::CString;
    use std::{mem, ptr};

    macro_rules! gl_assert_ok {
        () => {{
            let err = gl::GetError();
            assert_eq!(err, gl::NO_ERROR, "{}", gl_err_to_str(err));
        }};
    }

    pub fn gl_err_to_str(err: u32) -> &'static str {
        match err {
            gl::INVALID_ENUM => "INVALID_ENUM",
            gl::INVALID_VALUE => "INVALID_VALUE",
            gl::INVALID_OPERATION => "INVALID_OPERATION",
            gl::INVALID_FRAMEBUFFER_OPERATION => "INVALID_FRAMEBUFFER_OPERATION",
            gl::OUT_OF_MEMORY => "OUT_OF_MEMORY",
            gl::STACK_UNDERFLOW => "STACK_UNDERFLOW",
            gl::STACK_OVERFLOW => "STACK_OVERFLOW",
            _ => "Unknown error",
        }
    }

    /// The texture used to cache drawn glyphs
    pub struct GlGlyphTexture {
        pub name: GLuint,
    }

    impl GlGlyphTexture {
        pub fn new((width, height): (u32, u32)) -> Self {
            let mut name = 0;
            unsafe {
                // Create a texture for the glyphs
                // The texture holds 1 byte per pixel as alpha data
                gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
                gl::GenTextures(1, &mut name);
                gl::BindTexture(gl::TEXTURE_2D, name);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as _);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as _);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    gl::RED as _,
                    width as _,
                    height as _,
                    0,
                    gl::RED,
                    gl::UNSIGNED_BYTE,
                    ptr::null(),
                );
                gl_assert_ok!();

                Self { name }
            }
        }

        pub fn clear(&self) {
            unsafe {
                gl::BindTexture(gl::TEXTURE_2D, self.name);
                gl::ClearTexImage(
                    self.name,
                    0,
                    gl::RED,
                    gl::UNSIGNED_BYTE,
                    [12_u8].as_ptr() as _,
                );
                gl_assert_ok!();
            }
        }
    }

    impl Drop for GlGlyphTexture {
        fn drop(&mut self) {
            unsafe {
                gl::DeleteTextures(1, &self.name);
            }
        }
    }

    pub struct GlTextPipe {
        shaders: [GLuint; 2],
        program: GLuint,
        vao: GLuint,
        vbo: GLuint,
        transform_uniform: GLint,
        vertex_count: usize,
        vertex_buffer_len: usize,
    }
    pub type Res<T> = Result<T, Box<dyn std::error::Error>>;
    /// `[left_top * 3, right_bottom * 2, tex_left_top * 2, tex_right_bottom * 2, color * 4]`
    pub type Vertex = [GLfloat; 13];

    pub fn compile_shader(src: &str, ty: GLenum) -> Res<GLuint> {
        let shader;
        unsafe {
            shader = gl::CreateShader(ty);
            // Attempt to compile the shader
            let c_str = CString::new(src.as_bytes())?;
            gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
            gl::CompileShader(shader);

            // Get the compile status
            let mut status = GLint::from(gl::FALSE);
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

            // Fail on error
            if status != GLint::from(gl::TRUE) {
                let mut len = 0;
                gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
                let mut buf = Vec::with_capacity(len as usize);
                buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
                gl::GetShaderInfoLog(
                    shader,
                    len,
                    ptr::null_mut(),
                    buf.as_mut_ptr() as *mut GLchar,
                );
                return Err(std::str::from_utf8(&buf)?.into());
            }
        }
        Ok(shader)
    }

    pub fn link_program(vs: GLuint, fs: GLuint) -> Res<GLuint> {
        unsafe {
            let program = gl::CreateProgram();
            gl::AttachShader(program, vs);
            gl::AttachShader(program, fs);
            gl::LinkProgram(program);
            // Get the link status
            let mut status = GLint::from(gl::FALSE);
            gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);

            // Fail on error
            if status != GLint::from(gl::TRUE) {
                let mut len: GLint = 0;
                gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
                let mut buf = Vec::with_capacity(len as usize);
                buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
                gl::GetProgramInfoLog(
                    program,
                    len,
                    ptr::null_mut(),
                    buf.as_mut_ptr() as *mut GLchar,
                );
                return Err(std::str::from_utf8(&buf)?.into());
            }
            Ok(program)
        }
    }

    #[inline]
    pub fn to_vertex(
        glyph_brush::GlyphVertex {
            mut tex_coords,
            pixel_coords,
            bounds,
            extra,
        }: glyph_brush::GlyphVertex,
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
            tex_coords.max.y =
                tex_coords.min.y + tex_coords.height() * gl_rect.height() / old_height;
        }
        if gl_rect.min.y < gl_bounds.min.y {
            let old_height = gl_rect.height();
            gl_rect.min.y = gl_bounds.min.y;
            tex_coords.min.y =
                tex_coords.max.y - tex_coords.height() * gl_rect.height() / old_height;
        }

        [
            gl_rect.min.x,
            gl_rect.max.y,
            extra.z,
            gl_rect.max.x,
            gl_rect.min.y,
            tex_coords.min.x,
            tex_coords.max.y,
            tex_coords.max.x,
            tex_coords.min.y,
            extra.color[0],
            extra.color[1],
            extra.color[2],
            extra.color[3],
        ]
    }

    impl GlTextPipe {
        pub fn new() -> Res<Self> {
            let (w, h) = (256.0, 256.0);

            let vs = compile_shader(
                r#"
                #version 150

const mat4 INVERT_Y_AXIS = mat4(
    vec4(1.0, 0.0, 0.0, 0.0),
    vec4(0.0, -1.0, 0.0, 0.0),
    vec4(0.0, 0.0, 1.0, 0.0),
    vec4(0.0, 0.0, 0.0, 1.0)
);

uniform mat4 transform;

in vec3 left_top;
in vec2 right_bottom;
in vec2 tex_left_top;
in vec2 tex_right_bottom;
in vec4 color;

out vec2 f_tex_pos;
out vec4 f_color;

// generate positional data based on vertex ID
void main() {
    vec2 pos = vec2(0.0);
    float left = left_top.x;
    float right = right_bottom.x;
    float top = left_top.y;
    float bottom = right_bottom.y;

    switch (gl_VertexID) {
        case 0:
            pos = vec2(left, top);
            f_tex_pos = tex_left_top;
            break;
        case 1:
            pos = vec2(right, top);
            f_tex_pos = vec2(tex_right_bottom.x, tex_left_top.y);
            break;
        case 2:
            pos = vec2(left, bottom);
            f_tex_pos = vec2(tex_left_top.x, tex_right_bottom.y);
            break;
        case 3:
            pos = vec2(right, bottom);
            f_tex_pos = tex_right_bottom;
            break;
    }

    f_color = color;
    gl_Position = INVERT_Y_AXIS * transform * vec4(pos, left_top.z, 1.0);
}
"#,
                gl::VERTEX_SHADER,
            )?;
            let fs = compile_shader(
                r#"
               #version 150

uniform sampler2D font_tex;

in vec2 f_tex_pos;
in vec4 f_color;

out vec4 out_color;

void main() {
    float alpha = texture(font_tex, f_tex_pos).r;
    out_color = f_color * vec4(1.0, 1.0, 1.0, alpha);
}

                "#,
                gl::FRAGMENT_SHADER,
            )?;
            let program = link_program(vs, fs)?;

            let mut vao = 0;
            let mut vbo = 0;

            let transform_uniform = unsafe {
                // Create Vertex Array Object
                gl::GenVertexArrays(1, &mut vao);
                gl::BindVertexArray(vao);

                // Create a Vertex Buffer Object
                gl::GenBuffers(1, &mut vbo);
                gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

                // Use shader program
                gl::UseProgram(program);
                gl::BindFragDataLocation(program, 0, CString::new("out_color")?.as_ptr());

                // Specify the layout of the vertex data
                let uniform = gl::GetUniformLocation(program, CString::new("transform")?.as_ptr());
                if uniform < 0 {
                    return Err(format!("GetUniformLocation(\"transform\") -> {}", uniform).into());
                }
                let transform = ortho(0.0, w, 0.0, h, 1.0, -1.0);
                gl::UniformMatrix4fv(uniform, 1, 0, transform.as_ptr());

                let mut offset = 0;
                for (v_field, float_count) in &[
                    ("left_top", 3),
                    ("right_bottom", 2),
                    ("tex_left_top", 2),
                    ("tex_right_bottom", 2),
                    ("color", 4),
                ] {
                    let attr = gl::GetAttribLocation(program, CString::new(*v_field)?.as_ptr());
                    if attr < 0 {
                        return Err(format!("{} GetAttribLocation -> {}", v_field, attr).into());
                    }
                    gl::VertexAttribPointer(
                        attr as _,
                        *float_count,
                        gl::FLOAT,
                        gl::FALSE as _,
                        mem::size_of::<Vertex>() as _,
                        offset as _,
                    );
                    gl::EnableVertexAttribArray(attr as _);
                    gl::VertexAttribDivisor(attr as _, 1); // Important for use with DrawArraysInstanced

                    offset += float_count * 4;
                }

                // Enabled alpha blending
                // gl::Enable(gl::BLEND);
                // gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                // Use srgb for consistency with other examples
                // gl::Enable(gl::FRAMEBUFFER_SRGB);
                // gl::ClearColor(0.02, 0.02, 0.02, 1.0);
                gl_assert_ok!();

                uniform
            };

            Ok(Self {
                shaders: [vs, fs],
                program,
                vao,
                vbo,
                transform_uniform,
                vertex_count: 0,
                vertex_buffer_len: 0,
            })
        }

        pub fn upload_vertices(&mut self, vertices: &[Vertex]) {
            // Draw new vertices
            self.vertex_count = vertices.len();

            unsafe {
                gl::BindVertexArray(self.vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
                if self.vertex_buffer_len < self.vertex_count {
                    gl::BufferData(
                        gl::ARRAY_BUFFER,
                        (self.vertex_count * mem::size_of::<Vertex>()) as GLsizeiptr,
                        vertices.as_ptr() as _,
                        gl::DYNAMIC_DRAW,
                    );
                    self.vertex_buffer_len = self.vertex_count;
                } else {
                    gl::BufferSubData(
                        gl::ARRAY_BUFFER,
                        0,
                        (self.vertex_count * mem::size_of::<Vertex>()) as GLsizeiptr,
                        vertices.as_ptr() as _,
                    );
                }
                gl_assert_ok!();
            }
        }

        pub fn draw(&self) {
            unsafe {
                gl::UseProgram(self.program);
                gl::BindVertexArray(self.vao);
                // If implementing this yourself, make sure to set VertexAttribDivisor as well
                gl::DrawArraysInstanced(gl::TRIANGLE_STRIP, 0, 4, self.vertex_count as _);
                gl_assert_ok!();
            }
        }
    }

    impl Drop for GlTextPipe {
        fn drop(&mut self) {
            unsafe {
                gl::DeleteProgram(self.program);
                self.shaders.iter().for_each(|s| gl::DeleteShader(*s));
                gl::DeleteBuffers(1, &self.vbo);
                gl::DeleteVertexArrays(1, &self.vao);
            }
        }
    }

    pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> [f32; 16] {
        let tx = -(right + left) / (right - left);
        let ty = -(top + bottom) / (top - bottom);
        let tz = -(far + near) / (far - near);
        [
            2.0 / (right - left),
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / (top - bottom),
            0.0,
            0.0,
            0.0,
            0.0,
            -2.0 / (far - near),
            0.0,
            tx,
            ty,
            tz,
            1.0,
        ]
    }
}
