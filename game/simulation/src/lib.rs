// Exports from world so the renderer only needs to link against simulation
pub use world::{
    loader::{BlockForAllResult, ThreadedWorkerPool, WorkerPool, WorldLoader},
    presets, BaseVertex, SliceRange, WorldRef, WorldViewer,
};

pub use crate::backend::{EventsOutcome, ExitType, SimulationBackend};
pub use crate::render::{PhysicalComponent, Renderer};
pub use crate::simulation::{Simulation, ThreadedWorldLoader};
pub use crate::transform::TransformComponent;

pub const TICKS_PER_SECOND: usize = 20;

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
