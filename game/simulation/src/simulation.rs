use std::marker::PhantomData;

use crossbeam::crossbeam_channel::Receiver;
use specs::RunNow;

use common::*;
use world::loader::{ChunkUpdate, ThreadedWorkerPool, WorldLoader};
use world::{SliceRange, WorldRef};

use crate::ai::{ActivityComponent, AiComponent, AiSystem};
use crate::dev::SimulationDevExt;
use crate::ecs::{EcsWorld, EcsWorldFrameRef, WorldExt};
use crate::entity_builder::EntityBuilder;
use crate::input::{
    Blackboard, InputCommand, InputEvent, InputSystem, SelectedComponent, SelectedEntity,
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

pub type ThreadedWorldLoader = WorldLoader<ThreadedWorkerPool>;

#[derive(Copy, Clone, Default)]
pub struct Tick(pub u32);

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,

    #[allow(dead_code)] // TODO will be used when world can be modified
    world_loader: ThreadedWorldLoader,
    chunk_updates: Receiver<ChunkUpdate>,

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
        while let Ok(update) = self.chunk_updates.try_recv() {
            world.apply_update(update);
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
}

fn register_debug_renderers<R: Renderer>(
    r: &mut DebugRenderers<R>,
) -> Result<(), DebugRendererError> {
    r.register(AxesDebugRenderer, true)?;
    r.register(SteeringDebugRenderer, true)?;
    r.register(PathDebugRenderer::default(), false)?;
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
