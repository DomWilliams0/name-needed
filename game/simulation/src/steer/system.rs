use common::*;

use crate::ecs::*;
use crate::movement::DesiredMovementComponent;
use crate::steer::behaviour::{Arrive, CompleteAction, Nop, Seek};
use crate::steer::SteeringBehaviour;
use crate::TransformComponent;
use unit::world::WorldPoint;

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

impl Component for SteeringComponent {
    type Storage = VecStorage<Self>;
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

impl<'a> System<'a> for SteeringSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        ReadStorage<'a, TransformComponent>,
        WriteStorage<'a, SteeringComponent>,
        WriteStorage<'a, DesiredMovementComponent>,
    );

    fn run(&mut self, (entities, transform, mut steer, mut movement): Self::SystemData) {
        for (e, transform, mut steer, mut movement) in
            (&entities, &transform, &mut steer, &mut movement).join()
        {
            // clear previous tick's inputs
            *movement = DesiredMovementComponent::default();

            if let CompleteAction::Stop = steer.behaviour.tick(&transform, &mut movement) {
                debug!(
                    "{:?}: finished steering {:?}, reverting to default behaviour",
                    e, steer.behaviour
                );
                steer.behaviour = SteeringBehaviour::default();
            }
        }
    }
}
