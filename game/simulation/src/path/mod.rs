pub use debug::{NavigationAreaDebugRenderer, PathDebugRenderer};
pub use system::{
    ArrivedAtTargetEventComponent, FollowPathComponent, PathSteeringSystem, PathToken,
};
pub use wander::{WanderComponent, WanderPathAssignmentSystem};

mod debug;
mod follow;
mod system;
mod wander;

// TODO remove WANDER_SPEED
pub const WANDER_SPEED: f32 = 0.2;
