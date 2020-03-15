#[cfg(feature = "ipc")]
use serde::{Deserialize, Serialize};

pub type EntityId = u64;

#[derive(Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "ipc", derive(Serialize, Deserialize))]
pub enum EntityEvent {
    /// Entity has been created
    Create(EntityId),

    /// Entity is going to navigate to this position
    NewNavigationTarget(EntityId, (i32, i32, i32)),

    /// Entity reached its target
    NavigationTargetReached(EntityId),

    /// Entity intends to move in this direction
    MovementIntention(EntityId, (f32, f32)),

    /// Entity jumps
    Jump(EntityId),
}

impl EntityEvent {
    pub fn entity_id(&self) -> EntityId {
        *match self {
            EntityEvent::Create(e) => e,
            EntityEvent::NewNavigationTarget(e, _) => e,
            EntityEvent::Jump(e) => e,
            EntityEvent::NavigationTargetReached(e) => e,
            EntityEvent::MovementIntention(e, _) => e,
        }
    }
}
