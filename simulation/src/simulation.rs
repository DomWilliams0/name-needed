use log::info;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use specs::prelude::*;
pub use specs::System;
use specs::World as SpecsWorld;

use world::{SliceRange, WorldRef};

use crate::movement::{MovementSystem, Position, Velocity};
use crate::path::{FollowPath, PathSteeringSystem, TempPathAssignmentSystem};
use crate::render::{DebugRenderer, FrameRenderState, Physical, RenderSystem, Renderer};
use crate::steer::{Steering, SteeringSystem};

pub struct Simulation<'a, R: Renderer> {
    specs_world: SpecsWorld,
    specs_dispatcher: Dispatcher<'a, 'a>,

    world: WorldRef,

    renderer: PhantomData<R>,
    debug_renderers: Vec<Box<dyn DebugRenderer<R>>>,
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
            .build();

        // register components
        specs_world.register::<Position>();
        specs_world.register::<Velocity>();
        specs_world.register::<Physical>();
        specs_world.register::<Steering>();
        specs_world.register::<FollowPath>();

        // world as resource
        specs_world.insert(world.clone());

        // add dummy entities
        {
            let w = world.borrow();
            info!("adding dummy entities");
            specs_world
                .create_entity()
                .with(
                    Position::from_highest_safe_point(&w, 0.0, 8.5).expect("should be valid point"),
                )
                .with(Velocity { x: 0.0, y: 0.0 })
                .with(Physical {
                    color: (100, 10, 15),
                })
                .with(FollowPath::default())
                .with(Steering::default())
                .build();

            specs_world
                .create_entity()
                .with(
                    Position::from_highest_safe_point(&w, 0.2, 3.7).expect("should be valid point"),
                )
                .with(Physical {
                    color: (30, 200, 90),
                })
                .build();
        }

        Self {
            specs_world,
            specs_dispatcher,
            renderer: PhantomData,
            world,
            debug_renderers: vec![
                // Box::new(DummyDebugRenderer),

                // temporarily unimplemented
                // Box::new(NavigationMeshDebugRenderer),
            ],
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

        renderer.debug_finish();

        renderer.finish();
    }
}
