use crate::ecs::*;
use crate::event::pubsub::EventDispatcher;
use crate::event::EventSubscription;
use strum_macros::EnumDiscriminants;
use unit::world::WorldPoint;

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct EventsComponent {
    pub dispatcher: EventDispatcher,
}

#[derive(EnumDiscriminants, Clone, Debug)]
#[strum_discriminants(name(EntityEventType), derive(Hash))]
pub enum EntityEventPayload {
    /// Completed path finding to target
    Arrived(WorldPoint),

    /// Item entity picked up by a holder
    PickedUp(Entity),

    #[cfg(test)]
    Dummy,
}

#[derive(Clone, Debug)]
pub struct EntityEvent(pub Entity, pub EntityEventPayload);

#[derive(Clone)]
pub struct EntityEventSubscription(pub Entity, pub EventSubscription);
