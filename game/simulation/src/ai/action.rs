use std::convert::TryInto;

use unit::world::{WorldPoint, WorldPosition};

use crate::activity::{
    HaulPurpose, HaulSource, HaulTarget, LoggedEntityDecision, LoggedEntityEvent,
};
use crate::ecs::Entity;
use crate::job::{BuildDetails, SocietyJobHandle};

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

    /// Go work on the given build job, assuming its requirements are already present
    GoBuild {
        job: SocietyJobHandle,
        details: BuildDetails,
    },

    /// Follow the entity, keeping to the given distance
    Follow { target: Entity, radius: u8 },

    /// Haul the entity from the source to the destination target
    Haul(Entity, HaulSource, HaulTarget, HaulPurpose),
}

impl Default for AiAction {
    fn default() -> Self {
        AiAction::Nop
    }
}

impl TryInto<LoggedEntityEvent> for &AiAction {
    type Error = ();

    fn try_into(self) -> Result<LoggedEntityEvent, Self::Error> {
        use AiAction as A;
        use LoggedEntityDecision as B;
        use LoggedEntityEvent::*;

        Ok(AiDecision(match self {
            A::Nop => return Err(()),
            A::Wander => B::Wander,
            A::Goto(target) => B::Goto(*target),
            A::GoEquip(item) => B::GoEquip(*item),
            A::EatHeldItem(item) => B::EatHeldItem(*item),
            A::GoBreakBlock(pos) => B::GoBreakBlock(*pos),
            A::Follow { target, .. } => B::Follow(*target),
            A::Haul(e, _, tgt, _) => B::Haul {
                item: *e,
                dest: *tgt,
            },
            A::GoBuild { .. } => return Err(()), // TODO logging of new events
        }))
    }
}
