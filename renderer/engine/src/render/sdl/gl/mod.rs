#![allow(dead_code)]

mod capability;
mod context;
mod error;
mod pipeline;
mod shader;
mod texture;
mod vertex;

pub use capability::{Capability, ScopedCapability};
pub use shader::Program;
pub use vertex::{
    AttribType, Bindable, BufferUsage, Divisor, Normalized, Primitive, ScopedBind, ScopedBindable,
    ScopedMapMut, Vao, Vbo,
};

pub use context::Gl;
pub use error::{GlError, GlResult};
pub use pipeline::{InstancedPipeline, Pipeline};
pub use texture::Texture;
