use specs::prelude::*;
use specs_derive::Component;

/// World position
#[derive(Component, Debug, Copy, Clone)]
#[storage(VecStorage)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: i32,
}

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
            pos.x += vel.x * 0.1;
            pos.y += vel.y * 0.1;
        }
    }
}

impl Position {
    pub fn new(x: f32, y: f32, z: i32) -> Self {
        Self { x, y, z }
    }
}
