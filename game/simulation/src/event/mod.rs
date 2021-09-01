mod queue;
mod subscription;
mod timer;

pub use queue::EntityEventQueue;
pub use subscription::{
    EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType, EventSubscription,
};

pub mod prelude {
    pub use super::{
        EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType,
        EventSubscription,
    };
}

pub use timer::{Timer, TimerToken, Timers, Token};

pub type RuntimeTimers = Timers<crate::runtime::WeakTaskRef, TimerToken>;
