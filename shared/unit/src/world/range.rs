use common::num_traits::*;

use crate::world::{
    BlockCoord, BlockPosition, LocalSliceIndex, SlabPosition, WorldPoint, WorldPosition,
};
use std::ops::{Mul, SubAssign};

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum WorldRange<P: RangePosition> {
    /// Single block
    Single(P),

    /// Range [from, to)
    Range(P, P),
}

pub trait RangePosition: Copy + Sized {
    type XY: Num + Copy + PartialOrd + PartialEq + SubAssign + Mul + AsPrimitive<Self::Count>;
    type Z: Num + Copy + PartialOrd + PartialEq + SubAssign + Mul + AsPrimitive<Self::Count>;
    type Count: Num + Copy + 'static;
    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z);
    fn new(xyz: (Self::XY, Self::XY, Self::Z)) -> Self;

    fn below(self) -> Self {
        let (x, y, z) = self.xyz();
        Self::new((x, y, z - Self::Z::one()))
    }
}

impl<P: RangePosition> WorldRange<P> {
    /// Inclusive
    pub fn bounds(&self) -> (P, P) {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();
        (P::new((ax, ay, az)), P::new((bx, by, bz)))
    }

    /// Exclusive
    pub fn bounds_exclusive(&self) -> (P, P) {
        let ((ax, mut bx), (ay, mut by), (az, mut bz)) = self.ranges();
        if let WorldRange::Range(_, _) = self {
            bx -= P::XY::one();
            by -= P::XY::one();
            bz -= P::Z::one();
        }

        (P::new((ax, ay, az)), P::new((bx, by, bz)))
    }

    /// (min x, max x), (min y, max y), (min z, max z)
    pub fn ranges(&self) -> ((P::XY, P::XY), (P::XY, P::XY), (P::Z, P::Z)) {
        let (from, to) = match self {
            WorldRange::Single(pos) => (pos, pos),
            WorldRange::Range(from, to) => (from, to),
        };

        let (ax, ay, az) = from.xyz();
        let (bx, by, bz) = to.xyz();

        let (ax, bx) = if ax < bx { (ax, bx) } else { (bx, ax) };
        let (ay, by) = if ay < by { (ay, by) } else { (by, ay) };
        let (az, bz) = if az < bz { (az, bz) } else { (bz, az) };

        ((ax, bx), (ay, by), (az, bz))
    }

    pub fn count(&self) -> P::Count {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();
        let xy = (bx - ax) * (by - ay);
        let z = bz - az;

        xy.as_() * z.as_()
    }

    pub fn below(self) -> Self {
        match self {
            WorldRange::Single(pos) => WorldRange::Single(pos.below()),
            WorldRange::Range(from, to) => WorldRange::Range(from.below(), to.below()),
        }
    }
}

pub type WorldPositionRange = WorldRange<WorldPosition>;
pub type BlockPositionRange = WorldRange<BlockPosition>;
pub type SlabPositionRange = WorldRange<SlabPosition>;
pub type WorldPointRange = WorldRange<WorldPoint>;

impl RangePosition for WorldPosition {
    type XY = i32;
    type Z = i32;
    type Count = usize;

    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z) {
        (self.0, self.1, self.2.slice())
    }

    fn new(xyz: (Self::XY, Self::XY, Self::Z)) -> Self {
        xyz.into()
    }
}

impl RangePosition for SlabPosition {
    type XY = BlockCoord;
    type Z = i32;
    type Count = usize;

    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z) {
        (self.x(), self.y(), self.z().slice())
    }
    fn new((x, y, z): (Self::XY, Self::XY, Self::Z)) -> Self {
        Self::new(x, y, LocalSliceIndex::new(z))
    }
}

impl RangePosition for BlockPosition {
    type XY = BlockCoord;
    type Z = i32;
    type Count = usize;

    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z) {
        (self.x(), self.y(), self.z().slice())
    }
    fn new(xyz: (Self::XY, Self::XY, Self::Z)) -> Self {
        xyz.into()
    }
}

impl RangePosition for WorldPoint {
    type XY = f32;
    type Z = f32;
    type Count = f64;

    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z) {
        (*self).into()
    }
    fn new(xyz: (Self::XY, Self::XY, Self::Z)) -> Self {
        xyz.into()
    }
}
