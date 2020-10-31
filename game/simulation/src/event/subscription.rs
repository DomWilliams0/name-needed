use crate::activity::{EquipItemError, HaulError, PickupItemError};
use crate::ecs::*;
use crate::event::timer::TimerToken;

use crate::path::PathToken;
use common::{num_derive::FromPrimitive, num_traits};
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

    /// Item entity (subject) picked up by the given holder
    PickedUp(Result<Entity, PickupItemError>),

    /// Food entity has been fully eaten
    Eaten(Result<(), ()>),

    /// Item entity (subject) has been equipped in an equip slot of this entity
    Equipped(Result<Entity, EquipItemError>),

    /// Item entity (subject) has been picked up for hauling by a hauler
    Hauled(Result<Entity, HaulError>),

    /// Item entity has been removed from a container
    ExitedContainer(Result<Entity, HaulError>),

    /// Item entity has been inserted into a container
    EnteredContainer(Result<Entity, HaulError>),

    /// Timer elapsed
    TimerElapsed(TimerToken),

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

#[derive(Clone, Debug)]
pub enum EventSubscription {
    All,
    Specific(EntityEventType),
}

#[derive(Clone, Debug)]
pub struct EntityEventSubscription(#[doc = "Subject"] pub Entity, pub EventSubscription);

impl EntityEventSubscription {
    pub fn matches(&self, event: &EntityEvent) -> bool {
        if event.subject != self.0 {
            return false;
        }

        match self.1 {
            EventSubscription::All => true,
            EventSubscription::Specific(ty) => EntityEventType::from(&event.payload) == ty,
        }
    }
}

impl EntityEventPayload {
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::PickedUp(_) | Self::Eaten(_) | Self::Hauled(_) | Self::ExitedContainer(_) | Self::EnteredContainer(_))
    }
}
