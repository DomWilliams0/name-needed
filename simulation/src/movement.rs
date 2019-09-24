use specs::prelude::*;
use specs_derive::Component;

// TODO use cgmath vectors

/// World position
#[derive(Component, Debug, Copy, Clone, Default)]
#[storage(VecStorage)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: i32,
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

impl Position {
    pub fn new(x: f32, y: f32, z: i32) -> Self {
        Self { x, y, z }
    }
}
