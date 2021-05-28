use common::derive_more::*;
use common::*;

use crate::world::{
    BlockCoord, ChunkLocation, GlobalSliceIndex, SliceIndex, WorldPoint, WorldPosition, CHUNK_SIZE,
};
use std::ops::Add;

/// A block in a chunk. Only valid coords are represented by this type
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Into, From)]
pub struct BlockPosition(BlockCoord, BlockCoord, GlobalSliceIndex);

impl BlockPosition {
    pub fn new(x: BlockCoord, y: BlockCoord, z: GlobalSliceIndex) -> Self {
        // TODO return Option/implement TryFrom for all coord types instead of asserts
        assert!(x < CHUNK_SIZE.as_block_coord(), "x={} is out of range", x);
        assert!(y < CHUNK_SIZE.as_block_coord(), "y={} is out of range", y);
        Self(x, y, z)
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
        GlobalSliceIndex::try_from(self.2.slice() + dz).and_then(|z| {
            self.2 = z;
            self.try_add_xy((dx, dy))
        })
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

    pub fn xyz(self) -> [i32; 3] {
        [self.0 as i32, self.1 as i32, self.2.slice()]
    }
}

impl Display for BlockPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "BlockPosition({}, {}, {})",
            self.0,
            self.1,
            self.2.slice()
        )
    }
}

impl From<(BlockCoord, BlockCoord, i32)> for BlockPosition {
    fn from(pos: (BlockCoord, BlockCoord, i32)) -> Self {
        let (x, y, z) = pos;
        Self::new(x, y, SliceIndex::new(z))
    }
}

impl From<(i32, i32, i32)> for BlockPosition {
    fn from(pos: (i32, i32, i32)) -> Self {
        let (x, y, z) = pos;
        Self::new(x as BlockCoord, y as BlockCoord, SliceIndex::new(z))
    }
}

impl From<&[i32; 3]> for BlockPosition {
    //noinspection DuplicatedCode
    fn from(pos: &[i32; 3]) -> Self {
        let &[x, y, z] = pos;
        Self::new(x as BlockCoord, y as BlockCoord, SliceIndex::new(z))
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

impl Add<(BlockCoord, BlockCoord, i32)> for BlockPosition {
    type Output = Self;

    fn add(self, (x, y, z): (BlockCoord, BlockCoord, i32)) -> Self::Output {
        Self::new(self.0 + x, self.1 + y, self.2 + z)
    }
}
