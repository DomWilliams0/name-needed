use crate::area::SlabAreaIndex;
use crate::occlusion::BlockOcclusion;
use color::ColorRgb;

/// A single block in a chunk
#[derive(Debug, Default, Copy, Clone)]
pub struct Block {
    block_type: BlockType,
    height: BlockHeight,
    area: SlabAreaIndex,
    occlusion: BlockOcclusion,
}

impl Block {
    /// Called by BlockBuilder
    fn new(block_type: BlockType, height: BlockHeight) -> Self {
        Self {
            block_type,
            height,
            area: SlabAreaIndex::UNINITIALIZED,
            occlusion: BlockOcclusion::default(),
        }
    }

    pub const fn block_type(self) -> BlockType {
        self.block_type
    }

    pub fn block_type_mut(&mut self) -> &mut BlockType {
        &mut self.block_type
    }

    pub fn solid(self) -> bool {
        self.block_type.solid()
    }

    pub const fn block_height(self) -> BlockHeight {
        self.height
    }

    pub fn height(self) -> f32 {
        self.height.height()
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
    Stone,
}

impl BlockType {
    pub fn color(self) -> ColorRgb {
        match self {
            BlockType::Air => ColorRgb::new(0, 0, 0),
            BlockType::Dirt => ColorRgb::new(192, 57, 43),
            BlockType::Grass => ColorRgb::new(40, 102, 25),
            BlockType::Stone => ColorRgb::new(106, 106, 117),
        }
    }

    pub fn solid(self) -> bool {
        self != BlockType::Air
    }
}

impl Default for BlockType {
    fn default() -> Self {
        BlockType::Air
    }
}

/// The additional height offset for a block
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockHeight {
    Half = 0,
    Full = 1,
    // TODO Third?
}

impl BlockHeight {
    pub fn height(self) -> f32 {
        match self {
            BlockHeight::Full => 1.0,
            BlockHeight::Half => 0.5,
        }
    }

    pub fn solid(self) -> bool {
        self == BlockHeight::Full
    }

    /// Offset to subtract from z position to lower from the center of the block to the bottom
    pub fn offset_from_center(self) -> f32 {
        match self {
            BlockHeight::Full => 0.0,
            BlockHeight::Half => 0.25,
        }
    }
}

impl Default for BlockHeight {
    fn default() -> Self {
        BlockHeight::Full
    }
}

#[derive(Default)]
pub struct BlockBuilder {
    block_type: BlockType,
    height: BlockHeight,
}

impl BlockBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type(mut self, block_type: BlockType) -> Self {
        self.block_type = block_type;
        self
    }

    pub fn with_height(mut self, height: BlockHeight) -> Self {
        self.height = height;
        self
    }

    pub fn build(self) -> Block {
        Block::new(self.block_type, self.height)
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

impl Into<Block> for (BlockType, BlockHeight) {
    fn into(self) -> Block {
        BlockBuilder::new()
            .with_type(self.0)
            .with_height(self.1)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use crate::block::BlockHeight;

    #[test]
    fn ordering() {
        assert!(BlockHeight::Full > BlockHeight::Half);
    }
}
