use crate::world::{ChunkLocation, SlabIndex};
use common::derive_more::{From, Into};
use common::*;

/// A slab in the world
#[derive(Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Into, From, Debug)]
pub struct SlabLocation {
    pub chunk: ChunkLocation,
    pub slab: SlabIndex,
}

impl Display for SlabLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[slab {} in chunk {:?}]", self.slab.as_i32(), self.chunk)
    }
}

slog_value_display!(SlabLocation);
slog_kv_display!(SlabLocation, "slab");
