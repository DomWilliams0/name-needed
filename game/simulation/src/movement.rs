use common::*;

use crate::ecs::*;
use crate::path::FollowPathComponent;
use crate::steer::context::ContextMap;
use crate::steer::SteeringComponent;

use crate::physics::PhysicsComponent;

/// Desired movement by the brain
#[derive(Copy, Clone, Default, Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("desired-movement")]
#[clone(disallow)]
pub struct DesiredMovementComponent(pub ContextMap);

/// Converts *desired* movement from context steering map to *practical* movement.
/// this will depend on the entity's health and presence of necessary limbs -
/// you can't jump without legs, or see a jump without eyes
pub struct MovementFulfilmentSystem;

impl<'a> System<'a> for MovementFulfilmentSystem {
    type SystemData = (
        ReadStorage<'a, DesiredMovementComponent>,
        ReadStorage<'a, MovementConfigComponent>,
        WriteStorage<'a, PhysicsComponent>,
    );

    fn run(&mut self, (desired, config, mut physics): Self::SystemData) {
        for (desired, config, physics) in (&desired, &config, &mut physics).join() {
            let DesiredMovementComponent(context_map) = desired;

            // resolve context map to a direction
            let (angle, speed) = context_map.resolve();
            let direction = forward_angle(angle);

            // TODO actually use body health to determine how much movement is allowed

            // scale velocity based on acceleration
            let vel = direction * (speed * config.acceleration);
            physics.acceleration = vel;

            // TODO scale max speed based on applied effects?
            physics.max_speed = config.max_speed;
        }
    }
}

/// Movement speeds
#[derive(Clone, Component, EcsComponent, Debug)]
#[storage(DenseVecStorage)]
#[name("movement-cfg")]
#[clone(disallow)]
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
