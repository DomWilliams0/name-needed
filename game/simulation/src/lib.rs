#![allow(clippy::type_complexity, clippy::module_inception)]

// Exports from world so the renderer only needs to link against simulation
pub use world::{
    block::{BlockType, IntoEnumIterator},
    loader::{
        AsyncWorkerPool, BlockForAllError, GeneratedTerrainSource, PlanetParams,
        TerrainSourceError, TerrainUpdatesRes, WorldLoader, WorldTerrainUpdate,
    },
    presets, BaseVertex, SliceRange,
};

// Rexports for specialised world types
pub type WorldRef = world::WorldRef<simulation::WorldContext>;
pub type World = world::World<simulation::WorldContext>;
pub type InnerWorldRef<'a> = world::InnerWorldRef<'a, simulation::WorldContext>;
pub type WorldViewer = world::WorldViewer<simulation::WorldContext>;
pub type ThreadedWorldLoader = WorldLoader<simulation::WorldContext>;

pub use self::simulation::current_tick;
pub use crate::backend::{state, Exit, InitializedSimulationBackend, PersistentSimulationBackend};
pub use crate::render::{RenderComponent, Renderer, Shape2d};
pub use crate::simulation::{
    AssociatedBlockData, AssociatedBlockDataType, Simulation, SimulationRef, SimulationRefLite,
    Tick, WorldContext,
};
pub use crate::transform::{PhysicalComponent, TransformComponent};
pub use activity::{ActivityComponent, EntityLoggingComponent};
pub use definitions::EntityPosition;
pub use ecs::{Component, ComponentWorld, EcsWorld, Entity, E};
pub use item::{
    ConditionComponent, Container, ContainerComponent, EdibleItemComponent, InventoryComponent,
    ItemCondition, NameComponent,
};
pub use needs::HungerComponent;
pub use path::FollowPathComponent;
pub use perf::{Perf, PerfAvg, Timing};
pub use society::{job, PlayerSociety, Societies, SocietyComponent, SocietyHandle};
pub use unit::world::{
    all_slabs_in_range, BlockPosition, ChunkLocation, SlabLocation, WorldPosition,
    WorldPositionRange,
};

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
mod scripting;
mod senses;
mod simulation;
mod society;
mod spatial;
mod steer;
mod transform;
mod world_debug;
