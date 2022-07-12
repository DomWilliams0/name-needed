use crate::context::{BlockType, WorldContext};
use common::*;
use unit::world::GlobalSliceIndex;

use crate::navigation::{ChunkArea, SlabAreaIndex};
use crate::occlusion::BlockOcclusion;

/// A single block in a chunk
// TODO store sparse block data in the slab instead of inline in the block
#[derive(Derivative)]
#[derivative(Debug(bound = ""), Copy(bound = ""), Clone(bound = ""))]
pub struct Block<C: WorldContext> {
    block_type: C::BlockType,

    /// How damaged the block is
    durability: Proportion<BlockDurability>,

    /// Navigability
    area: SlabAreaIndex,
    /// Lighting
    occlusion: BlockOcclusion,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BlockOpacity {
    Transparent,
    Solid,
}

pub type BlockDurability = u8;

impl<C: WorldContext> Block<C> {
    pub fn with_block_type(block_type: C::BlockType) -> Self {
        Self {
            block_type,
            durability: {
                let max = block_type.durability();
                Proportion::with_value(max, max)
            },
            area: SlabAreaIndex::UNINITIALIZED,
            occlusion: BlockOcclusion::default(),
        }
    }

    pub const fn air() -> Self {
        Self {
            block_type: C::BlockType::AIR,
            durability: Proportion::default_empty(),
            area: SlabAreaIndex::UNINITIALIZED,
            occlusion: BlockOcclusion::default_const(),
        }
    }

    pub const fn block_type(self) -> C::BlockType {
        self.block_type
    }

    pub fn block_type_mut(&mut self) -> &mut C::BlockType {
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
        self.durability.value() == 0 || self.block_type.is_air()
    }
}

impl<C: WorldContext> Default for Block<C> {
    fn default() -> Self {
        Self::with_block_type(C::BlockType::AIR)
    }
}

impl BlockOpacity {
    pub fn solid(self) -> bool {
        matches!(self, Self::Solid)
    }

    pub fn transparent(self) -> bool {
        matches!(self, Self::Transparent)
    }
}
