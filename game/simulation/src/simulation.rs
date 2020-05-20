use std::marker::PhantomData;

use crossbeam::crossbeam_channel::Receiver;
use specs::{RunNow, WorldExt};

use common::*;
use world::loader::{ChunkUpdate, ThreadedWorkerPool, WorldLoader};
use world::{SliceRange, WorldRef};

use crate::ecs::{create_ecs_world, EcsWorld};
use crate::entity_builder::EntityBuilder;
use crate::movement::{DesiredMovementComponent, MovementFulfilmentSystem};
use crate::path::{FollowPathComponent, PathSteeringSystem, RandomPathAssignmentSystem};
use crate::physics::PhysicsSystem;
use crate::render::dummy::AxesDebugRenderer;
use crate::render::{DebugRenderer, PhysicalComponent, RenderSystem, Renderer};
use crate::steer::{SteeringComponent, SteeringDebugRenderer, SteeringSystem};
use crate::transform::TransformComponent;

pub type ThreadedWorldLoader = WorldLoader<ThreadedWorkerPool>;

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,

    #[allow(dead_code)] // TODO will be used when world can be modified
    world_loader: ThreadedWorldLoader,
    chunk_updates: Receiver<ChunkUpdate>,

    renderer: PhantomData<R>,
    debug_renderers: Vec<Box<dyn DebugRenderer<R>>>,
    debug_physics: bool,
}

impl<R: Renderer> Simulation<R> {
    /// world_loader should have had all chunks requested
    pub fn new(mut world_loader: ThreadedWorldLoader) -> Self {
        let mut ecs_world = create_ecs_world();

        // register components
        ecs_world.register::<TransformComponent>();
        ecs_world.register::<DesiredMovementComponent>();
        ecs_world.register::<PhysicalComponent>();
        ecs_world.register::<FollowPathComponent>();
        ecs_world.register::<SteeringComponent>();

        // insert resources
        let voxel_world = world_loader.world();
        ecs_world.insert(voxel_world.clone());

        let chunk_updates = world_loader.chunk_updates_rx().unwrap();

        Self {
            ecs_world,
            renderer: PhantomData,
            voxel_world,
            world_loader,
            chunk_updates,
            debug_renderers: vec![Box::new(AxesDebugRenderer), Box::new(SteeringDebugRenderer)],
            debug_physics: config::get().display.debug_physics,
        }
    }

    pub fn add_entity(&mut self) -> EntityBuilder {
        EntityBuilder::new(&mut self.ecs_world)
    }

    pub fn tick(&mut self) {
        // apply chunk updates
        // TODO limit time/count
        self.apply_chunk_updates();

        // tick systems
        let _span = enter_span(Span::Tick);

        // assign paths
        RandomPathAssignmentSystem.run_now(&self.ecs_world);

        // follow paths with steering
        PathSteeringSystem.run_now(&self.ecs_world);

        // apply steering
        SteeringSystem.run_now(&self.ecs_world);

        // attempt to fulfil desired velocity
        MovementFulfilmentSystem.run_now(&self.ecs_world);

        // apply physics
        PhysicsSystem.run_now(&self.ecs_world);
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
