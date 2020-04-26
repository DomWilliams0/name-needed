use std::fmt::{Display, Error, Formatter};

use derive_more::*;

use crate::dim::CHUNK_SIZE;
use crate::world::{BlockCoord, ChunkPosition, SliceIndex, WorldPoint, WorldPosition};

/// A block in a chunk
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Into, From)]
pub struct BlockPosition(pub BlockCoord, pub BlockCoord, pub SliceIndex);

impl BlockPosition {
    pub fn to_world_pos<P: Into<ChunkPosition>>(self, chunk_pos: P) -> WorldPosition {
        let ChunkPosition(cx, cy) = chunk_pos.into();
        let BlockPosition(x, y, SliceIndex(z)) = self;
        WorldPosition(
            i32::from(x) + cx * CHUNK_SIZE.as_i32(),
            i32::from(y) + cy * CHUNK_SIZE.as_i32(),
            z,
        )
    }
    pub fn to_world_point<P: Into<ChunkPosition>>(self, chunk_pos: P) -> WorldPoint {
        let WorldPosition(x, y, z) = self.to_world_pos(chunk_pos);
        WorldPoint(x as f32, y as f32, z as f32)
    }

    pub fn to_world_point_centered<P: Into<ChunkPosition>>(self, chunk_pos: P) -> WorldPoint {
        let WorldPoint(x, y, z) = self.to_world_point(chunk_pos);
        WorldPoint(x + 0.5, y + 0.5, z)
    }

    pub fn flatten(self) -> (BlockCoord, BlockCoord, SliceIndex) {
        self.into()
    }
}

impl Display for BlockPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "BlockPosition({}, {}, {})", self.0, self.1, (self.2).0)
    }
}

impl From<(BlockCoord, BlockCoord, i32)> for BlockPosition {
    fn from(pos: (BlockCoord, BlockCoord, i32)) -> Self {
        let (x, y, z) = pos;
        Self(x, y, z.into())
    }
}

impl From<(i32, i32, i32)> for BlockPosition {
    fn from(pos: (i32, i32, i32)) -> Self {
        let (x, y, z) = pos;
        assert!(x >= 0);
        assert!(y >= 0);
        Self(x as BlockCoord, y as BlockCoord, z.into())
    }
}

impl From<BlockPosition> for (BlockCoord, BlockCoord, i32) {
    fn from(pos: BlockPosition) -> Self {
        let (x, y, SliceIndex(z)) = pos.into();
        (x, y, z)
    }
}

impl From<&[i32; 3]> for BlockPosition {
    fn from(pos: &[i32; 3]) -> Self {
        let &[x, y, z] = pos;
        Self(x as BlockCoord, y as BlockCoord, SliceIndex(z))
    }
}

impl From<BlockPosition> for [i32; 3] {
    fn from(b: BlockPosition) -> Self {
        let BlockPosition(x, y, SliceIndex(z)) = b;
        [i32::from(x), i32::from(y), z]
    }
}

impl From<WorldPosition> for BlockPosition {
    fn from(wp: WorldPosition) -> Self {
        let WorldPosition(x, y, z) = wp;
        BlockPosition(
            x.rem_euclid(CHUNK_SIZE.as_i32()) as BlockCoord,
            y.rem_euclid(CHUNK_SIZE.as_i32()) as BlockCoord,
            SliceIndex(z),
        )
    }
}
