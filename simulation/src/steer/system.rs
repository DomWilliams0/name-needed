use common::*;
use world::WorldPoint;

use crate::ecs::*;
use crate::movement::DesiredMovementComponent;
use crate::steer::behaviour::{Arrive, CompleteAction, Nop, Seek};
use crate::steer::SteeringBehaviour;
use crate::TransformComponent;

/// Steering behaviour
pub struct SteeringComponent {
    pub behaviour: SteeringBehaviour,
}

impl Default for SteeringComponent {
    fn default() -> Self {
        Self {
            behaviour: SteeringBehaviour::Nop(Nop),
        }
    }
}

impl SteeringComponent {
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
    fn tick_system(&mut self, data: &mut TickData) {
        let query = <(
            Read<TransformComponent>,
            Write<SteeringComponent>,
            Write<DesiredMovementComponent>,
        )>::query();
        for (e, (transform, mut steer, mut movement)) in query.iter_entities(data.ecs_world) {
            // clear previous tick's inputs
            *movement = DesiredMovementComponent::default();

            use std::borrow::BorrowMut;
            if let CompleteAction::Stop = steer
                .behaviour
                .tick(transform.as_ref(), movement.borrow_mut())
            {
                debug!(
                    "{}: finished steering {:?}, reverting to default behaviour",
                    e, steer.behaviour
                );
                steer.behaviour = SteeringBehaviour::default();
            }
        }
    }
}
