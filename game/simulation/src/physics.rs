use crate::ecs::*;
use crate::movement::DesiredMovementComponent;
use crate::TransformComponent;

pub struct PhysicsSystem;

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        ReadStorage<'a, DesiredMovementComponent>,
        WriteStorage<'a, TransformComponent>,
    );

    fn run(&mut self, (movement, mut transform): Self::SystemData) {
        for (movement, transform) in (&movement, &mut transform).join() {
            // TODO physics
            let vel = movement.realized_velocity;

            transform.last_position = transform.position;
            transform.position += vel;
        }
    }
}
