use crate::dim::SmallUnsignedConstant;
use common::{derive_more::*, *};
use newtype_derive::*;
use std::ops::{Div, Mul};

/// Index of a slab in a chunk
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
pub struct SlabIndex(pub i32);

impl SlabIndex {
    pub const MIN: SlabIndex = SlabIndex(i32::MIN);
    pub const MAX: SlabIndex = SlabIndex(i32::MAX);

    pub const fn as_i32(self) -> i32 {
        self.0
    }

    pub fn floored(float: f32) -> Self {
        Self(float.floor() as i32)
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
