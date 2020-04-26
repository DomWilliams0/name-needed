use common::*;
use unit::world::{WorldPoint, WorldPosition};

use crate::ecs::{Component, VecStorage};

/// Position and rotation component
#[derive(Debug, Copy, Clone)]
pub struct TransformComponent {
    /// World position, center of entity
    pub position: WorldPoint,

    /// 1d rotation around z axis
    pub rotation: Quaternion,

    /// Used for render interpolation
    pub last_position: WorldPoint,
}

impl Component for TransformComponent {
    type Storage = VecStorage<Self>;
}

impl TransformComponent {
    pub fn new(position: WorldPoint) -> Self {
        Self {
            position,
            rotation: Quaternion::from_angle_z(Rad(0.0)), // TODO test
            last_position: WorldPoint::default(),
        }
    }

    pub fn from_block_centre(pos: WorldPosition) -> Self {
        let mut point = WorldPoint::from(pos);
        point.0 += 0.5;
        point.1 += 0.5;
        Self::new(point)
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
