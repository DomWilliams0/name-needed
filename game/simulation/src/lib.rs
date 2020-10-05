#![allow(clippy::type_complexity, clippy::module_inception)]

// Exports from world so the renderer only needs to link against simulation
pub use world::{
    block::{BlockType, IntoEnumIterator},
    loader::{
        BlockForAllError, GeneratedTerrainSource, ThreadedWorkerPool, WorkerPool, WorldLoader,
    },
    presets, BaseVertex, SliceRange, WorldRef, WorldViewer,
};

pub use self::simulation::current_tick;
pub use crate::backend::{state, Exit, InitializedSimulationBackend, PersistentSimulationBackend};
pub use crate::render::{RenderComponent, Renderer, Shape2d};
pub use crate::simulation::{Simulation, ThreadedWorldLoader};
pub use crate::transform::{PhysicalComponent, TransformComponent};
pub use activity::ActivityComponent;
pub use ecs::ComponentWorld;
pub use item::{BaseItemComponent, Container, InventoryComponent};
pub use needs::HungerComponent;
pub use perf::{Perf, PerfAvg, Render, Tick, Timing};
pub use society::{Societies, SocietyComponent, SocietyHandle};

pub const TICKS_PER_SECOND: usize = 20;

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
mod senses;
mod simulation;
mod society;
mod spatial;
mod steer;
mod transform;
