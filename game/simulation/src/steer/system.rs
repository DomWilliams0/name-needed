use common::*;

use crate::ecs::*;
use crate::movement::DesiredMovementComponent;
use crate::steer::behaviour::SteeringResult;
use crate::steer::context::ContextMap;
use crate::steer::SteeringBehaviour;
use crate::transform::PhysicalComponent;
use crate::{TransformComponent, WorldRef};

/// Steering context
#[derive(Default, Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("steering")]
#[clone(disallow)]
pub struct SteeringComponent {
    pub behaviour: SteeringBehaviour,
}

pub struct SteeringSystem;

impl<'a> System<'a> for SteeringSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, WorldRef>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, PhysicalComponent>,
        WriteStorage<'a, SteeringComponent>,
        WriteStorage<'a, DesiredMovementComponent>,
    );

    fn run(
        &mut self,
        (entities, world_ref, transform, physical, mut steer, mut movement): Self::SystemData,
    ) {
        let world = world_ref.borrow();
        for (e, transform, physical, mut steer, mut movement) in
            (&entities, &transform, &physical, &mut steer, &mut movement).join()
        {
            let e = Entity::from(e);
            log_scope!(o!("system" => "steering", e));

            let mut context_map = ContextMap::default();
            let bounding_radius = physical.max_dimension().metres() / 2.0;

            // populate steering interests from current behaviour
            let result =
                steer
                    .behaviour
                    .tick(transform, bounding_radius, context_map.interests_mut());

            if let SteeringResult::Finished = result {
                trace!(
                    "finished steering, reverting to default behaviour";
                    "finished" => ?steer.behaviour
                );

                steer.behaviour = SteeringBehaviour::default();
            }

            // avoid collision with world
            let avoidance = transform.feelers_bounds(bounding_radius);
            // TODO cache allocation in system
            let mut solids = Vec::new();
            if avoidance.find_solids(&*world, &mut solids) {
                for pos in &solids {
                    let angle = Rad::atan2(-pos.0 as f32, pos.1 as f32);
                    context_map.write_danger(angle, 0.05);
                    trace!("registering collision danger"; "angle" => ?angle);
                }
            }

            movement.0 = context_map;
        }
    }
}
