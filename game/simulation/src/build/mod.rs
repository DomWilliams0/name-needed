use crate::ecs::*;
use crate::job::SocietyJobHandle;
use crate::BlockType;
use common::*;

// TODO organise build module

#[derive(Hash, Clone, Eq, PartialEq)]
pub struct BuildMaterial {
    // TODO flexible list of reqs based on components
    definition_name: &'static str,
    quantity: u16,
}

pub trait Build: Debug {
    /// Target block
    fn output(&self) -> BlockType;

    // TODO can this somehow return an iterator of build materials?
    fn materials(&self, materials_out: &mut Vec<BuildMaterial>);
}

#[derive(Component, EcsComponent, Debug)]
#[storage(HashMapStorage)]
#[name("reserved-material")]
#[clone(disallow)]
pub struct ReservedMaterialComponent {
    pub build_job: SocietyJobHandle,
}

impl BuildMaterial {
    /// Quantity must be >0
    pub fn new(definition_name: &'static str, quantity: u16) -> Self {
        // TODO use NonZeroU16
        assert!(quantity > 0);
        Self {
            definition_name,
            quantity,
        }
    }

    pub fn definition(&self) -> &'static str {
        self.definition_name
    }
    pub fn quantity(&self) -> u16 {
        self.quantity
    }
}

impl Debug for BuildMaterial {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.quantity, self.definition_name)
    }
}

#[derive(Debug)]
pub struct StoneBrickWall;

impl Build for StoneBrickWall {
    fn output(&self) -> BlockType {
        // TODO stone wall block
        BlockType::Stone
    }

    fn materials(&self, materials_out: &mut Vec<BuildMaterial>) {
        materials_out.push(BuildMaterial::new("core_brick_stone", 2))
    }
}
