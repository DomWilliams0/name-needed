use common::derive_more::*;

use crate::world::WorldPoint;
use crate::world::SCALE;
use common::{Point2, Vector3};

/// A point anywhere in the world, in meters
#[derive(Debug, Copy, Clone, Default, Into, From)]
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

    pub const fn xyz(&self) -> (f32, f32, f32) {
        (self.0, self.1, self.2)
    }
}

impl From<Point2> for ViewPoint {
    fn from(p: Point2) -> Self {
        Self(p.x, p.y, 0.0)
    }
}

impl From<WorldPoint> for ViewPoint {
    fn from(pos: WorldPoint) -> Self {
        let (x, y, z) = pos.xyz();
        Self(x * SCALE, y * SCALE, z * SCALE)
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
