pub use collider::{ColliderData, ColliderHandle};
pub use world::{EntityJumpAction, PhysicsWorld, SlabCollider, StepType};

mod collider;
mod world;

pub const TICKS_PER_SECOND: usize = 20;
