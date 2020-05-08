use std::fmt::{Display, Error, Formatter};
use std::ops::Add;

use common::derive_more::*;

use common::Point3;

use crate::world::{ChunkPosition, WorldPoint};

/// A block anywhere in the world
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Into, From)]
pub struct WorldPosition(pub i32, pub i32, pub i32);

impl Display for WorldPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "WorldPosition({}, {}, {})", self.0, self.1, self.2)
    }
}

impl From<WorldPoint> for WorldPosition {
    fn from(pos: WorldPoint) -> Self {
        Self(
            pos.0.floor() as i32,
            pos.1.floor() as i32,
            pos.2.floor() as i32,
        )
    }
}

impl From<(u8, u8, i32)> for WorldPosition {
    fn from((x, y, z): (u8, u8, i32)) -> Self {
        Self(x as i32, y as i32, z)
    }
}

impl From<ChunkPosition> for WorldPosition {
    fn from(p: ChunkPosition) -> Self {
        WorldPoint::from(p).into()
    }
}

impl Add<(i32, i32, i32)> for WorldPosition {
    type Output = WorldPosition;

    fn add(self, (x, y, z): (i32, i32, i32)) -> Self::Output {
        WorldPosition(self.0 + x, self.1 + y, self.2 + z)
    }
}

impl From<&WorldPosition> for Point3 {
    fn from(pos: &WorldPosition) -> Self {
        Self {
            x: pos.0 as f32,
            y: pos.1 as f32,
            z: pos.2 as f32,
        }
    }
}
