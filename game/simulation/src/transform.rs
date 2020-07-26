use common::*;
use unit::world::WorldPoint;

use crate::ecs::{Component, VecStorage};
use crate::physics::Bounds;

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

    /// Number of blocks fallen in current fall
    pub fallen: u32,
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
            fallen: 0,
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

    pub fn bounds(&self) -> Bounds {
        // allow tiny overlap
        const MARGIN: f32 = 0.8;
        let radius = self.bounding_radius * MARGIN;
        Bounds::from_radius(self.position, radius, radius)
    }

    pub fn feelers_bounds(&self) -> Bounds {
        let feelers = self.velocity + (self.velocity.normalize() * self.bounding_radius);
        let centre = self.position + feelers;

        const EXTRA: f32 = 1.25;
        let length = self.bounding_radius * EXTRA;
        let width = 0.1; // will be floor'd/ceil'd to 0 and 1

        let (x, y) = if feelers.x > feelers.y {
            (width, length)
        } else {
            (length, width)
        };

        Bounds::from_radius(centre, x, y)
    }
}
