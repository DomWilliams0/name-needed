use std::marker::PhantomData;

use crossbeam::crossbeam_channel::Receiver;
use specs::RunNow;

use common::*;
use world::loader::{ThreadedWorkerPool, WorldLoader, WorldTerrainUpdate};
use world::{OcclusionChunkUpdate, SliceRange, WorldRef};

use crate::ai::{ActivityComponent, AiComponent, AiSystem};
use crate::dev::SimulationDevExt;
use crate::ecs::{EcsWorld, EcsWorldFrameRef, WorldExt};
use crate::entity_builder::EntityBuilder;
use crate::input::{
    Blackboard, BlockPlacement, InputCommand, InputEvent, InputSystem, SelectedComponent,
    SelectedEntity, SelectedTiles,
};
use crate::item::{
    BaseItemComponent, EdibleItemComponent, InventoryComponent, PickupItemComponent,
    PickupItemSystem, ThrowableItemComponent, UsingItemComponent,
};
use crate::movement::{DesiredMovementComponent, MovementFulfilmentSystem};
use crate::needs::{EatingSystem, HungerComponent, HungerSystem};
use crate::path::{
    FollowPathComponent, PathDebugRenderer, PathSteeringSystem, WanderComponent,
    WanderPathAssignmentSystem,
};
use crate::physics::PhysicsSystem;
use crate::queued_update::QueuedUpdates;
use crate::render::{AxesDebugRenderer, DebugRendererError, DebugRenderers};
use crate::render::{RenderComponent, RenderSystem, Renderer};
use crate::steer::{SteeringComponent, SteeringDebugRenderer, SteeringSystem};
use crate::transform::TransformComponent;
use crate::ComponentWorld;

pub type ThreadedWorldLoader = WorldLoader<ThreadedWorkerPool>;

#[derive(Copy, Clone, Default)]
pub struct Tick(pub u32);

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,

    #[allow(dead_code)] // TODO will be used when world can be modified
    world_loader: ThreadedWorldLoader,
    /// Occlusion updates received from world loader
    chunk_updates: Receiver<OcclusionChunkUpdate>,

    /// Terrain updates, queued and applied per tick
    terrain_changes: Vec<WorldTerrainUpdate>,

    renderer: PhantomData<R>,
    debug_renderers: DebugRenderers<R>,
    current_tick: Tick,
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

    pub fn tick(&mut self, commands: &[InputCommand]) {
        let _span = Span::Tick.begin();

        // update tick resource
        self.current_tick.0 += 1;
        self.ecs_world.insert(self.current_tick);

        // TODO sort out systems so they all have an ecs_world reference and can keep state
        // safety: only lives for the duration of this tick
        let ecs_ref = unsafe { EcsWorldFrameRef::init(&self.ecs_world) };
        self.ecs_world.insert(ecs_ref);

        // TODO limit time/count
        self.apply_chunk_updates();

        // apply player inputs
        self.process_input_commands(commands);

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

        #[cfg(debug_assertions)]
        crate::item::validation::InventoryValidationSystem(&self.ecs_world)
            .run_now(&self.ecs_world);

        // apply physics
        PhysicsSystem.run_now(&self.ecs_world);

        // we're about to go mutable, drop this fuzzy ball of unsafeness
        let _ = self.ecs_world.remove::<EcsWorldFrameRef>();

        // per tick maintenance
        // must remove resource from world first so we can use &mut ecs_world
        let mut entity_updates = self.ecs_world.remove::<QueuedUpdates>().unwrap();
        entity_updates.execute(&mut self.ecs_world);
        self.ecs_world.insert(entity_updates);

        self.ecs_world.maintain();
    }

    pub fn world(&self) -> WorldRef {
        self.voxel_world.clone()
    }

    fn apply_chunk_updates(&mut self) {
        let mut world = self.voxel_world.borrow_mut();

        // occlusion updates
        while let Ok(update) = self.chunk_updates.try_recv() {
            world.apply_occlusion_update(update);
        }

        // terrain changes, apply per chunk
        // TODO per tick alloc/reuse buf
        let groups = self
            .terrain_changes
            .drain(..)
            .flat_map(|world_update| world_update.into_chunk_updates())
            .sorted_by_key(|(chunk_pos, _)| *chunk_pos)
            .group_by(|(chunk_pos, _)| *chunk_pos);

        for (chunk, updates) in &groups {
            if let Some(new_terrain) =
                world.apply_terrain_updates(chunk, updates.map(|(_, update)| update))
            {
                trace!(
                    "submitting updated chunk terrain to worker pool for {:?}",
                    chunk
                );
                self.world_loader.update_chunk(chunk, new_terrain);
            }
        }
    }

    fn process_input_commands(&mut self, commands: &[InputCommand]) {
        for cmd in commands {
            match *cmd {
                InputCommand::ToggleDebugRenderer { ident, enabled } => {
                    if let Err(e) = self.debug_renderers.set_enabled(ident, enabled) {
                        warn!("failed to toggle debug renderer: {}", e);
                    }
                }

                InputCommand::FillSelectedTiles(placement, block_type) => {
                    let selection = self.ecs_world.resource::<SelectedTiles>();
                    if let Some((mut from, mut to)) = selection.bounds() {
                        if let BlockPlacement::Set = placement {
                            // move the range down 1 block to set those blocks instead of the air
                            // blocks above
                            from.2 -= 1;
                            to.2 -= 1;
                        }
                        self.terrain_changes
                            .push(WorldTerrainUpdate::with_range(from, to, block_type));
                    }
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
    ) -> (R::Target, Blackboard) {
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
        let blackboard = Blackboard::fetch(&self.ecs_world, &self.debug_renderers.summarise());

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
    register!(SteeringComponent);
    register!(DesiredMovementComponent);
    register!(WanderComponent);

    // ai
    register!(AiComponent);
    register!(HungerComponent);
    register!(ActivityComponent);

    // items
    register!(BaseItemComponent);
    register!(EdibleItemComponent);
    register!(ThrowableItemComponent);
    register!(InventoryComponent);
    register!(UsingItemComponent);
    register!(PickupItemComponent);

    // input
    register!(SelectedComponent);
}

fn register_resources(world: &mut EcsWorld) {
    world.insert(Tick::default());
    world.insert(QueuedUpdates::default());
    world.insert(SelectedEntity::default());
    world.insert(SelectedTiles::default());
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
