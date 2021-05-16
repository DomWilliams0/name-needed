use color::ColorRgb;

use crate::navigation::{ChunkArea, SlabAreaIndex};
use crate::occlusion::BlockOcclusion;
use common::derive_more::Display;
use common::Proportion;
pub use enum_iterator::IntoEnumIterator;
use unit::world::GlobalSliceIndex;

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

pub type BlockDurability = u8;

// TODO define block types in data instead of code
/// The type of a block
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, IntoEnumIterator, Display)]
pub enum BlockType {
    Air,
    Dirt,
    Grass,
    #[display(fmt = "Light grass")]
    LightGrass,
    Leaves,
    #[display(fmt = "Tree trunk")]
    TreeTrunk,
    Stone,
    Sand,
    #[display(fmt = "Solid water")]
    SolidWater,

    Chest,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BlockOpacity {
    Transparent,
    Solid,
}

impl BlockOpacity {
    pub fn solid(self) -> bool {
        matches!(self, Self::Solid)
    }

    pub fn transparent(self) -> bool {
        !self.solid()
    }
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

// TODO define these in data
impl BlockType {
    pub fn color(self) -> ColorRgb {
        match self {
            BlockType::Air => ColorRgb::new(0, 0, 0),
            BlockType::Dirt => ColorRgb::new(86, 38, 23),
            BlockType::Grass => ColorRgb::new(49, 152, 56),
            BlockType::LightGrass => ColorRgb::new(91, 152, 51),
            BlockType::Leaves => ColorRgb::new(49, 132, 2),
            BlockType::TreeTrunk => ColorRgb::new(79, 52, 16),
            BlockType::Stone => ColorRgb::new(106, 106, 117),
            BlockType::Sand => 0xBCA748FF.into(),
            BlockType::SolidWater => 0x3374BCFF.into(),
            BlockType::Chest => ColorRgb::new(184, 125, 31),
        }
    }

    pub fn opacity(self) -> BlockOpacity {
        if let BlockType::Air = self {
            BlockOpacity::Transparent
        } else {
            BlockOpacity::Solid
        }
    }

    fn durability(self) -> Proportion<BlockDurability> {
        use BlockType::*;
        let max = match self {
            Air => 0,
            Leaves => 10,
            Sand => 30,
            Dirt | Grass | LightGrass => 40,
            TreeTrunk => 70,
            Stone => 90,
            Chest => 60,
            SolidWater => u8::MAX,
        };

        Proportion::with_value(max, max)
    }

    /// TODO very temporary "walkability" for block types
    pub fn can_be_walked_on(self) -> bool {
        use BlockType::*;
        !matches!(self, Leaves | SolidWater)
    }
}

/// Helper
impl From<BlockType> for Block {
    fn from(bt: BlockType) -> Self {
        Block::with_block_type(bt)
    }
}
