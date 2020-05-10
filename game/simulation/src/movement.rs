use common::*;

use crate::ecs::*;
use crate::steer::context::ContextMap;
use std::hint::unreachable_unchecked;

/// Desired movement by the brain, and practical movement to be realized by the body
#[derive(Copy, Clone)]
pub enum DesiredMovementComponent {
    Desired(ContextMap),
    Realized(Vector2),
}

impl Component for DesiredMovementComponent {
    type Storage = VecStorage<Self>;
}

impl DesiredMovementComponent {
    pub unsafe fn context_map_mut_unchecked(&mut self) -> &mut ContextMap {
        match self {
            DesiredMovementComponent::Desired(map) => map,
            _ => unreachable_unchecked(),
        }
    }
}

/// Desired variant
impl Default for DesiredMovementComponent {
    fn default() -> Self {
        DesiredMovementComponent::Desired(ContextMap::default())
    }
}

/// Converts *desired* movement from context steering map to *practical* movement.
/// this will depend on the entity's health and presence of necessary limbs -
/// you can't jump without legs, or see a jump without eyes
pub struct MovementFulfilmentSystem;

impl<'a> System<'a> for MovementFulfilmentSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        WriteStorage<'a, DesiredMovementComponent>,
    );

    fn run(&mut self, (entities, mut movement): Self::SystemData) {
        for (e, movement) in (&entities, &mut movement).join() {
            let context_map = match movement {
                DesiredMovementComponent::Desired(cm) => cm,
                _ => unreachable!("movement fulfilment expects desired movement only"),
            };

            // resolve context map to a direction
            let (angle, speed) = context_map.resolve();
            let direction = forward_angle(angle);

            // scale velocity based on acceleration
            let vel = direction * (speed * config::get().simulation.acceleration);
            *movement = DesiredMovementComponent::Realized(vel);

            event_trace(Event::Entity(EntityEvent::MovementIntention(
                entity_id(e),
                (vel.x, vel.y),
            )));
        }
    }
}
