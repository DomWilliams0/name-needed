// these will be defined in data at some point

use crate::build::material::BuildMaterial;
use std::fmt::Debug;
use std::num::NonZeroU16;
use world::block::BlockType;

pub trait Build: Debug {
    /// Target block
    fn output(&self) -> BlockType;

    /// (number of steps required, ticks to sleep between each step)
    fn progression(&self) -> (u32, u32);

    // TODO can this somehow return an iterator of build materials?
    fn materials(&self, materials_out: &mut Vec<BuildMaterial>);
}

// -------

#[derive(Debug)]
pub struct StoneBrickWall;

impl Build for StoneBrickWall {
    fn output(&self) -> BlockType {
        BlockType::StoneBrickWall
    }

    fn progression(&self) -> (u32, u32) {
        (10, 4)
    }

    fn materials(&self, materials_out: &mut Vec<BuildMaterial>) {
        materials_out.push(BuildMaterial::new(
            "core_brick_stone",
            NonZeroU16::new(6).unwrap(),
        ))
    }
}
