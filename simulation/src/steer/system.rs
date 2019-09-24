use specs::prelude::*;
use specs_derive::Component;

use crate::movement::Velocity;
use crate::steer::behaviour::{Arrive, CompleteAction, Nop, Seek};
use crate::steer::SteeringBehaviour;
use crate::Position;

/// Steering behaviour
#[derive(Component)]
#[storage(VecStorage)]
pub struct Steering {
    pub behaviour: SteeringBehaviour,
}

impl Default for Steering {
    fn default() -> Self {
        Self {
            behaviour: SteeringBehaviour::Nop(Nop),
        }
    }
}

impl Steering {
    pub fn seek(target: Position) -> Self {
        Self {
            behaviour: SteeringBehaviour::Seek(Seek { target }),
        }
    }

    pub fn arrive(target: Position) -> Self {
        Self {
            behaviour: SteeringBehaviour::Arrive(Arrive {
                target,
                approach_radius: 5.0,
                arrival_radius: 1.0,
            }),
        }
    }
}

pub struct SteeringSystem;

impl<'a> System<'a> for SteeringSystem {
    type SystemData = (
        ReadStorage<'a, Position>,
        WriteStorage<'a, Steering>,
        WriteStorage<'a, Velocity>,
    );

    fn run(&mut self, (pos, mut steer, mut vel): Self::SystemData) {
        for (pos, steer, vel) in (&pos, &mut steer, &mut vel).join() {
            if let CompleteAction::Stop = steer.behaviour.tick(*pos, vel) {
                steer.behaviour = SteeringBehaviour::default();
            }
        }
    }
}
