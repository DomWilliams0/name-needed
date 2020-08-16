#![allow(clippy::type_complexity, deprecated)]

// Exports from world so the renderer only needs to link against simulation
pub use world::{
    block::{BlockType, IntoEnumIterator},
    loader::{
        BlockForAllError, GeneratedTerrainSource, ThreadedWorkerPool, WorkerPool, WorldLoader,
    },
    presets, BaseVertex, SliceRange, WorldRef, WorldViewer,
};

pub use crate::backend::{state, Exit, InitializedSimulationBackend, PersistentSimulationBackend};
pub use crate::render::{PhysicalShape, RenderComponent, Renderer};
pub use crate::simulation::{Simulation, ThreadedWorldLoader};
pub use crate::transform::TransformComponent;
pub use ecs::ComponentWorld;
pub use item::{BaseItemComponent, InventoryComponent};
pub use needs::HungerComponent;
pub use perf::{Perf, PerfAvg, Render, Tick, Timing};
pub use simulation::current_tick;
pub use society::{Societies, SocietyComponent, SocietyHandle};

pub const TICKS_PER_SECOND: usize = 20;

#[cfg(test)]
pub use simulation::register_components;

mod activity;
mod ai;
mod backend;
mod definitions;
pub mod dev;
mod ecs;
mod event;
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
