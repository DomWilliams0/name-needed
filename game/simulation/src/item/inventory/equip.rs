use std::fmt::{Display, Formatter};

use crate::ecs::Entity;

use crate::item::inventory::HeldEntity;

// TODO equip slots will require a lot of integration with the body tree, so dont flesh out properly

/// Slot that can equip an item for use, e.g. a hand, dog's mouth
#[derive(Debug)]
pub enum EquipSlot {
    Empty,
    /// First slot holding this entity
    Occupied(HeldEntity),
    /// Extra slots holding a large entity
    Overflow(Entity),
}

impl EquipSlot {
    pub fn is_empty(&self) -> bool {
        matches!(self, EquipSlot::Empty)
    }

    /// Some(e) only for Occupied slots
    pub fn ok(&self) -> Option<Entity> {
        match self {
            EquipSlot::Empty | EquipSlot::Overflow(_) => None,
            EquipSlot::Occupied(HeldEntity { entity, .. }) => Some(*entity),
        }
    }
}

impl Display for EquipSlot {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EquipSlot::Empty => write!(f, "[ ]"),
            EquipSlot::Occupied(e) => write!(f, "[ {} ]", e.entity),
            EquipSlot::Overflow(_) => write!(f, "[ .. ]"),
        }
    }
}
