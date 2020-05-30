use std::marker::PhantomData;

use crossbeam::crossbeam_channel::Receiver;
use specs::RunNow;

use common::*;
use world::loader::{ChunkUpdate, ThreadedWorkerPool, WorldLoader};
use world::{SliceRange, WorldRef};

use crate::ai::{ActivityComponent, AiComponent, AiSystem};
use crate::dev::SimulationDevExt;
use crate::ecs::{EcsWorld, WorldExt};
use crate::entity_builder::EntityBuilder;
use crate::item::{
    BaseItemComponent, EdibleItemComponent, InventoryComponent, PickupItemComponent,
    PickupItemSystem, ThrowableItemComponent, UsingItemComponent,
};
use crate::movement::{DesiredMovementComponent, MovementFulfilmentSystem};
use crate::needs::{EatingSystem, HungerComponent, HungerSystem};
use crate::path::{
    FollowPathComponent, PathSteeringSystem, WanderComponent, WanderPathAssignmentSystem,
};
use crate::physics::PhysicsSystem;
use crate::queued_update::QueuedUpdates;
use crate::render::dummy::AxesDebugRenderer;
use crate::render::{DebugRenderer, RenderComponent, RenderSystem, Renderer};
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
    debug_renderers: Vec<Box<dyn DebugRenderer<R>>>,
    debug_physics: bool,
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
        ecs_world.insert(Tick::default());
        ecs_world.insert(QueuedUpdates::default());

        let chunk_updates = world_loader.chunk_updates_rx().unwrap();

        Self {
            ecs_world,
            renderer: PhantomData,
            voxel_world,
            world_loader,
            chunk_updates,
            debug_renderers: vec![Box::new(AxesDebugRenderer), Box::new(SteeringDebugRenderer)],
            debug_physics: config::get().display.debug_physics,
            current_tick: Tick::default(),
        }
    }

    pub fn add_entity(&mut self) -> EntityBuilder<EcsWorld> {
        EntityBuilder::new(&mut self.ecs_world)
    }

    pub fn tick(&mut self) {
        // update tick resource
        self.current_tick.0 += 1;
        self.ecs_world.insert(self.current_tick);

        // TODO limit time/count
        self.apply_chunk_updates();

        let _span = enter_span(Span::Tick);

        // TODO sort out systems so they always have a ecs_world reference

        // needs
        HungerSystem.run_now(&self.ecs_world);
        EatingSystem.run_now(&self.ecs_world);

        // choose activity
        AiSystem(&self.ecs_world).run_now(&self.ecs_world);

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

    // target is for this frame only
    pub fn render(
        &mut self,
        slices: SliceRange,
        target: R::Target,
        renderer: &mut R,
        interpolation: f64,
    ) -> R::Target {
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
        // TODO needs interpolation?
        {
            renderer.debug_start();

            for debug_renderer in self.debug_renderers.iter_mut() {
                debug_renderer.render(renderer, self.voxel_world.clone(), &self.ecs_world, slices);
            }
            if let Err(e) = renderer.debug_finish() {
                warn!("render debug_finish() failed: {:?}", e);
            }
        }

        // end frame
        renderer.deinit()
    }

    /// Toggles and returns new enabled state
    pub fn toggle_physics_debug_rendering(&mut self) -> bool {
        self.debug_physics = !self.debug_physics;
        self.debug_physics
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
}

impl<R: Renderer> SimulationDevExt for Simulation<R> {
    fn world(&self) -> &EcsWorld {
        &self.ecs_world
    }

    fn world_mut(&mut self) -> &mut EcsWorld {
        &mut self.ecs_world
    }
}
