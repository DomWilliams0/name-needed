#![allow(dead_code)]
#![deny(clippy::missing_safety_doc)]

pub use petgraph::prelude::NodeIndex;

pub use self::chunk::{
    flatten_coords, slice_navmesh::ABSOLUTE_MAX_FREE_VERTICAL_SPACE, unflatten_index,
    BlockDamageResult, Chunk, ChunkBuilder, ChunkDescriptor, DeepClone, OcclusionChunkUpdate, Slab,
    SlabType,
};
pub use self::context::{
    BlockType, GeneratedTerrainSource, NopGeneratedTerrainSource, WorldContext, SLICE_SIZE,
};
pub use self::mesh::BaseVertex;
pub use self::navigation::{EdgeCost, NavigationError, SearchGoal, WorldArea, WorldPath};
pub use self::navigationv2::world_graph::{
    Path, SearchError, SearchResultFuture, WorldArea as WorldAreaV2,
};
pub use self::viewer::{SliceRange, WorldViewer};
pub use self::world::{helpers, ExplorationFilter, ExplorationResult, World, WorldChangeEvent};
pub use self::world_ref::{InnerWorldRef, InnerWorldRefMut, WorldRef};
pub use occlusion::{BlockOcclusion, OcclusionFace};
pub use ray::VoxelRay;

pub mod block;
mod chunk;
mod context;
pub mod loader;
mod mesh;
#[deprecated]
mod navigation;
mod navigationv2;
mod neighbour;
mod occlusion;
pub mod presets;
mod ray;
mod viewer;
mod world;
mod world_ref;
