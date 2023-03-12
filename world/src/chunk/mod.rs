pub use slab::DeepClone;

pub use self::builder::{ChunkBuilder, ChunkDescriptor, WorldBuilder};
pub use self::chunk::{Chunk, ChunkId, VerticalSpaceOrWait};
pub use self::slab::{Slab, SlabType};
pub use self::slice::{flatten_coords, unflatten_index};
pub(crate) use self::terrain::RawChunkTerrain;
pub use self::terrain::{BlockDamageResult, OcclusionChunkUpdate};

mod builder;

#[allow(clippy::module_inception)]
mod chunk;

mod double_sided_vec;
pub(crate) mod slab;
pub(crate) mod slice;
pub(crate) mod slice_navmesh;
mod terrain;
