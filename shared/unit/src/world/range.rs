use common::num_traits::*;

use crate::world::{
    BlockCoord, BlockPosition, LocalSliceIndex, SlabPosition, WorldPoint, WorldPosition,
};
use std::ops::{Add, Mul, SubAssign};

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum WorldRange<P: RangePosition> {
    /// Single block
    Single(P),

    /// Inclusive range
    Range(P, P),
}

pub trait RangeNum: Num + Zero + Copy + PartialOrd + PartialEq + SubAssign + Mul {
    fn range_step() -> Self;
}

pub trait RangePosition: Copy + Sized {
    type XY: RangeNum + AsPrimitive<Self::Count>;
    type Z: RangeNum + AsPrimitive<Self::Count>;
    type Count: Num + Copy + 'static;
    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z);
    fn new(xyz: (Self::XY, Self::XY, Self::Z)) -> Self;

    fn below(self) -> Self {
        let (x, y, z) = self.xyz();
        Self::new((x, y, z - Self::Z::one()))
    }
}

impl<P: RangePosition> WorldRange<P> {
    pub fn with_single<I: Into<P>>(pos: I) -> Self {
        Self::Single(pos.into())
    }

    /// `[from, to]`
    pub fn with_inclusive_range<F: Into<P>, T: Into<P>>(from: F, to: T) -> Self {
        Self::Range(from.into(), to.into())
    }

    /// `[from, to)`
    pub fn with_exclusive_range<F: Into<P>, T: Into<P>>(from: F, to: T) -> Self {
        let inclusive = Self::with_inclusive_range(from, to);
        let (min, max) = inclusive.bounds();

        // -1 from max
        let (mut x, mut y, mut z) = max.xyz();
        x -= P::XY::range_step();
        y -= P::XY::range_step();
        z -= P::Z::range_step();

        // limit max to min
        let (min_x, min_y, min_z) = min.xyz();
        if x < min_x {
            x = min_x
        }
        if y < min_y {
            y = min_y
        }
        if z < min_z {
            z = min_z
        }

        Self::Range(min, P::new((x, y, z)))
    }

    /// Inclusive
    pub fn bounds(&self) -> (P, P) {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();
        (P::new((ax, ay, az)), P::new((bx, by, bz)))
    }

    /// (min x, max x), (min y, max y), (min z, max z) inclusive
    #[allow(clippy::type_complexity)]
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

        // b? is guaranteed to be greater than a? from ranges()
        // add range step for inclusive count

        let dxy = {
            let dx = (bx - ax) + P::XY::range_step();
            let dy = (by - ay) + P::XY::range_step();
            dx * dy
        };
        let dz = (bz - az) + P::Z::range_step();

        // replace 0 with 1 for multiplication
        let xy = if dxy.is_zero() { P::XY::one() } else { dxy };
        let z = if dz.is_zero() { P::Z::one() } else { dz };

        xy.as_() * z.as_()
    }

    pub fn below(self) -> Self {
        match self {
            WorldRange::Single(pos) => WorldRange::Single(pos.below()),
            WorldRange::Range(from, to) => WorldRange::Range(from.below(), to.below()),
        }
    }
}

impl<XY: Copy, Z: Copy, P: RangePosition + Add<(XY, XY, Z), Output = P>> Add<(XY, XY, Z)>
    for WorldRange<P>
{
    type Output = Self;

    fn add(self, delta: (XY, XY, Z)) -> Self::Output {
        match self {
            Self::Single(pos) => Self::Single(pos + delta),
            Self::Range(from, to) => Self::Range(from + delta, to + delta),
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

impl RangeNum for i32 {
    fn range_step() -> Self {
        1
    }
}
impl RangeNum for BlockCoord {
    fn range_step() -> Self {
        1
    }
}
impl RangeNum for f32 {
    fn range_step() -> Self {
        // no difference between inclusive and exclusive ranges
        0.0
    }
}

#[cfg(test)]
mod tests {
    use crate::world::{WorldPointRange, WorldPositionRange};
    use common::ApproxEq;

    #[test]
    fn count() {
        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (2, 2, 1));
        assert_eq!(range.count(), 3 * 3 * 2);

        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (1, 1, 0));
        assert_eq!(range.count(), 2 * 2);

        let range = WorldPointRange::with_inclusive_range((0.0, 0.0, 0.0), (0.5, 1.2, 0.0));
        let x = range.count();
        assert!(x.approx_eq(0.5 * 1.2, (0.00001, 2)));
    }

    #[test]
    fn bounds() {
        let range = WorldPositionRange::with_single((5, 5, 5));
        assert_eq!(range.bounds(), ((5, 5, 5).into(), (5, 5, 5).into()));

        let range = WorldPositionRange::with_inclusive_range((2, 2, 2), (0, 0, 0));
        assert_eq!(range.ranges(), ((0, 2), (0, 2), (0, 2)));

        let range = WorldPositionRange::with_exclusive_range((2, 2, 2), (0, 0, 0));
        assert_eq!(range.ranges(), ((0, 1), (0, 1), (0, 1)));

        let range = WorldPositionRange::with_inclusive_range((4, 5, 0), (3, 6, -2));
        assert_eq!(range.ranges(), ((3, 4), (5, 6), (-2, 0)));
        assert_eq!(range.count(), 2 * 2 * 3);
    }
}
