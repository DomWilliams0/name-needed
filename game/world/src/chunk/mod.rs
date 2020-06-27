pub use slab_pointer::DeepClone;

pub use self::builder::{ChunkBuilder, ChunkDescriptor};
pub use self::chunk::{Chunk, ChunkId, BLOCK_COUNT_SLICE};
pub use self::terrain::{BaseTerrain, BlockDamageResult, OcclusionChunkUpdate};
pub(crate) use self::terrain::{ChunkTerrain, RawChunkTerrain, WhichChunk};

mod builder;

#[allow(clippy::module_inception)]
mod chunk;

mod double_sided_vec;
pub(crate) mod slab;
pub(crate) mod slab_pointer;
pub(crate) mod slice;
mod terrain;
