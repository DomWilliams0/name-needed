pub use debug::PathDebugRenderer;
pub use system::{
    ArrivedAtTargetEventComponent, FollowPathComponent, PathSteeringSystem, WanderComponent,
    WanderPathAssignmentSystem,
};

mod debug;
mod follow;
mod system;

// TODO remove WANDER_SPEED
pub const WANDER_SPEED: f32 = 0.2;
