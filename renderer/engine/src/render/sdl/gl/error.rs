use common::*;
use gl::types::*;
use resources::ResourceError;

#[derive(Debug, Error)]
pub enum GlError {
    #[error("Failed to load resource: {0}")]
    LoadingResource(#[from] ResourceError),

    #[error("Invalid font")]
    InvalidFont,

    // TODO proper errors
    #[error("TODO temporary: {0}")]
    Temporary(#[from] Box<dyn Error>),

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
