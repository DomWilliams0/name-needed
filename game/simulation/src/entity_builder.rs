use crate::ecs::{entity_id, Component, EcsWorld, Entity};
use crate::movement::DesiredMovementComponent;
use crate::path::FollowPathComponent;
use crate::steer::SteeringComponent;
use crate::{PhysicalComponent, TransformComponent};
use color::ColorRgb;
use common::*;
use specs::{Builder, WorldExt};
use std::ops::Deref;
use unit::world::WorldPosition;
use world::WorldRef;

pub struct EntityBuilder<'a> {
    ecs_world: &'a mut EcsWorld,

    block_pos: Option<(i32, i32, Option<i32>)>,

    physical: Option<PhysicalComponent>,
    steering: Option<SteeringComponent>,
    desired_movement: Option<DesiredMovementComponent>,
    follow_path: Option<FollowPathComponent>,
}

impl<'a> EntityBuilder<'a> {
    pub fn new(ecs_world: &'a mut EcsWorld) -> Self {
        Self {
            ecs_world,

            block_pos: None,
            physical: None,
            steering: None,
            desired_movement: None,
            follow_path: None,
        }
    }

    pub fn with_wandering_human_archetype(&mut self) -> &mut Self {
        self.steering = Some(Default::default());
        self.desired_movement = Some(Default::default());
        self.follow_path = Some(Default::default());
        self
    }

    pub fn with_transform(&mut self, block_pos: (i32, i32, Option<i32>)) -> &mut Self {
        self.block_pos = Some(block_pos);
        self
    }

    pub fn with_physical(&mut self, diameter: f32, color: ColorRgb) -> &mut Self {
        self.physical = Some(PhysicalComponent::new(color, diameter, 1.0));
        self
    }

    pub fn build(&mut self) -> Result<Entity, &'static str> {
        let voxel_world: WorldRef = {
            let voxel_world_resource = self.ecs_world.read_resource::<WorldRef>();
            voxel_world_resource.deref().clone()
        };

        let transform = {
            let world_pos = match self.block_pos {
                None => return Err("no transform"),
                Some((x, y, Some(z))) => WorldPosition(x, y, z),
                Some((x, y, None)) => voxel_world
                    .borrow()
                    .find_accessible_block_in_column(x, y)
                    .ok_or("couldn't find highest safe point")?,
            };

            let mut transform = TransformComponent::from_block_centre(world_pos);

            if let Some(phys) = self.physical.as_ref() {
                // stand on top
                transform.position.2 += phys.height() / 2.0;
            }

            transform
        };

        let mut builder = self.ecs_world.create_entity().with(transform);

        // disgusting
        builder = add_component(builder, self.physical.take());
        builder = add_component(builder, self.steering.take());
        builder = add_component(builder, self.desired_movement.take());
        builder = add_component(builder, self.follow_path.take());

        let entity = builder.build();
        event_verbose(Event::Entity(EntityEvent::Create(entity_id(entity))));
        Ok(entity)
    }
}

// helper
fn add_component<C: Component>(
    mut builder: specs::EntityBuilder,
    comp: Option<C>,
) -> specs::EntityBuilder {
    if let Some(comp) = comp {
        builder = builder.with(comp);
    }

    builder
}
