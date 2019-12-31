pub use ::physics::TICKS_PER_SECOND;

pub use crate::movement::{Transform, AXIS_FWD, AXIS_UP};
pub use crate::render::{Physical, Renderer};
pub use crate::simulation::Simulation;

mod movement;
mod sync;
mod path;
mod physics;
mod render;
mod simulation;
mod steer;
mod ecs;

