pub use debug::{NavigationAreaDebugRenderer, PathDebugRenderer};
pub use system::{FollowPathComponent, PathSteeringSystem, PathToken};

mod debug;
mod follow;
mod system;

// TODO remove WANDER_SPEED
pub const WANDER_SPEED: f32 = 0.2;
