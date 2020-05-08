// pub use ::physics::TICKS_PER_SECOND;
pub const TICKS_PER_SECOND: usize = 20;

pub use crate::backend::{EventsOutcome, ExitType, SimulationBackend};
pub use crate::movement::{AXIS_FWD, AXIS_UP};
pub use crate::render::{PhysicalComponent, Renderer};
pub use crate::simulation::{Simulation, ThreadedWorldLoader};
pub use crate::transform::TransformComponent;

// Exports from world so the renderer only needs to link against simulation
pub use world::{
    loader::{ThreadedWorkerPool, WorkerPool, WorldLoader},
    presets, BaseVertex, WorldRef, WorldViewer,
};

mod backend;
mod ecs;
mod movement;
mod path;
mod physics;
mod render;
mod simulation;
mod steer;
// mod sync;
mod entity_builder;
mod transform;
