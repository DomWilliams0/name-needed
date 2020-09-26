use common::*;

use crate::ecs::*;
use crate::path::FollowPathComponent;
use crate::steer::context::ContextMap;
use crate::steer::SteeringComponent;
use specs::{Builder, EntityBuilder};
use std::hint::unreachable_unchecked;

/// Desired movement by the brain, and practical movement to be realized by the body
#[derive(Copy, Clone, Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("desired-movement")]
pub enum DesiredMovementComponent {
    Desired(ContextMap),
    Realized(Vector2),
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
        WriteStorage<'a, DesiredMovementComponent>,
        ReadStorage<'a, MovementConfigComponent>,
    );

    fn run(&mut self, (mut movement, config): Self::SystemData) {
        for (movement, config) in (&mut movement, &config).join() {
            let context_map = match movement {
                DesiredMovementComponent::Desired(cm) => cm,
                _ => unreachable!("movement fulfilment expects desired movement only"),
            };

            // resolve context map to a direction
            let (angle, speed) = context_map.resolve();
            let direction = forward_angle(angle);

            // scale velocity based on acceleration
            let vel = direction * (speed * config.acceleration);
            *movement = DesiredMovementComponent::Realized(vel);
        }
    }
}

/// Movement speeds
#[derive(Clone, Component, EcsComponent, Debug)]
#[storage(DenseVecStorage)]
#[name("movement-cfg")]
pub struct MovementConfigComponent {
    pub max_speed: f32,
    pub acceleration: f32,
}

impl<V: Value> ComponentTemplate<V> for MovementConfigComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        Ok(Box::new(Self {
            max_speed: values.get_float("max_speed")?,
            acceleration: values.get_float("acceleration")?,
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder
            .with(self.clone())
            .with(SteeringComponent::default())
            .with(FollowPathComponent::default())
            .with(DesiredMovementComponent::default())
    }
}

register_component_template!("movement", MovementConfigComponent);
