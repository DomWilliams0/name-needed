pub use self::builder::{ChunkBuilder, ChunkDescriptor};
pub use self::chunk::{Chunk, ChunkId, BLOCK_COUNT_SLICE};
pub use self::terrain::BaseTerrain;
pub(crate) use self::terrain::{ChunkTerrain, RawChunkTerrain, WhichChunk};

mod builder;

#[allow(clippy::module_inception)]
mod chunk;

mod double_sided_vec;
pub(crate) mod slab;
pub(crate) mod slice;
mod terrain;
