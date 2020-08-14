use crate::ecs::*;
use crate::event::pubsub::EventDispatcher;
use crate::event::EventSubscription;
use crate::item::PickupItemError;
use common::{num_derive::FromPrimitive, num_traits};
use strum_macros::EnumDiscriminants;
use unit::world::WorldPoint;

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
    /// Completed path finding to target
    // TODO include path assignment token here
    // TODO Result for when path is aborted or invalided during navigation
    Arrived(WorldPoint),

    /// Item entity picked up by a holder
    /// (item, picker upper)
    PickedUp(Result<(Entity, Entity), PickupItemError>),

    #[doc(hidden)]
    #[cfg(test)]
    Dummy,
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
