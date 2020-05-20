use common::derive_more::*;

use crate::world::WorldPoint;
use crate::world::SCALE;
use common::{Point2, Vector3};

/// A point anywhere in the world, in meters
#[derive(Debug, Copy, Clone, Default, Into, From)]
pub struct ViewPoint(pub f32, pub f32, pub f32);

impl From<Point2> for ViewPoint {
    fn from(p: Point2) -> Self {
        Self(p.x, p.y, 0.0)
    }
}

impl From<WorldPoint> for ViewPoint {
    fn from(pos: WorldPoint) -> Self {
        let WorldPoint(x, y, z) = pos;
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
