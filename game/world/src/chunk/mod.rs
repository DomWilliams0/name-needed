pub use slab::DeepClone;

pub use self::builder::{ChunkBuilder, ChunkDescriptor};
pub use self::chunk::{Chunk, ChunkId};
pub use self::terrain::{BaseTerrain, BlockDamageResult, OcclusionChunkUpdate};
pub(crate) use self::terrain::{RawChunkTerrain, WhichChunk};

mod builder;

#[allow(clippy::module_inception)]
mod chunk;

mod double_sided_vec;
pub(crate) mod slab;
pub(crate) mod slice;
mod terrain;
