pub use slab::DeepClone;

pub use self::builder::{ChunkBuilder, ChunkDescriptor};
pub(crate) use self::chunk::SlabLoadingStatus;
pub use self::chunk::{Chunk, ChunkId};
pub use self::terrain::{BaseTerrain, BlockDamageResult, OcclusionChunkUpdate};
pub(crate) use self::terrain::{ChunkTerrain, RawChunkTerrain, WhichChunk};
use unit::world::{ChunkLocation, SlabIndex, SlabLocation};

mod builder;

#[allow(clippy::module_inception)]
mod chunk;

mod double_sided_vec;
pub(crate) mod slab;
pub(crate) mod slice;
mod terrain;

/// Inclusive range
pub fn all_slabs_in_range(
    from: SlabLocation,
    to: SlabLocation,
) -> (impl Iterator<Item = SlabLocation> + Clone, usize) {
    use common::Itertools;
    use std::iter::repeat;

    let SlabLocation {
        slab: SlabIndex(min_slab),
        chunk: ChunkLocation(min_chunk_x, min_chunk_y),
    } = from;

    let SlabLocation {
        slab: SlabIndex(max_slab),
        chunk: ChunkLocation(max_chunk_x, max_chunk_y),
    } = to;

    assert!(min_slab < max_slab && min_chunk_x < max_chunk_x && min_chunk_y < max_chunk_y);

    let chunks = (min_chunk_x..=max_chunk_x).cartesian_product(min_chunk_y..=max_chunk_y);
    let all_slabs = chunks
        .flat_map(move |chunk| {
            let slabs = min_slab..=max_slab;
            slabs.zip(repeat(chunk))
        })
        .map(|(slab, chunk)| SlabLocation::new(slab, chunk));

    let slab_count = max_slab - min_slab + 1;
    let chunk_count = (max_chunk_x - min_chunk_x + 1) * (max_chunk_y - min_chunk_y + 1);

    let full_count = slab_count as usize * chunk_count as usize;

    if cfg!(debug_assertions) {
        let actual_count = all_slabs.clone().count();
        assert_eq!(
            actual_count, full_count,
            "count is wrong, should be {} but is {}",
            full_count, actual_count
        );
    }

    (all_slabs, full_count)
}
