// #![allow(dead_code)]
#![deny(clippy::missing_safety_doc, unused_must_use)]

pub use petgraph::prelude::NodeIndex;

pub use self::chunk::{
    flatten_coords, slice_navmesh::ABSOLUTE_MAX_FREE_VERTICAL_SPACE, unflatten_index, AreaInfo,
    BlockDamageResult, Chunk, ChunkBuilder, ChunkDescriptor, DeepClone, Slab, SlabGrid,
    SlabGridImpl,
};
pub use self::context::{
    BlockType, GeneratedTerrainSource, NopGeneratedTerrainSource, WorldContext, SLICE_SIZE,
};
pub use self::mesh::BaseVertex;
pub use self::navigation::{EdgeCost, NavigationError, SearchGoal, WorldArea, WorldPath};
pub use self::navigationv2::{
    world_graph::{Path, SearchError, SearchResultFuture, WorldArea as WorldAreaV2},
    NavRequirement,
};
pub use self::viewer::{SliceRange, WorldViewer};
pub use self::world::{helpers, ExplorationFilter, ExplorationResult, World, WorldChangeEvent};
pub use self::world_ref::{InnerWorldRef, InnerWorldRefMut, WorldRef};
pub use occlusion::{BlockOcclusion, OcclusionFace};
pub use ray::{VoxelRay, VoxelRayOutput};

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

pub fn iter_slice_xy() -> impl Iterator<Item = unit::world::SliceBlock> {
    use misc::Itertools;
    use unit::world::{SliceBlock, CHUNK_SIZE};

    (0..CHUNK_SIZE.as_block_coord())
        .cartesian_product(0..CHUNK_SIZE.as_block_coord())
        .map(|(y, x)| SliceBlock::new_srsly_unchecked(x, y))
}
