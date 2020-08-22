use common::*;

use crate::ecs::*;
use crate::movement::DesiredMovementComponent;
use crate::steer::behaviour::SteeringResult;
use crate::steer::SteeringBehaviour;
use crate::TransformComponent;
use world::WorldRef;

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
        Read<'a, WorldRef>,
        ReadStorage<'a, TransformComponent>,
        WriteStorage<'a, SteeringComponent>,
        WriteStorage<'a, DesiredMovementComponent>,
    );

    fn run(&mut self, (entities, world_ref, transform, mut steer, mut movement): Self::SystemData) {
        let world = world_ref.borrow();
        for (e, transform, mut steer, movement) in
            (&entities, &transform, &mut steer, &mut movement).join()
        {
            log_scope!(o!("system" => "steering", E(e)));

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
                    "finished steering, reverting to default behaviour";
                    "finished" => ?steer.behaviour
                );

                steer.behaviour = SteeringBehaviour::default();
            }

            // avoid collision with world
            let avoidance = transform.feelers_bounds();
            // TODO cache allocation in system
            let mut solids = Vec::new();
            if avoidance.find_solids(&*world, &mut solids) {
                for pos in &solids {
                    let angle = Rad::atan2(-pos.0 as f32, pos.1 as f32);
                    context_map.write_danger(angle, 0.05);
                    trace!("registering collision danger"; "angle" => ?angle);
                }
            }
        }
    }
}
