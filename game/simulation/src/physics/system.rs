use crate::ecs::*;
use crate::movement::DesiredMovementComponent;
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
        WriteStorage<'a, TransformComponent>,
    );

    fn run(&mut self, (world_ref, entities, movement, mut transform): Self::SystemData) {
        let (max_speed, mut friction) = {
            let cfg = &config::get().simulation;
            (cfg.max_speed, cfg.friction)
        };

        let world = world_ref.borrow();

        for (e, movement, transform) in (&entities, &movement, &mut transform).join() {
            // update last position for render interpolation
            transform.last_position = transform.position;

            let bounds = transform.bounds();

            // resolve vertical collision i.e. step up
            if !bounds.check(&*world).is_all_air() {
                let resolved_bounds = bounds.resolve_vertical_collision(&*world);
                let new_pos = resolved_bounds.into_position();

                trace!(
                    "{:?}) resolving collision at pos {} by moving to {}",
                    e,
                    transform.position,
                    new_pos
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
            } else {
                // on the ground
                let fallen = std::mem::take(&mut transform.fallen);
                if fallen > 0 {
                    // TODO apply fall damage if applicable
                    if fallen > 1 {
                        debug!("{:?} fell {} blocks", e, fallen);
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
            velocity = truncate(velocity + acceleration, max_speed);

            // apply velocity
            transform.position += velocity;
            transform.velocity = velocity;
        }
    }
}
