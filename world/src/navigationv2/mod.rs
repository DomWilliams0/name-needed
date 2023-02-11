use crate::chunk::slice_navmesh::SliceAreaIndex;
use unit::world::{LocalSliceIndex, SlabIndex};

/// Area within a slab
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct SlabArea {
    pub slice_idx: LocalSliceIndex, // TODO change to u8 from i32
    pub slice_area: SliceAreaIndex,
}

/// Area within a chunk
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ChunkArea {
    pub slab_idx: SlabIndex,
    pub slab_area: SlabArea,
}
