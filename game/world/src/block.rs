pub use common::block::{BlockDurability, BlockOpacity, BlockType};
use common::Proportion;
use unit::world::GlobalSliceIndex;

use crate::navigation::{ChunkArea, SlabAreaIndex};
use crate::occlusion::BlockOcclusion;

/// A single block in a chunk
// TODO store sparse block data in the slab instead of inline in the block
#[derive(Debug, Copy, Clone)]
pub struct Block {
    block_type: BlockType,

    /// How damaged the block is
    durability: Proportion<BlockDurability>,

    /// Navigability
    area: SlabAreaIndex,
    /// Lighting
    occlusion: BlockOcclusion,
}

impl Block {
    pub fn with_block_type(block_type: BlockType) -> Self {
        Self {
            block_type,
            durability: block_type.durability(),
            area: SlabAreaIndex::UNINITIALIZED,
            occlusion: BlockOcclusion::default(),
        }
    }

    pub const fn air() -> Self {
        Self {
            block_type: BlockType::Air,
            durability: Proportion::default_empty(),
            area: SlabAreaIndex::UNINITIALIZED,
            occlusion: BlockOcclusion::default_const(),
        }
    }

    pub const fn block_type(self) -> BlockType {
        self.block_type
    }

    pub fn block_type_mut(&mut self) -> &mut BlockType {
        &mut self.block_type
    }

    pub fn opacity(self) -> BlockOpacity {
        self.block_type.opacity()
    }

    pub fn walkable(self) -> bool {
        self.area.initialized()
    }

    pub fn walkable_area(self) -> Option<SlabAreaIndex> {
        if self.area.initialized() {
            Some(self.area)
        } else {
            None
        }
    }

    pub(crate) fn area_index(self) -> SlabAreaIndex {
        // TODO this should return an Option if area is uninitialized
        self.area
    }
    pub(crate) fn area_mut(&mut self) -> &mut SlabAreaIndex {
        &mut self.area
    }
    pub(crate) fn chunk_area(self, slice: GlobalSliceIndex) -> Option<ChunkArea> {
        if self.area.initialized() {
            Some(ChunkArea {
                slab: slice.slab_index(),
                area: self.area,
            })
        } else {
            None
        }
    }

    pub fn occlusion_mut(&mut self) -> &mut BlockOcclusion {
        &mut self.occlusion
    }
    pub fn occlusion(&self) -> &BlockOcclusion {
        &self.occlusion
    }

    pub(crate) fn durability_mut(&mut self) -> &mut Proportion<BlockDurability> {
        &mut self.durability
    }

    pub fn durability(&self) -> Proportion<BlockDurability> {
        self.durability
    }

    /// True if air or durability == 0
    pub fn is_destroyed(&self) -> bool {
        self.durability.value() == 0 || self.block_type == BlockType::Air
    }
}

impl Default for Block {
    fn default() -> Self {
        Self::with_block_type(BlockType::Air)
    }
}

/// Helper
impl From<BlockType> for Block {
    fn from(bt: BlockType) -> Self {
        Block::with_block_type(bt)
    }
}
