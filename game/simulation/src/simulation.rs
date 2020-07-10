use std::marker::PhantomData;

use crossbeam::crossbeam_channel::Receiver;
use specs::RunNow;

use common::derive_more::Deref;
use common::*;
use world::loader::{TerrainUpdatesRes, ThreadedWorkerPool, WorldLoader, WorldTerrainUpdate};
use world::{OcclusionChunkUpdate, SliceRange, WorldRef, WorldViewer};

use crate::ai::{
    ActivityComponent, AiAction, AiComponent, AiSystem, DivineCommandCompletionSystem,
    DivineCommandComponent,
};
use crate::dev::SimulationDevExt;
use crate::ecs::{EcsWorld, EcsWorldFrameRef, WorldExt};
use crate::entity_builder::EntityBuilder;
use crate::input::{
    BlockPlacement, DivineInputCommand, InputEvent, InputSystem, SelectedComponent, SelectedEntity,
    SelectedTiles, SocietyInputCommand, UiBlackboard, UiCommand,
};
use crate::item::{
    BaseItemComponent, EdibleItemComponent, InventoryComponent, PickupItemComponent,
    PickupItemSystem, ThrowableItemComponent, UsingItemComponent,
};
use crate::movement::{DesiredMovementComponent, MovementFulfilmentSystem};
use crate::needs::{EatingSystem, HungerComponent, HungerSystem};
use crate::path::{
    ArrivedAtTargetEventComponent, FollowPathComponent, PathDebugRenderer, PathSteeringSystem,
    WanderComponent, WanderPathAssignmentSystem,
};
use crate::physics::PhysicsSystem;
use crate::queued_update::QueuedUpdates;
use crate::render::{AxesDebugRenderer, DebugRendererError, DebugRenderers};
use crate::render::{RenderComponent, RenderSystem, Renderer};
use crate::society::job::{BreakBlocksJob, Job};
use crate::society::{PlayerSociety, SocietyComponent};
use crate::steer::{SteeringComponent, SteeringDebugRenderer, SteeringSystem};
use crate::transform::TransformComponent;
use crate::{ComponentWorld, Societies, SocietyHandle};

pub type ThreadedWorldLoader = WorldLoader<ThreadedWorkerPool>;

/// Monotonically increasing tick counter
#[derive(Copy, Clone, Eq, PartialEq, Deref)]
pub struct Tick(u32);

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,

    world_loader: ThreadedWorldLoader,
    /// Occlusion updates received from world loader
    chunk_updates: Receiver<OcclusionChunkUpdate>,

    /// Terrain updates, queued and applied per tick
    terrain_changes: Vec<WorldTerrainUpdate>,

    renderer: PhantomData<R>,
    debug_renderers: DebugRenderers<R>,
    current_tick: Tick,
}

/// The tick BEFORE the game starts, never produced in tick()
impl Default for Tick {
    fn default() -> Self {
        Self(0)
    }
}

impl<R: Renderer> Simulation<R> {
    /// world_loader should have had all chunks requested
    pub fn new(mut world_loader: ThreadedWorldLoader) -> Self {
        let mut ecs_world = EcsWorld::new();

        register_components(&mut ecs_world);

        // insert resources
        let voxel_world = world_loader.world();
        ecs_world.insert(voxel_world.clone());
        register_resources(&mut ecs_world);

        let chunk_updates = world_loader.chunk_updates_rx().unwrap();
        let mut debug_renderers = DebugRenderers::new();
        if let Err(e) = register_debug_renderers(&mut debug_renderers) {
            // TODO return Result instead of panic!, even though this only happens during game init
            panic!("failed to register debug renderers: {}", e);
        }

        Self {
            ecs_world,
            renderer: PhantomData,
            voxel_world,
            world_loader,
            chunk_updates,
            debug_renderers,
            current_tick: Tick::default(),
            terrain_changes: Vec::with_capacity(1024),
        }
    }

    pub fn add_entity(&mut self) -> EntityBuilder<EcsWorld> {
        EntityBuilder::new(&mut self.ecs_world)
    }

    pub fn tick(&mut self, commands: &[UiCommand], world_viewer: &mut WorldViewer) {
        let _span = Span::Tick.begin();

        // update tick resource
        self.current_tick.0 += 1;
        self.ecs_world.insert(self.current_tick);

        // TODO sort out systems so they all have an ecs_world reference and can keep state
        // safety: only lives for the duration of this tick
        let ecs_ref = unsafe { EcsWorldFrameRef::init(&self.ecs_world) };
        self.ecs_world.insert(ecs_ref);

        // TODO limit time/count
        self.apply_world_updates(world_viewer);

        // apply player inputs
        self.process_ui_commands(commands);

        // needs
        HungerSystem.run_now(&self.ecs_world);
        EatingSystem.run_now(&self.ecs_world);

        // choose activity
        AiSystem.run_now(&self.ecs_world);

        // assign paths for wandering
        WanderPathAssignmentSystem.run_now(&self.ecs_world);

        // follow paths with steering
        PathSteeringSystem.run_now(&self.ecs_world);

        // apply steering
        SteeringSystem.run_now(&self.ecs_world);

        // attempt to fulfil desired velocity
        MovementFulfilmentSystem.run_now(&self.ecs_world);

        // pick up items
        PickupItemSystem.run_now(&self.ecs_world);

        // remove completed divine commands
        DivineCommandCompletionSystem.run_now(&self.ecs_world);

        #[cfg(debug_assertions)]
        crate::item::validation::InventoryValidationSystem(&self.ecs_world)
            .run_now(&self.ecs_world);

        // apply physics
        PhysicsSystem.run_now(&self.ecs_world);

        // we're about to go mutable, drop this fuzzy ball of unsafeness
        let _ = self.ecs_world.remove::<EcsWorldFrameRef>();

        // per tick maintenance
        // must remove resource from world first so we can use &mut ecs_world
        let mut updates = self.ecs_world.remove::<QueuedUpdates>().unwrap();
        updates.execute(&mut self.ecs_world);
        self.ecs_world.insert(updates);

        self.ecs_world.maintain();
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
                        warn!("failed to toggle debug renderer: {}", e);
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
                        self.terrain_changes
                            .push(WorldTerrainUpdate::with_range(from, to, block_type));
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
                            warn!("no selected entity to issue divine command to");
                            continue;
                        }
                    };

                    let command = match divine_command {
                        DivineInputCommand::Goto(pos) => AiAction::Goto(pos.centred()),
                        DivineInputCommand::Break(pos) => AiAction::GoBreakBlock(pos.below()),
                    };

                    match self.ecs_world.component_mut::<AiComponent>(entity) {
                        Err(e) => warn!("can't issue divine command: {}", e),
                        Ok(ai) => {
                            // add DSE
                            ai.add_divine_command(command.clone());

                            // add component for tracking completion
                            self.ecs_world
                                .add_lazy(entity, DivineCommandComponent(command));
                        }
                    }
                }
                UiCommand::IssueSocietyCommand(society, ref command) => {
                    let society = match self.societies().society_by_handle_mut(society) {
                        Some(s) => s,
                        None => {
                            warn!("unknown society with handle {:?}", society);
                            continue;
                        }
                    };

                    let job: Box<dyn Job> = Box::new(match command {
                        SocietyInputCommand::BreakBlocks(range) => {
                            BreakBlocksJob::new(range.clone().below())
                        }
                    });

                    debug!("submitting job {:?} to society {:?}", job, society);
                    society.jobs_mut().submit(job);
                }
            }
        }
    }

    // target is for this frame only
    pub fn render(
        &mut self,
        slices: SliceRange,
        target: R::Target,
        renderer: &mut R,
        interpolation: f64,
        input: &[InputEvent],
    ) -> (R::Target, UiBlackboard) {
        let _span = Span::Render(interpolation).begin();

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
                    slices,
                    interpolation: interpolation as f32,
                };

                render_system.run_now(&self.ecs_world);
            }
            if let Err(e) = renderer.sim_finish() {
                warn!("render sim_finish() failed: {:?}", e);
            }
        }

        // render debug shapes
        {
            renderer.debug_start();
            let ecs_world = &self.ecs_world;
            let voxel_world = self.voxel_world.borrow();

            self.debug_renderers
                .iter_enabled()
                .for_each(|r| r.render(renderer, &voxel_world, ecs_world, slices));

            if let Err(e) = renderer.debug_finish() {
                warn!("render debug_finish() failed: {:?}", e);
            }
        }

        // end frame
        let target = renderer.deinit();

        // gather blackboard for ui
        let blackboard = UiBlackboard::fetch(&self.ecs_world, &self.debug_renderers.summarise());

        (target, blackboard)
    }
}

fn register_components(_world: &mut EcsWorld) {
    macro_rules! register {
        ($comp:ty) => {
            _world.register::<$comp>()
        };
    }

    // common
    register!(TransformComponent);
    register!(RenderComponent);

    // movement
    register!(DesiredMovementComponent);
    register!(FollowPathComponent);
    register!(ArrivedAtTargetEventComponent);
    register!(SteeringComponent);
    register!(DesiredMovementComponent);
    register!(WanderComponent);

    // ai
    register!(AiComponent);
    register!(HungerComponent);
    register!(ActivityComponent);
    register!(SocietyComponent);

    // items
    register!(BaseItemComponent);
    register!(EdibleItemComponent);
    register!(ThrowableItemComponent);
    register!(InventoryComponent);
    register!(UsingItemComponent);
    register!(PickupItemComponent);

    // input
    register!(SelectedComponent);

    // dev
    register!(DivineCommandComponent);
}

fn register_resources(world: &mut EcsWorld) {
    world.insert(Tick::default());
    world.insert(QueuedUpdates::default());
    world.insert(SelectedEntity::default());
    world.insert(SelectedTiles::default());
    world.insert(TerrainUpdatesRes::default());
    world.insert(Societies::default());
    world.insert(PlayerSociety::default());
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
