use std::path::PathBuf;

use gl::types::*;

use common::*;

use crate::errchk;
use crate::render::sdl::gl::vertex::{Bindable, ScopedBind};
use crate::render::sdl::gl::{GlError, GlResult};

pub struct Shader(GLuint);

pub enum ShaderType {
    Vertex,
    Fragment,
}

impl Shader {
    pub fn load(name: &str, shader_type: ShaderType) -> GlResult<Self> {
        let ext = match shader_type {
            ShaderType::Vertex => "glslv",
            ShaderType::Fragment => "glslf",
        };

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/render/sdl/shaders");
        path.push(name);
        path.set_extension(ext);

        debug!(
            "loading shader from {:?}",
            path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap()
        );
        let src = std::fs::read_to_string(path).map_err(GlError::LoadingShader)?;

        Self::from_source(&src, shader_type)
    }

    pub fn from_source(src: &str, shader_type: ShaderType) -> GlResult<Self> {
        let shader_type = match shader_type {
            ShaderType::Vertex => gl::VERTEX_SHADER,
            ShaderType::Fragment => gl::FRAGMENT_SHADER,
        };

        unsafe {
            let shader = Shader(errchk!(gl::CreateShader(shader_type))?);
            let len = src.len();
            let src = src.as_ptr() as *const i8;
            errchk!(gl::ShaderSource(
                shader.0,
                1,
                &src as *const *const i8,
                &len as *const usize as *const _,
            ))?;
            errchk!(gl::CompileShader(shader.0))?;

            let mut status: GLint = 0;
            gl::GetShaderiv(shader.0, gl::COMPILE_STATUS, &mut status as *mut GLint);

            if status == gl::FALSE as GLint {
                let mut log_len = 0usize;
                gl::GetShaderiv(
                    shader.0,
                    gl::INFO_LOG_LENGTH,
                    &mut log_len as *mut usize as *mut _,
                );

                let mut err_string = Vec::<u8>::with_capacity(log_len);
                let mut length = 0usize;
                gl::GetShaderInfoLog(
                    shader.0,
                    log_len as GLint,
                    &mut length as *mut usize as *mut _,
                    err_string.as_mut_ptr() as *mut _,
                );
                err_string.set_len(length);

                Err(GlError::CompilingShader(String::from_utf8_unchecked(
                    err_string,
                )))
            } else {
                Ok(shader)
            }
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.0);
        }
    }
}

pub struct Program(GLuint);

impl Program {
    pub fn load(vertex: &str, fragment: &str) -> GlResult<Self> {
        let vertex = Shader::load(vertex, ShaderType::Vertex)?;
        let fragment = Shader::load(fragment, ShaderType::Fragment)?;

        Self::with_shaders(&[vertex, fragment])
    }

    pub fn from_source(vertex: &str, fragment: &str) -> GlResult<Self> {
        let vertex = Shader::from_source(vertex, ShaderType::Vertex)?;
        let fragment = Shader::from_source(fragment, ShaderType::Fragment)?;

        Self::with_shaders(&[vertex, fragment])
    }

    fn with_shaders(shaders: &[Shader]) -> GlResult<Self> {
        unsafe {
            let program = Program(errchk!(gl::CreateProgram())?);
            for shader in shaders {
                gl::AttachShader(program.0, shader.0);
            }
            gl::LinkProgram(program.0);

            let mut status = 0;
            gl::GetProgramiv(program.0, gl::LINK_STATUS, &mut status as *mut _);
            if status as GLboolean == gl::FALSE {
                Err(GlError::LinkingProgram)
            } else {
                gl::ProgramParameteri(program.0, gl::PROGRAM_SEPARABLE, gl::TRUE as GLint);

                for shader in shaders {
                    gl::DetachShader(program.0, shader.0);
                }

                Ok(program)
            }
        }
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.0);
        }
    }
}

impl Bindable for Program {
    unsafe fn bind(&self) {
        gl::UseProgram(self.0);
    }

    unsafe fn unbind(&self) {
        gl::UseProgram(0);
    }
}

impl ScopedBind<'_, Program> {
    /// Name must be null terminated
    pub fn set_uniform_matrix(&self, name: &'static str, matrix: *const F) {
        assert_eq!(
            name.chars().last(),
            Some('\0'),
            "name must be null terminated"
        );
        unsafe {
            let do_set = || -> GlResult<()> {
                let location = gl::GetUniformLocation(self.0, name.as_ptr() as *const _);
                if location == -1 {
                    Err(GlError::UnknownUniform(name))
                } else {
                    errchk!(gl::UniformMatrix4fv(location, 1, gl::FALSE, matrix))
                }
            };

            if let Err(e) = do_set() {
                warn!("failed to set uniform {:?}: {:?}", name, e);
            }
        }
    }
}
