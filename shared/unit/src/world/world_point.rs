use std::convert::TryFrom;
use std::ops::{Add, AddAssign, Sub};

use common::derive_more::*;

use common::{Display, FmtResult, Formatter, Point3, Vector2, Vector3};

use crate::space::view::ViewPoint;
use crate::world::{GlobalSliceIndex, WorldPosition, BLOCKS_PER_METRE};

/// A point anywhere in the world. All possible non-NaN and finite values are valid
#[derive(Debug, Copy, Clone, PartialEq, Default, Into, From, PartialOrd)]
pub struct WorldPoint(f32, f32, f32);

impl WorldPoint {
    /// None if any coord is not finite
    pub fn new(x: f32, y: f32, z: f32) -> Option<Self> {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            Some(Self(x, y, z))
        } else {
            None
        }
    }

    /// Panics if not finite
    pub fn new_unchecked(x: f32, y: f32, z: f32) -> Self {
        Self::new(x, y, z).unwrap_or_else(|| panic!("bad coords {:?}", (x, y, z)))
    }

    pub fn slice(&self) -> GlobalSliceIndex {
        GlobalSliceIndex::new(self.2 as i32)
    }

    pub fn floored(&self) -> Self {
        Self(self.0.floor(), self.1.floor(), self.2.floor())
    }

    pub fn floor(&self) -> WorldPosition {
        WorldPosition(
            self.0.floor() as i32,
            self.1.floor() as i32,
            GlobalSliceIndex::new(self.2.floor() as i32),
        )
    }

    pub fn ceil(&self) -> WorldPosition {
        WorldPosition(
            self.0.ceil() as i32,
            self.1.ceil() as i32,
            GlobalSliceIndex::new(self.2.ceil() as i32),
        )
    }

    pub fn round(&self) -> WorldPosition {
        WorldPosition(
            self.0.round() as i32,
            self.1.round() as i32,
            GlobalSliceIndex::new(self.2.round() as i32),
        )
    }

    pub fn is_almost(&self, other: &Self, horizontal_distance: f32) -> bool {
        (self.2 - other.2).abs() <= 1.0
            && ((self.0 - other.0).powi(2) + (self.1 - other.1).powi(2))
                <= horizontal_distance.powi(2)
    }

    pub fn distance2(&self, other: impl Into<Self>) -> f32 {
        let other = other.into();
        (self.0 - other.0).powi(2) + (self.1 - other.1).powi(2) + (self.2 - other.2).powi(2)
    }

    pub const fn xyz(&self) -> (f32, f32, f32) {
        (self.0, self.1, self.2)
    }

    pub const fn x(&self) -> f32 {
        self.0
    }

    pub const fn y(&self) -> f32 {
        self.1
    }

    pub const fn z(&self) -> f32 {
        self.2
    }

    /// Panics if new value is not finite
    pub fn modify_z(&mut self, f: impl FnOnce(f32) -> f32) {
        let new_z = f(self.2);
        assert!(new_z.is_finite());
        self.2 = new_z;
    }

    /// Panics if new value is not finite
    pub fn modify_x(&mut self, f: impl FnOnce(f32) -> f32) {
        let new_x = f(self.0);
        assert!(new_x.is_finite());
        self.0 = new_x;
    }
}

impl From<WorldPoint> for Vector3 {
    fn from(p: WorldPoint) -> Self {
        Vector3 {
            x: p.0,
            y: p.1,
            z: p.2,
        }
    }
}

impl From<WorldPoint> for Vector2 {
    fn from(p: WorldPoint) -> Self {
        Vector2 { x: p.0, y: p.1 }
    }
}

impl From<WorldPoint> for Point3 {
    fn from(p: WorldPoint) -> Self {
        Point3 {
            x: p.0,
            y: p.1,
            z: p.2,
        }
    }
}

impl TryFrom<Vector3> for WorldPoint {
    type Error = ();

    fn try_from(vec: Vector3) -> Result<Self, Self::Error> {
        Self::new(vec.x, vec.y, vec.z).ok_or(())
    }
}

impl From<WorldPosition> for WorldPoint {
    fn from(pos: WorldPosition) -> Self {
        Self::new_unchecked(pos.0 as f32, pos.1 as f32, pos.2.slice() as f32)
    }
}

impl AddAssign<Vector2> for WorldPoint {
    fn add_assign(&mut self, rhs: Vector2) {
        self.0 += rhs.x;
        self.1 += rhs.y;
    }
}

impl From<WorldPoint> for [f32; 3] {
    fn from(p: WorldPoint) -> Self {
        let WorldPoint(x, y, z) = p;
        [x, y, z]
    }
}

impl Add<Vector2> for WorldPoint {
    type Output = Self;

    fn add(self, rhs: Vector2) -> Self::Output {
        Self(self.0 + rhs.x, self.1 + rhs.y, self.2)
    }
}

impl TryFrom<&[f32]> for WorldPoint {
    type Error = ();

    fn try_from(slice: &[f32]) -> Result<Self, Self::Error> {
        if slice.len() == 3 {
            let x = slice[0];
            let y = slice[1];
            let z = slice[2];
            WorldPoint::new(x, y, z).ok_or(())
        } else {
            Err(())
        }
    }
}

impl Add<GlobalSliceIndex> for WorldPoint {
    type Output = Self;

    fn add(self, rhs: GlobalSliceIndex) -> Self::Output {
        Self(self.0, self.1, self.2 + rhs.slice() as f32)
    }
}

impl Add<(f32, f32, f32)> for WorldPoint {
    type Output = Self;

    fn add(self, (x, y, z): (f32, f32, f32)) -> Self::Output {
        Self(self.0 + x, self.1 + y, self.2 + z)
    }
}

impl Display for WorldPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "({:.2}, {:.2}, {:.2})", self.0, self.1, self.2)
    }
}

/// No NaNs allowed (sorry grandma)
impl Eq for WorldPoint {}

impl Sub for WorldPoint {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0, self.1 - rhs.1, self.2 - rhs.2)
    }
}

impl From<ViewPoint> for WorldPoint {
    fn from(pos: ViewPoint) -> Self {
        // guaranteed valid coords from viewpoint
        let (x, y, z) = pos.xyz();
        const SCALE: f32 = BLOCKS_PER_METRE as f32;
        Self(x * SCALE, y * SCALE, z * SCALE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Shows it's necessary to floor a float before casting to i32
    fn flooring() {
        assert_eq!(
            WorldPoint(1.1, 2.2, 3.6).floor(),
            WorldPosition::from((1, 2, 3))
        );
        assert_eq!(
            WorldPoint(-2.6, -10.8, -0.2).floor(),
            WorldPosition::from((-3, -11, -1))
        );
        assert_eq!(
            WorldPoint(-1.2, -1.9, -0.9).floor(),
            WorldPosition::from((-2, -2, -1))
        );
    }

    #[test]
    fn validation() {
        assert!(WorldPoint::new(2.0, 5.1, -2.2).is_some());
        assert!(WorldPoint::new(2.0, -0.0, -2.2).is_some());

        assert!(WorldPoint::new(f32::INFINITY, 1.0, 1.0).is_none());
        assert!(WorldPoint::new(5.0, 10.0 / 0.0, 1.0).is_none());

        let mut me = WorldPoint::default();
        // no panics
        me.modify_z(|z| z + 1.0);
        me.modify_z(|z| z / 100.0);
    }

    #[test]
    #[should_panic]
    fn invalid_modification() {
        let mut me = WorldPoint::default();
        me.modify_z(|z| z / 0.0);
    }
}
