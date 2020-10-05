use std::fmt::{Display, Formatter};

use unit::length::Length3;
use unit::volume::Volume;

use crate::ecs::{Entity, E};

mod component;
mod container;
mod equip;

pub use component::{FoundSlot, InventoryComponent};
pub use container::{Container, ContainerError};

#[derive(Debug, Clone)]
pub struct HeldEntity {
    pub entity: Entity,
    pub volume: Volume,
    pub half_dims: Length3,
}

impl Display for HeldEntity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (size {})", E(self.entity), self.volume)
    }
}
