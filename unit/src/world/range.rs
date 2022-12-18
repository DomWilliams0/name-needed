use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, SubAssign};

use misc::num_traits::{AsPrimitive, Num, One, Zero};
use misc::*;

use crate::world::{BlockCoord, BlockPosition, CHUNK_SIZE, GlobalSliceIndex, LocalSliceIndex, SlabLocation, SlabPosition, WorldPoint, WorldPosition};

#[derive(Clone, Debug, PartialOrd)]
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

    /// None if invalid coords
    fn new(xyz: (Self::XY, Self::XY, Self::Z)) -> Option<Self>;

    /// Panics if invalid coords
    fn new_unchecked(xyz: (Self::XY, Self::XY, Self::Z)) -> Self;

    fn below(self) -> Option<Self> {
        let (x, y, z) = self.xyz();
        Self::new((x, y, z - Self::Z::one()))
    }

    fn above(self) -> Option<Self> {
        let (x, y, z) = self.xyz();
        Self::new((x, y, z + Self::Z::one()))
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

        let max = P::new_unchecked((x, y, z)); // derived from valid `P`
        Self::Range(min, max)
    }

    /// Inclusive
    pub fn bounds(&self) -> (P, P) {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();
        // derived from valid `P`s
        (
            P::new_unchecked((ax, ay, az)),
            P::new_unchecked((bx, by, bz)),
        )
    }

    /// (min x, max x), (min y, max y), (min z, max z) inclusive
    #[allow(clippy::type_complexity)]
    pub fn ranges(&self) -> ((P::XY, P::XY), (P::XY, P::XY), (P::Z, P::Z)) {
        // TODO fix on creation so no need to redo this every time
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

    pub fn below(&self) -> Option<Self> {
        match self {
            WorldRange::Single(pos) => pos.below().map(WorldRange::Single),
            WorldRange::Range(from, to) => from
                .below()
                .zip(to.below())
                .map(|(from, to)| WorldRange::Range(from, to)),
        }
    }

    pub fn above(&self) -> Option<Self> {
        match self {
            WorldRange::Single(pos) => pos.above().map(WorldRange::Single),
            WorldRange::Range(from, to) => from
                .above()
                .zip(to.above())
                .map(|(from, to)| WorldRange::Range(from, to)),
        }
    }

    pub fn contains(&self, pos: &P) -> bool {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();
        let (x, y, z) = pos.xyz();

        ax <= x && x <= bx && ay <= y && y <= by && az <= z && z <= bz
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

    fn new(xyz: (Self::XY, Self::XY, Self::Z)) -> Option<Self> {
        Some(xyz.into())
    }

    fn new_unchecked(xyz: (Self::XY, Self::XY, Self::Z)) -> Self {
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

    fn new((x, y, z): (Self::XY, Self::XY, Self::Z)) -> Option<Self> {
        LocalSliceIndex::new(z).and_then(|z| Self::new(x, y, z))
    }

    fn new_unchecked((x, y, z): (Self::XY, Self::XY, Self::Z)) -> Self {
        Self::new_unchecked(x, y, LocalSliceIndex::new_unchecked(z))
    }
}

impl RangePosition for BlockPosition {
    type XY = BlockCoord;
    type Z = i32;
    type Count = usize;

    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z) {
        (self.x(), self.y(), self.z().slice())
    }

    fn new((x, y, z): (Self::XY, Self::XY, Self::Z)) -> Option<Self> {
        Self::new(x, y, GlobalSliceIndex::new(z))
    }

    fn new_unchecked((x, y, z): (Self::XY, Self::XY, Self::Z)) -> Self {
        Self::new_unchecked(x, y, GlobalSliceIndex::new(z))
    }
}

impl RangePosition for WorldPoint {
    type XY = f32;
    type Z = f32;
    type Count = f64;

    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z) {
        (*self).into()
    }

    fn new((x, y, z): (Self::XY, Self::XY, Self::Z)) -> Option<Self> {
        Self::new(x, y, z)
    }

    fn new_unchecked((x, y, z): (Self::XY, Self::XY, Self::Z)) -> Self {
        Self::new_unchecked(x, y, z)
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

impl<P: RangePosition + Eq> Eq for WorldRange<P> {}
impl<P: RangePosition + Hash> Hash for WorldRange<P> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            WorldRange::Single(a) => a.hash(state),
            WorldRange::Range(a, b) => {
                a.hash(state);
                b.hash(state);
            }
        }
    }
}

impl<P: RangePosition + PartialEq> PartialEq for WorldRange<P> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (WorldRange::Single(a0), WorldRange::Single(b0)) => a0 == b0,
            (WorldRange::Range(a0, a1), WorldRange::Range(b0, b1)) => a0 == b0 && a1 == b1,
            _ => false,
        }
    }
}
impl<P: RangePosition + Display> Display for WorldRange<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldRange::Single(b) => write!(f, "{}", b),
            WorldRange::Range(from, to) => write!(f, "({} -> {})", from, to),
        }
    }
}

impl WorldRange<WorldPosition> {
    pub fn iter_blocks(&self) -> impl Iterator<Item = WorldPosition> {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();
        (az..=bz)
            .cartesian_product(ay..=by)
            .cartesian_product(ax..=bx)
            .map(move |((z, y), x)| (x, y, z).into())
    }

    pub fn iter_columns(&self) -> impl Iterator<Item = (i32, i32)> {
        let ((ax, bx), (ay, by), (_, _)) = self.ranges();
        (ay..=by)
            .cartesian_product(ax..=bx)
            .map(move |(y, x)| (x, y))
    }

    /// 2d outline in xy, keeps full z range
    pub fn iter_outline(&self) -> Option<impl Iterator<Item = Self> + '_> {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();

        let w = bx - ax;
        let h = by - ay;

        // too narrow
        if w < 2 || h < 2 {
            return None;
        }

        Some(
            [
                WorldPositionRange::with_inclusive_range((ax, ay, az), (bx, ay, bz)),
                WorldPositionRange::with_inclusive_range((ax, ay + 1, az), (ax, by, bz)),
                WorldPositionRange::with_inclusive_range((bx, ay + 1, az), (bx, by, bz)),
                WorldPositionRange::with_inclusive_range((ax + 1, by, az), (bx - 1, by, bz)),
            ]
            .into_iter(),
        )
    }

    pub fn contains_slab(&self, slab: SlabLocation) -> bool {
        let (bottom, top) = slab.slab.slice_range();
        let min = BlockPosition::new_unchecked(0, 0, bottom).to_world_position(slab.chunk);
        let max = BlockPosition::new_unchecked(CHUNK_SIZE.as_block_coord(), CHUNK_SIZE.as_block_coord(), top).to_world_position(slab.chunk);

        true // TODO FIXME
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use misc::{ApproxEq, Itertools};

    use crate::world::{WorldPoint, WorldPointRange, WorldPositionRange};

    #[test]
    fn count() {
        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (2, 2, 1));
        assert_eq!(range.count(), 3 * 3 * 2);

        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (1, 1, 0));
        assert_eq!(range.count(), 2 * 2);

        let range = WorldPointRange::with_inclusive_range(
            WorldPoint::new_unchecked(0.0, 0.0, 0.0),
            WorldPoint::new_unchecked(0.5, 1.2, 0.0),
        );
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

    #[test]
    fn outline_no_overlap() {
        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (3, 3, 3));
        let outline_blocks = range
            .iter_outline()
            .unwrap()
            .flat_map(|range| range.iter_blocks())
            .collect_vec();

        assert_eq!(
            outline_blocks.iter().cloned().collect::<HashSet<_>>().len(),
            outline_blocks.len()
        );
    }

    #[test]
    fn outline_all_within() {
        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (2, 2, 3));
        let outline_blocks = range
            .iter_outline()
            .unwrap()
            .flat_map(|range| range.iter_blocks())
            .collect_vec();

        assert!(outline_blocks.iter().all(|b| range.contains(b)));
    }

    #[test]
    fn outline_too_narrow() {
        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (1, 1, 3));
        assert!(range.iter_outline().is_none());

        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (1, 2, 1));
        assert!(range.iter_outline().is_none());

        let range = WorldPositionRange::with_inclusive_range((0, 0, 0), (2, 2, 0));
        let outline = range
            .iter_outline()
            .expect("smallest possible")
            .collect_vec();
        assert_eq!(outline.iter().flat_map(|r| r.iter_blocks()).count(), 8);
    }
}
