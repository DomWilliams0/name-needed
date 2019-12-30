use common::*;
use world::{InnerWorldRef, WorldPoint, WorldPosition};

use crate::ecs::*;

/// Position and rotation
#[derive(Debug, Copy, Clone)]
pub struct Transform {
    /// World position, center of entity
    pub position: WorldPoint,

    /// Rotation
    pub rotation: Vector3,
}

impl Component for Transform {}

impl Default for Transform {
    fn default() -> Self {
        Self {
            rotation: Vector3::new(0.0, 0.0, 0.0),
            position: WorldPoint::default(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DesiredVelocity {
    /// Normalized
    pub velocity: Vector3,
}

impl Component for DesiredVelocity {}

impl Default for DesiredVelocity {
    fn default() -> Self {
        Self {
            velocity: Vector3::new(0.0, 0.0, 0.0),
        }
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
