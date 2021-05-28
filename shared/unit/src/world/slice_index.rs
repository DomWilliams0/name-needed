use common::derive_more::*;
use common::*;

use crate::world::{SlabIndex, SLAB_SIZE};
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// A slice in the world
pub type GlobalSliceIndex = SliceIndex<Chunk>;

/// A slice in a single slab
pub type LocalSliceIndex = SliceIndex<Slab>;

pub trait SliceIndexScale {
    /// Must be valid
    const MIN: i32;
    /// Must be valid
    const MAX: i32;
}

/// A slice in the world
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Into, From)]
pub struct Chunk;

/// A slice in a single slab
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Into, From)]
pub struct Slab;

impl SliceIndexScale for Chunk {
    const MIN: i32 = i32::MIN;
    const MAX: i32 = i32::MAX;
}
impl SliceIndexScale for Slab {
    const MIN: i32 = 0;
    const MAX: i32 = SLAB_SIZE.as_i32() - 1;
}

/// A slice of blocks in a chunk, z coordinate
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Into, From)]
pub struct SliceIndex<S: SliceIndexScale>(i32, PhantomData<S>);

impl<S: SliceIndexScale> SliceIndex<S> {
    pub fn abs(mut self) -> Self {
        self.0 = self.0.abs();
        self
    }

    #[inline]
    pub fn slice(self) -> i32 {
        self.0
    }

    pub fn new(slice: i32) -> Self {
        // TODO return option and have unchecked version
        assert!(slice >= S::MIN, "slice {} is invalid for its scale", slice);
        assert!(slice <= S::MAX, "slice {} is invalid for its scale", slice);

        Self(slice, PhantomData)
    }

    /// Last valid slice index
    pub fn top() -> Self {
        Self::new(S::MAX)
    }
    pub fn bottom() -> Self {
        Self::new(S::MIN)
    }

    pub fn range() -> impl Iterator<Item = Self> {
        (S::MIN..=S::MAX).map(Self::new)
    }

    pub fn try_from(slice: i32) -> Option<Self> {
        (S::MIN..=S::MAX).contains(&slice).as_some(Self::new(slice))
    }
}

impl SliceIndex<Chunk> {
    pub fn to_local(self) -> LocalSliceIndex {
        let mut idx = self.0;
        idx %= SLAB_SIZE.as_i32(); // cap at slab size
        if idx.is_negative() {
            // negative slices flip
            idx += SLAB_SIZE.as_i32();
        }
        LocalSliceIndex::new(idx)
    }

    pub fn slab_index(self) -> SlabIndex {
        SlabIndex(self.slice().div_euclid(SLAB_SIZE.as_i32()))
    }
}

impl SliceIndex<Slab> {
    pub fn to_global(self, slab: SlabIndex) -> GlobalSliceIndex {
        let z_offset = slab * SLAB_SIZE;
        GlobalSliceIndex::new(z_offset.as_i32() + self.0)
    }

    /// All slices 0..=SLAB_SIZE-1
    pub fn slices() -> impl DoubleEndedIterator<Item = Self> {
        (Slab::MIN..=Slab::MAX).map(|i| Self(i, PhantomData))
    }
    /// All slices except the last, 0..=SLAB_SIZE-2
    pub fn slices_except_last() -> impl Iterator<Item = Self> {
        (Slab::MIN..Slab::MAX).map(|i| Self(i, PhantomData))
    }

    pub fn slice_unsigned(&self) -> u32 {
        debug_assert!(self.0 >= 0);
        self.0 as u32
    }
}

impl<S: SliceIndexScale> Add<i32> for SliceIndex<S> {
    type Output = Self;

    fn add(self, rhs: i32) -> Self::Output {
        Self::new(self.0 + rhs)
    }
}

impl AddAssign<i32> for GlobalSliceIndex {
    fn add_assign(&mut self, rhs: i32) {
        self.0 += rhs;
    }
}

impl Sub<i32> for GlobalSliceIndex {
    type Output = Self;

    fn sub(self, rhs: i32) -> Self::Output {
        Self::new(self.0 - rhs)
    }
}

impl<S: SliceIndexScale> SubAssign<i32> for SliceIndex<S> {
    fn sub_assign(&mut self, rhs: i32) {
        *self = Self::new(self.0 - rhs);
    }
}

impl<S: SliceIndexScale> Sub<Self> for SliceIndex<S> {
    type Output = Self;

    fn sub(self, rhs: SliceIndex<S>) -> Self::Output {
        Self::new(self.0 - rhs.0)
    }
}

impl From<i32> for GlobalSliceIndex {
    fn from(slice: i32) -> Self {
        Self::new(slice)
    }
}

impl From<i32> for LocalSliceIndex {
    fn from(slice: i32) -> Self {
        Self::new(slice)
    }
}

impl From<SliceIndex<Chunk>> for SlabIndex {
    fn from(slice: SliceIndex<Chunk>) -> Self {
        slice.slab_index()
    }
}

impl Debug for SliceIndex<Chunk> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("GlobalSliceIndex").field(&self.0).finish()
    }
}

impl Debug for SliceIndex<Slab> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("LocalSliceIndex").field(&self.0).finish()
    }
}
