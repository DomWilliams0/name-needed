pub use slab::DeepClone;

pub use self::builder::{ChunkBuilder, ChunkDescriptor, WorldBuilder};
pub use self::chunk::{AreaInfo, Chunk, ChunkId, SlabAvailability, SlabThingOrWait};
pub use self::slab::{Slab, SlabGrid, SlabGridImpl, SlabType};
pub use self::slice::{flatten_coords, unflatten_index};
pub use self::terrain::{
    BlockDamageResult, NeighbourAreaHash, OcclusionChunkUpdate, SlabNeighbour, SparseGrid,
    SparseGridExtension,
};
pub(crate) use self::terrain::{SlabData, SlabStorage};

mod builder;

#[allow(clippy::module_inception)]
mod chunk;

mod double_sided_vec;
pub(crate) mod slab;
pub(crate) mod slice;
pub(crate) mod slice_navmesh;
mod terrain;
