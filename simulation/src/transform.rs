use num_traits::zero;

use common::*;
use unit::world::{WorldPoint, WorldPosition};
use world::InnerWorldRef;

use crate::ecs::*;
use crate::movement::angle_from_direction;
use crate::AXIS_FWD;

/// Position and rotation component
#[derive(Debug, Copy, Clone)]
pub struct TransformComponent {
    /// World position, center of entity
    pub position: WorldPoint,

    /// Rotation angle, corresponds to `rotation_dir`
    rotation: Rad<F>,

    /// Rotation direction
    rotation_dir: Vector2,
}

impl Default for TransformComponent {
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

impl TransformComponent {
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
        // TODO this doesn't belong in Transform anyway, should be in world
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

impl From<TransformComponent> for WorldPosition {
    fn from(transform: TransformComponent) -> Self {
        transform.position.into()
    }
}

impl From<WorldPoint> for TransformComponent {
    fn from(pos: WorldPoint) -> Self {
        Self::new(pos)
    }
}
