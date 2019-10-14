mod boundary;
pub(crate) mod discovery;
mod graph;

/// Area index in a slab. 0 is uninitialized, starts at 1
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct SlabAreaIndex(pub u8);

// TODO these could probably do with better names

/// An area in a chunk
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(crate) struct SlabArea {
    pub slab: SlabIndex,
    pub area: SlabAreaIndex,
}

/// An area in the world
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct Area {
    pub chunk: ChunkPosition,
    pub slab: SlabIndex,
    pub area: SlabAreaIndex,
}

pub use boundary::ChunkBoundary;
use crate::chunk::slab::SlabIndex;
use crate::ChunkPosition;
pub use graph::AreaGraph;

impl SlabAreaIndex {
    pub const UNINITIALIZED: SlabAreaIndex = SlabAreaIndex(0);
    pub const FIRST: SlabAreaIndex = SlabAreaIndex(1);

    pub fn initialized(self) -> bool {
        self.0 != 0
    }

    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

impl SlabArea {
    pub fn into_area(self, chunk_pos: ChunkPosition) -> Area {
        Area {
            chunk: chunk_pos,
            slab: self.slab,
            area: self.area,
        }
    }
}
