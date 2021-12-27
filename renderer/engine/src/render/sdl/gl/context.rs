use crate::render::sdl::gl::Capability;
use color::Color;
use common::*;
use gl::types::*;
use sdl2::video::{GLContext, Window};
use sdl2::VideoSubsystem;
use std::ffi::{c_void, CStr};
use std::ptr::null;

pub struct Gl(GLContext);

struct GlHex(u64);
struct GlString<'a>(&'a CStr);

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

        Ok(Self(gl_context))
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
