use crate::ecs::*;

use crate::item::HauledItemComponent;
use crate::transform::PhysicalComponent;
use crate::TransformComponent;
use crate::WorldRef;
use common::*;

const STOP_LIMIT: f32 = 0.01;
const FALL_SLOWDOWN: f32 = 0.5;

pub struct PhysicsSystem;

#[derive(Debug, Component, EcsComponent)]
#[storage(VecStorage)]
#[name("physics")]
pub struct PhysicsComponent {
    /// Number of blocks fallen in current fall
    pub fallen: u32,

    /// Acceleration velocity to apply this frame
    pub acceleration: Vector2,

    /// Max speed limit to apply this frame
    pub max_speed: f32,
}

impl Default for PhysicsComponent {
    fn default() -> Self {
        PhysicsComponent {
            fallen: 0,
            acceleration: Vector2::zero(),
            max_speed: 0.0,
        }
    }
}

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        Read<'a, WorldRef>,
        Read<'a, EntitiesRes>,
        ReadStorage<'a, PhysicalComponent>,
        WriteStorage<'a, TransformComponent>,
        WriteStorage<'a, PhysicsComponent>,
        ReadStorage<'a, HauledItemComponent>,
    );

    fn run(
        &mut self,
        (world_ref, entities, physical, mut transform, mut physics, hauled): Self::SystemData,
    ) {
        let mut friction = config::get().simulation.friction;

        let world = world_ref.borrow();

        // apply physics to NON-HAULED entities i.e. those that independently move and aren't anchored to another
        for (e, physical, transform, physics, _) in
            (&entities, &physical, &mut transform, &mut physics, !&hauled).join()
        {
            let e = Entity::from(e);
            log_scope!(o!("system" => "physics", e));

            // update last position for render interpolation
            transform.last_position = transform.position;

            // update last accessible position
            {
                let floor_pos = transform.position.floor();
                if world.area(floor_pos).ok().is_some() {
                    if let Some(old) = transform.accessible_position.replace(floor_pos) {
                        if old != floor_pos {
                            trace!("updating accessible position"; "position" => %floor_pos);
                        }
                    }
                }
            }

            let bounds = transform.bounds(physical.max_dimension().metres() / 2.0);

            // resolve vertical collision i.e. step up
            if !bounds.check(&*world).is_all_air() {
                let resolved_bounds = bounds.resolve_vertical_collision(&*world);
                let new_pos = resolved_bounds.into_position();

                trace!(
                    "resolving collision";
                    "from" => %transform.position,
                    "to" => %new_pos
                );

                transform.position = new_pos;
            }

            // apply gravity
            if bounds.check_ground(&*world).is_all_air() {
                // floating!

                // slow down even more horizontally
                friction *= FALL_SLOWDOWN;

                // plop down 1 block
                transform.position.modify_z(|z| z - 1.0);
                physics.fallen += 1;

                // forget about now-invalid last accessible position
                if physics.fallen > 2 {
                    trace!("removing last accessible position due to fall");
                    transform.accessible_position = None;
                }
            } else {
                // on the ground
                let fallen = std::mem::take(&mut physics.fallen);
                if fallen > 0 {
                    // TODO apply fall damage if applicable
                    if fallen > 1 {
                        debug!("fell {fallen} blocks", fallen = fallen);
                    }
                }
            }

            let mut velocity = transform.velocity;

            // slow down over time
            velocity *= friction;
            if velocity.magnitude2() < STOP_LIMIT * STOP_LIMIT {
                // practically zero
                velocity.set_zero();
            }

            // accelerate and limit to max speed
            let acceleration = std::mem::replace(&mut physics.acceleration, Vector2::zero());
            velocity = truncate(velocity + acceleration, physics.max_speed);

            // apply velocity
            transform.position += velocity;
            transform.velocity = velocity;

            // face direction of travel
            if !velocity.is_zero() {
                // TODO lerp towards new rotation
                transform.rotation = Basis2::from_angle(AXIS_FWD_2.angle(velocity));
            }
        }
    }
}
