use std::convert::TryInto;

use common::trace;
use unit::world::{WorldPoint, WorldPosition};

use crate::activity::{
    HaulPurpose, HaulSource, HaulTarget, LoggedEntityDecision, LoggedEntityEvent,
};
use crate::ecs::Entity;
use crate::job::{BuildDetails, SocietyJobHandle};
use crate::{ComponentWorld, EcsWorld, ItemStackComponent, Tick};

// TODO speed should be specified as an enum for all go??? actions

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum AiAction {
    /// Standing still stupidly and looking stupid
    Nop,

    /// Wander aimlessly
    Wander,

    /// Navigate to the given target
    Goto(WorldPoint),

    /// Move towards the herd leader
    ReturnToHerd,

    /// Go and pickup the given item
    GoEquip(Entity),

    /// Go and eat the given entity without picking it up
    GoEat(Entity),

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

impl ai::Action for AiAction {
    type Arg = EcsWorld;

    fn cmp(&self, other: &Self, world: &EcsWorld) -> bool {
        use AiAction::*;

        match (self, other) {
            (
                Haul(old_item, old_src, old_tgt, old_purpose),
                Haul(new_item, new_src, new_tgt, new_purpose),
            ) if old_item != new_item
                && old_src == new_src
                && old_tgt == new_tgt
                && old_purpose == new_purpose =>
            {
                // only entity differs
                if let Ok(Some((split_from, tick))) = world
                    .component::<ItemStackComponent>(*new_item)
                    .map(|comp| comp.split_from)
                {
                    if split_from == *old_item && Tick::fetch().elapsed_since(tick) < 100 {
                        trace!("detected haul of split stack, not changing decision";
                                "original_stack" => old_item, "hauled_split_stack" => new_item);
                        return true;
                    }
                }

                false
            }
            (a, b) => a == b,
        }
    }
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
            A::ReturnToHerd => B::ReturnToHerd,
            A::GoEquip(item) => B::GoEquip(*item),
            A::GoEat(item) => B::GoEat(*item),
            A::EatHeldItem(item) => B::EatHeldItem(*item),
            A::GoBreakBlock(pos) => B::GoBreakBlock(*pos),
            A::Follow { target, .. } => B::Follow(*target),
            A::Haul(e, _, tgt, _) => B::Haul {
                item: *e,
                dest: *tgt,
            },
            A::GoBuild { details, .. } => B::GoBuild(details.clone()),
        }))
    }
}
