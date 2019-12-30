pub use collider::{Collider, ColliderData};
pub use world::{PhysicsWorld, SlabCollider, StepType};

mod collider;
mod world;

pub const TICKS_PER_SECOND: usize = 20;
