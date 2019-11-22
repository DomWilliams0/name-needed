use cgmath::prelude::*;
use cgmath::Vector3;

use crate::movement::Velocity;
use crate::Position;
use world::WorldPoint;

impl From<Position> for Vector3<f32> {
    fn from(pos: Position) -> Self {
        Self {
            x: pos.x(),
            y: pos.y(),
            z: pos.z(),
        }
    }
}

#[derive(Debug)]
pub enum SteeringBehaviour {
    Nop(Nop),
    Seek(Seek),
    Arrive(Arrive),
}

/// When steering is complete
pub enum CompleteAction {
    /// Not yet complete
    Continue,

    /// Complete
    Stop,
}

impl Default for SteeringBehaviour {
    fn default() -> Self {
        Self::Nop(Nop)
    }
}

impl SteeringBehaviour {
    pub fn tick(&mut self, current_pos: Position, vel: &mut Velocity) -> CompleteAction {
        match self {
            SteeringBehaviour::Nop(behaviour) => behaviour.tick(current_pos, vel),
            SteeringBehaviour::Seek(behaviour) => behaviour.tick(current_pos, vel),
            SteeringBehaviour::Arrive(behaviour) => behaviour.tick(current_pos, vel),
        }
    }
}

// TODO populate "desired velocity" in Velocity component which is normalized
// then movement system can use that on its current movement speed

trait DoASteer {
    fn tick(&mut self, current_pos: Position, vel: &mut Velocity) -> CompleteAction;
}

// nop
#[derive(Default, Debug)]
pub struct Nop;

impl DoASteer for Nop {
    fn tick(&mut self, _current_pos: Position, _vel: &mut Velocity) -> CompleteAction {
        // it never ends
        CompleteAction::Continue
    }
}

// seek
#[derive(Default, Debug)]
pub struct Seek {
    pub target: WorldPoint,
}

impl DoASteer for Seek {
    fn tick(&mut self, current_pos: Position, vel: &mut Velocity) -> CompleteAction {
        let target: Vector3<f32> = self.target.into();
        let current_pos: Vector3<f32> = current_pos.into();

        let delta = (target - current_pos).normalize();

        vel.x = delta.x;
        vel.y = delta.y;
        // TODO z?

        // seek forever
        CompleteAction::Continue
    }
}

// arrive
#[derive(Default, Debug)]
pub struct Arrive {
    pub target: WorldPoint,
    pub approach_radius: f32,
    pub arrival_radius: f32,
}

impl DoASteer for Arrive {
    fn tick(&mut self, current_pos: Position, vel: &mut Velocity) -> CompleteAction {
        let target: Vector3<f32> = self.target.into();
        let current_pos: Vector3<f32> = current_pos.into();
        let distance = current_pos.distance2(target);

        let (new_vel, action) = if distance < self.arrival_radius.powi(2) {
            // arrive
            (Vector3::zero(), CompleteAction::Stop)
        } else {
            let delta = (target - current_pos).normalize();
            let vel = if distance < self.approach_radius.powi(2) {
                // approach
                delta * (distance.sqrt() / self.approach_radius) // TODO expensive sqrt avoidable?
            } else {
                // seek as usual
                delta
            };

            (vel, CompleteAction::Continue)
        };

        // TODO speed?
        // TODO lerp towards desired velocity in movement system?
        vel.x = new_vel.x;
        vel.y = new_vel.y;
        // TODO z?

        action
    }
}
