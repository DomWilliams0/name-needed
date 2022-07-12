//! Block type definitions, shared between procgen, the voxel world and simulation. Will eventually
//! be defined in data instead
// TODO define block types in data

use common::derive_more::Display;
use strum::{EnumIter, EnumString};

impl Default for BlockType {
    fn default() -> Self {
        todo!()
    }
}

#[derive(
    Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, EnumIter, EnumString, Display,
)]
pub enum BlockType {
    Nope, // Air,
          // Dirt,
          // Grass,
          // #[display(fmt = "Light grass")]
          // LightGrass,
          // Leaves,
          // #[display(fmt = "Tree trunk")]
          // TreeTrunk,
          // Stone,
          // Sand,
          // #[display(fmt = "Solid water")]
          // SolidWater,
          //
          // /// Temporary substitute for something to build
          // #[display(fmt = "Stone wall")]
          // StoneBrickWall,
          //
          // Chest,
}
