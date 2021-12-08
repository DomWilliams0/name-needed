use std::f32::consts::PI;
use std::ffi::{c_void, CStr};
use std::ptr::null;

use gl::types::*;
use sdl2::video::{GLContext, Window};
use sdl2::VideoSubsystem;

pub use capability::{Capability, ScopedCapability};
use color::Color;
use common::*;
use resources::ResourceError;
pub use shader::Program;
pub use vertex::{
    AttribType, Bindable, BufferUsage, Divisor, Normalized, Primitive, ScopedBind, ScopedBindable,
    ScopedMapMut, Vao, Vbo,
};

mod capability;
mod shader;
mod vertex;

pub struct Gl {
    #[allow(dead_code)]
    gl_context: GLContext,
}

#[derive(Debug, Error)]
pub enum GlError {
    #[error("Failed to load shader: {0}")]
    LoadingShader(#[from] ResourceError),

    #[error("Failed to compile shader: {0}")]
    CompilingShader(String),

    #[error("Failed to link program")]
    LinkingProgram,

    #[error("Unknown uniform {0:?}")]
    UnknownUniform(&'static str),

    #[error("GL error: {0}")]
    Gl(GLenum),

    #[error("Buffer is too small, requested {requested_len} but size is {real_len}")]
    BufferTooSmall {
        real_len: usize,
        requested_len: usize,
    },
}

pub type GlResult<T> = Result<T, GlError>;

#[macro_export]
macro_rules! errchk {
    ($val:expr) => {
        match gl::GetError() {
            gl::NO_ERROR => Ok($val),
            err => Err($crate::render::sdl::gl::GlError::Gl(err)),
        }
    };
}

struct GlHex(u64);
struct GlString<'a>(&'a CStr);

extern "system" fn on_debug_message(
    source: GLenum,
    gltype: GLenum,
    _id: GLuint,
    severity: GLenum,
    _length: GLsizei,
    message: *const GLchar,
    _user_param: *mut c_void,
) {
    let msg = GlString(unsafe { CStr::from_ptr(message) });
    if gltype == gl::DEBUG_TYPE_ERROR {
        error!("GL error"; "severity" => severity, "message" => msg);
    } else {
        if severity == gl::DEBUG_SEVERITY_NOTIFICATION && !cfg!(feature = "gl-trace-log") {
            // shush
            return;
        }

        let o =
            o!("source" => GlHex(source.into()), "type" => GlHex(gltype.into()), "message" => msg);

        match severity {
            gl::DEBUG_SEVERITY_HIGH => warn!("GL message"; o),
            gl::DEBUG_SEVERITY_MEDIUM => info!("GL message"; o),
            _ => debug!("GL message"; o),
        };
    }
}

impl Gl {
    pub fn new(window: &Window, video: &VideoSubsystem) -> Result<Self, String> {
        let gl_context = window.gl_create_context()?;
        gl::load_with(|s| video.gl_get_proc_address(s) as *const _);

        // debug messages
        Capability::DebugOutput.enable();
        unsafe {
            gl::DebugMessageCallback(Some(on_debug_message), null());
        }

        // enable depth test for everything by default
        Capability::DepthTest.enable();

        // enable blending for alpha
        unsafe {
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }
        Capability::Blend.enable();

        Ok(Self { gl_context })
    }

    pub fn set_clear_color(color: Color) {
        let [r, g, b, a]: [f32; 4] = color.into();
        unsafe { gl::ClearColor(r, g, b, a) }
    }

    pub fn clear() {
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }
    }

    pub fn set_viewport(width: i32, height: i32) {
        unsafe { gl::Viewport(0, 0, width, height) }
    }
}

pub struct Pipeline {
    pub program: Program,
    pub vao: Vao,
    pub vbo: Vbo,
}

impl Pipeline {
    pub fn new(program: Program) -> Self {
        Self {
            program,
            vao: Vao::new(),
            vbo: Vbo::array_buffer(),
        }
    }
    pub fn bind_all(&self) -> (ScopedBind<Program>, ScopedBind<Vao>, ScopedBind<Vbo>) {
        let prog = self.program.scoped_bind();
        let vao = self.vao.scoped_bind();
        let vbo = self.vbo.scoped_bind();
        (prog, vao, vbo)
    }
}

pub fn generate_circle_mesh(n: usize) -> Vec<[f32; 3]> {
    let half_n = n / 2;
    let mut vec = vec![[0.0, 0.0, 0.0]; n];

    vec[0] = [1.0, 0.0, 0.0];

    let mut xc = 1.0;
    let mut yc = 0.0;

    let div = PI / half_n as f32;
    let (sin, cos) = div.sin_cos();

    let mut i = 0;
    for _ in 1..half_n {
        let new_xc = cos * xc - sin * yc;
        yc = sin * xc + cos * yc;
        xc = new_xc;

        vec[i] = [xc, yc, 0.0];
        vec[i + 1] = [xc, -yc, 0.0];
        i += 2;
    }

    vec[n - 1] = [-1.0, 0.0, 0.0];

    vec
}

pub fn generate_quad_mesh() -> [[f32; 3]; 4] {
    [
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ]
}

pub struct InstancedPipeline {
    pub program: Program,
    pub vao: Vao,
    pub shared_vbo: Vbo,
    pub instanced_vbo: Vbo,
}

impl InstancedPipeline {
    pub fn new(program: Program) -> Self {
        Self {
            program,
            vao: Vao::new(),
            shared_vbo: Vbo::array_buffer(),
            instanced_vbo: Vbo::array_buffer(),
        }
    }
}

impl slog::Value for GlHex {
    //noinspection DuplicatedCode
    fn serialize(
        &self,
        _: &Record,
        key: &'static str,
        serializer: &mut dyn Serializer,
    ) -> SlogResult<()> {
        serializer.emit_arguments(key, &format_args!("{:#x}", self.0))
    }
}

impl slog::Value for GlString<'_> {
    //noinspection DuplicatedCode
    fn serialize(
        &self,
        _: &Record,
        key: &'static str,
        serializer: &mut dyn Serializer,
    ) -> SlogResult<()> {
        serializer.emit_arguments(key, &format_args!("{:?}", self.0))
    }
}
