use common::*;
use world::WorldPoint;

use crate::ecs::*;
use crate::movement::DesiredVelocity;
use crate::steer::behaviour::{Arrive, CompleteAction, Nop, Seek};
use crate::steer::SteeringBehaviour;
use crate::Transform;

/// Steering behaviour
pub struct Steering {
    pub behaviour: SteeringBehaviour,
}

impl Component for Steering {}

impl Default for Steering {
    fn default() -> Self {
        Self {
            behaviour: SteeringBehaviour::Nop(Nop),
        }
    }
}

impl Steering {
    pub fn seek(target: WorldPoint) -> Self {
        Self {
            behaviour: SteeringBehaviour::Seek(Seek { target }),
        }
    }

    pub fn arrive(target: WorldPoint) -> Self {
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

impl System for SteeringSystem {
    fn tick_system(&mut self, data: &TickData) {
        data.ecs_world
            .matcher_with_entities::<All<(Read<Transform>, Write<Steering>, Write<DesiredVelocity>)>>(
            )
            .for_each(|(e, (transform, steer, vel))| {
                if let CompleteAction::Stop = steer.behaviour.tick(transform, vel) {
                    debug!(
                        "{}: finished steering {:?}, reverting to default behaviour",
                        NiceEntity(e),
                        steer.behaviour
                    );
                    steer.behaviour = SteeringBehaviour::default();
                }
            });
    }
}
