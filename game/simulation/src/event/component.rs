use crate::activity::{EquipItemError, UseHeldItemError};
use crate::ecs::*;
use crate::event::pubsub::EventDispatcher;
use crate::event::EventSubscription;
use crate::item::{PickupItemError, SlotReference};
use crate::path::PathToken;
use common::{num_derive::FromPrimitive, num_traits};
use strum_macros::EnumDiscriminants;
use unit::world::WorldPoint;
use world::NavigationError;

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct EventsComponent {
    pub dispatcher: EventDispatcher,
}

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

    /// Item entity has been used to completion
    UsedUp(Result<(), UseHeldItemError>),

    /// Item entity has been equipped in the specified base slot
    Equipped(Result<SlotReference, EquipItemError>),

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
