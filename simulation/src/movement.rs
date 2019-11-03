use specs::prelude::*;
use specs_derive::Component;
use world::{InnerWorldRef, WorldPoint, WorldPosition};

// TODO use cgmath vectors

/// World position
#[derive(Component, Debug, Copy, Clone, Default)]
#[storage(VecStorage)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
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

            pos.x += vx;
            pos.y += vy;
        }
    }
}

// ----

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn from_highest_safe_point(world: &InnerWorldRef, x: f32, y: f32) -> Option<Self> {
        world
            .find_accessible_block_in_column(x as i32, y as i32)
            .map(|pos| {
                let mut point = WorldPoint::from(pos);
                point.0 = x;
                point.1 = y;
                point.into()
            })
    }

    /* pub */
    fn _place_safely(_world: &InnerWorldRef, _search_from: (i32, i32)) -> Self {
        // TODO guaranteed place safely
        unimplemented!()
    }

    pub fn slice(&self) -> i32 {
        self.z as i32
    }
}

impl From<&[f32; 3]> for Position {
    fn from(arr: &[f32; 3]) -> Self {
        let [x, y, z] = arr;
        Position::new(*x, *y, *z)
    }
}

impl From<Position> for WorldPosition {
    fn from(pos: Position) -> Self {
        WorldPosition(pos.x as i32, pos.y as i32, pos.z as i32)
    }
}

impl From<WorldPoint> for Position {
    fn from(pos: WorldPoint) -> Self {
        Self {
            x: pos.0,
            y: pos.1,
            z: pos.2,
        }
    }
}
