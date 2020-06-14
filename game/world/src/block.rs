use color::ColorRgb;

use crate::navigation::{ChunkArea, SlabAreaIndex};
use crate::occlusion::{BlockOcclusion, Opacity};
use unit::world::GlobalSliceIndex;

/// A single block in a chunk
#[derive(Debug, Default, Copy, Clone)]
pub struct Block {
    block_type: BlockType,
    area: SlabAreaIndex,
    occlusion: BlockOcclusion,
}

impl Block {
    /// Called by BlockBuilder
    fn new(block_type: BlockType) -> Self {
        Self {
            block_type,
            area: SlabAreaIndex::UNINITIALIZED,
            occlusion: BlockOcclusion::default(),
        }
    }

    pub const fn default_const() -> Self {
        Self {
            block_type: BlockType::Air,
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

    pub fn opacity(self) -> Opacity {
        self.block_type.opacity()
    }

    pub fn walkable(self) -> bool {
        self.area.initialized()
    }

    pub(crate) fn area_index(self) -> SlabAreaIndex {
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
}

/// The type of a block
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BlockType {
    Air,
    Dirt,
    Grass,
    LightGrass,
    Stone,
}

impl BlockType {
    pub fn color(self) -> ColorRgb {
        match self {
            BlockType::Air => ColorRgb::new(0, 0, 0),
            BlockType::Dirt => ColorRgb::new(86, 38, 23),
            BlockType::Grass => ColorRgb::new(49, 152, 56),
            BlockType::LightGrass => ColorRgb::new(91, 152, 51),
            BlockType::Stone => ColorRgb::new(106, 106, 117),
        }
    }

    pub fn opacity(self) -> Opacity {
        if let BlockType::Air = self {
            Opacity::Transparent
        } else {
            Opacity::Solid
        }
    }
}

impl Default for BlockType {
    fn default() -> Self {
        BlockType::Air
    }
}

// kinda useless now
#[derive(Default)]
pub struct BlockBuilder {
    block_type: BlockType,
}

impl BlockBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type(mut self, block_type: BlockType) -> Self {
        self.block_type = block_type;
        self
    }

    pub fn build(self) -> Block {
        Block::new(self.block_type)
    }
}

impl Into<Block> for BlockBuilder {
    fn into(self) -> Block {
        self.build()
    }
}

// helpful
impl Into<Block> for BlockType {
    fn into(self) -> Block {
        BlockBuilder::new().with_type(self).build()
    }
}
