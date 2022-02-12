pub use queue::EntityEventQueue;
#[cfg(feature = "testing")]
pub use subscription::debug_events::{EntityEventDebugPayload, TaskResultSummary};
pub use subscription::{
    DeathReason, EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType,
    EventSubscription,
};
pub use timer::{Timer, TimerToken, Timers};

mod queue;
mod subscription;
mod timer;

pub mod prelude {
    pub use super::{
        EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType,
        EventSubscription,
    };
}

pub type RuntimeTimers = Timers<crate::runtime::WeakTaskRef>;
