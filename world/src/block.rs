/// A single block in a chunk
#[derive(Debug, Default, Copy, Clone)]
pub struct Block {
    pub block_type: BlockType,
    pub height: BlockHeight,
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
    pub fn color_as_u8(self) -> (u8, u8, u8) {
        match self {
            BlockType::Air => (0, 0, 0),
            BlockType::Dirt => (192, 57, 43),
            BlockType::Grass => (40, 102, 25),
            BlockType::Stone => (106, 106, 117),
        }
    }

    pub fn color_as_f32(self) -> (f32, f32, f32) {
        let (r, g, b) = self.color_as_u8();
        (
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        )
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

#[cfg(test)]
mod tests {
    use crate::block::BlockHeight;

    #[test]
    fn ordering() {
        assert!(BlockHeight::Full > BlockHeight::Half);
    }
}
