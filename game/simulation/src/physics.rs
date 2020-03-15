use std::os::raw::c_void;

use debug_draw::{DebugDrawer, FrameBlob};
use physics::{ColliderHandle, StepType};
use unit::world::WorldPoint;
use world::{InnerWorldRefMut, WorldRef};

use crate::ecs::*;
use crate::render::{DebugRenderer, FrameRenderState};
use crate::{PhysicalComponent, Renderer, TransformComponent};

/// Collisions and gravity component
pub struct PhysicsComponent {
    /// Handle to collider in physics world
    pub collider: ColliderHandle,
}

impl PhysicsComponent {
    /// position = center position
    pub fn new(
        mut world: InnerWorldRefMut,
        transform: &TransformComponent,
        physical: &PhysicalComponent,
    ) -> Self {
        let pos = transform.position;
        let dims = physical.dimensions;
        let collider = world.physics_world_mut().add_entity(pos, dims);

        Self { collider }
    }
}

/// Ticks the bullet physics world and (TODO one day) emits collision events
pub struct PhysicsSystem;

impl System for PhysicsSystem {
    fn tick_system(&mut self, data: &mut TickData) {
        let mut world = data.voxel_world.borrow_mut();
        let physics_world = world.physics_world_mut();

        // step physics world
        physics_world.step(StepType::Tick);

        // handle collision events
        // physics_world.handle_collision_events();
    }
}

impl<'a, R: Renderer> DebugRenderer<R> for DebugDrawer {
    fn render(
        &mut self,
        renderer: &mut R,
        world: WorldRef,
        _ecs_world: &EcsWorld,
        _frame_state: &FrameRenderState<R>,
    ) {
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
