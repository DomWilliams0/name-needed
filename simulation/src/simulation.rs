use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use log::info;
use specs::prelude::*;
pub use specs::System;
use specs::World as SpecsWorld;

use world::{SliceRange, WorldRef};

use crate::movement::{MovementSystem, Position, Velocity};
use crate::path::{FollowPath, PathSteeringSystem, TempPathAssignmentSystem};
use crate::physics::{Physics, PhysicsSystem};
use crate::render::dummy::DummyDebugRenderer;
use crate::render::{DebugRenderer, FrameRenderState, Physical, RenderSystem, Renderer};
use crate::steer::{Steering, SteeringSystem};
use debug_draw::DebugDrawer;

pub struct Simulation<'a, R: Renderer> {
    specs_world: SpecsWorld,
    specs_dispatcher: Dispatcher<'a, 'a>,

    world: WorldRef,

    renderer: PhantomData<R>,
    debug_renderers: Vec<Box<dyn DebugRenderer<R>>>,

    has_physics_debug_renderer: bool,
    is_physics_debug_renderer_enabled: bool,
}

impl<'a, R: Renderer> Simulation<'a, R> {
    pub fn new(world: WorldRef) -> Self {
        let mut specs_world = SpecsWorld::new();

        info!("registering systems and components");

        // register systems
        let specs_dispatcher = DispatcherBuilder::new()
            .with(TempPathAssignmentSystem, "pathassign", &[])
            .with(PathSteeringSystem, "pathsteering", &["pathassign"])
            .with(SteeringSystem, "steering", &["pathsteering"])
            .with(MovementSystem, "movement", &["steering"])

            // TODO system order
            .with(PhysicsSystem, "physics", &["movement"])
            .build();

        // register components
        specs_world.register::<Position>();
        specs_world.register::<Velocity>();
        specs_world.register::<Physical>();
        specs_world.register::<Physics>();
        specs_world.register::<Steering>();
        specs_world.register::<FollowPath>();

        // world as resource
        specs_world.insert(world.clone());

        // add dummy entities
        {
            let mut create_entity =
                |block_position: (i32, i32, Option<i32>), color, dimensions: (f32, f32, f32)| {
                    let position = match block_position {
                        (x, y, Some(z)) => Position::from_block_center(x, y, z),
                        (x, y, None) => {
                            let mut pos = Position::from_highest_safe_point(&world.borrow(), x, y)
                                .expect("should be valid position");

                            // stand on top
                            pos.pos.2 += dimensions.2 / 4.0;

                            pos
                        }
                    };
                    let physical = Physical { color, dimensions };
                    let physics = Physics::new(world.borrow_mut(), &position, &physical);

                    specs_world
                        .create_entity()
                        .with(position)
                        .with(physical)
                        .with(physics)
                        .build()
                };

            info!("adding dummy entities");

            create_entity((3, 4, Some(5)), (10, 10, 255), (1.2, 0.6, 1.95));

            create_entity((1, 1, None), (100, 10, 15), (1.1, 0.75, 1.85));
        }

        // add physics debug renderer but don't enable
        let has_physics_debug_renderer = true;
        let is_physics_debug_renderer_enabled = false;
        world
            .borrow_mut()
            .physics_world_mut()
            .set_debug_drawer(has_physics_debug_renderer);

        Self {
            specs_world,
            specs_dispatcher,
            renderer: PhantomData,
            world,
            debug_renderers: vec![Box::new(DummyDebugRenderer)],
            has_physics_debug_renderer,
            is_physics_debug_renderer_enabled,
        }
    }

    pub fn tick(&mut self) {
        self.specs_dispatcher.dispatch(&self.specs_world);
        self.specs_world.maintain();
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

        renderer.init(frame_state.target.clone());
        renderer.start();

        {
            let mut render_system = RenderSystem {
                renderer,
                frame_state: frame_state.clone(),
            };

            // TODO interpolation as resource
            render_system.run_now(&self.specs_world);
        }

        renderer.debug_start();

        for debug_renderer in self.debug_renderers.iter_mut() {
            debug_renderer.render(renderer, self.world.clone(), &frame_state);
        }
        if self.is_physics_debug_renderer_enabled && self.has_physics_debug_renderer {
            DebugDrawer.render(renderer, self.world.clone(), &frame_state);
        }

        renderer.debug_finish();

        renderer.finish();
    }

    pub fn toggle_physics_debug_rendering(&mut self) -> bool {
        self.is_physics_debug_renderer_enabled = !self.is_physics_debug_renderer_enabled;
        self.is_physics_debug_renderer_enabled
    }
}
