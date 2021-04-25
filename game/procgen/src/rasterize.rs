use grid::{grid_declare, GridImpl};
use unit::world::{CHUNK_SIZE, SLAB_SIZE};

// TODO custom block types for procgen that are translated to game blocks
#[derive(Clone, Default, Debug, Copy)]
pub struct GeneratedBlock {
    pub ty: BlockType,
}

// redeclaration of slab grid
grid_declare!(pub struct SlabGrid<SlabGridImpl, GeneratedBlock>,
    CHUNK_SIZE.as_usize(),
    CHUNK_SIZE.as_usize(),
    SLAB_SIZE.as_usize()
);

#[derive(Debug, Copy, Clone)]
pub enum BlockType {
    Air,
    Stone,
    Dirt,
    LightGrass,
    Leaves,
    TreeTrunk,
    Grass,
    Sand,

    /// Temporary flowing water substitute
    SolidWater,
}

impl GeneratedBlock {
    pub fn is_air(&self) -> bool {
        matches!(self.ty, BlockType::Air)
    }
}

impl Default for BlockType {
    fn default() -> Self {
        Self::Air
    }
}

impl BlockType {
    /// (hue, saturation)
    #[cfg(feature = "bin")]
    pub fn color_hs(&self) -> (f32, f32) {
        match self {
            BlockType::Air => (0.0, 0.0), // unused
            BlockType::Stone => (0.66, 0.005),
            BlockType::Dirt => (0.06, 0.4),
            BlockType::Grass => (0.26, 0.16),
            BlockType::LightGrass => (0.26, 0.10),
            BlockType::Leaves => (0.24, 0.50),
            BlockType::TreeTrunk => (0.1, 0.3),
            BlockType::Sand => (0.14, 0.19),
            BlockType::SolidWater => (0.22, 0.22),
        }
    }
}

impl From<BlockType> for GeneratedBlock {
    fn from(ty: BlockType) -> Self {
        Self { ty }
    }
}
