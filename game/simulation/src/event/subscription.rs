use crate::activity::{EquipItemError, HaulError, LoggedEntityEvent};
use crate::ecs::*;
use crate::event::timer::TimerToken;

use crate::needs::FoodEatingError;
use crate::path::PathToken;
use common::{num_derive::FromPrimitive, num_traits};
use std::convert::TryInto;
use strum_macros::EnumDiscriminants;
use unit::world::WorldPoint;
use world::NavigationError;

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

    /// Item entity (subject) has been equipped in an equip slot of this entity
    BeenEquipped(Result<Entity, EquipItemError>),

    /// Entity (subject) has equipped the given item entity that was already in their inventory
    HasEquipped(Entity),

    /// Item entity (subject) has been picked up for hauling by a hauler
    Hauled(Result<Entity, HaulError>),

    /// Item entity has been removed from a container
    ExitedContainer(Result<Entity, HaulError>),

    /// Item entity has been inserted into a container
    EnteredContainer(Result<Entity, HaulError>),

    #[doc(hidden)]
    #[cfg(test)]
    DummyA,

    #[doc(hidden)]
    #[cfg(test)]
    DummyB,
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
    pub fn is_destructive(&self) -> bool {
        use EntityEventPayload::*;
        // only destructive on success
        match self {
            BeenPickedUp(_, Ok(_))
            | BeenEaten(Ok(_))
            | Hauled(Ok(_))
            | ExitedContainer(Ok(_))
            | EnteredContainer(Ok(_)) => true,

            Arrived(_, _)
            | BeenPickedUp(_, Err(_))
            | BeenEaten(Err(_))
            | Hauled(Err(_))
            | ExitedContainer(Err(_))
            | EnteredContainer(Err(_))
            | HasPickedUp(_)
            | HasEaten(_)
            | HasEquipped(_)
            | BeenEquipped(_) => false,
            #[cfg(test)]
            DummyA | DummyB => false,
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
            BeenEaten(_)
            | BeenPickedUp(_, _)
            | Arrived(_, _)
            | BeenEquipped(_)
            | Hauled(_)
            | ExitedContainer(_)
            | EnteredContainer(_) => Err(()),
            #[cfg(test)]
            DummyA | DummyB => Err(()),
        }
    }
}
