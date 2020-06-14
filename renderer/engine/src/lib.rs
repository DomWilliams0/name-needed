#[cfg(any(feature = "use-sdl"))]
mod render;

#[cfg(feature = "use-sdl")]
pub use render::sdl::{SdlBackendInit, SdlBackendPersistent};

#[cfg(feature = "lite")]
mod lite;
#[cfg(feature = "lite")]
pub use lite::{DummyBackendInit, DummyBackendPersistent};

mod engine;
pub use crate::engine::Engine;
