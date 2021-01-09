use common::derive_more::{From, Into};
use common::*;

use crate::world::{SlabIndex, SlabLocation, WorldPosition, CHUNK_SIZE};
use std::convert::From;
use std::ops::{Add, Sub};

/// Location of a chunk in the world
#[derive(Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Into, From)]
pub struct ChunkLocation(pub i32, pub i32);

impl ChunkLocation {
    pub fn get_slab(&self, slab: impl Into<SlabIndex>) -> SlabLocation {
        SlabLocation {
            chunk: *self,
            slab: slab.into(),
        }
    }

    pub const MIN: Self = ChunkLocation(i32::MIN, i32::MIN);
    pub const MAX: Self = ChunkLocation(i32::MAX, i32::MAX);
}

impl From<WorldPosition> for ChunkLocation {
    fn from(wp: WorldPosition) -> Self {
        let WorldPosition(x, y, _) = wp;
        ChunkLocation(
            x.div_euclid(CHUNK_SIZE.as_i32()),
            y.div_euclid(CHUNK_SIZE.as_i32()),
        )
    }
}

impl Add<(i32, i32)> for ChunkLocation {
    type Output = Self;

    fn add(self, rhs: (i32, i32)) -> Self::Output {
        Self(self.0 + rhs.0, self.1 + rhs.1)
    }
}
impl Add<(i16, i16)> for ChunkLocation {
    type Output = Self;

    fn add(self, rhs: (i16, i16)) -> Self::Output {
        Self(self.0 + rhs.0 as i32, self.1 + rhs.1 as i32)
    }
}

impl Debug for ChunkLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "[{}, {}]", self.0, self.1)
    }
}

impl Sub<Self> for ChunkLocation {
    type Output = Self;

    fn sub(self, rhs: ChunkLocation) -> Self::Output {
        Self(self.0 - rhs.0, self.1 - rhs.1)
    }
}

slog_value_debug!(ChunkLocation);
slog_kv_debug!(ChunkLocation, "chunk");
