use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use common::*;
use debug_draw::DebugDrawer;
use world::{SliceRange, WorldRef};

use crate::ecs::{EcsWorld, Entity, System, TickData};
use crate::movement::{DesiredVelocity, Transform};
use crate::path::{FollowPath, PathDebugRenderer, PathSteeringSystem, TempPathAssignmentSystem};
use crate::physics::{Physics, PhysicsSystem};
use crate::render::dummy::DummyDebugRenderer;
use crate::render::{DebugRenderer, FrameRenderState, Physical, RenderSystem, Renderer};
use crate::steer::{Steering, SteeringSystem};
use crate::sync::{SyncFromPhysicsSystem, SyncToPhysicsSystem};

pub struct Simulation<R: Renderer> {
    ecs_world: EcsWorld,
    voxel_world: WorldRef,

    renderer: PhantomData<R>,
    debug_renderers: Vec<Box<dyn DebugRenderer<R>>>,

    has_physics_debug_renderer: bool,
    is_physics_debug_renderer_enabled: bool,
}

impl<R: Renderer> Simulation<R> {
    pub fn new(world: WorldRef) -> Self {
        let ecs_world = EcsWorld::new();

        // add physics debug renderer
        let has_physics_debug_renderer = true;
        let is_physics_debug_renderer_enabled = config::get().display.debug_physics;
        world
            .borrow_mut()
            .physics_world_mut()
            .set_debug_drawer(has_physics_debug_renderer);

        Self {
            ecs_world,
            renderer: PhantomData,
            voxel_world: world,
            debug_renderers: vec![Box::new(DummyDebugRenderer), Box::new(PathDebugRenderer)],
            has_physics_debug_renderer,
            is_physics_debug_renderer_enabled,
        }
    }

    // TODO return result
    // TODO entity builder
    pub fn add_entity(
        &mut self,
        block_pos: (i32, i32, Option<i32>),
        color: (u8, u8, u8),
        dimensions: (f32, f32, f32),
    ) {
        let world = &self.voxel_world;
        let transform = match block_pos {
            (x, y, Some(z)) => Transform::from_block_center(x, y, z),
            (x, y, None) => {
                let mut transform = Transform::from_highest_safe_point(&world.borrow(), x, y)
                    .expect("should be valid position");

                // stand on top
                transform.position.2 += dimensions.2 / 4.0;

                transform
            }
        };

        let physical = Physical { color, dimensions };
        let physics = Physics::new(world.borrow_mut(), &transform, &physical);

        info!("adding an entity at {:?}", transform.position);

        self.ecs_world.append_components(Some((
            transform,
            DesiredVelocity::default(),
            physical,
            physics,
            FollowPath::default(),
            // Steering::seek(WorldPoint(15.0, 3.0, 3.0)),
            Steering::default(),
        )));
    }

    fn tick_data(&mut self) -> TickData {
        TickData {
            voxel_world: self.voxel_world.clone(),
            ecs_world: &mut self.ecs_world,
        }
    }

    pub fn tick(&mut self) {
        // tick systems
        let tick_data = self.tick_data();

        // assign paths
        TempPathAssignmentSystem.tick_system(&tick_data);

        // follow paths with steering
        PathSteeringSystem.tick_system(&tick_data);

        // apply steering
        SteeringSystem.tick_system(&tick_data);

        // apply physics
        SyncToPhysicsSystem.tick_system(&tick_data);
        PhysicsSystem.tick_system(&tick_data);
        SyncFromPhysicsSystem.tick_system(&tick_data);
    }

    pub fn entities<'s>(&'s self) -> impl Iterator<Item = Entity> + 's {
        self.ecs_world.entities()
    }

    pub fn world(&self) -> WorldRef {
        self.voxel_world.clone()
    }

    // target is for this frame only
    pub fn render(
        &mut self,
        slices: SliceRange,
        target: Rc<RefCell<R::Target>>,
        renderer: &mut R,
        interpolation: f64,
    ) {
        let frame_state = FrameRenderState { target, slices };

        // start frame
        renderer.init(frame_state.target.clone());

        // render simulation
        {
            renderer.start();
            {
                let mut render_system = RenderSystem {
                    renderer,
                    frame_state: frame_state.clone(),
                    interpolation,
                };

                render_system.tick_system(&self.tick_data());
            }
            renderer.finish();
        }

        // render debug shapes
        // TODO needs interpolation?
        {
            renderer.debug_start();

            for debug_renderer in self.debug_renderers.iter_mut() {
                debug_renderer.render(
                    renderer,
                    self.voxel_world.clone(),
                    &self.ecs_world,
                    &frame_state,
                );
            }
            if self.is_physics_debug_renderer_enabled && self.has_physics_debug_renderer {
                DebugDrawer.render(
                    renderer,
                    self.voxel_world.clone(),
                    &self.ecs_world,
                    &frame_state,
                );
            }

            renderer.debug_finish();
        }

        // end frame
        renderer.deinit();
    }

    pub fn toggle_physics_debug_rendering(&mut self) -> bool {
        self.is_physics_debug_renderer_enabled = !self.is_physics_debug_renderer_enabled;
        self.is_physics_debug_renderer_enabled
    }
}
