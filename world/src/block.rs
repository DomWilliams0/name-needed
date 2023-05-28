use crate::chunk::slice_navmesh::SliceAreaIndex;
use crate::context::{BlockType, WorldContext};
use misc::*;
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
    // TODO put into slab data too
    durability: Proportion<BlockDurability>,

    /// Navigability
    #[deprecated]
    area: SlabAreaIndex,

    /// Only for navigation if [is_accessible] is set
    #[deprecated]
    nav_area: SliceAreaIndex,

    // TODO pack into a bit
    #[deprecated]
    is_accessible: bool,
}

/// Enriched with info from slab data that isnt stored inline in blocks
pub struct BlockEnriched<C: WorldContext> {
    pub block_type: C::BlockType,
    pub occlusion: BlockOcclusion,
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
            nav_area: SliceAreaIndex::DEFAULT, // irrelevant because not accessible
            is_accessible: false,
        }
    }

    pub const fn air() -> Self {
        Self {
            block_type: C::BlockType::AIR,
            durability: Proportion::default_empty(),
            area: SlabAreaIndex::UNINITIALIZED,
            nav_area: SliceAreaIndex::DEFAULT, // irrelevant because not accessible
            is_accessible: false,
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

    pub fn nav_area(&self) -> Option<SliceAreaIndex> {
        self.is_accessible.then_some(self.nav_area)
    }

    pub fn is_walkable(&self) -> bool {
        self.is_accessible
    }

    pub fn clear_nav_area(&mut self) {
        self.is_accessible = false;
        // no need to clear actual area
    }

    pub fn set_nav_area(&mut self, area: SliceAreaIndex) {
        self.nav_area = area;
        self.is_accessible = true;
    }

    #[deprecated]
    pub fn walkable(self) -> bool {
        self.area.initialized()
    }

    #[deprecated]
    pub fn walkable_area(self) -> Option<SlabAreaIndex> {
        if self.area.initialized() {
            Some(self.area)
        } else {
            None
        }
    }

    #[deprecated]
    pub(crate) fn area_index(self) -> SlabAreaIndex {
        // TODO this should return an Option if area is uninitialized
        self.area
    }
    #[deprecated]
    pub(crate) fn area_mut(&mut self) -> &mut SlabAreaIndex {
        &mut self.area
    }

    #[deprecated]
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

impl<C: WorldContext> BlockEnriched<C> {
    pub fn block_type(&self) -> C::BlockType {
        self.block_type
    }

    pub fn occlusion(&self) -> &BlockOcclusion {
        &self.occlusion
    }
}

impl<C: WorldContext> Default for Block<C> {
    fn default() -> Self {
        Self::with_block_type(C::BlockType::AIR)
    }
}

impl BlockOpacity {
    #[inline]
    pub fn solid(self) -> bool {
        matches!(self, Self::Solid)
    }

    #[inline]
    pub fn transparent(self) -> bool {
        matches!(self, Self::Transparent)
    }
}
