use common::*;

use crate::ecs::*;
use crate::movement::DesiredMovementComponent;
use crate::steer::behaviour::SteeringResult;
use crate::steer::SteeringBehaviour;
use crate::TransformComponent;

/// Steering context
#[derive(Default, Component)]
#[storage(DenseVecStorage)]
pub struct SteeringComponent {
    pub behaviour: SteeringBehaviour,
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
        for (e, transform, mut steer, movement) in
            (&entities, &transform, &mut steer, &mut movement).join()
        {
            // reset context map for this tick
            *movement = DesiredMovementComponent::default();

            // safety: definitely Desired from above
            let context_map = unsafe { movement.context_map_mut_unchecked() };

            // populate steering interests from current behaviour
            let result = steer
                .behaviour
                .tick(&transform, context_map.interests_mut());

            if let SteeringResult::Finished = result {
                trace!(
                    "{:?}: finished steering {:?}, reverting to default behaviour",
                    e,
                    steer.behaviour
                );
                // TODO struclog event
                steer.behaviour = SteeringBehaviour::default();
            }

            // TODO populate danger interests from world/other entity collisions
        }
    }
}
