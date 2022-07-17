use crate::world::{ChunkLocation, SlabIndex};
use misc::derive_more::{From, Into};
use misc::*;

/// A slab in the world
#[derive(Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Into, From)]
pub struct SlabLocation {
    pub chunk: ChunkLocation,
    pub slab: SlabIndex,
}

impl SlabLocation {
    pub fn new<S: Into<SlabIndex>, C: Into<ChunkLocation>>(slab: S, chunk: C) -> Self {
        SlabLocation {
            chunk: chunk.into(),
            slab: slab.into(),
        }
    }

    pub fn below(mut self) -> Self {
        self.slab.0 -= 1;
        self
    }
}

impl Debug for SlabLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}, {}]", self.chunk.0, self.chunk.1, self.slab.0)
    }
}

impl Display for SlabLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[slab {} in chunk {:?}]", self.slab.as_i32(), self.chunk)
    }
}

/// Inclusive range. Is sorted by chunk then slab
pub fn all_slabs_in_range(
    from: SlabLocation,
    to: SlabLocation,
) -> (impl Iterator<Item = SlabLocation> + Clone, usize) {
    use std::iter::repeat;

    let SlabLocation {
        slab: SlabIndex(min_slab),
        chunk: ChunkLocation(min_chunk_x, min_chunk_y),
    } = from;

    let SlabLocation {
        slab: SlabIndex(max_slab),
        chunk: ChunkLocation(max_chunk_x, max_chunk_y),
    } = to;

    assert!(min_slab <= max_slab && min_chunk_x <= max_chunk_x && min_chunk_y <= max_chunk_y);

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

slog_value_display!(SlabLocation);
slog_kv_display!(SlabLocation, "slab");
