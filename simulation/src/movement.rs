use specs::prelude::*;
use specs_derive::Component;

use world::{InnerWorldRef, WorldPoint, WorldPosition};

/// World position, center of entity
#[derive(Component, Debug, Copy, Clone, Default)]
#[storage(VecStorage)]
pub struct Position {
    pub pos: WorldPoint,
}

/// Desired velocity, should be normalized
#[derive(Component, Debug, Copy, Clone)]
#[storage(VecStorage)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

pub struct MovementSystem;

impl<'a> System<'a> for MovementSystem {
    type SystemData = (ReadStorage<'a, Velocity>, WriteStorage<'a, Position>);

    fn run(&mut self, (vel, mut pos): Self::SystemData) {
        for (vel, pos) in (&vel, &mut pos).join() {
            // TODO expand normalized desired velocity to real velocity
            let speed = 0.2;
            let (vx, vy) = (vel.x * speed, vel.y * speed);

            pos.pos.0 += vx;
            pos.pos.1 += vy;
        }
    }
}

// ----

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        let pos = WorldPoint(x, y, z);
        Self { pos }
    }

    pub fn from_block_center(x: i32, y: i32, z: i32) -> Self {
        Self::new(x as f32 + 0.5, y as f32 + 0.5, z as f32)
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

                Self { pos }
            })
    }

    /* pub */
    fn _place_safely(_world: &InnerWorldRef, _search_from: (i32, i32)) -> Self {
        // TODO guaranteed place safely
        unimplemented!()
    }

    pub fn slice(&self) -> i32 {
        self.pos.2 as i32
    }

    pub const fn x(&self) -> f32 {
        self.pos.0
    }
    pub const fn y(&self) -> f32 {
        self.pos.1
    }
    pub const fn z(&self) -> f32 {
        self.pos.2
    }
}

impl From<Position> for WorldPosition {
    fn from(pos: Position) -> Self {
        pos.pos.into()
    }
}

impl From<WorldPoint> for Position {
    fn from(pos: WorldPoint) -> Self {
        Self { pos }
    }
}
