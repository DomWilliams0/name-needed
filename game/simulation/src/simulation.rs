use std::ops::Add;

use common::*;
use resources::Resources;
use strum_macros::EnumDiscriminants;
use unit::world::WorldPositionRange;
use world::block::BlockType;
use world::loader::{TerrainUpdatesRes, WorldTerrainUpdate};
use world::WorldChangeEvent;

use crate::activity::{ActivityEventSystem, ActivitySystem};
use crate::ai::{AiAction, AiComponent, AiSystem};

use crate::ecs::*;
use crate::event::{EntityEventQueue, EntityTimers};
use crate::input::{
    BlockPlacement, DivineInputCommand, InputEvent, InputSystem, SelectedEntity, SelectedTiles,
    UiCommand, UiRequest, UiResponsePayload,
};
use crate::item::{ContainerComponent, HaulSystem};
use crate::movement::MovementFulfilmentSystem;
use crate::needs::{EatingSystem, HungerSystem};
use crate::path::{NavigationAreaDebugRenderer, PathDebugRenderer, PathSteeringSystem};
use crate::physics::PhysicsSystem;
use crate::queued_update::QueuedUpdates;
use crate::render::{
    AxesDebugRenderer, ChunkBoundariesDebugRenderer, DebugRendererError, DebugRenderers,
    DebugRenderersState,
};
use crate::render::{RenderSystem, Renderer};
use crate::senses::{SensesDebugRenderer, SensesSystem};

use crate::scripting::ScriptingContext;
use crate::society::PlayerSociety;
use crate::spatial::{Spatial, SpatialSystem};
use crate::steer::{SteeringDebugRenderer, SteeringSystem};
use crate::world_debug::FeatureBoundaryDebugRenderer;
use crate::{
    definitions, EntityLoggingComponent, Exit, ThreadedWorldLoader, WorldRef, WorldViewer,
};
use crate::{ComponentWorld, Societies, SocietyHandle};
use std::collections::HashSet;

#[derive(Debug, EnumDiscriminants)]
#[strum_discriminants(name(AssociatedBlockDataType))]
#[non_exhaustive]
pub enum AssociatedBlockData {
    Container(Entity),
}

pub struct WorldContext;

/// Monotonically increasing tick counter. Defaults to 0, the tick BEFORE the game starts, never
/// produced in tick()
static mut TICK: u32 = 0;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
/// Represents a game tick
pub struct Tick(u32);

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,

    world_loader: ThreadedWorldLoader,

    /// Terrain updates, queued and applied per tick
    /// TODO if order matters, use an IndexSet instead
    terrain_changes: HashSet<WorldTerrainUpdate>,

    /// World change events populated during terrain updates, consumed every tick
    change_events: Vec<WorldChangeEvent>,

    debug_renderers: DebugRenderers<R>,
    scripting: ScriptingContext,
}

/// A little bundle of references to the game state without the generic [Renderer] param on [Simulation]
pub struct SimulationRef<'s> {
    pub ecs: &'s EcsWorld,
    pub world: &'s WorldRef,
    pub loader: &'s ThreadedWorldLoader,
    pub viewer: &'s WorldViewer,
    pub debug_renderers: &'s DebugRenderersState,
}

impl world::WorldContext for WorldContext {
    type AssociatedBlockData = AssociatedBlockData;
}

impl<R: Renderer> Simulation<R> {
    /// world_loader should have had some slabs requested
    pub fn new(world_loader: ThreadedWorldLoader, resources: Resources) -> BoxedResult<Self> {
        // load entity definitions from file system
        let definitions = {
            let def_root = resources.definitions()?;
            definitions::load(def_root)?
        };

        let voxel_world = world_loader.world();

        // make ecs world and insert resources
        let mut ecs_world = EcsWorld::new();
        ecs_world.insert(voxel_world.clone());
        ecs_world.insert(definitions);
        register_resources(&mut ecs_world);

        let debug_renderers = register_debug_renderers()?;

        // ensure tick is reset
        reset_tick();

        Ok(Self {
            ecs_world,
            voxel_world,
            world_loader,
            debug_renderers,
            terrain_changes: HashSet::with_capacity(1024),
            change_events: Vec::with_capacity(1024),
            scripting: ScriptingContext::new()?,
        })
    }

    pub fn tick(
        &mut self,
        commands: impl Iterator<Item = UiCommand>,
        world_viewer: &mut WorldViewer,
    ) -> Option<Exit> {
        // update tick
        increment_tick();

        // TODO sort out systems so they all have an ecs_world reference and can keep state
        // safety: only lives for the duration of this tick
        let ecs_ref = unsafe { EcsWorldFrameRef::init(&self.ecs_world) };
        self.ecs_world.insert(ecs_ref);

        // TODO limit time/count
        self.apply_world_updates(world_viewer);

        // apply player inputs
        let exit = self.process_ui_commands(commands);

        // tick game logic
        self.tick_systems();

        // we're about to go mutable, drop this fuzzy ball of unsafeness
        let _ = self.ecs_world.remove::<EcsWorldFrameRef>();

        // per tick maintenance
        // must remove resource from world first so we can use &mut ecs_world
        let mut updates = self.ecs_world.remove::<QueuedUpdates>().unwrap();
        updates.execute(&mut self.ecs_world);
        self.ecs_world.insert(updates);

        self.ecs_world.maintain();

        exit
    }

    fn tick_systems(&mut self) {
        // validate inventory soundness
        #[cfg(debug_assertions)]
        crate::item::validation::InventoryValidationSystem.run_now(&self.ecs_world);

        // needs
        HungerSystem.run_now(&self.ecs_world);
        EatingSystem.run_now(&self.ecs_world);

        // update senses
        SensesSystem.run_now(&self.ecs_world);

        // choose and tick activity
        AiSystem.run_now(&self.ecs_world);
        ActivitySystem.run_now(&self.ecs_world);

        // follow paths with steering
        PathSteeringSystem.run_now(&self.ecs_world);

        // apply steering
        SteeringSystem.run_now(&self.ecs_world);

        // attempt to fulfil desired velocity
        MovementFulfilmentSystem.run_now(&self.ecs_world);

        // process entity events
        ActivityEventSystem.run_now(&self.ecs_world);

        // apply physics
        PhysicsSystem.run_now(&self.ecs_world);

        // sync hauled item positions
        HaulSystem.run_now(&self.ecs_world);

        // update spatial
        SpatialSystem.run_now(&self.ecs_world);
    }

    pub fn voxel_world(&self) -> WorldRef {
        self.voxel_world.clone()
    }

    pub fn world_mut(&mut self) -> &mut EcsWorld {
        &mut self.ecs_world
    }

    pub fn world(&self) -> &EcsWorld {
        &self.ecs_world
    }

    pub fn societies(&mut self) -> &mut Societies {
        self.ecs_world.resource_mut()
    }

    pub fn player_society(&mut self) -> &mut Option<SocietyHandle> {
        &mut self.ecs_world.resource_mut::<PlayerSociety>().0
    }

    fn apply_world_updates(&mut self, world_viewer: &mut WorldViewer) {
        // request new slabs
        let discovered = empty(); // TODO include slabs discovered by members of player's society
        let requested_slabs = world_viewer.requested_slabs(discovered);
        let actual_requested_slabs = requested_slabs.as_ref().iter().copied();
        self.world_loader.request_slabs(actual_requested_slabs);
        drop(requested_slabs);

        let mut world = self.voxel_world.borrow_mut();

        // apply occlusion updates
        self.world_loader
            .iter_occlusion_updates(|update| world.apply_occlusion_update(update));

        // mark modified slabs as dirty in world viewer, which will cache it until the slab is visible
        world.dirty_slabs().for_each(|s| world_viewer.mark_dirty(s));
        drop(world);

        // aggregate all terrain changes for this tick
        let updates = &mut self.terrain_changes;
        self.world_loader.steal_queued_block_updates(updates);
        updates.extend(self.ecs_world.resource_mut::<TerrainUpdatesRes>().drain(..));

        // apply all applicable terrain changes
        {
            let n_before = updates.len();
            self.world_loader
                .apply_terrain_updates(updates, &mut self.change_events);
            let n_after = updates.len();

            debug_assert!(n_after <= n_before);
            if n_before > 0 {
                debug!(
                    "applied {applied} terrain updates, deferring {deferred}",
                    applied = n_before - n_after,
                    deferred = n_after
                );
            }
        }

        // consume change events
        let mut events = std::mem::take(&mut self.change_events);
        events
            .drain(..)
            .filter(|e| e.new != e.prev)
            .for_each(|e| self.on_world_change(e));

        // swap storage back and forget empty vec
        let empty = std::mem::replace(&mut self.change_events, events);
        debug_assert!(empty.is_empty());
        std::mem::forget(empty);
    }

    fn process_ui_commands(&mut self, commands: impl Iterator<Item = UiCommand>) -> Option<Exit> {
        let mut exit = None;
        for cmd in commands {
            let (req, resp) = cmd.consume();
            match req {
                UiRequest::DisableAllDebugRenderers => {
                    self.debug_renderers.disable_all();
                }

                UiRequest::SetDebugRendererEnabled { ident, enabled } => {
                    if let Err(e) = self.debug_renderers.set_enabled(ident, enabled) {
                        warn!("failed to set debug renderer state"; "error" => %e);
                    }
                }

                UiRequest::FillSelectedTiles(placement, block_type) => {
                    let selection = self.ecs_world.resource::<SelectedTiles>();
                    if let Some((mut from, mut to)) = selection.bounds() {
                        if let BlockPlacement::PlaceAbove = placement {
                            // move the range up 1 block
                            from = from.above();
                            to = to.above();
                        }
                        let range = WorldPositionRange::with_inclusive_range(from, to);
                        debug!("filling in block range"; "range" => ?range, "block_type" => ?block_type);

                        self.terrain_changes
                            .insert(WorldTerrainUpdate::new(range, block_type));
                    }
                }
                UiRequest::IssueDivineCommand(ref divine_command) => {
                    let entity = match self
                        .ecs_world
                        .resource_mut::<SelectedEntity>()
                        .get(&self.ecs_world)
                    {
                        Some(e) => e,
                        None => {
                            warn!("no selected entity to issue divine command to"; "command" => ?divine_command);
                            continue;
                        }
                    };

                    let command = match divine_command {
                        DivineInputCommand::Goto(pos) => AiAction::Goto {
                            target: pos.centred(),
                            reason: "I said so",
                        },
                        DivineInputCommand::Break(pos) => AiAction::GoBreakBlock(*pos),
                    };

                    match self.ecs_world.component_mut::<AiComponent>(entity) {
                        Err(e) => warn!("can't issue divine command"; "error" => %e),
                        Ok(ai) => {
                            // add DSE
                            ai.add_divine_command(command.clone());
                        }
                    }
                }
                UiRequest::IssueSocietyCommand(society, command) => {
                    let job = match command.into_job(&self.ecs_world) {
                        Ok(job) => job,
                        Err(cmd) => {
                            warn!("failed to issue society command"; "command" => ?cmd);
                            continue;
                        }
                    };

                    let society = match self.societies().society_by_handle_mut(society) {
                        Some(s) => s,
                        None => {
                            warn!("invalid society while issuing job"; "society" => ?society, "job" => ?job);
                            continue;
                        }
                    };

                    debug!("submitting job to society"; "society" => ?society, "job" => ?job);
                    society.jobs_mut().submit(job);
                }
                UiRequest::SetContainerOwnership {
                    container,
                    owner,
                    communal,
                } => {
                    match self
                        .ecs_world
                        .component_mut::<ContainerComponent>(container)
                    {
                        Err(e) => {
                            warn!("invalid container entity"; "entity" => E(container), "error" => %e);
                            continue;
                        }
                        Ok(c) => {
                            if let Some(owner) = owner {
                                c.owner = owner;
                                info!("set container owner"; "container" => E(container), "owner" => owner.map(E))
                            }

                            if let Some(communal) = communal {
                                if let Err(e) = self
                                    .ecs_world
                                    .helpers_containers()
                                    .set_container_communal(container, communal)
                                {
                                    warn!("failed to set container society"; "container" => E(container), "society" => ?communal, "error" => %e);
                                }
                            }
                        }
                    }
                }
                UiRequest::ExitGame(ex) => exit = Some(ex),
                UiRequest::ExecuteScript(path) => {
                    info!("executing script"; "path" => %path.display());
                    let result = self
                        .scripting
                        .eval_path(&path, &self.ecs_world, &self.voxel_world)
                        .map(|output| output.into_string());

                    if let Err(err) = result.as_ref() {
                        warn!("script errored"; "error" => %err);
                    }

                    resp.set_response(UiResponsePayload::ScriptOutput(result));
                }
                UiRequest::ToggleEntityLogging { entity, enabled } => {
                    if enabled {
                        let _ = self
                            .ecs_world
                            .add_now::<EntityLoggingComponent>(entity, Default::default());
                    } else {
                        let _ = self.ecs_world.remove_now::<EntityLoggingComponent>(entity);
                    }
                }
            }
        }

        exit
    }

    /// Target is for this frame only
    pub fn render(
        &mut self,
        world_viewer: &WorldViewer,
        target: R::Target,
        renderer: &mut R,
        interpolation: f64,
        input: &[InputEvent],
    ) -> R::Target {
        // process input before rendering
        InputSystem { events: input }.run_now(&self.ecs_world);

        // start frame
        renderer.init(target);

        // render simulation
        {
            renderer.sim_start();
            {
                let mut render_system = RenderSystem {
                    renderer,
                    slices: world_viewer.entity_range(),
                    interpolation: interpolation as f32,
                };

                render_system.run_now(&self.ecs_world);
            }
            if let Err(e) = renderer.sim_finish() {
                warn!("render sim_finish() failed"; "error" => %e);
            }
        }

        // render debug shapes
        {
            renderer.debug_start();
            let ecs_world = &self.ecs_world;
            let voxel_world = self.voxel_world.borrow();
            let world_loader = &self.world_loader;

            self.debug_renderers.iter_enabled().for_each(|r| {
                r.render(
                    renderer,
                    &voxel_world,
                    world_loader,
                    ecs_world,
                    world_viewer,
                )
            });

            if let Err(e) = renderer.debug_finish() {
                warn!("render debug_finish() failed"; "error" => %e);
            }
        }

        // end frame
        renderer.deinit()
    }

    fn on_world_change(&mut self, WorldChangeEvent { pos, prev, new }: WorldChangeEvent) {
        debug_assert_ne!(prev, new);

        match (prev, new) {
            (_, BlockType::Chest) => {
                // new chest placed
                if let Err(err) = self
                    .ecs_world
                    .helpers_containers()
                    .create_container(pos, "core_storage_chest")
                {
                    error!("failed to create container entity"; "error" => %err);
                }
            }

            (BlockType::Chest, _) => {
                // chest destroyed
                if let Err(err) = self.ecs_world.helpers_containers().destroy_container(pos) {
                    error!("failed to destroy container"; "error" => %err);
                }
            }

            _ => {}
        }
    }

    pub fn as_ref<'a>(&'a self, viewer: &'a WorldViewer) -> SimulationRef<'a> {
        SimulationRef {
            ecs: &self.ecs_world,
            world: &self.voxel_world,
            loader: &self.world_loader,
            viewer,
            debug_renderers: self.debug_renderers.state(),
        }
    }
}

fn increment_tick() {
    // safety: called before ticking systems
    unsafe {
        TICK += 1;
    }
}

fn reset_tick() {
    // safety: called before ticking systems
    unsafe {
        TICK = 0;
    }
}

pub fn current_tick() -> u32 {
    // safety: only modified between ticks
    unsafe { TICK }
}

impl Tick {
    pub fn fetch() -> Self {
        Self(current_tick())
    }

    pub fn value(self) -> u32 {
        self.0
    }

    #[cfg(test)]
    pub fn with(tick: u32) -> Self {
        Self(tick)
    }
}

impl Add<u32> for Tick {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0 + rhs)
    }
}

fn register_resources(world: &mut EcsWorld) {
    world.insert(QueuedUpdates::default());
    world.insert(SelectedEntity::default());
    world.insert(SelectedTiles::default());
    world.insert(TerrainUpdatesRes::default());
    world.insert(Societies::default());
    world.insert(PlayerSociety::default());
    world.insert(EntityEventQueue::default());
    world.insert(Spatial::default());
    world.insert(EntityTimers::default());
}

fn register_debug_renderers<R: Renderer>() -> Result<DebugRenderers<R>, DebugRendererError> {
    let mut builder = DebugRenderers::builder();

    // order is preserved in ui
    builder.register::<AxesDebugRenderer>()?;
    builder.register::<ChunkBoundariesDebugRenderer>()?;
    builder.register::<SteeringDebugRenderer>()?;
    builder.register::<PathDebugRenderer>()?;
    builder.register::<NavigationAreaDebugRenderer>()?;
    builder.register::<SensesDebugRenderer>()?;
    builder.register::<FeatureBoundaryDebugRenderer>()?;

    Ok(builder.build())
}
