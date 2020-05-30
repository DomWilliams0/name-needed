use common::*;
use unit::world::WorldPoint;

use crate::ecs::{Component, VecStorage};

/// Position and rotation component
#[derive(Debug, Copy, Clone, Component)]
#[storage(VecStorage)]
pub struct TransformComponent {
    /// Position in world, center of entity in x/y and bottom of entity in z
    pub position: WorldPoint,

    /// Height in z axis
    pub height: f32,

    /// Bounding box radius
    pub bounding_radius: f32,

    /// Used for render interpolation
    pub last_position: WorldPoint,

    /// 1d rotation around z axis
    pub rotation: Basis2,

    /// Current velocity
    pub velocity: Vector2,
}

impl TransformComponent {
    pub fn new(position: WorldPoint, bounding_radius: f32, height: f32) -> Self {
        Self {
            position,
            bounding_radius,
            height,
            rotation: Basis2::from_angle(rad(0.0)),
            last_position: position,
            velocity: Zero::zero(),
        }
    }

    pub fn set_height(&mut self, z: i32) {
        let z = z as f32;
        self.position.2 = z as f32;
    }

    pub const fn slice(&self) -> i32 {
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
