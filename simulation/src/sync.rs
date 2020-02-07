use num_traits::zero;

use common::*;

use crate::ecs::*;
use crate::movement::{angle_from_direction, DesiredMovementComponent};
use crate::physics::PhysicsComponent;
use crate::TransformComponent;

/// Syncs entity transforms to physics world pre-step
pub struct SyncToPhysicsSystem;

impl System for SyncToPhysicsSystem {
    fn tick_system(&mut self, data: &mut TickData) {
        let mut w = data.voxel_world.borrow_mut();
        let phys_world = w.physics_world_mut();

        let query = <(
            Read<DesiredMovementComponent>,
            Read<TransformComponent>,
            Write<PhysicsComponent>,
        )>::query();
        for (e, (movement, transform, mut physics)) in query.iter_entities(data.ecs_world) {
            let Rad(new_rotation) = if movement.desired_velocity != zero() {
                // lerp towards direction of travel
                let per_tick = config::get().simulation.lerp_sharpness;
                let target = movement.desired_velocity;
                let current = transform.rotation_dir();

                let lerped = current.lerp(target, per_tick);
                angle_from_direction(lerped)
            } else {
                // keep current rotation
                transform.rotation_angle()
            };

            // convert scaled jump force into physics force
            let jump_force = movement.jump_force * config::get().simulation.jump_impulse;

            if !phys_world.sync_to(
                &mut physics.collider,
                &transform.position,
                new_rotation,
                &movement.realized_velocity.extend(0.0),
                jump_force,
            ) {
                warn!(
                    "{}: failed to sync transform to physics world - is the entity still alive?",
                    e
                );
            }
        }
    }
}

/// Syncs entity transforms back from physics world post-step
pub struct SyncFromPhysicsSystem;

impl System for SyncFromPhysicsSystem {
    fn tick_system(&mut self, data: &mut TickData) {
        let mut w = data.voxel_world.borrow_mut();
        let phys_world = w.physics_world_mut();
        let query = <(Write<TransformComponent>, Write<PhysicsComponent>)>::query();
        for (e, (mut transform, mut physics)) in query.iter_entities(data.ecs_world) {
            let mut rotation = zero();
            if phys_world.sync_from(
                &mut physics.collider,
                &mut transform.position,
                &mut rotation,
            ) {
                transform.set_rotation_from_direction(rotation);
            } else {
                warn!(
                    "{}: failed to sync transform from physics world - is the entity still alive?",
                    e
                );
            }
        }
    }
}
