use cgmath::prelude::*;
use cgmath::Vector3;

use crate::movement::DesiredVelocity;
use crate::Transform;
use world::WorldPoint;

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
    pub fn tick(&mut self, transform: &Transform, vel: &mut DesiredVelocity) -> CompleteAction {
        match self {
            SteeringBehaviour::Nop(behaviour) => behaviour.tick(transform, vel),
            SteeringBehaviour::Seek(behaviour) => behaviour.tick(transform, vel),
            SteeringBehaviour::Arrive(behaviour) => behaviour.tick(transform, vel),
        }
    }
}

// TODO populate "desired velocity" in DesiredVelocity component which is normalized
// then movement system can use that on its current movement speed

trait DoASteer {
    fn tick(&mut self, transform: &Transform, vel: &mut DesiredVelocity) -> CompleteAction;
}

// nop
#[derive(Default, Debug)]
pub struct Nop;

impl DoASteer for Nop {
    fn tick(&mut self, _transform: &Transform, _vel: &mut DesiredVelocity) -> CompleteAction {
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
    fn tick(&mut self, transform: &Transform, vel: &mut DesiredVelocity) -> CompleteAction {
        let target: Vector3<f32> = self.target.into();
        let current_pos: Vector3<f32> = transform.position.into();

        let delta = (target - current_pos).normalize();

        vel.velocity.x = delta.x;
        vel.velocity.y = delta.y;
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
    fn tick(&mut self, transform: &Transform, vel: &mut DesiredVelocity) -> CompleteAction {
        let target: Vector3<f32> = self.target.into();
        let current_pos: Vector3<f32> = transform.position.into();
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
        vel.velocity.x = new_vel.x;
        vel.velocity.y = new_vel.y;
        // TODO z?

        action
    }
}
