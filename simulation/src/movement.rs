use num_traits::zero;

use common::*;

use crate::ecs::*;
use crate::physics::PhysicsComponent;

pub const AXIS_UP: Vector3 = Vector3::new(0.0, 0.0, 1.0);
pub const AXIS_FWD: Vector3 = Vector3::new(0.0, 1.0, 0.0);

/// Desired movement by the brain, and practical movement to be realized by the body
#[derive(Debug, Copy, Clone)]
pub struct DesiredMovementComponent {
    pub realized_velocity: Vector2,

    pub desired_velocity: Vector2,

    /// 0-1
    pub jump_force: f32,
}

impl Default for DesiredMovementComponent {
    fn default() -> Self {
        Self {
            realized_velocity: zero(),
            desired_velocity: zero(),
            jump_force: 0.0,
        }
    }
}

/// Converts *desired* movement to *practical* movement.
/// this will depend on the entity's health and presence of necessary limbs -
/// you can't jump without legs, or see a jump without eyes
pub struct MovementFulfilmentSystem;

impl System for MovementFulfilmentSystem {
    fn tick_system(&mut self, data: &mut TickData) {
        let query = <(Write<DesiredMovementComponent>, Write<PhysicsComponent>)>::query();
        for (mut movement, mut physics) in query.iter(data.ecs_world) {
            // scale velocity based on max speed
            let vel = movement.desired_velocity * config::get().simulation.move_speed;

            let jump_force = {
                if movement.jump_force > 0.0 {
                    // preserve desired jump force
                    movement.jump_force
                } else if physics.collider.jump_sensor_occluded() {
                    // jump if sensor is occluded
                    // TODO always?
                    // TODO get block height+type at sensor (or self.pos + fwd*1), dont jump up half blocks
                    1.0 // TODO jump force
                } else {
                    0.0
                }
            };

            debug_assert!((0.0f32..=1.0).contains(&jump_force));

            movement.realized_velocity = vel;
            movement.jump_force = jump_force;
        }
    }
}

pub fn angle_from_direction(direction: Vector2) -> Rad<F> {
    let direction = direction.extend(0.0);
    let mut angle = direction.angle(AXIS_FWD);

    if direction.cross(AXIS_FWD).dot(AXIS_UP).is_sign_positive() {
        angle = -angle;
    }

    angle
}

#[cfg(test)]
mod test {
    use cgmath::{Quaternion, Rotation, Rotation3};

    use common::*;

    use super::*;

    fn do_rot_non_normal<V: Into<Vector2>>(vec_in: V) {
        do_rot(vec_in.into().normalize())
    }

    fn do_rot<V: Into<Vector2>>(vec_in: V) {
        let vec_in = vec_in.into();
        let angle = angle_from_direction(vec_in);

        let quat = Quaternion::from_axis_angle(AXIS_UP, angle);
        let vec_out = quat.rotate_vector(AXIS_FWD);

        assert!(vec_out.x.approx_eq(vec_in.x, (0.0001, 2)));
        assert!(vec_out.y.approx_eq(vec_in.y, (0.0001, 2)));
    }

    #[test]
    fn angle_from_rotation_right() {
        do_rot((1.0, 0.0));
    }

    #[test]
    fn angle_from_rotation_left() {
        do_rot((-1.0, 0.0));
    }

    #[test]
    fn angle_from_rotation_up() {
        do_rot((0.0, 1.0));
    }

    #[test]
    fn angle_from_rotation_down() {
        do_rot((0.0, -1.0));
    }

    #[test]
    fn angle_from_rotation_various() {
        do_rot_non_normal((0.2, 0.4));
        do_rot_non_normal((0.7, 0.133));
        do_rot_non_normal((0.5, 0.5));

        let mut rando = thread_rng();
        for _ in 0..50 {
            do_rot_non_normal((
                rando.gen_range(0.0f32, 1.0f32),
                rando.gen_range(0.0f32, 1.0f32),
            ));
        }
    }
}
