mod container;
mod error;
mod resource;

pub use memmap::Mmap;

pub use container::{recurse, ReadResource, ResourceContainer, ResourceFile, ResourcePath};
pub use error::{ResourceError, ResourceErrorKind};
pub use resource::*;
