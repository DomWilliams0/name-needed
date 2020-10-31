use unit::world::{WorldPoint, WorldPosition};

use crate::activity::HaulTarget;
use crate::ecs::Entity;
use crate::item::ItemsToPickUp;

// TODO speed should be specified as an enum for all go??? actions

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum AiAction {
    /// Standing still stupidly and looking stupid
    Nop,

    /// Wander aimlessly
    Wander,

    /// Navigate to the destination for the given reason
    Goto {
        target: WorldPoint,
        reason: &'static str,
    },

    /// Go pickup the (1) best item
    GoPickUp(ItemsToPickUp),

    /// Equip and eat the given entity, assuming it's already in the inventory
    EatHeldItem(Entity),

    /// Go break the given block
    GoBreakBlock(WorldPosition),

    /// Follow the entity, keeping to the given distance
    Follow { target: Entity, radius: u8 },

    /// Haul the entity from the source to the destination target
    Haul(Entity, HaulTarget, HaulTarget),
}

impl Default for AiAction {
    fn default() -> Self {
        AiAction::Nop
    }
}
