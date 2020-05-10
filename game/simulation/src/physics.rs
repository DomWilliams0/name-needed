use crate::ecs::*;
use crate::movement::DesiredMovementComponent;
use crate::TransformComponent;
use common::*;

pub struct PhysicsSystem;

const STOP_LIMIT: f32 = 0.01;

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        ReadStorage<'a, DesiredMovementComponent>,
        WriteStorage<'a, TransformComponent>,
    );

    fn run(&mut self, (movement, mut transform): Self::SystemData) {
        let (max_speed, friction) = {
            let cfg = &config::get().simulation;
            (cfg.max_speed, cfg.friction)
        };

        for (movement, transform) in (&movement, &mut transform).join() {
            let acceleration = match movement {
                DesiredMovementComponent::Realized(vel) => *vel,
                _ => unreachable!("physics expects realized movement only"),
            };

            transform.last_position = transform.position;

            // slow down over time
            transform.velocity *= friction;
            if transform.velocity.magnitude2() < STOP_LIMIT * STOP_LIMIT {
                // practically zero
                transform.velocity.set_zero();
            }

            let velocity = truncate(transform.velocity + acceleration, max_speed);

            transform.velocity = velocity;
            transform.position += velocity;
        }
    }
}
