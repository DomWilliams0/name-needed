use std::ops::{Add, AddAssign, Sub, SubAssign};

use common::derive_more::*;

// TODO differentiate slice in a slab and slice in a chunk
// TODO move slab to unit

/// A slice of blocks in a chunk, z coordinate
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Into,
    From,
    Add,
    AddAssign,
    Sub,
    SubAssign,
)]
pub struct SliceIndex(pub i32);

impl SliceIndex {
    pub const MIN: SliceIndex = Self(std::i32::MIN);
    pub const MAX: SliceIndex = Self(std::i32::MAX);

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }
}

impl Add<i32> for SliceIndex {
    type Output = SliceIndex;

    fn add(self, rhs: i32) -> Self::Output {
        SliceIndex(self.0 + rhs)
    }
}

impl AddAssign<i32> for SliceIndex {
    fn add_assign(&mut self, rhs: i32) {
        self.0 += rhs;
    }
}

impl Sub<i32> for SliceIndex {
    type Output = SliceIndex;

    fn sub(self, rhs: i32) -> Self::Output {
        SliceIndex(self.0 - rhs)
    }
}

impl SubAssign<i32> for SliceIndex {
    fn sub_assign(&mut self, rhs: i32) {
        self.0 -= rhs;
    }
}
