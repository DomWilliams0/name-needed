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

    /// Item entity picked up by a holder
    /// (item, picker upper)
    PickedUp(Result<(Entity, Entity), PickupItemError>),

    /// Food entity has been fully eaten
    Eaten(Result<(), ()>),

    /// Item entity (subject) has been equipped in an equip slot of this entity
    Equipped(Result<Entity, EquipItemError>),

    /// Item entity has been picked up for hauling by a hauler
    /// (item, hauler)
    Hauled(Result<(Entity, Entity), HaulError>),

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
        match self {
            Self::PickedUp(_) | Self::Eaten(_) | Self::Hauled(_) => true,
            _ => false,
        }
    }
}
