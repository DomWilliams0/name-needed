mod queue;
mod subscription;
mod timer;

pub use queue::EntityEventQueue;
pub use subscription::{
    DeathReason, EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType,
    EventSubscription,
};

#[cfg(feature = "testing")]
pub use subscription::debug_events::{EntityEventDebugPayload, TaskResultSummary};

pub mod prelude {
    pub use super::{
        EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType,
        EventSubscription,
    };
}

pub use timer::{Timer, TimerToken, Timers, Token};

pub type RuntimeTimers = Timers<crate::runtime::WeakTaskRef, TimerToken>;
