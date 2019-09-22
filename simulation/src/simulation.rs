use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use specs::prelude::*;
pub use specs::System;
use specs::World as SpecsWorld;

use crate::movement::{MovementSystem, Position, Velocity};
use crate::render::{Physical, RenderSystem, Renderer};
use world::{SliceRange, CHUNK_SIZE};

pub struct Simulation<'a, R: Renderer> {
    specs_world: SpecsWorld,
    specs_dispatcher: Dispatcher<'a, 'a>,

    renderer: PhantomData<R>,
}

impl<'a, R: Renderer> Simulation<'a, R> {
    pub fn new() -> Self {
        let mut specs_world = SpecsWorld::new();

        // register systems
        let specs_dispatcher = DispatcherBuilder::new()
            .with(MovementSystem, "movement", &[])
            .build();

        // register components
        specs_world.register::<Position>();
        specs_world.register::<Velocity>();
        specs_world.register::<Physical>();

        // add dummy entities
        {
            specs_world
                .create_entity()
                .with(Position {
                    x: 0.0,
                    y: CHUNK_SIZE as f32, // should be at the top of the chunk
                    z: 0,
                })
                .with(Velocity { x: 0.5, y: -1.25 })
                .with(Physical {
                    color: (100, 10, 15),
                })
                .build();

            specs_world
                .create_entity()
                .with(Position {
                    x: 0.2,
                    y: 3.7,
                    z: 2,
                })
                .with(Physical {
                    color: (30, 200, 90),
                })
                .build();
        }

        Self {
            specs_world,
            specs_dispatcher,
            renderer: PhantomData,
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
        let mut render_system = RenderSystem {
            target,
            slices,
            renderer,
        };

        // TODO interpolation as resource
        render_system.run_now(&self.specs_world);
    }
}
