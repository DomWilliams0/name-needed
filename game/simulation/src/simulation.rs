use std::marker::PhantomData;

use common::*;
use world::{SliceRange, WorldRef};

use crate::ecs::{create_ecs_world, EcsWorld};
use crate::entity_builder::EntityBuilder;
use crate::movement::{DesiredMovementComponent, MovementFulfilmentSystem};
use crate::path::{FollowPathComponent, PathSteeringSystem, TempPathAssignmentSystem};
use crate::physics::PhysicsSystem;
use crate::render::{PhysicalComponent, RenderSystem, Renderer};
use crate::steer::{SteeringComponent, SteeringSystem};
use crate::transform::TransformComponent;
use specs::{RunNow, WorldExt};

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,

    renderer: PhantomData<R>,
    //debug_renderers: Vec<Box<dyn DebugRenderer<R>>>,
    debug_physics: bool,
}

impl<R: Renderer> Simulation<R> {
    pub fn new(world: WorldRef) -> Self {
        let mut ecs_world = create_ecs_world();

        // register components
        ecs_world.register::<TransformComponent>();
        ecs_world.register::<DesiredMovementComponent>();
        ecs_world.register::<PhysicalComponent>();
        ecs_world.register::<FollowPathComponent>();
        ecs_world.register::<SteeringComponent>();

        // insert resources
        ecs_world.insert(world.clone());

        Self {
            ecs_world,
            renderer: PhantomData,
            voxel_world: world,
            // debug_renderers: vec![Box::new(DummyDebugRenderer), Box::new(PathDebugRenderer)],
            debug_physics: config::get().display.debug_physics,
        }
    }

    pub fn add_entity(&mut self) -> EntityBuilder {
        EntityBuilder::new(&mut self.ecs_world)
    }

    pub fn tick(&mut self) {
        // tick systems
        let _span = enter_span(Span::Tick);

        // assign paths
        TempPathAssignmentSystem.run_now(&self.ecs_world);

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
            renderer.sim_finish();
        }

        // render debug shapes
        // TODO needs interpolation?
        {
            renderer.debug_start();

            // TODO debug renderers
            /*
            for debug_renderer in self.debug_renderers.iter_mut() {
                debug_renderer.render(
                    renderer,
                    self.voxel_world.clone(),
                    &self.ecs_world,
                    &frame_state,
                );
            }
            */
            /*
            if self.debug_physics {
                DebugDrawer.render(
                    renderer,
                    self.voxel_world.clone(),
                    &self.ecs_world,
                    &frame_state,
                );
            }
            */

            renderer.debug_finish();
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
