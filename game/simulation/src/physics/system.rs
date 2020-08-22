use crate::ecs::*;
use crate::movement::{DesiredMovementComponent, MovementConfigComponent};
use crate::TransformComponent;
use common::*;
use world::WorldRef;

const STOP_LIMIT: f32 = 0.01;
const FALL_SLOWDOWN: f32 = 0.5;

pub struct PhysicsSystem;

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        Read<'a, WorldRef>,
        Read<'a, EntitiesRes>,
        ReadStorage<'a, DesiredMovementComponent>,
        ReadStorage<'a, MovementConfigComponent>,
        WriteStorage<'a, TransformComponent>,
    );

    fn run(
        &mut self,
        (world_ref, entities, movement, movement_cfg, mut transform): Self::SystemData,
    ) {
        let mut friction = config::get().simulation.friction;

        let world = world_ref.borrow();

        for (e, movement, cfg, transform) in
            (&entities, &movement, &movement_cfg, &mut transform).join()
        {
            log_scope!(o!("system" => "physics", E(e)));

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

            let bounds = transform.bounds();

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
                transform.position.2 -= 1.0;
                transform.fallen += 1;

                // forget about now-invalid last accessible position
                if transform.fallen > 2 {
                    trace!("removing last accessible position due to fall");
                    transform.accessible_position = None;
                }
            } else {
                // on the ground
                let fallen = std::mem::take(&mut transform.fallen);
                if fallen > 0 {
                    // TODO apply fall damage if applicable
                    if fallen > 1 {
                        debug!("fell {fallen} blocks", fallen = fallen);
                    }
                }
            }

            let acceleration = match movement {
                DesiredMovementComponent::Realized(vel) => *vel,
                _ => unreachable!("physics expects realized movement only"),
            };

            let mut velocity = transform.velocity;

            // slow down over time
            velocity *= friction;
            if velocity.magnitude2() < STOP_LIMIT * STOP_LIMIT {
                // practically zero
                velocity.set_zero();
            }

            // accelerate and limit to max speed
            velocity = truncate(velocity + acceleration, cfg.max_speed);

            // apply velocity
            transform.position += velocity;
            transform.velocity = velocity;
        }
    }
}
