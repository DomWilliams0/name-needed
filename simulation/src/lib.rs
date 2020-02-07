pub use ::physics::TICKS_PER_SECOND;

pub use crate::backend::{EventsOutcome, SimulationBackend};
pub use crate::movement::{AXIS_FWD, AXIS_UP};
pub use crate::render::{PhysicalComponent, Renderer};
pub use crate::simulation::Simulation;
pub use crate::transform::TransformComponent;

mod transform;
mod movement;
mod sync;
mod path;
mod physics;
mod render;
mod simulation;
mod steer;
mod ecs;
mod backend;

