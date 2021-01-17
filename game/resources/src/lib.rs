mod container;
mod error;
mod resource;

pub use memmap::Mmap;

pub use container::{recurse, ReadResource, ResourceContainer};
pub use error::{ResourceError, ResourceErrorKind};
pub use resource::*;
