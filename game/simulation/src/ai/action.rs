use std::convert::TryInto;

use unit::world::{WorldPoint, WorldPosition};

use crate::activity::{HaulTarget, LoggedEntityDecision, LoggedEntityEvent};
use crate::ecs::Entity;
use crate::BlockType;

// TODO speed should be specified as an enum for all go??? actions

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum AiAction {
    /// Standing still stupidly and looking stupid
    Nop,

    /// Wander aimlessly
    Wander,

    /// Navigate to the given target
    Goto(WorldPoint),

    /// Go and pickup the given item
    GoEquip(Entity),

    /// Equip and eat the given entity, assuming it's already in the inventory
    EatHeldItem(Entity),

    /// Go break the given block
    GoBreakBlock(WorldPosition),

    /// Go build the given block
    GoBuildBlock(WorldPosition, BlockType),

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

impl TryInto<LoggedEntityEvent> for &AiAction {
    type Error = ();

    fn try_into(self) -> Result<LoggedEntityEvent, Self::Error> {
        use LoggedEntityDecision::*;
        use LoggedEntityEvent::*;

        Ok(AiDecision(match self {
            AiAction::Nop => return Err(()),
            AiAction::Wander => Wander,
            AiAction::Goto(target) => Goto(*target),
            AiAction::GoEquip(item) => GoEquip(*item),
            AiAction::EatHeldItem(item) => EatHeldItem(*item),
            AiAction::GoBreakBlock(pos) => GoBreakBlock(*pos),
            AiAction::Follow { target, .. } => Follow(*target),
            AiAction::Haul(e, _, tgt) => Haul {
                item: *e,
                dest: *tgt,
            },
            AiAction::GoBuildBlock(_, _) => return Err(()), // TODO logging of new events
        }))
    }
}
