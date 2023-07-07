use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, SubAssign};

use misc::num_traits::{AsPrimitive, Num, One, Zero};
use misc::*;

use crate::world::{
    BlockCoord, BlockPosition, GlobalSliceIndex, LocalSliceIndex, SlabLocation, SlabPosition,
    SliceIndex, WorldPoint, WorldPosition,
};

#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
pub struct WorldRange<P: RangePosition> {
    /// Inclusive and always valid element-wise
    min: P,
    /// Inclusive and always valid element-wise
    max: P,
}

pub trait RangeNum: Num + Zero + Copy + PartialOrd + PartialEq + SubAssign + Mul {
    fn range_step() -> Self;
    fn checked_sub(self, rhs: Self) -> Option<Self>;
}

pub trait RangePosition: Copy + Sized + PartialEq {
    type XY: RangeNum + AsPrimitive<Self::Count>;
    type Z: RangeNum + AsPrimitive<Self::Count>;
    type Count: Num + Copy + 'static;
    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z);
    fn get_z(&self) -> Self::Z;

    /// None if invalid coords
    fn new(xyz: (Self::XY, Self::XY, Self::Z)) -> Option<Self>;

    /// Panics if invalid coords
    fn new_unchecked(xyz: (Self::XY, Self::XY, Self::Z)) -> Self;

    fn below(&self) -> Option<Self> {
        let (x, y, z) = self.xyz();
        let z = z.checked_sub(Self::Z::one())?;
        Self::new((x, y, z))
    }

    fn above(&self) -> Option<Self> {
        let (x, y, z) = self.xyz();
        Self::new((x, y, z + Self::Z::one()))
    }
}

impl<P: RangePosition> WorldRange<P> {
    pub fn with_single<I: Into<P>>(pos: I) -> Self {
        let p = pos.into();
        Self { min: p, max: p }
    }

    pub fn is_single(&self) -> bool {
        self.min == self.max
    }

    pub fn as_single(&self) -> Option<P> {
        self.is_single().then_some(self.min)
    }

    fn min_max(a: P, b: P) -> (P, P) {
        let (ax, ay, az) = a.xyz();
        let (bx, by, bz) = b.xyz();

        let (ax, bx) = if ax < bx { (ax, bx) } else { (bx, ax) };
        let (ay, by) = if ay < by { (ay, by) } else { (by, ay) };
        let (az, bz) = if az < bz { (az, bz) } else { (bz, az) };

        (
            P::new_unchecked((ax, ay, az)),
            P::new_unchecked((bx, by, bz)),
        )
    }

    pub fn with_inclusive_range_unchecked<F: Into<P>, T: Into<P>>(from: F, to: T) -> Self {
        let from = from.into();
        let to = to.into();
        if cfg!(debug_assertions) {
            let (min, max) = Self::min_max(from, to);
            assert!((min, max) == (from, to), "incorrect min and max");
        }

        Self { min: from, max: to }
    }

    pub fn min(&self) -> P {
        self.min
    }
    pub fn max(&self) -> P {
        self.max
    }

    /// `[from, to]`
    pub fn with_inclusive_range<F: Into<P>, T: Into<P>>(from: F, to: T) -> Self {
        let (min, max) = Self::min_max(from.into(), to.into());
        Self { min, max }
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
        Self { min, max }
    }

    /// Inclusive
    pub fn bounds(&self) -> (P, P) {
        (self.min, self.max)
    }

    pub fn center(&self) -> P {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();
        P::new_unchecked((
            (ax + bx) / (P::XY::one() + P::XY::one()), // lmao
            (ay + by) / (P::XY::one() + P::XY::one()),
            (az + bz) / (P::Z::one() + P::Z::one()),
        ))
    }

    /// (min x, max x), (min y, max y), (min z, max z) inclusive
    #[allow(clippy::type_complexity)]
    pub fn ranges(&self) -> ((P::XY, P::XY), (P::XY, P::XY), (P::Z, P::Z)) {
        let (ax, ay, az) = self.min.xyz();
        let (bx, by, bz) = self.max.xyz();
        ((ax, bx), (ay, by), (az, bz))
    }

    pub fn count(&self) -> P::Count {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();

        // b* is guaranteed to be >= than a* from ranges()

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
        match (self.min.below(), self.max.below()) {
            (Some(min), Some(max)) => Some(Self { min, max }),
            _ => None,
        }
    }

    pub fn above(&self) -> Option<Self> {
        match (self.min.above(), self.max.above()) {
            (Some(min), Some(max)) => Some(Self { min, max }),
            _ => None,
        }
    }

    pub fn expand_up(&self) -> Option<Self> {
        self.max.above().map(|max| Self { max, ..*self })
    }

    pub fn expand_down(&self) -> Option<Self> {
        self.min.below().map(|min| Self { min, ..*self })
    }

    pub fn contract_up(&self) -> Option<Self> {
        if self.min.get_z() == self.max.get_z() {
            None
        } else {
            self.min.above().map(|min| Self { min, ..*self })
        }
    }

    pub fn contract_down(&self) -> Option<Self> {
        if self.min.get_z() == self.max.get_z() {
            None
        } else {
            self.max.below().map(|max| Self { max, ..*self })
        }
    }

    pub fn contains(&self, pos: &P) -> bool {
        let ((ax, bx), (ay, by), (az, bz)) = self.ranges();
        let (x, y, z) = pos.xyz();

        ax <= x && x <= bx && ay <= y && y <= by && az <= z && z <= bz
    }

    pub fn intersects(&self, other: &Self) -> bool {
        let ((ax1, ax2), (ay1, ay2), (az1, az2)) = self.ranges();
        let ((bx1, bx2), (by1, by2), (bz1, bz2)) = other.ranges();

        ax1 <= bx2 && ax2 >= bx1 && ay1 <= by2 && ay2 >= by1 && az1 <= bz2 && az2 >= bz1
    }
}

impl<XY: Copy, Z: Copy, P: RangePosition + Add<(XY, XY, Z), Output = P>> Add<(XY, XY, Z)>
    for WorldRange<P>
{
    type Output = Self;

    fn add(self, delta: (XY, XY, Z)) -> Self::Output {
        // can't fail because P implements Add already
        Self {
            min: self.min + delta,
            max: self.max + delta,
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

    fn get_z(&self) -> Self::Z {
        self.2.slice()
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
    type Z = u8;
    type Count = usize;

    fn xyz(&self) -> (Self::XY, Self::XY, Self::Z) {
        (self.x(), self.y(), self.z().slice())
    }

    fn get_z(&self) -> Self::Z {
        self.z().slice()
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
    fn get_z(&self) -> Self::Z {
        self.z().slice()
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

    fn get_z(&self) -> Self::Z {
        self.z()
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

    fn checked_sub(self, rhs: Self) -> Option<Self> {
        i32::checked_sub(self, rhs)
    }
}

impl RangeNum for BlockCoord {
    fn range_step() -> Self {
        1
    }

    fn checked_sub(self, rhs: Self) -> Option<Self> {
        BlockCoord::checked_sub(self, rhs)
    }
}

impl RangeNum for f32 {
    fn range_step() -> Self {
        // no difference between inclusive and exclusive ranges
        0.0
    }

    fn checked_sub(self, rhs: Self) -> Option<Self> {
        Some(self - rhs)
    }
}

impl<P: RangePosition + Eq> Eq for WorldRange<P> {}

impl<P: RangePosition + Hash> Hash for WorldRange<P> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        self.min.hash(state);
        self.max.hash(state);
    }
}

impl<P: RangePosition + Display> Display for WorldRange<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_single() {
            write!(f, "{}", self.min)
        } else {
            write!(f, "({} -> {})", self.min, self.max)
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
        // let (bottom, top) = slab.slab.slice_range();
        // let min = BlockPosition::new_unchecked(0, 0, bottom).to_world_position(slab.chunk);
        // let max = BlockPosition::new_unchecked(CHUNK_SIZE.as_block_coord(), CHUNK_SIZE.as_block_coord(), top).to_world_position(slab.chunk);

        true // TODO FIXME
    }

    pub fn expand(&self, by: u16) -> Self {
        let by = by as i32;
        Self {
            min: self.min + (-by, -by, -by),
            max: self.max + (by, by, by),
        }
    }
}

impl From<WorldPositionRange> for WorldPointRange {
    fn from(range: WorldPositionRange) -> Self {
        WorldPointRange {
            min: range.min.floored(),
            max: range.max.floored(),
        }
    }
}

impl WorldRange<WorldPoint> {
    pub fn to_world_position(self) -> WorldPositionRange {
        WorldRange {
            min: self.min.floor(),
            max: self.max.ceil(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use misc::{ApproxEq, Itertools};

    use crate::world::{
        BlockPositionRange, WorldPoint, WorldPointRange, WorldPositionRange, CHUNK_SIZE,
    };

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

    #[test]
    fn expand() {
        let range = WorldPositionRange::with_inclusive_range((5, 5, 0), (7, 7, 0));
        assert!(range.contract_down().is_none());
        assert!(range.contract_up().is_none());

        let up = range.expand_up().unwrap();
        assert_eq!(up.bounds().1, (7, 7, 1).into());
    }
}
