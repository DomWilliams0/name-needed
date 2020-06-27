use std::fmt::{Display, Error, Formatter};
use std::ops::Add;

use common::derive_more::*;

use common::Point3;

use crate::view::ViewPoint;
use crate::world::{ChunkPosition, GlobalSliceIndex, WorldPoint, SCALE};

/// A block anywhere in the world
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Into, From, PartialOrd)]
pub struct WorldPosition(pub i32, pub i32, pub GlobalSliceIndex);

impl WorldPosition {
    pub const fn slice(self) -> GlobalSliceIndex {
        self.2
    }

    pub fn centred(self) -> WorldPoint {
        WorldPoint(
            self.0 as f32 + 0.5,
            self.1 as f32 + 0.5,
            self.2.slice() as f32,
        )
    }

    pub fn below(self) -> WorldPosition {
        Self(self.0, self.1, self.2 - 1)
    }

    pub fn above(self) -> WorldPosition {
        Self(self.0, self.1, self.2 + 1)
    }
}

impl Display for WorldPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "({}, {}, {})", self.0, self.1, self.2.slice())
    }
}

impl From<(i32, i32, i32)> for WorldPosition {
    fn from((x, y, z): (i32, i32, i32)) -> Self {
        Self(x, y, GlobalSliceIndex::new(z))
    }
}

impl From<(u8, u8, GlobalSliceIndex)> for WorldPosition {
    fn from((x, y, z): (u8, u8, GlobalSliceIndex)) -> Self {
        Self(x as i32, y as i32, z)
    }
}

impl From<ChunkPosition> for WorldPosition {
    fn from(p: ChunkPosition) -> Self {
        WorldPoint::from(p).floor()
    }
}

impl From<ViewPoint> for WorldPosition {
    fn from(v: ViewPoint) -> Self {
        Self(
            (v.0 / SCALE).floor() as i32,
            (v.1 / SCALE).floor() as i32,
            GlobalSliceIndex::new((v.2 / SCALE).floor() as i32),
        )
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
            z: pos.2.slice() as f32,
        }
    }
}

impl From<WorldPosition> for [i32; 3] {
    fn from(pos: WorldPosition) -> Self {
        [pos.0, pos.1, pos.2.slice()]
    }
}

impl From<[i32; 3]> for WorldPosition {
    fn from([x, y, z]: [i32; 3]) -> Self {
        (x, y, z).into()
    }
}
