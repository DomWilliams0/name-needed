use specs::prelude::*;
use specs_derive::Component;

use physics;
use physics::Collider;
use world::{InnerWorldRefMut, WorldRef};

use crate::render::{DebugRenderer, FrameRenderState};
use crate::{Physical, Position, Renderer};
use debug_draw::{DebugDrawer, FrameBlob};
use std::os::raw::c_void;
use unit::world::WorldPoint;

/// Collisions and gravity
#[derive(Component)]
#[storage(VecStorage)]
pub struct Physics {
    collider: Collider,
}

impl Physics {
    /// position = center position
    pub fn new(mut world: InnerWorldRefMut, position: &Position, physical: &Physical) -> Self {
        let pos = position.pos;
        let dims = physical.dimensions;
        let collider = world.physics_world_mut().add_entity(pos, dims);

        Self { collider }
    }
}

pub struct PhysicsSystem;

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        WriteStorage<'a, Position>,
        ReadStorage<'a, Physics>,
        Read<'a, WorldRef>,
    );

    fn run(&mut self, (mut pos, phys, world): Self::SystemData) {
        let mut world = world.borrow_mut();
        let physics_world = world.physics_world_mut();

        // step physics world
        physics_world.step();

        // handle collision events
        physics_world.handle_collision_events();

        // TODO sync transform from physics position
        for (mut pos, phys) in (&mut pos, &phys).join() {
            let phys_pos: [f32; 3] = physics_world
                .position(&phys.collider)
                .expect("body should exist") // TODO it might not
                .into();
            pos.pos = phys_pos.into();
        }
    }
}

impl<'a, R: Renderer> DebugRenderer<R> for DebugDrawer {
    fn render(&mut self, renderer: &mut R, world: WorldRef, _frame_state: &FrameRenderState<R>) {
        let mut draw_line = |from: WorldPoint, to: WorldPoint, color| {
            renderer.debug_add_line(from.into(), to.into(), color);
        };

        let mut blob = FrameBlob {
            draw_line: &mut draw_line,
        };
        let blob_ptr = &mut blob as *mut FrameBlob as *mut c_void;
        world.borrow_mut().physics_world_mut().debug_draw(blob_ptr);
    }
}
