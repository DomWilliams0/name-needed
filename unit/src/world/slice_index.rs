use misc::derive_more::*;
use misc::*;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

use crate::world::{SlabIndex, SLAB_SIZE};
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// A slice in the world
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct GlobalSliceIndex(i32);

/// A slice in a slab
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct LocalSliceIndex(u8);

pub trait SliceIndex: Sized {
    type Inner: Copy;
    const MIN: Self::Inner;
    const MAX: Self::Inner;

    /// Last valid slice index
    fn top() -> Self {
        Self::new_srsly_unchecked(Self::MAX)
    }
    fn bottom() -> Self {
        Self::new_srsly_unchecked(Self::MIN)
    }

    fn slice(self) -> Self::Inner;
    fn new_srsly_unchecked(slice: Self::Inner) -> Self;
}

impl SliceIndex for GlobalSliceIndex {
    type Inner = i32;
    const MIN: Self::Inner = i32::MIN;
    const MAX: Self::Inner = i32::MAX;

    fn slice(self) -> Self::Inner {
        self.0
    }
    fn new_srsly_unchecked(slice: Self::Inner) -> Self {
        Self(slice)
    }
}

impl SliceIndex for LocalSliceIndex {
    type Inner = u8;
    const MIN: Self::Inner = 0;
    const MAX: Self::Inner = SLAB_SIZE.as_u8() - 1;

    fn slice(self) -> Self::Inner {
        self.0
    }
    fn new_srsly_unchecked(slice: Self::Inner) -> Self {
        Self(slice)
    }
}

impl GlobalSliceIndex {
    pub fn new(slice: i32) -> Self {
        Self(slice)
    }

    pub fn to_local(self) -> LocalSliceIndex {
        let mut idx = self.0;
        idx %= SLAB_SIZE.as_i32(); // cap at slab size
        if idx.is_negative() {
            // negative slices flip
            idx += SLAB_SIZE.as_i32();
        }

        LocalSliceIndex::new_srsly_unchecked(idx as u8)
    }

    pub fn slab_index(self) -> SlabIndex {
        SlabIndex(self.slice().div_euclid(SLAB_SIZE.as_i32()))
    }

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }
}

impl LocalSliceIndex {
    /// None if out of range for scale
    pub fn new(slice: impl TryInto<u8>) -> Option<Self> {
        slice.try_into().ok().and_then(|s| {
            (<Self as SliceIndex>::MIN..=<Self as SliceIndex>::MAX)
                .contains(&s)
                .then_some(Self(s))
        })
    }

    /// Panics if out of range for scale
    pub fn new_unchecked(slice: impl TryInto<u8> + Display + Copy) -> Self {
        Self::new(slice).unwrap_or_else(|| panic!("slice {} is invalid for its scale", slice))
    }

    pub fn to_global(self, slab: SlabIndex) -> GlobalSliceIndex {
        let z_offset = slab * SLAB_SIZE;
        GlobalSliceIndex(z_offset.as_i32() + self.0 as i32)
    }

    /// All slices 0..=SLAB_SIZE-1
    pub fn slices() -> impl Iterator<Item = Self> + DoubleEndedIterator + ExactSizeIterator {
        (0..SLAB_SIZE.as_u8()).map(Self)
    }
    /// All slices except the last, 0..=SLAB_SIZE-2
    pub fn slices_except_last() -> impl Iterator<Item = LocalSliceIndexBelowTop> + ExactSizeIterator
    {
        (0..SLAB_SIZE.as_u8() - 1).map(|s| LocalSliceIndexBelowTop(Self(s)))
    }

    pub fn slice_unsigned(self) -> u32 {
        self.0 as u32
    }

    pub fn range() -> impl Iterator<Item = Self> {
        (<Self as SliceIndex>::MIN..=<Self as SliceIndex>::MAX).map(Self)
    }

    pub fn second_from_bottom() -> Self {
        Self(1)
    }
}

impl Display for LocalSliceIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Slice index in range 0..MAX, so a single z+1 operation is infallible
#[derive(Copy, Clone)]
pub struct LocalSliceIndexBelowTop(LocalSliceIndex);

impl LocalSliceIndexBelowTop {
    pub fn current(self) -> LocalSliceIndex {
        self.0
    }

    pub fn above(self) -> LocalSliceIndex {
        let above = (self.0).0 + 1;
        debug_assert!(above < SLAB_SIZE.as_u8());
        SliceIndex::new_srsly_unchecked(above)
    }
}

// TODO ideally handle global slice integer overflow, although unlikely
impl Add<i32> for GlobalSliceIndex {
    type Output = Self;

    fn add(self, rhs: i32) -> Self::Output {
        Self(self.0 + rhs)
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

impl SubAssign<i32> for GlobalSliceIndex {
    fn sub_assign(&mut self, rhs: i32) {
        *self = Self::new(self.0 - rhs);
    }
}

impl Sub<Self> for GlobalSliceIndex {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.0 - rhs.0)
    }
}

impl From<i32> for GlobalSliceIndex {
    fn from(slice: i32) -> Self {
        Self::new(slice)
    }
}

impl From<GlobalSliceIndex> for SlabIndex {
    fn from(slice: GlobalSliceIndex) -> Self {
        slice.slab_index()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sz() {
        assert_eq!(std::mem::size_of::<GlobalSliceIndex>(), 4);
        assert_eq!(std::mem::size_of::<LocalSliceIndex>(), 1);
    }
}
