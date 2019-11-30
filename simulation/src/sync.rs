use log::*;

use crate::ecs::*;
use crate::movement::DesiredVelocity;
use crate::physics::Physics;
use crate::Transform;

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
                // TODO should this be in another system?
                let scaled_velocity = vel.velocity * config::get().simulation.move_speed;
                let new_rotation = vel.velocity;

                if !phys_world.sync_to(
                    &physics.collider,
                    &transform.position,
                    &new_rotation,
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
                if !phys_world.sync_from(
                    &physics.collider,
                    &mut transform.position,
                    &mut transform.rotation,
                ) {
                    // TODO entity id
                    warn!("{}: failed to sync transform from physics world - is the entity still alive?", NiceEntity(e));
                }
            })
    }
}
