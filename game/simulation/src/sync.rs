use num_traits::zero;

use common::*;
use physics::EntityJumpAction;

use crate::ecs::*;
use crate::movement::{angle_from_direction, DesiredJumpBehavior, DesiredMovementComponent};
use crate::physics::PhysicsComponent;
use crate::TransformComponent;

/// Syncs entity transforms to physics world pre-step
pub struct SyncToPhysicsSystem;

impl System for SyncToPhysicsSystem {
    fn tick_system(&mut self, data: &mut TickData) {
        let mut w = data.voxel_world.borrow_mut();
        let phys_world = w.physics_world_mut();
        phys_world.sync_config();

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

            // TODO jump *this tick* if all other conditions are already met, the only last one
            // to check is collusion of the sensor in physics world
            let jump_action = {
                // could possibly jump if wanted (TODO depends on physical condition)
                let physically_possible = true;

                match (physically_possible, movement.jump_behavior) {
                    (false, _) => EntityJumpAction::NOPE,
                    (true, DesiredJumpBehavior::ManuallyRightNow) => {
                        EntityJumpAction::UNCONDITIONAL
                    }
                    (true, DesiredJumpBehavior::OnSensorObstruction) => {
                        EntityJumpAction::IF_SENSOR_OCCLUDED
                    }
                }
            };

            if !phys_world.sync_to(
                &mut physics.collider,
                &transform.position,
                new_rotation,
                &movement.realized_velocity.extend(0.0),
                jump_action,
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
