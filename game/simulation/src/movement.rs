use num_traits::zero;

use common::*;

use crate::ecs::*;

pub const AXIS_UP: Vector3 = Vector3::new(0.0, 0.0, 1.0);
pub const AXIS_FWD: Vector3 = Vector3::new(0.0, 1.0, 0.0);

/// Desired movement by the brain, and practical movement to be realized by the body
#[derive(Debug, Copy, Clone)]
pub struct DesiredMovementComponent {
    pub realized_velocity: Vector2,

    pub desired_velocity: Vector2,

    pub jump_behavior: DesiredJumpBehavior,
}

impl Component for DesiredMovementComponent {
    type Storage = VecStorage<Self>;
}

impl Default for DesiredMovementComponent {
    fn default() -> Self {
        Self {
            realized_velocity: zero(),
            desired_velocity: zero(),
            jump_behavior: DesiredJumpBehavior::default(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DesiredJumpBehavior {
    /// Only jump if possible and the jump sensor is occluded
    OnSensorObstruction,

    /// Ignore jump sensor and jump if possible now
    #[allow(unused)]
    ManuallyRightNow,
}

impl Default for DesiredJumpBehavior {
    fn default() -> Self {
        DesiredJumpBehavior::OnSensorObstruction
    }
}

/// Converts *desired* movement to *practical* movement.
/// this will depend on the entity's health and presence of necessary limbs -
/// you can't jump without legs, or see a jump without eyes
pub struct MovementFulfilmentSystem;

impl<'a> System<'a> for MovementFulfilmentSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        WriteStorage<'a, DesiredMovementComponent>,
    );

    fn run(&mut self, (entities, mut movement): Self::SystemData) {
        for (e, mut movement) in (&entities, &mut movement).join() {
            // scale velocity based on max speed
            let vel = movement.desired_velocity * config::get().simulation.move_speed;

            movement.realized_velocity = vel;

            event_trace(Event::Entity(EntityEvent::MovementIntention(
                entity_id(e),
                (vel.x, vel.y),
            )));
        }
    }
}
