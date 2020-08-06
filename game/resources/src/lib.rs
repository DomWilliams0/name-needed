mod container;
mod error;
pub mod resource;

pub use memmap::Mmap;

pub use container::{recurse, ReadResource, ResourceContainer};
pub use error::{ResourceError, ResourceErrorKind};
