use misc::derive_more::*;

use crate::world::{BlockPosition, GlobalSliceIndex, LocalSliceIndex, SlabPosition, CHUNK_SIZE};

/// A block in a chunk x/y coordinate. Must be < chunk size
pub type BlockCoord = u8;

/// A block in a slice
#[derive(Debug, Copy, Clone, PartialEq, Eq, Into, From, Ord, PartialOrd)]
pub struct SliceBlock(BlockCoord, BlockCoord);

impl SliceBlock {
    /// None if x/y are out of range
    pub fn new(x: BlockCoord, y: BlockCoord) -> Option<Self> {
        if x < CHUNK_SIZE.as_block_coord() && y < CHUNK_SIZE.as_block_coord() {
            Some(Self(x, y))
        } else {
            None
        }
    }

    /// Panics if x/y are out of range
    pub fn new_unchecked(x: BlockCoord, y: BlockCoord) -> Self {
        Self::new(x, y).unwrap_or_else(|| panic!("coords out of range: {:?}", (x, y)))
    }

    pub fn to_block_position(self, slice: GlobalSliceIndex) -> BlockPosition {
        BlockPosition::new_unchecked(self.0, self.1, slice)
    }

    pub fn to_slab_position(self, slice: LocalSliceIndex) -> SlabPosition {
        SlabPosition::new_unchecked(self.0, self.1, slice)
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

    pub fn xy(self) -> (BlockCoord, BlockCoord) {
        (self.0, self.1)
    }
}

impl From<BlockPosition> for SliceBlock {
    fn from(b: BlockPosition) -> Self {
        Self(b.x(), b.y())
    }
}
impl From<SlabPosition> for SliceBlock {
    fn from(s: SlabPosition) -> Self {
        Self(s.x(), s.y())
    }
}
