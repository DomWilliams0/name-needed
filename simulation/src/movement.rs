use common::*;
use world::{InnerWorldRef, WorldPoint, WorldPosition};

use crate::ecs::*;
use num_traits::zero;

pub const AXIS_UP: Vector3 = Vector3::new(0.0, 0.0, 1.0);
pub const AXIS_FWD: Vector3 = Vector3::new(0.0, 1.0, 0.0);

/// Position and rotation
#[derive(Debug, Copy, Clone)]
pub struct Transform {
    /// World position, center of entity
    pub position: WorldPoint,

    /// Rotation angle, corresponds to `rotation_dir`
    rotation: Rad<F>,

    /// Rotation direction
    rotation_dir: Vector2,
}

impl Component for Transform {}

impl Default for Transform {
    fn default() -> Self {
        let mut t = Self {
            rotation: Rad(0.0),
            rotation_dir: zero(),
            position: WorldPoint::default(),
        };
        t.set_rotation_from_direction(AXIS_FWD.truncate()); // just to make sure
        t
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DesiredVelocity {
    /// Normalized
    pub velocity: Vector2,
}

impl Component for DesiredVelocity {}

impl Default for DesiredVelocity {
    fn default() -> Self {
        Self { velocity: zero() }
    }
}

impl Transform {
    pub fn new(position: WorldPoint) -> Self {
        Self {
            position,
            ..Self::default()
        }
    }

    pub fn from_block_center(x: i32, y: i32, z: i32) -> Self {
        let position = WorldPoint(x as f32 + 0.5, y as f32 + 0.5, z as f32);
        Self::new(position)
    }

    pub fn from_highest_safe_point(
        world: &InnerWorldRef,
        block_x: i32,
        block_y: i32,
    ) -> Option<Self> {
        // TODO doesn't take into account width and depth of entity, they might not fit
        world
            .find_accessible_block_in_column(block_x, block_y)
            .map(|pos| {
                let mut pos = WorldPoint::from(pos);

                // center of block
                pos.0 += 0.5;
                pos.1 += 0.5;

                Self::new(pos)
            })
    }

    /* pub */
    fn _place_safely(_world: &InnerWorldRef, _search_from: (i32, i32)) -> Self {
        // TODO guaranteed place safely
        unimplemented!()
    }

    pub fn set_rotation_from_direction(&mut self, direction: Vector2) {
        self.rotation = angle_from_direction(direction);
        self.rotation_dir = direction;
    }

    pub fn rotation_dir(&self) -> Vector2 {
        self.rotation_dir
    }
    pub fn rotation_angle(&self) -> Rad<F> {
        self.rotation
    }

    pub fn slice(&self) -> i32 {
        self.position.2 as i32
    }

    pub const fn x(&self) -> f32 {
        self.position.0
    }
    pub const fn y(&self) -> f32 {
        self.position.1
    }
    pub const fn z(&self) -> f32 {
        self.position.2
    }
}

impl From<Transform> for WorldPosition {
    fn from(transform: Transform) -> Self {
        transform.position.into()
    }
}

impl From<WorldPoint> for Transform {
    fn from(pos: WorldPoint) -> Self {
        Self::new(pos)
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
    use super::*;
    use cgmath::{Quaternion, Rotation, Rotation3};
    use common::*;

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
