use std::fmt::{Debug, Display, Formatter};
use std::ops::Add;

use misc::derive_more::*;
use misc::*;

use crate::space::view::ViewPoint;
use crate::world::{GlobalSliceIndex, SliceIndex, WorldPoint, BLOCKS_SCALE};

/// A block anywhere in the world. All possible values are valid
#[derive(Copy, Clone, PartialEq, Eq, Hash, Into, From, PartialOrd, Ord)]
pub struct WorldPosition(pub i32, pub i32, pub GlobalSliceIndex);

impl WorldPosition {
    pub const MAX: Self = Self(i32::MAX, i32::MAX, GlobalSliceIndex::MAX);

    pub fn new(x: i32, y: i32, z: GlobalSliceIndex) -> Self {
        Self(x, y, z)
    }

    pub const fn slice(self) -> GlobalSliceIndex {
        self.2
    }

    pub fn centred(self) -> WorldPoint {
        WorldPoint::new_unchecked(
            self.0 as f32 + 0.5,
            self.1 as f32 + 0.5,
            self.2.slice() as f32,
        )
    }

    pub fn floored(self) -> WorldPoint {
        WorldPoint::new_unchecked(self.0 as f32, self.1 as f32, self.2.slice() as f32)
    }

    pub fn below(self) -> WorldPosition {
        Self(self.0, self.1, self.2 - 1)
    }

    pub fn above(self) -> WorldPosition {
        Self(self.0, self.1, self.2 + 1)
    }

    pub fn distance2(&self, other: impl Into<Self>) -> i32 {
        let other = other.into();
        (self.0 - other.0).pow(2)
            + (self.1 - other.1).pow(2)
            + (self.2.slice() - other.2.slice()).pow(2)
    }

    pub fn xy(&self) -> (i32, i32) {
        (self.0, self.1)
    }
}

impl Display for WorldPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.0, self.1, self.2.slice())
    }
}

impl Debug for WorldPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl From<(i32, i32, i32)> for WorldPosition {
    fn from((x, y, z): (i32, i32, i32)) -> Self {
        Self(x, y, GlobalSliceIndex::new(z))
    }
}

impl From<((i32, i32), GlobalSliceIndex)> for WorldPosition {
    fn from(((x, y), z): ((i32, i32), GlobalSliceIndex)) -> Self {
        Self(x, y, z)
    }
}

impl From<(u8, u8, GlobalSliceIndex)> for WorldPosition {
    fn from((x, y, z): (u8, u8, GlobalSliceIndex)) -> Self {
        Self(x as i32, y as i32, z)
    }
}

impl From<ViewPoint> for WorldPosition {
    fn from(v: ViewPoint) -> Self {
        // floor() required for negative values
        let (x, y, z) = v.xyz();
        Self(
            (x / BLOCKS_SCALE).floor() as i32,
            (y / BLOCKS_SCALE).floor() as i32,
            GlobalSliceIndex::new((z / BLOCKS_SCALE).floor() as i32),
        )
    }
}

impl Add<(i32, i32, i32)> for WorldPosition {
    type Output = WorldPosition;

    fn add(self, (x, y, z): (i32, i32, i32)) -> Self::Output {
        WorldPosition(self.0 + x, self.1 + y, self.2 + z)
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
