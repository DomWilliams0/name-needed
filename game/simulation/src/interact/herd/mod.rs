pub use component::{HerdableComponent, HerdedComponent};
pub use debug::HerdDebugRenderer;
pub use herds::{HerdHandle, HerdInfo, Herds};
pub use system::HerdJoiningSystem;

mod component;
mod debug;
mod herds;
mod system;
