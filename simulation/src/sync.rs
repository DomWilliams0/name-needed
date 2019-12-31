use common::*;

use crate::ecs::*;
use crate::movement::{angle_from_direction, DesiredVelocity};
use crate::physics::Physics;
use crate::Transform;
use num_traits::zero;

/// Syncs entity transforms to physics world pre-step
pub struct SyncToPhysicsSystem;

impl System for SyncToPhysicsSystem {
    fn tick_system(&mut self, data: &TickData) {
        let mut w = data.voxel_world.borrow_mut();
        let phys_world = w.physics_world_mut();

        data.ecs_world
            .matcher_with_entities::<All<(Read<DesiredVelocity>, Read<Transform>, Write<Physics>)>>(
            )
            .for_each(|(e, (vel, transform, physics))| {
                let Rad(new_rotation) = if vel.velocity != zero() {
                    // lerp towards direction of travel
                    let per_tick = config::get().simulation.lerp_sharpness;
                    let target = vel.velocity;
                    let current = transform.rotation_dir();

                    let lerped = current.lerp(target, per_tick);
                    angle_from_direction(lerped)
                } else {
                    // keep current rotation
                    transform.rotation_angle()
                };

                // TODO should this be in another system?
                let scaled_velocity =
                    vel.velocity.extend(0.0) * config::get().simulation.move_speed;

                if !phys_world.sync_to(
                    &physics.collider,
                    &transform.position,
                    new_rotation,
                    &scaled_velocity,
                ) {
                    warn!("{}: failed to sync transform to physics world - is the entity still alive?", NiceEntity(e));
                }
            });
    }
}

/// Syncs entity transforms back from physics world post-step
pub struct SyncFromPhysicsSystem;

impl System for SyncFromPhysicsSystem {
    fn tick_system(&mut self, data: &TickData) {
        let mut w = data.voxel_world.borrow_mut();
        let phys_world = w.physics_world_mut();
        data.ecs_world
            .matcher_with_entities::<All<(Write<Transform>, Read<Physics>)>>()
            .for_each(|(e, (transform, physics))| {
                let mut rotation = zero();
                if phys_world.sync_from(&physics.collider, &mut transform.position, &mut rotation) {
                    transform.set_rotation_from_direction(rotation);
                } else {
                    warn!("{}: failed to sync transform from physics world - is the entity still alive?", NiceEntity(e));
                }
            })
    }
}
