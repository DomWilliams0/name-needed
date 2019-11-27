use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use log::info;
use rand::Rng;

use debug_draw::DebugDrawer;
use world::{SliceRange, WorldRef};

use crate::ecs::{EcsWorld, System, TickData};
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
        let mut ecs_world = EcsWorld::new();

        // add dummy entities
        {
            let mut create_entity =
                |block_position: (i32, i32, Option<i32>), color, dimensions: (f32, f32, f32)| {
                    let transform = match block_position {
                        (x, y, Some(z)) => Transform::from_block_center(x, y, z),
                        (x, y, None) => {
                            let mut transform =
                                Transform::from_highest_safe_point(&world.borrow(), x, y)
                                    .expect("should be valid position");

                            // stand on top
                            transform.position.2 += dimensions.2 / 4.0;

                            transform
                        }
                    };
                    let physical = Physical { color, dimensions };
                    let physics = Physics::new(world.borrow_mut(), &transform, &physical);

                    ecs_world.append_components(Some((
                        transform,
                        physical,
                        physics,
                        Steering::default(),
                        FollowPath::default(),
                        DesiredVelocity::default(),
                    )))
                };

            {
                let dummies = &config::get().simulation.initial_entities;
                info!("adding {} dummy entities", dummies.len());
                for desc in dummies {
                    create_entity(desc.pos, desc.color, desc.size);
                }
            }

            let randoms = config::get().simulation.random_count;
            if randoms > 0 {
                info!("adding {} random entities", randoms);
                let mut rng = rand::thread_rng();
                for _ in 0..randoms {
                    let pos = (4 + rng.gen_range(-4, 4), 4 + rng.gen_range(-4, 4), Some(3));
                    let color = (
                        rng.gen_range(20, 230u8),
                        rng.gen_range(20, 230u8),
                        rng.gen_range(20, 230u8),
                    );
                    let dims = (
                        rng.gen_range(0.8, 1.1),
                        rng.gen_range(0.9, 1.1),
                        rng.gen_range(1.4, 2.0),
                    );

                    create_entity(pos, color, dims);
                }
            }
        }

        // add physics debug renderer but don't enable
        let has_physics_debug_renderer = true;
        let is_physics_debug_renderer_enabled = false;
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

    // target is for this frame only
    pub fn render(
        &mut self,
        slices: SliceRange,
        target: Rc<RefCell<R::Target>>,
        renderer: &mut R,
        _interpolation: f64,
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
                };

                render_system.tick_system(&self.tick_data());
            }
            renderer.finish();
        }

        // TODO RenderData instead of TickData with intepolation/dt etc

        // render debug shapes
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
