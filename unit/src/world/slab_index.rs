use crate::dim::SmallUnsignedConstant;
use crate::world::{GlobalSliceIndex, SLAB_SIZE};
use common::{derive_more::*, *};
use newtype_derive::*;
use std::ops::{Div, Mul};

#[derive(
    Debug,
    Default,
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
/// Index of a slab in a chunk
pub struct SlabIndex(pub i32);

impl SlabIndex {
    pub const MIN: SlabIndex = SlabIndex(i32::MIN);
    pub const MAX: SlabIndex = SlabIndex(i32::MAX);

    pub const fn as_i32(self) -> i32 {
        self.0
    }

    /// Bottom block of slab as global slice
    pub fn as_slice(self) -> GlobalSliceIndex {
        GlobalSliceIndex::new(self.0 * SLAB_SIZE.as_i32())
    }

    /// [bottom slice, top slice)
    pub fn slice_range(self) -> (GlobalSliceIndex, GlobalSliceIndex) {
        let bottom = self.as_slice();
        let top = bottom + SLAB_SIZE.as_i32();
        (bottom, top)
    }

    pub fn try_add(self, dz: i32) -> Option<Self> {
        self.0.checked_add(dz).map(Self)
    }
}

NewtypeAdd! {(i32) pub struct SlabIndex(i32);}
NewtypeSub! {(i32) pub struct SlabIndex(i32);}
NewtypeMul! {(i32) pub struct SlabIndex(i32);}
NewtypeDeref! {() pub struct SlabIndex(i32);}

impl Mul<SmallUnsignedConstant> for SlabIndex {
    type Output = Self;

    fn mul(self, rhs: SmallUnsignedConstant) -> Self::Output {
        self * rhs.as_i32()
    }
}

impl Div<SmallUnsignedConstant> for SlabIndex {
    type Output = Self;

    fn div(self, rhs: SmallUnsignedConstant) -> Self::Output {
        SlabIndex(self.0 / rhs.as_i32())
    }
}

impl From<SlabIndex> for f32 {
    fn from(SlabIndex(slab): SlabIndex) -> Self {
        slab as f32
    }
}

slog_value_debug!(SlabIndex);
slog_kv_debug!(SlabIndex, "slab");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::SLAB_SIZE;

    fn check(slab: i32, slice: i32) {
        let slab = SlabIndex(slab);
        assert_eq!(slab.as_slice().slice(), slice);
        assert_eq!(slab.as_slice().slab_index(), slab);
    }

    #[test]
    fn slab_index_to_slice() {
        // (slab, slice)
        check(0, 0);
        check(1, SLAB_SIZE.as_i32());
        check(4, SLAB_SIZE.as_i32() * 4);
        check(-2, SLAB_SIZE.as_i32() * -2);
    }
}
