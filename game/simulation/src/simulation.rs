use std::collections::HashSet;
use std::ops::{Add, Deref};
use std::pin::Pin;

use strum::EnumDiscriminants;

use common::*;
use resources::Resources;

use unit::world::{WorldPosition, WorldPositionRange};
use world::block::BlockType;
use world::loader::{TerrainUpdatesRes, WorldTerrainUpdate};
use world::WorldChangeEvent;
use world_types::EntityDescription;

use crate::activity::ActivitySystem;
use crate::ai::{AiComponent, AiSystem};
use crate::alloc::FrameAllocator;
use crate::backend::TickResponse;
use crate::ecs::*;
use crate::event::{DeathReason, EntityEventQueue, RuntimeTimers};
use crate::input::{
    BlockPlacement, InputEvent, InputSystem, MouseLocation, SelectedEntities, SelectedTiles,
    UiCommand, UiPopup, UiRequest, UiResponsePayload,
};
use crate::interact::herd::{HerdDebugRenderer, HerdJoiningSystem, Herds};
use crate::item::{ContainerComponent, HaulSystem};
use crate::movement::MovementFulfilmentSystem;
use crate::needs::food::{EatingSystem, HungerSystem};
use crate::path::{NavigationAreaDebugRenderer, PathDebugRenderer, PathSteeringSystem};
use crate::physics::PhysicsSystem;
use crate::queued_update::QueuedUpdates;
use crate::render::{
    AxesDebugRenderer, ChunkBoundariesDebugRenderer, DebugRendererError, DebugRenderers,
    DebugRenderersState, UiElementPruneSystem,
};
use crate::render::{RenderSystem, Renderer};
use crate::runtime::{Runtime, RuntimeSystem};
use crate::scripting::ScriptingContext;
use crate::senses::{SensesDebugRenderer, SensesSystem};
use crate::society::{NameGeneration, PlayerSociety};
use crate::spatial::{Spatial, SpatialSystem};
use crate::steer::{SteeringDebugRenderer, SteeringSystem};
use crate::string::StringCache;
use crate::world_debug::FeatureBoundaryDebugRenderer;
use crate::{
    definitions, BackendData, EntityEvent, EntityEventPayload, EntityLoggingComponent,
    ThreadedWorldLoader, WorldRef, WorldViewer,
};
use crate::{ComponentWorld, Societies, SocietyHandle};

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

#[derive(Copy, Clone)]
enum RunStatus {
    Running,
    Paused,
}

pub struct Simulation<R: Renderer> {
    ecs_world: Pin<Box<EcsWorld>>,
    voxel_world: WorldRef,
    running: RunStatus,

    /// Last interpolation passed to renderer, to reuse when paused
    last_interpolation: f64,

    world_loader: ThreadedWorldLoader,

    /// Terrain updates, queued and applied per tick
    /// TODO if order matters, use an IndexSet instead
    terrain_changes: HashSet<WorldTerrainUpdate>,

    /// World change events populated during terrain updates, consumed every tick
    change_events: Vec<WorldChangeEvent>,

    debug_renderers: DebugRenderers<R>,
    scripting: ScriptingContext,

    /// One off system that caches some allocations
    display_text_system: DisplayTextSystem,
}

/// A little bundle of references to the game state without the generic [Renderer] param
/// on [Simulation], and no renderer fields
pub struct SimulationRefLite<'s> {
    pub ecs: &'s EcsWorld,
    pub world: &'s WorldRef,
    pub loader: &'s ThreadedWorldLoader,
}

/// A little bundle of references to the game state without the generic [Renderer] param
/// on [Simulation], with renderer fields
pub struct SimulationRef<'s> {
    pub ecs: &'s EcsWorld,
    pub world: &'s WorldRef,
    pub loader: &'s ThreadedWorldLoader,
    pub viewer: &'s WorldViewer,
    pub debug_renderers: &'s DebugRenderersState,
}

/// Resource to get the world reference in a system
pub struct EcsWorldRef(Pin<&'static EcsWorld>);

impl world::WorldContext for WorldContext {
    type AssociatedBlockData = AssociatedBlockData;
}

impl<R: Renderer> Simulation<R> {
    /// world_loader should have had some slabs requested
    pub fn new(world_loader: ThreadedWorldLoader, resources: Resources) -> BoxedResult<Self> {
        let string_cache = StringCache::default();

        // load entity definitions from file system
        let definitions = {
            let def_root = resources.definitions()?;
            definitions::load(def_root, &string_cache)?
        };

        let voxel_world = world_loader.world();

        // make ecs world and insert resources
        let mut ecs_world = EcsWorld::with_definitions(definitions)?;
        ecs_world.insert(voxel_world.clone());
        ecs_world.insert(string_cache);
        register_resources(&mut ecs_world, resources)?;

        // get a self referential ecs world resource pointing to itself
        // safety: static lifetime is as long as the game is running, as any system that uses it
        // lives within in
        let mut pinned_world = Box::pin(ecs_world);
        unsafe {
            let w = Pin::get_unchecked_mut(pinned_world.as_mut());
            let world_ptr = w as *mut EcsWorld as *const EcsWorld;

            let pinned_ref = Pin::new(&*world_ptr as &'static EcsWorld);
            w.insert(EcsWorldRef(pinned_ref));
        }

        let debug_renderers = register_debug_renderers()?;

        // ensure tick is reset
        reset_tick();

        Ok(Self {
            ecs_world: pinned_world,
            voxel_world,
            last_interpolation: 0.0,
            running: RunStatus::Running,
            world_loader,
            debug_renderers,
            terrain_changes: HashSet::with_capacity(1024),
            change_events: Vec::with_capacity(1024),
            scripting: ScriptingContext::new()?,
            display_text_system: DisplayTextSystem::default(),
        })
    }

    pub fn tick(
        &mut self,
        commands: impl Iterator<Item = UiCommand>,
        world_viewer: &mut WorldViewer,
        backend_data: &BackendData,
        response: &mut TickResponse,
    ) {
        let _span = tracy_client::Span::new("tick", "Simulation::tick", file!(), line!(), 100);

        // update tick
        if !self.is_paused() {
            increment_tick();
        }

        // TODO sort out systems so they all have an ecs_world reference and can keep state

        // TODO limit time/count
        self.apply_world_updates(world_viewer);

        // process backend input
        if let Some(point) = backend_data.mouse_position {
            let z = {
                let w = self.voxel_world.borrow();
                let range = world_viewer.entity_range();
                let start_from =
                    WorldPosition::new(point.x() as i32, point.y() as i32, range.top());
                match w.find_accessible_block_in_column_with_range(start_from, Some(range.bottom()))
                {
                    Some(pos) => pos.2,
                    None => range.bottom(),
                }
            };
            let pos = point.into_world_point(NotNan::new(z.slice() as f32).unwrap()); // z is not nan
            self.ecs_world.insert(MouseLocation(pos));
        }

        // apply player inputs
        self.process_ui_commands(commands, response);

        // tick game logic
        self.tick_systems();

        // per tick maintenance
        // must remove resource from world first so we can use &mut ecs_world
        let mut updates = self.ecs_world.remove::<QueuedUpdates>().unwrap();
        updates.execute(Pin::as_mut(&mut self.ecs_world));
        self.ecs_world.insert(updates);

        self.delete_queued_entities();
        self.ecs_world.maintain();
    }

    fn is_paused(&self) -> bool {
        matches!(self.running, RunStatus::Paused)
    }

    fn tick_systems(&mut self) {
        macro_rules! run {
            ($system:expr, $name:expr) => {{
                let _span = tracy_client::Span::new($name, "tick_systems", file!(), line!(), 1);
                $system.run_now(&self.ecs_world);
            }};

            ($system:expr) => {
                run!($system, std::stringify!($system))
            };
        }

        // validate inventory soundness
        #[cfg(debug_assertions)]
        {
            use crate::item::validation::InventoryValidationSystem;
            run!(InventoryValidationSystem);
        }

        if !self.is_paused() {
            // needs
            run!(HungerSystem);
            run!(EatingSystem);

            // update senses
            run!(SensesSystem);

            // update herds
            run!(HerdJoiningSystem);

            // choose and tick activity
            run!(AiSystem);
            run!(
                ActivitySystem(Pin::as_ref(&self.ecs_world)),
                "ActivitySystem"
            );
            {
                let _span = tracy_client::Span::new("Runtime", "tick_systems", file!(), line!(), 1);
                self.ecs_world.resource::<Runtime>().tick();
            }

            // follow paths with steering
            run!(PathSteeringSystem);

            // apply steering
            run!(SteeringSystem);

            // attempt to fulfil desired velocity
            run!(MovementFulfilmentSystem);

            // process entity events
            run!(RuntimeSystem);

            // apply physics
            run!(PhysicsSystem);

            // sync hauled item positions
            run!(HaulSystem);

            // update spatial
            run!(SpatialSystem);

            // prune ui elements
            run!(UiElementPruneSystem);

            // prune dead entities from selection
            self.ecs_world
                .resource_mut::<SelectedEntities>()
                .prune(&self.ecs_world);
        }

        // update display text for rendering
        run!(self.display_text_system, "DisplayTextSystem");

        // reset frame bump allocator
        self.ecs_world.resource_mut::<FrameAllocator>().reset();
    }

    fn delete_queued_entities(&mut self) {
        let deathlist_ref = self.ecs_world.resource_mut::<EntitiesToKill>();
        let n = deathlist_ref.count();
        if n > 0 {
            // take out of resource so we can get a mutable world ref
            let deathlist = deathlist_ref.replace_entities(Vec::new());

            if let Err(err) = self.ecs_world.delete_entities(&deathlist) {
                error!("failed to kill entities"; "entities" => ?deathlist, "error" => %err);
            }

            // put it back
            let deathlist_ref = self.ecs_world.resource_mut::<EntitiesToKill>();
            let empty = deathlist_ref.replace_entities(deathlist);
            debug_assert!(empty.is_empty());
            std::mem::forget(empty);

            // post events
            let event_queue = self.ecs_world.resource_mut::<EntityEventQueue>();
            event_queue.post_multiple(deathlist_ref.iter().map(|(e, reason)| EntityEvent {
                subject: e,
                payload: EntityEventPayload::Died(reason),
            }));

            debug!("killed {} entities", n);

            deathlist_ref.clear();
        }
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

    pub fn societies_mut(&mut self) -> &mut Societies {
        self.ecs_world.resource_mut()
    }

    pub fn set_player_society(&mut self, soc: SocietyHandle) {
        self.ecs_world.insert(PlayerSociety::with_society(soc));
    }

    fn apply_world_updates(&mut self, world_viewer: &mut WorldViewer) {
        // request new slabs
        let discovered = empty(); // TODO include slabs discovered by members of player's society
        let requested_slabs = world_viewer.requested_slabs(discovered);
        let actual_requested_slabs = requested_slabs.as_ref().iter().copied();
        self.world_loader.request_slabs(actual_requested_slabs);
        drop(requested_slabs);

        let mut entities_to_spawn = Vec::new();
        {
            let mut world = self.voxel_world.borrow_mut();

            // apply occlusion updates
            self.world_loader
                .iter_occlusion_updates(|update| world.apply_occlusion_update(update));

            // mark modified slabs as dirty in world viewer, which will cache it until the slab is visible
            world.dirty_slabs().for_each(|s| world_viewer.mark_dirty(s));

            // move entity descriptions out of world to release lock asap
            entities_to_spawn.extend(world.entities_to_spawn());
        }

        // spawn entities for newly generated terrain
        self.spawn_entities_from_descriptions(&entities_to_spawn);

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
        self.on_world_changes(&events);
        events.clear();

        // swap storage back and forget empty vec
        std::mem::forget(std::mem::replace(&mut self.change_events, events));
    }

    fn spawn_entities_from_descriptions(&mut self, entities: &[EntityDescription]) {
        for entity in entities {
            // features are generated in parallel and might overlap, so skip entities that collide
            // with blocks
            // TODO depends on bounds of the physical entity size
            // TODO cant hold voxel lock for long, but taking and releasing like this is insane
            {
                let voxel_world = self.voxel_world.borrow();
                match voxel_world.block(entity.position.floor()) {
                    Some(b) if b.block_type().is_air() => { /* safe to place */ }
                    _ => {
                        warn!("skipping plant due to block collision"; "pos" => %entity.position);
                        continue;
                    }
                }
            }

            let builder = match self.ecs_world.build_entity(&entity.desc.species) {
                Ok(b) => b,
                Err(e) => {
                    warn!(
                        "unknown species '{species}'",
                        species = entity.desc.species.as_ref();
                        "error" => %e,
                    );
                    continue;
                }
            };

            // TODO procgen specifies plant rotation too?
            let res = builder
                .with_position(entity.position)
                .doesnt_need_to_be_accessible()
                .spawn();

            match res {
                Err(err) => {
                    warn!("failed to spawn plant: {}", err);
                }
                Ok(e) => {
                    debug!("spawned plant"; e, "pos" => %entity.position);
                }
            };
        }
    }

    fn process_ui_commands(
        &mut self,
        commands: impl Iterator<Item = UiCommand>,
        tick: &mut TickResponse,
    ) {
        for cmd in commands {
            let (req, resp) = cmd.consume();
            match req {
                UiRequest::DisableAllDebugRenderers => {
                    self.debug_renderers.disable_all(&self.ecs_world);
                }

                UiRequest::SetDebugRendererEnabled { ident, enabled } => {
                    if let Err(e) =
                        self.debug_renderers
                            .set_enabled(ident, enabled, &self.ecs_world)
                    {
                        warn!("failed to set debug renderer state"; "error" => %e);
                        if cfg!(debug_assertions) {
                            panic!("unknown debug renderer: {}", e)
                        }
                    }
                }

                UiRequest::FillSelectedTiles(placement, block_type) => {
                    let selection = self.ecs_world.resource::<SelectedTiles>();
                    if let Some((mut from, mut to)) =
                        selection.current_selected().map(|sel| sel.range().bounds())
                    {
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
                UiRequest::IssueDivineCommand(_) | UiRequest::CancelDivineCommand => {
                    let mut ais = self.ecs_world.write_storage::<AiComponent>();
                    let selected_entities = self.ecs_world.resource_mut::<SelectedEntities>();
                    for selected in selected_entities.iter() {
                        if let Some(ai) = selected.get_mut(&mut ais) {
                            if let UiRequest::IssueDivineCommand(ref command) = req {
                                ai.add_divine_command(command.clone());
                            } else {
                                ai.remove_divine_command();
                            }
                        }
                    }
                }
                UiRequest::IssueSocietyCommand(society, command) => {
                    let society = match self
                        .world()
                        .resource::<Societies>()
                        .society_by_handle(society)
                    {
                        Some(s) => s,
                        None => {
                            warn!("invalid society while issuing command"; "society" => ?society, "command" => ?command);
                            continue;
                        }
                    };

                    debug!("submitting command to society"; "society" => ?society, "command" => ?command);
                    if let Err(command) = command.submit_job_to_society(society, &self.ecs_world) {
                        warn!("failed to issue society command"; "command" => ?command);
                        continue;
                    }
                }

                UiRequest::CancelJob(job) => {
                    if let Some(society) = self
                        .world()
                        .resource::<Societies>()
                        .society_by_handle(job.society())
                    {
                        society.jobs_mut().cancel(job);
                    }
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
                            warn!("invalid container entity"; "entity" => container, "error" => %e);
                            continue;
                        }
                        Ok(mut c) => {
                            if let Some(owner) = owner {
                                c.owner = owner;
                                info!("set container owner"; "container" => container, "owner" => owner)
                            }

                            if let Some(communal) = communal {
                                if let Err(e) = self
                                    .ecs_world
                                    .helpers_containers()
                                    .set_container_communal(container, communal)
                                {
                                    warn!("failed to set container society"; "container" => container, "society" => ?communal, "error" => %e);
                                }
                            }
                        }
                    }
                }
                UiRequest::ExitGame(ex) => tick.exit = Some(ex),
                UiRequest::ExecuteScript(path) => {
                    info!("executing script"; "path" => %path.display());
                    let result = self
                        .scripting
                        .eval_path(&path, &*self.ecs_world)
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

                UiRequest::ModifySelection(modification) => {
                    let sel = self.ecs_world.resource_mut::<SelectedTiles>();
                    sel.modify(modification, &self.voxel_world);
                }

                UiRequest::CancelSelection => {
                    // close current popup if there is one
                    let popup = self.ecs_world.resource_mut::<UiPopup>();
                    if !popup.close() {
                        // fallback to clearing tile and entity selections
                        let tiles = self.ecs_world.resource_mut::<SelectedTiles>();
                        let entities = self.ecs_world.resource_mut::<SelectedEntities>();

                        tiles.clear();
                        entities.unselect_all(&self.ecs_world);
                    }
                }
                UiRequest::CancelPopup => {
                    // close current popup only
                    self.ecs_world.resource_mut::<UiPopup>().close();
                }

                UiRequest::TogglePaused => {
                    self.running = match self.running {
                        RunStatus::Running => RunStatus::Paused,
                        RunStatus::Paused => RunStatus::Running,
                    };

                    debug!(
                        "{} gameplay",
                        if self.is_paused() {
                            "paused"
                        } else {
                            "resumed"
                        }
                    )
                }
                UiRequest::ChangeGameSpeed(change) => {
                    tick.speed_change = Some(change);
                }
                UiRequest::Kill(e) => {
                    debug!("killing entity with god powers"; e);
                    self.ecs_world.kill_entity(e, DeathReason::Unknown);
                }
            }
        }
    }

    /// Target is for this frame only
    pub fn render(
        &mut self,
        world_viewer: &WorldViewer,
        target: R::FrameContext,
        renderer: &mut R,
        interpolation: f64,
        input: &[InputEvent],
    ) -> R::FrameContext {
        // process input before rendering
        InputSystem::with_events(input).run_now(&*self.ecs_world);

        // start frame
        renderer.init(target);

        let interpolation = if self.is_paused() {
            // reuse last interpolation to avoid jittering
            self.last_interpolation
        } else {
            self.last_interpolation = interpolation;
            interpolation
        };

        // render simulation
        {
            renderer.sim_start();
            {
                let mut render_system = RenderSystem {
                    renderer,
                    slices: world_viewer.entity_range(),
                    interpolation: interpolation as f32,
                };

                render_system.run_now(&*self.ecs_world);
            }
            if let Err(e) = renderer.sim_finish() {
                warn!("render sim_finish() failed"; "error" => %e);
            }
        }

        // render debug shapes
        {
            renderer.debug_start();
            let ecs_world = &*self.ecs_world;
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

    fn on_world_changes(&mut self, events: &[WorldChangeEvent]) {
        let selection = self.ecs_world.resource_mut::<SelectedTiles>();
        let mut selection_modified = false;

        for &WorldChangeEvent { pos, prev, new } in events {
            match (prev, new) {
                (a, b) if a == b => continue,
                (_, BlockType::Chest) => {
                    // new chest placed
                    if let Err(err) = self
                        .ecs_world
                        .helpers_containers()
                        .create_container_voxel(pos, "core_storage_chest")
                    {
                        error!("failed to create container entity"; "error" => %err);
                    }
                }

                (BlockType::Chest, _) => {
                    // chest destroyed
                    self.ecs_world.resource::<QueuedUpdates>().queue(
                        "destroy container",
                        move |world| match world
                            .helpers_containers()
                            .destroy_container(pos, DeathReason::BlockDestroyed)
                        {
                            Err(err) => {
                                error!("failed to destroy container"; "error" => %err);
                                Err(err.into())
                            }
                            Ok(()) => Ok(()),
                        },
                    )
                }
                _ => {}
            }

            if !selection_modified {
                if let Some(sel) = selection.current_selected() {
                    if sel.range().contains(&pos) {
                        selection_modified = true;
                    }
                }
            }
        }

        if selection_modified {
            selection.on_world_change(&self.voxel_world);
        }
    }

    pub fn as_lite_ref(&self) -> SimulationRefLite {
        SimulationRefLite {
            ecs: &*self.ecs_world,
            world: &self.voxel_world,
            loader: &self.world_loader,
        }
    }

    pub fn as_ref<'a>(&'a self, viewer: &'a WorldViewer) -> SimulationRef<'a> {
        SimulationRef {
            ecs: &*self.ecs_world,
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

    /// self is later
    pub fn elapsed_since(self, other: Self) -> u32 {
        self.0.saturating_sub(other.0)
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

fn register_resources(world: &mut EcsWorld, resources: Resources) -> BoxedResult<()> {
    world.insert(QueuedUpdates::default());
    world.insert(EntitiesToKill::default());
    world.insert(SelectedEntities::default());
    world.insert(SelectedTiles::default());
    world.insert(TerrainUpdatesRes::default());
    world.insert(Societies::default());
    world.insert(PlayerSociety::default());
    world.insert(EntityEventQueue::default());
    world.insert(Spatial::default());
    world.insert(RuntimeTimers::default());
    world.insert(Runtime::default());
    world.insert(MouseLocation::default());
    world.insert(NameGeneration::load(&resources)?);
    world.insert(FrameAllocator::default());
    world.insert(UiPopup::default());
    world.insert(Herds::default());

    Ok(())
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
    builder.register::<EntityIdDebugRenderer>()?;
    builder.register::<AllSocietyVisibilityDebugRenderer>()?;
    builder.register::<HerdDebugRenderer>()?;

    Ok(builder.build())
}

impl Default for EcsWorldRef {
    fn default() -> Self {
        // register manually
        unreachable!()
    }
}

impl Deref for EcsWorldRef {
    type Target = EcsWorld;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
