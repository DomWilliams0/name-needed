//! Block type definitions, shared between procgen, the voxel world and simulation. Will eventually
//! be defined in data instead
// TODO define block types in data

use common::{derive_more::Display, Proportion};
use strum::{EnumIter, EnumString};

#[derive(
    Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, EnumIter, EnumString, Display,
)]
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

    /// Temporary substitute for something to build
    #[display(fmt = "Stone wall")]
    StoneBrickWall,

    Chest,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BlockOpacity {
    Transparent,
    Solid,
}

pub type BlockDurability = u8;

impl BlockOpacity {
    pub fn solid(self) -> bool {
        matches!(self, Self::Solid)
    }

    pub fn transparent(self) -> bool {
        matches!(self, Self::Transparent)
    }
}

impl BlockType {
    pub fn opacity(self) -> BlockOpacity {
        if let BlockType::Air = self {
            BlockOpacity::Transparent
        } else {
            BlockOpacity::Solid
        }
    }

    pub fn durability(self) -> Proportion<BlockDurability> {
        use BlockType::*;
        let max = match self {
            Air => 0,
            Leaves => 10,
            Sand => 30,
            Dirt | Grass | LightGrass => 40,
            TreeTrunk => 70,
            Stone => 90,
            Chest | StoneBrickWall => 60,
            SolidWater => u8::MAX,
        };

        Proportion::with_value(max, max)
    }

    /// TODO very temporary "walkability" for block types
    pub fn can_be_walked_on(self) -> bool {
        use BlockType::*;
        !matches!(self, Air | Leaves | SolidWater)
    }

    pub fn is_air(self) -> bool {
        matches!(self, BlockType::Air)
    }
}
