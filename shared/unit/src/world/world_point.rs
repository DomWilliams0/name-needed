use std::convert::TryFrom;
use std::ops::{Add, AddAssign};

use common::derive_more::*;

use common::{Point3, Vector2, Vector3};

use crate::dim::CHUNK_SIZE;
use crate::world::{ChunkPosition, GlobalSliceIndex, SliceIndex, WorldPosition};
use std::fmt::{Display, Formatter};
use std::iter::{once, once_with};

/// A point anywhere in the world
// TODO assert fields are not NaN in points
#[derive(Debug, Copy, Clone, PartialEq, Default, Into, From, PartialOrd)]
pub struct WorldPoint(pub f32, pub f32, pub f32);

impl WorldPoint {
    pub fn slice(&self) -> GlobalSliceIndex {
        SliceIndex::new(self.2 as i32)
    }

    pub fn floored(self) -> Self {
        Self(self.0.floor(), self.1.floor(), self.2.floor())
    }
    pub fn floor(self) -> WorldPosition {
        WorldPosition(
            self.0.floor() as i32,
            self.1.floor() as i32,
            SliceIndex::new(self.2.floor() as i32),
        )
    }

    pub fn ceil(self) -> WorldPosition {
        WorldPosition(
            self.0.ceil() as i32,
            self.1.ceil() as i32,
            SliceIndex::new(self.2.ceil() as i32),
        )
    }

    pub fn floor_then_ceil(self) -> impl Iterator<Item = WorldPosition> {
        // TODO floor_then_ceil is terribly inefficient, try without the lazy eval
        once(self.floor()).chain(once_with(move || self.ceil()))
    }

    pub fn round(self) -> WorldPosition {
        WorldPosition(
            self.0.round() as i32,
            self.1.round() as i32,
            SliceIndex::new(self.2.round() as i32),
        )
    }

    pub fn is_almost(self, other: Self, horizontal_distance: f32) -> bool {
        (self.2 - other.2).abs() <= 1.0
            && ((self.0 - other.0).powi(2) + (self.1 - other.1).powi(2))
                <= horizontal_distance.powi(2)
    }
}

impl From<WorldPoint> for Vector3 {
    fn from(p: WorldPoint) -> Self {
        Self {
            x: p.0,
            y: p.1,
            z: p.2,
        }
    }
}

impl From<WorldPoint> for Vector2 {
    fn from(p: WorldPoint) -> Self {
        Self { x: p.0, y: p.1 }
    }
}

impl From<WorldPoint> for Point3 {
    fn from(p: WorldPoint) -> Self {
        Self {
            x: p.0,
            y: p.1,
            z: p.2,
        }
    }
}

impl From<Vector3> for WorldPoint {
    fn from(v: Vector3) -> Self {
        Self(v.x, v.y, v.z)
    }
}

impl From<WorldPosition> for WorldPoint {
    fn from(pos: WorldPosition) -> Self {
        Self(pos.0 as f32, pos.1 as f32, pos.2.slice() as f32)
    }
}

impl AddAssign<Vector2> for WorldPoint {
    fn add_assign(&mut self, rhs: Vector2) {
        self.0 += rhs.x;
        self.1 += rhs.y;
    }
}

impl From<ChunkPosition> for WorldPoint {
    fn from(p: ChunkPosition) -> Self {
        Self(
            (p.0 * CHUNK_SIZE.as_i32()) as f32,
            (p.1 * CHUNK_SIZE.as_i32()) as f32,
            0.0,
        )
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

impl From<[f32; 3]> for WorldPoint {
    fn from([x, y, z]: [f32; 3]) -> Self {
        WorldPoint(x, y, z)
    }
}

impl TryFrom<&[f32]> for WorldPoint {
    type Error = ();

    fn try_from(slice: &[f32]) -> Result<Self, Self::Error> {
        if slice.len() == 3 {
            let x = slice[0];
            let y = slice[1];
            let z = slice[2];
            Ok(WorldPoint(x, y, z))
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

impl Display for WorldPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.2}, {:.2}, {:.2})", self.0, self.1, self.2)
    }
}

/// No NaNs allowed (sorry grandma)
impl Eq for WorldPoint {}
