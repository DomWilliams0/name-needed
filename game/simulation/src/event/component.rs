use crate::ecs::*;
use crate::event::pubsub::EventDispatcher;
use crate::event::EventSubscription;
use crate::item::PickupItemError;
use strum_macros::EnumDiscriminants;
use unit::world::WorldPoint;

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct EventsComponent {
    pub dispatcher: EventDispatcher,
}

#[derive(EnumDiscriminants, Clone, Debug)]
#[strum_discriminants(name(EntityEventType), derive(Hash))]
#[non_exhaustive]
pub enum EntityEventPayload {
    /// Completed path finding to target
    // TODO include path assignment token here
    // TODO Result for when path is aborted or invalided during navigation
    Arrived(WorldPoint),

    /// Item entity picked up by a holder
    PickedUp(Result<Entity, PickupItemError>),

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
