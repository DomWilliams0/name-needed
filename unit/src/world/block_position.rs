use std::convert::TryFrom;
use std::fmt::{Debug, Display, Formatter};

use misc::derive_more::*;
use misc::*;

use crate::world::{
    BlockCoord, ChunkLocation, GlobalSliceIndex, SliceIndex, WorldPoint, WorldPosition, CHUNK_SIZE,
};

/// A block in a chunk. Only valid coords are represented by this type
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Into, From)]
pub struct BlockPosition(BlockCoord, BlockCoord, GlobalSliceIndex);

impl BlockPosition {
    /// None if x/y are out of range
    pub fn new(x: BlockCoord, y: BlockCoord, z: GlobalSliceIndex) -> Option<Self> {
        if x < CHUNK_SIZE.as_block_coord() && y < CHUNK_SIZE.as_block_coord() {
            Some(Self(x, y, z))
        } else {
            None
        }
    }

    /// Panics if x/y are out of range
    pub fn new_unchecked(x: BlockCoord, y: BlockCoord, z: GlobalSliceIndex) -> Self {
        Self::new(x, y, z).unwrap_or_else(|| panic!("coords out of range: {:?}", (x, y, z)))
    }

    pub fn to_world_position<P: Into<ChunkLocation>>(self, chunk_pos: P) -> WorldPosition {
        let ChunkLocation(cx, cy) = chunk_pos.into();
        let BlockPosition(x, y, z) = self;
        WorldPosition(
            i32::from(x) + cx * CHUNK_SIZE.as_i32(),
            i32::from(y) + cy * CHUNK_SIZE.as_i32(),
            z,
        )
    }
    pub fn to_world_point<P: Into<ChunkLocation>>(self, chunk_pos: P) -> WorldPoint {
        let WorldPosition(x, y, z) = self.to_world_position(chunk_pos);
        WorldPoint::new_unchecked(x as f32, y as f32, z.slice() as f32)
    }

    pub fn to_world_point_centered<P: Into<ChunkLocation>>(self, chunk_pos: P) -> WorldPoint {
        let (x, y, z) = self.to_world_point(chunk_pos).xyz();
        WorldPoint::new_unchecked(x + 0.5, y + 0.5, z)
    }

    pub fn flatten(self) -> (BlockCoord, BlockCoord, GlobalSliceIndex) {
        self.into()
    }

    pub fn try_add_xy(self, (dx, dy): (i16, i16)) -> Option<Self> {
        let x = (self.0 as i16) + dx;
        let y = (self.1 as i16) + dy;

        if x >= 0 && x < CHUNK_SIZE.as_i16() && y >= 0 && y < CHUNK_SIZE.as_i16() {
            Some(Self(x as BlockCoord, y as BlockCoord, self.2))
        } else {
            None
        }
    }

    pub fn try_add_xyz(mut self, (dx, dy, dz): (i16, i16, i32)) -> Option<Self> {
        self.2 = GlobalSliceIndex::new(self.2.slice() + dz);
        self.try_add_xy((dx, dy))
    }

    pub const fn x(self) -> BlockCoord {
        self.0
    }
    pub const fn y(self) -> BlockCoord {
        self.1
    }
    pub const fn z(self) -> GlobalSliceIndex {
        self.2
    }

    pub fn xyz(self) -> (BlockCoord, BlockCoord, GlobalSliceIndex) {
        (self.0, self.1, self.2)
    }

    pub fn above_by(self, n: i32) -> Self {
        Self(self.0, self.1, self.2 + n)
    }

    pub fn is_edge(self) -> bool {
        let (x, y) = (self.0, self.1);
        x == 0
            || x == (CHUNK_SIZE.as_block_coord() - 1)
            || y == 0
            || y == (CHUNK_SIZE.as_block_coord() - 1)
    }
}

impl Display for BlockPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BlockPosition({}, {}, {})",
            self.0,
            self.1,
            self.2.slice()
        )
    }
}

impl Debug for BlockPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl TryFrom<(i32, i32, i32)> for BlockPosition {
    type Error = ();

    fn try_from((x, y, z): (i32, i32, i32)) -> Result<Self, Self::Error> {
        match (BlockCoord::try_from(x), BlockCoord::try_from(y)) {
            (Ok(x), Ok(y)) => Self::new(x, y, GlobalSliceIndex::new(z)).ok_or(()),
            _ => Err(()),
        }
    }
}

impl From<BlockPosition> for [i32; 3] {
    fn from(b: BlockPosition) -> Self {
        let BlockPosition(x, y, z) = b;
        [i32::from(x), i32::from(y), z.slice()]
    }
}

impl From<WorldPosition> for BlockPosition {
    fn from(wp: WorldPosition) -> Self {
        let WorldPosition(x, y, z) = wp;
        BlockPosition(
            x.rem_euclid(CHUNK_SIZE.as_i32()) as BlockCoord,
            y.rem_euclid(CHUNK_SIZE.as_i32()) as BlockCoord,
            z,
        )
    }
}
