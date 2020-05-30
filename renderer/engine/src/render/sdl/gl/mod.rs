use std::f32::consts::PI;
use std::ffi::{c_void, CStr};
use std::ptr::null;

use gl::types::*;
use sdl2::video::{GLContext, Window};
use sdl2::VideoSubsystem;

pub use capability::{Capability, ScopedCapability};
use color::ColorRgb;
use common::*;
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

#[derive(Debug)]
pub enum GlError {
    LoadingShader(std::io::Error),
    CompilingShader(String),
    LinkingProgram,
    UnknownUniform(&'static str),
    Gl(GLenum),
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

extern "system" fn on_debug_message(
    source: GLenum,
    gltype: GLenum,
    _id: GLuint,
    severity: GLenum,
    _length: GLsizei,
    message: *const GLchar,
    _user_param: *mut c_void,
) {
    let msg = unsafe { CStr::from_ptr(message) };
    if gltype == gl::DEBUG_TYPE_ERROR {
        error!("GL error: severity = {:#x}, message = {:?}", severity, msg);
    } else {
        let level = match severity {
            gl::DEBUG_SEVERITY_HIGH => Level::Error,
            gl::DEBUG_SEVERITY_MEDIUM => Level::Info,
            gl::DEBUG_SEVERITY_LOW => Level::Debug,
            gl::DEBUG_SEVERITY_NOTIFICATION => {
                if cfg!(feature = "gl-trace-log") {
                    Level::Trace
                } else {
                    return;
                }
            }
            _ => Level::Debug,
        };

        log!(
            level,
            "GL message: source: {:#x}, type = {:#x}, message: {:?}",
            source,
            gltype,
            msg
        );
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

        Ok(Self { gl_context })
    }

    pub fn set_clear_color(color: ColorRgb) {
        let [r, g, b]: [f32; 3] = color.into();
        unsafe { gl::ClearColor(r, g, b, 1.0) }
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
