//! Block type definitions, shared between procgen, the voxel world and simulation. Will eventually
//! be defined in data instead
// TODO define block types in data

use common::derive_more::Display;
use strum::{EnumIter, EnumString};
use world::block::{BlockDurability, BlockOpacity};

impl Default for BlockType {
    fn default() -> Self {
        todo!()
    }
}

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

use color::Color;
use BlockType::*;

impl world::BlockType for BlockType {
    const AIR: Self = Air;

    fn opacity(&self) -> BlockOpacity {
        if let Air = self {
            BlockOpacity::Transparent
        } else {
            BlockOpacity::Solid
        }
    }

    fn durability(&self) -> BlockDurability {
        match self {
            Air => 0,
            Leaves => 10,
            Sand => 30,
            Dirt | Grass | LightGrass => 40,
            TreeTrunk => 70,
            Stone => 90,
            Chest | StoneBrickWall => 60,
            SolidWater => u8::MAX,
        }
    }

    fn is_air(&self) -> bool {
        matches!(self, Air)
    }

    fn can_be_walked_on(&self) -> bool {
        !matches!(self, Air | Leaves | SolidWater)
    }

    fn render_color(&self) -> Color {
        match self {
            Air => Color::rgb(0, 0, 0),
            Dirt => Color::rgb(86, 38, 23),
            Grass => Color::rgb(49, 152, 56),
            LightGrass => Color::rgb(91, 152, 51),
            Leaves => Color::rgb(49, 132, 2),
            TreeTrunk => Color::rgb(79, 52, 16),
            Stone => Color::rgb(106, 106, 117),
            Sand => 0xBCA748FF.into(),
            SolidWater => 0x3374BCFF.into(),
            StoneBrickWall => 0x4A4A4AFF.into(),
            Chest => Color::rgb(184, 125, 31),
        }
    }
}
