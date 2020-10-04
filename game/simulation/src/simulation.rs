use std::marker::PhantomData;
use std::ops::Add;
use std::sync::atomic::{AtomicU32, Ordering};

use crossbeam::crossbeam_channel::Receiver;

use common::*;
use resources::resource::Resources;
use unit::world::WorldPositionRange;
use world::loader::{TerrainUpdatesRes, ThreadedWorkerPool, WorldLoader, WorldTerrainUpdate};
use world::{OcclusionChunkUpdate, WorldRef, WorldViewer};

use crate::activity::{ActivityEventSystem, ActivitySystem};
use crate::ai::{AiAction, AiComponent, AiSystem};
use crate::definitions;
use crate::definitions::{DefinitionBuilder, DefinitionErrorKind};
use crate::dev::SimulationDevExt;
use crate::ecs::*;
use crate::event::{EntityEventQueue, EntityTimers};
use crate::input::{
    BlockPlacement, DivineInputCommand, InputEvent, InputSystem, SelectedEntity, SelectedTiles,
    SocietyInputCommand, UiBlackboard, UiCommand,
};
use crate::item::HaulSystem;
use crate::movement::MovementFulfilmentSystem;
use crate::needs::{EatingSystem, HungerSystem};
use crate::path::{NavigationAreaDebugRenderer, PathDebugRenderer, PathSteeringSystem};
use crate::physics::PhysicsSystem;
use crate::queued_update::QueuedUpdates;
use crate::render::{AxesDebugRenderer, DebugRendererError, DebugRenderers};
use crate::render::{RenderSystem, Renderer};
use crate::senses::{SensesDebugRenderer, SensesSystem};
use crate::society::job::{BreakBlocksJob, Job};
use crate::society::{PlayerSociety, Society};
use crate::spatial::{Spatial, SpatialSystem};
use crate::steer::{SteeringDebugRenderer, SteeringSystem};
use crate::{ComponentWorld, Societies, SocietyHandle};

pub type ThreadedWorldLoader = WorldLoader<ThreadedWorkerPool>;

/// Monotonically increasing tick counter. Defaults to 0, the tick BEFORE the game starts, never
/// produced in tick()
static mut TICK: AtomicU32 = AtomicU32::new(0);

#[derive(Copy, Clone, Eq, PartialEq, Default)]
/// Represents a game tick
pub struct Tick(u32);

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,
    definitions: definitions::Registry,

    world_loader: ThreadedWorldLoader,
    /// Occlusion updates received from world loader
    chunk_updates: Receiver<OcclusionChunkUpdate>,

    /// Terrain updates, queued and applied per tick
    terrain_changes: Vec<WorldTerrainUpdate>,

    renderer: PhantomData<R>,
    debug_renderers: DebugRenderers<R>,
}

impl<R: Renderer> Simulation<R> {
    /// world_loader should have had all chunks requested
    pub fn new(mut world_loader: ThreadedWorldLoader, resources: Resources) -> BoxedResult<Self> {
        // load entity definitions from file system
        let definitions = {
            let def_root = resources.definitions()?;
            definitions::load(def_root)?
        };

        // make world and register components
        let mut ecs_world = EcsWorld::new();

        // insert resources
        let voxel_world = world_loader.world();
        ecs_world.insert(voxel_world.clone());
        register_resources(&mut ecs_world);

        let chunk_updates = world_loader.chunk_updates_rx().unwrap();
        let mut debug_renderers = DebugRenderers::new();
        register_debug_renderers(&mut debug_renderers)?;

        Ok(Self {
            definitions,
            ecs_world,
            renderer: PhantomData,
            voxel_world,
            world_loader,
            chunk_updates,
            debug_renderers,
            terrain_changes: Vec::with_capacity(1024),
        })
    }

    pub fn entity_builder(
        &mut self,
        definition_uid: &str,
    ) -> Result<DefinitionBuilder<EcsWorld>, DefinitionErrorKind> {
        self.definitions
            .instantiate(definition_uid, &mut self.ecs_world)
    }

    pub fn tick(&mut self, commands: &[UiCommand], world_viewer: &mut WorldViewer) {
        // update tick
        increment_tick();

        // TODO sort out systems so they all have an ecs_world reference and can keep state
        // safety: only lives for the duration of this tick
        let ecs_ref = unsafe { EcsWorldFrameRef::init(&self.ecs_world) };
        self.ecs_world.insert(ecs_ref);

        // TODO limit time/count
        self.apply_world_updates(world_viewer);

        // apply player inputs
        self.process_ui_commands(commands);

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
    }

    fn tick_systems(&mut self) {
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

        // validate inventory soundness
        #[cfg(debug_assertions)]
        crate::item::validation::InventoryValidationSystem.run_now(&self.ecs_world);

        // apply physics
        PhysicsSystem.run_now(&self.ecs_world);

        // sync hauled item positions
        HaulSystem.run_now(&self.ecs_world);

        // update spatial
        SpatialSystem.run_now(&self.ecs_world);
    }

    pub fn world(&self) -> WorldRef {
        self.voxel_world.clone()
    }

    pub fn societies(&mut self) -> &mut Societies {
        self.ecs_world.resource_mut()
    }

    pub fn player_society(&mut self) -> &mut Option<SocietyHandle> {
        &mut self.ecs_world.resource_mut::<PlayerSociety>().0
    }

    fn apply_world_updates(&mut self, world_viewer: &mut WorldViewer) {
        {
            let mut world = self.voxel_world.borrow_mut();

            // occlusion updates
            while let Ok(update) = self.chunk_updates.try_recv() {
                world.apply_occlusion_update(update);
            }

            // mark modified chunks as dirty in world viewer
            world
                .dirty_chunks()
                .for_each(|c| world_viewer.mark_dirty(c));
        }

        // terrain changes
        // TODO per tick alloc/reuse buf
        let terrain_updates = self.terrain_changes.drain(..).chain(
            self.ecs_world
                .resource_mut::<TerrainUpdatesRes>()
                .0
                .drain(..),
        );

        self.world_loader.apply_terrain_updates(terrain_updates);
    }

    fn process_ui_commands(&mut self, commands: &[UiCommand]) {
        for cmd in commands {
            match *cmd {
                UiCommand::ToggleDebugRenderer { ident, enabled } => {
                    if let Err(e) = self.debug_renderers.set_enabled(ident, enabled) {
                        warn!("failed to toggle debug renderer"; "renderer" => ident, "error" => %e);
                    }
                }

                UiCommand::FillSelectedTiles(placement, block_type) => {
                    let selection = self.ecs_world.resource::<SelectedTiles>();
                    if let Some((mut from, mut to)) = selection.bounds() {
                        if let BlockPlacement::Set = placement {
                            // move the range down 1 block to set those blocks instead of the air
                            // blocks above
                            from = from.below();
                            to = to.below();
                        }
                        let range = WorldPositionRange::with_inclusive_range(from, to);
                        self.terrain_changes
                            .push(WorldTerrainUpdate::new(range, block_type));
                    }
                }
                UiCommand::IssueDivineCommand(ref divine_command) => {
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
                        DivineInputCommand::Break(pos) => AiAction::GoBreakBlock(pos.below()),
                    };

                    match self.ecs_world.component_mut::<AiComponent>(entity) {
                        Err(e) => warn!("can't issue divine command"; "error" => %e),
                        Ok(ai) => {
                            // add DSE
                            ai.add_divine_command(command.clone());
                        }
                    }
                }
                UiCommand::IssueSocietyCommand(society, ref command) => {
                    let society = match self.societies().society_by_handle_mut(society) {
                        Some(s) => s,
                        None => {
                            warn!("invalid society while issuing command"; "society" => ?society, "command" => ?command);
                            continue;
                        }
                    };

                    let job: Box<dyn Job> = Box::new(match command {
                        SocietyInputCommand::BreakBlocks(range) => {
                            BreakBlocksJob::new(range.clone().below())
                        }
                    });

                    debug!("submitting job to society {society:?}", society = society as &Society; "job" => ?job);
                    society.jobs_mut().submit(job);
                }
            }
        }
    }

    // target is for this frame only
    pub fn render(
        &mut self,
        world_viewer: &WorldViewer,
        target: R::Target,
        renderer: &mut R,
        interpolation: f64,
        input: &[InputEvent],
    ) -> (R::Target, UiBlackboard) {
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

            self.debug_renderers
                .iter_enabled()
                .for_each(|r| r.render(renderer, &voxel_world, ecs_world, world_viewer));

            if let Err(e) = renderer.debug_finish() {
                warn!("render debug_finish() failed"; "error" => %e);
            }
        }

        // end frame
        let target = renderer.deinit();

        // gather blackboard for ui
        let blackboard = UiBlackboard::fetch(&self.ecs_world, &self.debug_renderers.summarise());

        (target, blackboard)
    }
}

fn increment_tick() {
    unsafe {
        TICK.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn current_tick() -> u32 {
    unsafe { TICK.load(Ordering::SeqCst) }
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

fn register_debug_renderers<R: Renderer>(
    r: &mut DebugRenderers<R>,
) -> Result<(), DebugRendererError> {
    r.register(AxesDebugRenderer, true)?;
    r.register(SteeringDebugRenderer, true)?;
    r.register(
        PathDebugRenderer::default(),
        config::get().display.nav_paths_by_default,
    )?;
    r.register(NavigationAreaDebugRenderer::default(), false)?;
    r.register(SensesDebugRenderer::default(), false)?;
    Ok(())
}

impl<R: Renderer> SimulationDevExt for Simulation<R> {
    fn world(&self) -> &EcsWorld {
        &self.ecs_world
    }

    fn world_mut(&mut self) -> &mut EcsWorld {
        &mut self.ecs_world
    }
}
