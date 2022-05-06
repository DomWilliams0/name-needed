#![allow(
    clippy::type_complexity,
    clippy::module_inception,
    clippy::non_send_fields_in_send_ty
)]
#![deny(unused_must_use)]

// Exports from world so the renderer only needs to link against simulation
pub use world::{
    block::BlockType,
    loader::{
        AsyncWorkerPool, BlockForAllError, TerrainSourceError, TerrainUpdatesRes, WorldLoader,
        WorldTerrainUpdate,
    },
    presets, BaseVertex, SliceRange,
};

#[cfg(feature = "procgen")]
pub use world::loader::{GeneratedTerrainSource, PlanetParams};

// Rexports for specialised world types
pub type WorldRef = world::WorldRef<simulation::WorldContext>;
pub type World = world::World<simulation::WorldContext>;
pub type InnerWorldRef<'a> = world::InnerWorldRef<'a, simulation::WorldContext>;
pub type WorldViewer = world::WorldViewer<simulation::WorldContext>;
pub type ThreadedWorldLoader = WorldLoader<simulation::WorldContext>;

pub use self::ai::AiAction;
pub use self::simulation::current_tick;
pub use crate::backend::{
    state, BackendData, Exit, GameSpeedChange, InitializedSimulationBackend,
    PersistentSimulationBackend, TickResponse,
};
pub use crate::render::{RenderComponent, Renderer, Shape2d, UiElementComponent};
pub use crate::simulation::{
    AssociatedBlockData, AssociatedBlockDataType, Simulation, SimulationRef, SimulationRefLite,
    Tick, WorldContext,
};
pub use crate::transform::{PhysicalComponent, TransformComponent, TransformRenderDescription};
pub use activity::{
    ActivityComponent, EntityLoggingComponent, HaulPurpose, HaulSource, HaulTarget,
    LoggedEntityDecision, LoggedEntityEvent,
};
pub use definitions::EntityPosition;
#[cfg(feature = "utils")]
pub use definitions::load as load_definitions;

pub use ecs::{
    Component, ComponentRef, ComponentRefMut, ComponentWorld, EcsWorld, Entity, KindComponent,
    NameComponent,
};
pub use event::{DeathReason, EntityEvent, EntityEventPayload};
#[cfg(feature = "testing")]
pub use event::{EntityEventDebugPayload, TaskResultSummary};

pub use interact::herd::{HerdedComponent, Herds};

pub use build::{BuildMaterial, BuildTemplate};
#[cfg(debug_assertions)]
pub use item::validation::validate_all_inventories;
pub use item::{
    ConditionComponent, ContainedInComponent, Container, ContainerComponent, ContainersError,
    EdibleItemComponent, InventoryComponent, ItemCondition, ItemStack, ItemStackComponent,
    ItemStackError, StackableComponent,
};
pub use needs::food::HungerComponent;
pub use path::FollowPathComponent;
pub use perf::{Perf, PerfAvg, Timing};
pub use queued_update::QueuedUpdates;
pub use runtime::Runtime;
pub use society::{
    job, NameGeneration, PlayerSociety, Societies, SocietyComponent, SocietyHandle,
    SocietyVisibility,
};
pub use species::SpeciesComponent;
pub use string::{CachedStr, StringCache};
pub use strum::IntoEnumIterator;
pub use unit::world::{
    all_slabs_in_range, BlockPosition, ChunkLocation, SlabLocation, WorldPosition,
    WorldPositionRange,
};

pub const TICKS_PER_SECOND: usize = 20;

#[macro_export]
macro_rules! as_any {
    () => {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    };
}

#[macro_export]
macro_rules! as_any_impl {
    () => {
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    };
}

mod activity;
mod ai;
mod alloc;
mod backend;
mod build;
mod definitions;
pub mod dev;
mod ecs;
mod event;
pub mod input;
mod interact;
mod item;
mod movement;
mod needs;
mod path;
mod perf;
mod physics;
mod queued_update;
mod render;
mod runtime;
mod scripting;
mod senses;
mod simulation;
mod society;
mod spatial;
mod species;
mod steer;
mod string;
mod transform;
mod world_debug;
