use unit::length::Length3;
use unit::volume::Volume;

use crate::ecs::Entity;

mod component;
mod container;
mod equip;

pub use component::{ContainerComponent, ContainerResolver, FoundSlot, InventoryComponent};
pub use container::{Container, ContainerError};

#[derive(Debug, Clone)]
pub struct HeldEntity {
    pub entity: Entity,
    pub volume: Volume,
    pub size: Length3,
}
