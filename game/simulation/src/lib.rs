#![allow(clippy::type_complexity)]

// Exports from world so the renderer only needs to link against simulation
pub use world::{
    block::{BlockType, IntoEnumIterator},
    loader::{
        BlockForAllResult, GeneratedTerrainSource, ThreadedWorkerPool, WorkerPool, WorldLoader,
    },
    presets, BaseVertex, SliceRange, WorldRef, WorldViewer,
};

pub use crate::backend::{state, Exit, InitializedSimulationBackend, PersistentSimulationBackend};
pub use crate::render::{PhysicalShape, RenderComponent, Renderer};
pub use crate::simulation::{Simulation, ThreadedWorldLoader};
pub use crate::transform::TransformComponent;
pub use ecs::ComponentWorld;
pub use item::InventoryComponent;
pub use perf::{Perf, PerfAvg, Render, Tick, Timing};
pub use society::{Societies, SocietyHandle};

pub const TICKS_PER_SECOND: usize = 20;

mod ai;
mod backend;
pub mod dev;
mod ecs;
mod entity_builder;
pub mod input;
mod item;
mod movement;
mod needs;
mod path;
mod perf;
mod physics;
mod queued_update;
mod render;
mod simulation;
mod society;
mod steer;
mod transform;
