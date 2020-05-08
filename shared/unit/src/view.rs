use common::derive_more::*;

use crate::scale::BLOCK_DIAMETER;
use crate::world::WorldPoint;

/// A point anywhere in the world, in meters
#[derive(Debug, Copy, Clone, Default, Into, From)]
pub struct ViewPoint(pub f32, pub f32, pub f32);

impl From<WorldPoint> for ViewPoint {
    fn from(pos: WorldPoint) -> Self {
        let WorldPoint(x, y, z) = pos;
        Self(x * BLOCK_DIAMETER, y * BLOCK_DIAMETER, z * BLOCK_DIAMETER)
    }
}
