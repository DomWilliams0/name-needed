use crate::block::{Block, BlockDurability, BlockOpacity};
pub use crate::chunk::slice::SLICE_SIZE;
use std::fmt::Debug;
use std::hash::Hash;

pub trait WorldContext: 'static + Send + Sync + Sized {
    type AssociatedBlockData;
    type BlockType: BlockType;
    type GeneratedEntityDesc: GeneratedEntityDesc;

    fn air_slice() -> &'static [Block<Self>; SLICE_SIZE];

    // non-air, solid, walkable blocks to use in presets
    const PRESET_TYPES: [Self::BlockType; 3];
}

pub trait BlockType: Copy + Debug + Eq + Hash + Sync + Send {
    const AIR: Self;

    fn opacity(&self) -> BlockOpacity;
    fn durability(&self) -> BlockDurability;

    fn is_air(&self) -> bool;
    /// TODO very temporary "walkability" for block types
    fn can_be_walked_on(&self) -> bool;

    fn render_color(&self) -> color::Color;
}

pub trait GeneratedEntityDesc: Send + Sync {}
