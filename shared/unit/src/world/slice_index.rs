use std::ops::Add;

use derive_more::*;

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
