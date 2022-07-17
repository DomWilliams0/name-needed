use misc::derive_more::{From, Into};
use misc::*;

use crate::world::{GlobalSliceIndex, SlabIndex, SlabLocation, WorldPosition, CHUNK_SIZE};
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

    pub const fn x(&self) -> i32 {
        self.0
    }

    pub const fn y(&self) -> i32 {
        self.1
    }

    pub const fn xy(&self) -> (i32, i32) {
        (self.0, self.1)
    }

    /// Inclusive
    pub fn iter_until(self, other: Self) -> impl Iterator<Item = ChunkLocation> {
        let ChunkLocation(x0, y0) = self;
        let ChunkLocation(x1, y1) = other;

        let iter = (x0..=x1)
            .cartesian_product(y0..=y1)
            .map(|(x, y)| ChunkLocation(x, y));

        debug_assert!(iter.clone().count() > 0, "chunk range is empty");

        iter
    }

    pub fn get_block(self, z: impl Into<GlobalSliceIndex>) -> WorldPosition {
        WorldPosition(
            self.0 * CHUNK_SIZE.as_i32(),
            self.1 * CHUNK_SIZE.as_i32(),
            z.into(),
        )
    }

    pub fn try_add(self, (dx, dy): (i32, i32)) -> Option<Self> {
        match (self.0.checked_add(dx), self.1.checked_add(dy)) {
            (Some(x), Some(y)) => Some(Self(x, y)),
            _ => None,
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
