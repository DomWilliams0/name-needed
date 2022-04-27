use std::convert::TryInto;

use strum::EnumDiscriminants;

use common::{num_derive::FromPrimitive, num_traits, Display};
use unit::world::WorldPoint;
use world::NavigationError;

use crate::activity::{EquipItemError, HaulError, LoggedEntityEvent};
use crate::ecs::*;
use crate::interact::herd::HerdHandle;
use crate::needs::food::FoodEatingError;
use crate::path::PathToken;

#[derive(EnumDiscriminants, Clone, Debug)]
#[strum_discriminants(
    name(EntityEventType),
    derive(Hash, FromPrimitive),
    num_traits = "num_traits",
    repr(usize)
)]
#[non_exhaustive]
pub enum EntityEventPayload {
    /// Path finding ended
    Arrived(PathToken, Result<WorldPoint, NavigationError>),

    /// Item entity (subject) picked up by the given holder (.0)
    BeenPickedUp(Entity, Result<(), EquipItemError>),

    /// Entity (subject) has picked up the given item entity
    HasPickedUp(Entity),

    /// Food entity (subject) has been fully eaten by the given living entity
    BeenEaten(Result<Entity, FoodEatingError>),

    /// Hungry entity (subject) has finished eating the given food entity
    HasEaten(Entity),

    /// Item entity (subject) has been equipped in an equip slot of the given entity
    BeenEquipped(Result<Entity, EquipItemError>),

    /// Entity (subject) has equipped the given item entity that was already in their inventory
    HasEquipped(Entity),

    /// Item entity (subject) has been picked up for hauling by the given hauler (.0)
    Hauled(Entity, Result<(), HaulError>),

    /// Item stack entity (subject) was split and then that split stack (Ok(split_stack)) has been
    /// picked up for hauling by the given hauler (.0)
    HauledButSplit(Entity, Result<Entity, HaulError>),

    /// Item entity has been removed from the given container
    ExitedContainer(Result<Entity, HaulError>),

    /// Item entity has been inserted into the given container
    EnteredContainer(Result<Entity, HaulError>),

    /// Item entity (subject) has been added to the given stack entity
    JoinedStack(Entity),

    /// Entity died for the given reason
    Died(DeathReason),

    /// Subject has been promoted to leader of its herd
    PromotedToHerdLeader,

    /// Subject is no longer the leader of the given herd
    DemotedFromHerdLeader(HerdHandle),

    /// Debug event needed for tests only
    #[cfg(feature = "testing")]
    Debug(crate::event::subscription::debug_events::EntityEventDebugPayload),

    #[doc(hidden)]
    #[cfg(test)]
    DummyA,

    #[doc(hidden)]
    #[cfg(test)]
    DummyB,
}

#[cfg_attr(feature = "testing", derive(Eq, PartialEq))]
#[derive(Copy, Clone, Debug, Display)]
// display: "because ..."
pub enum DeathReason {
    /// of unknown reasons
    Unknown,

    /// it was used in a build
    CompletedBuild,

    /// the containing item stack was destroyed
    ParentStackDestroyed,

    /// the block was destroyed
    BlockDestroyed,

    /// it was collapsed into an identical item stack
    CollapsedIntoIdenticalInStack,
}

#[cfg(feature = "testing")]
pub mod debug_events {
    use crate::runtime::TaskResult;

    #[cfg(not(debug_assertions))]
    compile_error!("no testing in release builds!");

    #[derive(Debug, Clone)]
    pub enum TaskResultSummary {
        Cancelled,
        Succeeded,
        Failed(String),
    }

    #[derive(Debug, Clone)]
    pub enum EntityEventDebugPayload {
        /// Current activity finished
        FinishedActivity {
            /// Gross but the only activity description we can get at the moment
            /// TODO type name of activity instead?
            description: String,
            result: TaskResultSummary,
        },
    }

    impl From<&TaskResult> for TaskResultSummary {
        fn from(res: &TaskResult) -> Self {
            match res {
                TaskResult::Cancelled => Self::Cancelled,
                TaskResult::Finished(Ok(_)) => Self::Succeeded,
                TaskResult::Finished(Err(err)) => Self::Failed(err.to_string()),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct EntityEvent {
    pub subject: Entity,
    pub payload: EntityEventPayload,
}

#[derive(Clone, Copy, Debug)]
pub enum EventSubscription {
    All,
    Specific(EntityEventType),
}

#[derive(Clone, Copy, Debug)]
pub struct EntityEventSubscription {
    pub subject: Entity,
    pub subscription: EventSubscription,
}

impl EntityEventSubscription {
    pub fn matches(&self, subject: Entity, event_ty: EntityEventType) -> bool {
        if subject != self.subject {
            return false;
        }

        match self.subscription {
            EventSubscription::All => true,
            EventSubscription::Specific(ty) => event_ty == ty,
        }
    }
}

impl EntityEventPayload {
    /// doer is the living entity interesting in this one, e.g. the hauler, the eater
    pub fn is_destructive_for(&self, doer: Option<Entity>) -> bool {
        use EntityEventPayload::*;

        match self {
            // not destructive if successful and done by the interested entity
            BeenPickedUp(me, Ok(_))
            | BeenEaten(Ok(me))
            | Hauled(me, Ok(_))
            | HauledButSplit(me, _)
                if doer == Some(*me) =>
            {
                false
            }

            // destructive if successful and done by anyone else
            BeenPickedUp(_, Ok(_))
            | BeenEaten(Ok(_))
            | Hauled(_, Ok(_))
            | HauledButSplit(_, Ok(_))
            | ExitedContainer(Ok(_))
            | EnteredContainer(Ok(_)) => true,

            // not destructive on failure
            BeenPickedUp(_, Err(_))
            | BeenEaten(Err(_))
            | Hauled(_, Err(_))
            | HauledButSplit(_, Err(_))
            | ExitedContainer(Err(_))
            | EnteredContainer(Err(_)) => false,

            // not destructive in any case
            Arrived(_, _)
            | HasPickedUp(_)
            | HasEaten(_)
            | HasEquipped(_)
            | BeenEquipped(_)
            | PromotedToHerdLeader
            | DemotedFromHerdLeader(_) => false,

            // always destructive
            JoinedStack(_) | Died(_) => true,

            #[cfg(test)]
            DummyA | DummyB => false,
            #[cfg(feature = "testing")]
            Debug(_) => false,
        }
    }
}

impl TryInto<LoggedEntityEvent> for &EntityEventPayload {
    type Error = ();

    fn try_into(self) -> Result<LoggedEntityEvent, Self::Error> {
        use EntityEventPayload::*;
        use LoggedEntityEvent as E;

        match self {
            HasEquipped(e) => Ok(E::Equipped(*e)),
            HasEaten(e) => Ok(E::Eaten(*e)),
            HasPickedUp(e) => Ok(E::PickedUp(*e)),
            Died(reason) => Ok(E::Died(*reason)),

            PromotedToHerdLeader => E::dev("promoted to herd leader"),
            DemotedFromHerdLeader(h) => E::dev(format!("demoted from leader of {:?}", h)),

            BeenEaten(_)
            | BeenPickedUp(_, _)
            | Arrived(_, _)
            | BeenEquipped(_)
            | Hauled(_, _)
            | HauledButSplit(_, _)
            | ExitedContainer(_)
            | EnteredContainer(_)
            | JoinedStack(_) => Err(()),
            #[cfg(test)]
            DummyA | DummyB => Err(()),
            #[cfg(feature = "testing")]
            Debug(_) => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_event() {
        let world = EcsWorld::new();
        let item = Entity::from(world.create_entity().build());
        let holder = Entity::from(world.create_entity().build());
        let other = Entity::from(world.create_entity().build());

        let non_destructive = vec![
            EntityEvent {
                subject: item,
                payload: EntityEventPayload::Hauled(holder, Ok(())),
            },
            EntityEvent {
                subject: item,
                payload: EntityEventPayload::BeenEaten(Ok(holder)),
            },
        ];

        let destructive = vec![
            EntityEvent {
                subject: item,
                payload: EntityEventPayload::Hauled(other, Ok(())),
            },
            EntityEvent {
                subject: item,
                payload: EntityEventPayload::BeenEaten(Ok(other)),
            },
        ];

        for e in non_destructive {
            assert!(
                !e.payload.is_destructive_for(Some(holder)),
                "event should be non destructive for {}: {:?}",
                holder,
                e
            );
        }

        for e in destructive {
            assert!(
                e.payload.is_destructive_for(Some(holder)),
                "event should be destructive for {}: {:?}",
                holder,
                e
            );
        }
    }
}
