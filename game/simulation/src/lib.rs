#![allow(clippy::type_complexity, clippy::module_inception)]

// Exports from world so the renderer only needs to link against simulation
pub use world::{
    block::{BlockType, IntoEnumIterator},
    loader::{
        BlockForAllError, GeneratedTerrainSource, TerrainUpdatesRes, ThreadedWorkerPool,
        WorkerPool, WorldLoader, WorldTerrainUpdate,
    },
    presets, BaseVertex, SliceRange,
};

// Rexports for specialised world types
pub type WorldRef = world::WorldRef<simulation::AssociatedBlockData>;
pub type World = world::World<simulation::AssociatedBlockData>;
pub type InnerWorldRef<'a> = world::InnerWorldRef<'a, simulation::AssociatedBlockData>;
pub type WorldViewer = world::WorldViewer<simulation::AssociatedBlockData>;

pub use self::simulation::current_tick;
pub use crate::backend::{state, Exit, InitializedSimulationBackend, PersistentSimulationBackend};
pub use crate::render::{RenderComponent, Renderer, Shape2d};
pub use crate::simulation::{AssociatedBlockData, Simulation, ThreadedWorldLoader};
pub use crate::transform::{PhysicalComponent, TransformComponent};
pub use activity::ActivityComponent;
pub use definitions::EntityPosition;
pub use ecs::{ComponentWorld, EcsWorld, Entity, E};
pub use item::{ConditionComponent, Container, InventoryComponent, NameComponent};
pub use needs::HungerComponent;
pub use perf::{Perf, PerfAvg, Render, Tick, Timing};
pub use society::{job, Societies, SocietyComponent, SocietyHandle};
pub use unit::world::{WorldPosition, WorldPositionRange};

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
