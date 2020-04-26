use derive_more::*;

use crate::dim::CHUNK_SIZE;
use crate::world::{BlockPosition, SliceIndex};

/// A block in a chunk x/y coordinate. Must be < chunk size
pub type BlockCoord = u16;

/// A block in a slice
#[derive(Debug, Copy, Clone, PartialEq, Eq, Into, From)]
pub struct SliceBlock(pub BlockCoord, pub BlockCoord);

impl SliceBlock {
    pub fn to_block_position(self, slice: SliceIndex) -> BlockPosition {
        BlockPosition(self.0, self.1, slice)
    }

    /// Returns None on overflow around 0..CHUNK_SIZE
    pub fn try_add(self, (dx, dy): (i16, i16)) -> Option<Self> {
        let x = (self.0 as i16) + dx;
        let y = (self.1 as i16) + dy;

        if x < 0 || x >= CHUNK_SIZE.as_i16() || y < 0 || y >= CHUNK_SIZE.as_i16() {
            None
        } else {
            Some(Self(x as BlockCoord, y as BlockCoord))
        }
    }
}

impl From<BlockPosition> for SliceBlock {
    fn from(b: BlockPosition) -> Self {
        Self(b.0, b.1)
    }
}
