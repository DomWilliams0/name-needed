use crate::render::sdl::gl::{Program, ScopedBind, ScopedBindable, Vao, Vbo};

pub struct Pipeline {
    pub program: Program,
    pub vao: Vao,
    pub vbo: Vbo,
}

pub struct InstancedPipeline {
    pub program: Program,
    pub vao: Vao,
    pub shared_vbo: Vbo,
    pub instanced_vbo: Vbo,
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
