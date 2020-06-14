use common::derive_more::*;

use common::Vector3;

use crate::world::BlockPosition;

/// A point anywhere in a chunk, x and y > 0 and < chunk size
#[derive(Debug, Copy, Clone, PartialEq, Into, From)]
pub struct ChunkPoint(pub f32, pub f32, pub f32);

impl From<BlockPosition> for ChunkPoint {
    fn from(b: BlockPosition) -> Self {
        let (x, y, z) = b.into();
        ChunkPoint(f32::from(x), f32::from(y), z.slice() as f32)
    }
}

impl From<ChunkPoint> for Vector3 {
    fn from(p: ChunkPoint) -> Self {
        let ChunkPoint(x, y, z) = p;
        Vector3 { x, y, z }
    }
}
