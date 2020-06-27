use unit::world::{WorldPoint, WorldPosition};

use crate::ai::activity::ItemsToPickUp;
use crate::item::LooseItemReference;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum AiAction {
    Nop,

    Wander,

    Goto(WorldPoint),

    GoPickUp(ItemsToPickUp),

    UseHeldItem(LooseItemReference),

    GoBreakBlock(WorldPosition),
}

impl Default for AiAction {
    fn default() -> Self {
        AiAction::Nop
    }
}
