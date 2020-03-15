pub use ::physics::TICKS_PER_SECOND;

pub use crate::backend::{EventsOutcome, ExitType, SimulationBackend};
pub use crate::movement::{AXIS_FWD, AXIS_UP};
pub use crate::render::{PhysicalComponent, Renderer};
pub use crate::simulation::Simulation;
pub use crate::transform::TransformComponent;

// Exports from world so the renderer only needs to link against simulation
pub use world::{presets, Vertex as WorldVertex, WorldRef, WorldViewer};

mod backend;
mod ecs;
mod movement;
mod path;
mod physics;
mod render;
mod simulation;
mod steer;
mod sync;
mod transform;
