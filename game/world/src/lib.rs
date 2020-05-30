#![allow(dead_code)]
#![deny(clippy::missing_safety_doc)]

pub use petgraph::prelude::NodeIndex;

pub use self::chunk::{BaseTerrain, Chunk};
pub use self::mesh::BaseVertex;
pub use self::navigation::{EdgeCost, NavigationError, WorldArea, WorldPath};
pub use self::viewer::{SliceRange, WorldViewer};
pub use self::world::World;
pub use self::world_ref::{InnerWorldRef, InnerWorldRefMut, WorldRef};
#[cfg(any(test, feature = "benchmarking"))]
pub use self::{chunk::ChunkBuilder, chunk::ChunkDescriptor, world::world_from_chunks};

pub mod block;
mod chunk;
mod grid;
pub mod loader;
mod mesh;
mod navigation;
mod occlusion;
pub mod presets;
mod viewer;
mod world;
mod world_ref;
