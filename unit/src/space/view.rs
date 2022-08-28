use misc::derive_more::*;

use crate::world::{WorldPoint, BLOCKS_SCALE};
use misc::{Point2, Vector3};
use std::convert::TryFrom;

/// A point anywhere in the world, in meters
#[derive(Debug, Copy, Clone, Default, Into, From, PartialEq)]
pub struct ViewPoint(f32, f32, f32);

//noinspection DuplicatedCode
impl ViewPoint {
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

    pub fn new_arr([x, y, z]: [f32; 3]) -> Option<Self> {
        Self::new(x, y, z)
    }

    pub const fn xyz(&self) -> (f32, f32, f32) {
        (self.0, self.1, self.2)
    }

    pub const fn xyz_arr(&self) -> [f32; 3] {
        [self.0, self.1, self.2]
    }
}

impl TryFrom<Point2> for ViewPoint {
    type Error = ();

    fn try_from(point: Point2) -> Result<Self, Self::Error> {
        Self::new(point.x, point.y, 0.0).ok_or(())
    }
}

impl From<WorldPoint> for ViewPoint {
    fn from(pos: WorldPoint) -> Self {
        // guaranteed valid coords from worldpoint
        let (x, y, z) = pos.xyz();
        Self(x * BLOCKS_SCALE, y * BLOCKS_SCALE, z * BLOCKS_SCALE)
    }
}

impl From<ViewPoint> for Vector3 {
    fn from(v: ViewPoint) -> Self {
        Self::new(v.0, v.1, v.2)
    }
}

impl From<ViewPoint> for [f32; 3] {
    fn from(v: ViewPoint) -> Self {
        [v.0, v.1, v.2]
    }
}

/// No NaNs allowed (sorry grandma)
impl Eq for ViewPoint {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_to_world() {
        // 4 metres
        let vp = ViewPoint::new_unchecked(4.0, 0.0, 0.0);

        // 12 blocks
        let wp = WorldPoint::new_unchecked(12.0, 0.0, 0.0);

        assert_eq!(WorldPoint::from(vp), wp);
        assert_eq!(ViewPoint::from(wp), vp);
    }
}
