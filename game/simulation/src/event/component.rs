use crate::ecs::*;
use crate::event::pubsub::EventDispatcher;

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct EventsComponent {
    dispatcher: EventDispatcher,
}
