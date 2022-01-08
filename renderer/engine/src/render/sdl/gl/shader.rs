use gl::types::*;
use std::cell::RefCell;

use common::*;

use crate::errchk;
use crate::render::sdl::gl::vertex::{Bindable, ScopedBind};
use crate::render::sdl::gl::{GlError, GlResult};
use resources::Shaders;
use resources::{ReadResource, ResourceContainer};

pub struct Shader(GLuint);

pub enum ShaderType {
    Vertex,
    Fragment,
}

pub struct Program(GLuint, RefCell<UniformCache>);

#[derive(Default)]
struct UniformCache(ArrayVec<(&'static str, GLint), 3>);

impl Shader {
    pub fn load(res: &Shaders, name: &str, shader_type: ShaderType) -> GlResult<Self> {
        let ext = match shader_type {
            ShaderType::Vertex => "glslv",
            ShaderType::Fragment => "glslf",
        };

        let file_name = format!("{}.{}", name, ext);
        debug!("loading shader from file"; "file" => &file_name);

        let src = res
            .get_file(&*file_name)
            .and_then(String::read_resource)
            .map_err(GlError::LoadingResource)?;

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

impl Program {
    pub fn load(res: &Shaders, vertex: &str, fragment: &str) -> GlResult<Self> {
        let vertex = Shader::load(res, vertex, ShaderType::Vertex)?;
        let fragment = Shader::load(res, fragment, ShaderType::Fragment)?;

        Self::with_shaders(&[vertex, fragment])
    }

    /*
    pub fn from_source(vertex: &str, fragment: &str) -> GlResult<Self> {
        let vertex = Shader::from_source(vertex, ShaderType::Vertex)?;
        let fragment = Shader::from_source(fragment, ShaderType::Fragment)?;

        Self::with_shaders(&[vertex, fragment])
    }
    */

    fn with_shaders(shaders: &[Shader]) -> GlResult<Self> {
        unsafe {
            let program = errchk!(gl::CreateProgram())?;
            for shader in shaders {
                gl::AttachShader(program, shader.0);
            }
            gl::LinkProgram(program);

            let mut status = 0;
            gl::GetProgramiv(program, gl::LINK_STATUS, &mut status as *mut _);
            if status as GLboolean == gl::FALSE {
                Err(GlError::LinkingProgram)
            } else {
                gl::ProgramParameteri(program, gl::PROGRAM_SEPARABLE, gl::TRUE as GLint);

                for shader in shaders {
                    gl::DetachShader(program, shader.0);
                }
                Ok(Program(program, Default::default()))
            }
        }
    }
}

impl UniformCache {
    /// Name must be null terminated
    fn resolve(&mut self, program: GLuint, name: &'static str) -> GlResult<GLint> {
        if let Some((_, i)) = self.0.iter().find(|(s, _)| name == *s) {
            return Ok(*i);
        }

        ensure_null_terminated(name);

        let location = unsafe { gl::GetUniformLocation(program, name.as_ptr() as *const _) };
        if location == -1 {
            Err(GlError::UnknownUniform(name))
        } else {
            if self.0.is_full() {
                self.0.swap_remove(0);
            }
            self.0.push((name, location));
            Ok(location)
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
    fn bind(&self) {
        unsafe { gl::UseProgram(self.0) }
    }

    fn unbind(&self) {
        unsafe { gl::UseProgram(0) }
    }
}

fn ensure_null_terminated(s: &str) {
    assert_eq!(s.chars().last(), Some('\0'), "name must be null terminated");
}

impl ScopedBind<'_, Program> {
    /// Name must be null terminated
    pub fn set_uniform_matrix(&self, name: &'static str, matrix: *const F) {
        let mut cache = self.1.borrow_mut();
        let result = cache.resolve(self.0, name).and_then(|location| unsafe {
            errchk!(gl::UniformMatrix4fv(location, 1, gl::FALSE, matrix))
        });

        if let Err(e) = result {
            warn!("failed to set uniform"; "uniform" => name, "error" => %e);
        }
    }

    /// Name must be null terminated
    pub fn bind_frag_data_location(&self, color: u32, name: &str) -> GlResult<()> {
        ensure_null_terminated(name);
        unsafe { errchk!(gl::BindFragDataLocation(self.0, color, name.as_ptr() as _)) }
    }
}
