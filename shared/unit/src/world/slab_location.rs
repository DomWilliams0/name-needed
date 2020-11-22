use crate::world::{ChunkLocation, SlabIndex};
use common::derive_more::{From, Into};
use common::*;

/// A slab in the world
#[derive(Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Into, From, Debug)]
pub struct SlabLocation {
    pub chunk: ChunkLocation,
    pub slab: SlabIndex,
}

slog_value_debug!(SlabLocation);
slog_kv_debug!(SlabLocation, "slab");
