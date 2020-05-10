use common::derive_more::*;

use crate::world::WorldPoint;
use crate::world::SCALE;

/// A point anywhere in the world, in meters
#[derive(Debug, Copy, Clone, Default, Into, From)]
pub struct ViewPoint(pub f32, pub f32, pub f32);

impl From<WorldPoint> for ViewPoint {
    fn from(pos: WorldPoint) -> Self {
        let WorldPoint(x, y, z) = pos;
        Self(x * SCALE, y * SCALE, z * SCALE)
    }
}
