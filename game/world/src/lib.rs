#![allow(dead_code)]

pub use petgraph::prelude::NodeIndex;

pub use self::area::{EdgeCost, WorldPath, WorldPathSlice};
pub use self::chunk::*;
pub use self::mesh::Vertex;
pub use self::viewer::{SliceRange, WorldViewer};
pub use self::world::World;
pub use self::world_ref::{InnerWorldRef, InnerWorldRefMut, WorldRef};

mod area;
pub mod block;
mod chunk;
mod grid;
mod mesh;
pub mod presets;
mod viewer;
mod world;
mod world_ref;
