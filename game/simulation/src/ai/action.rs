use unit::world::{WorldPoint, WorldPosition};

use crate::item::{ItemsToPickUp, LooseItemReference};

// TODO speed should be specified as an enum for all go??? actions

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum AiAction {
    Nop,

    Wander,

    Goto {
        target: WorldPoint,
        reason: &'static str,
    },

    GoPickUp(ItemsToPickUp),

    UseHeldItem(LooseItemReference),

    GoBreakBlock(WorldPosition),
}

impl Default for AiAction {
    fn default() -> Self {
        AiAction::Nop
    }
}
