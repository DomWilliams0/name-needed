use color::ColorRgb;
use common::*;
use unit::world::WorldPosition;
use world::InnerWorldRef;

use crate::ai::{ActivityComponent, AiComponent};
use crate::ecs::{entity_id, ComponentBuilder, ComponentWorld, Entity};
use crate::item::{
    BaseItemComponent, EdibleItemComponent, InventoryComponent, ItemClass, ItemCondition,
    ThrowableItemComponent,
};
use crate::movement::DesiredMovementComponent;
use crate::needs::HungerComponent;
use crate::path::FollowPathComponent;
use crate::render::PhysicalShape;
use crate::steer::SteeringComponent;
use crate::{RenderComponent, TransformComponent};

// TODO add must_use to all builder patterns
#[must_use = "Use a build_* function to create the entity"]
pub struct EntityBuilder<'a, W: ComponentWorld> {
    world: &'a mut W,

    block_pos: Option<Box<dyn EntityPosition>>,
    height: Option<f32>,
    shape: Option<PhysicalShape>,
    color: Option<ColorRgb>,
}

impl<'a, W: ComponentWorld> EntityBuilder<'a, W> {
    pub fn new(world: &'a mut W) -> Self {
        Self {
            world,

            block_pos: None,
            height: None,
            shape: None,
            color: None,
        }
    }

    pub fn with_pos<P: EntityPosition + 'static>(mut self, pos: P) -> Self {
        self.block_pos = Some(Box::new(pos));
        self
    }

    pub fn with_height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn with_shape(mut self, shape: PhysicalShape) -> Self {
        self.shape = Some(shape);
        self
    }

    pub fn with_color(mut self, color: ColorRgb) -> Self {
        self.color = Some(color);
        self
    }

    pub fn build_human(self, starting_hunger: NormalizedFloat) -> Result<Entity, &'static str> {
        let entity = self
            .build()?
            .with_(DesiredMovementComponent::default())
            .with_(SteeringComponent::default())
            .with_(FollowPathComponent::default())
            .with_(HungerComponent::new(starting_hunger, 3000))
            .with_(ActivityComponent::default())
            .with_(AiComponent::human())
            .with_(InventoryComponent::new(2 /* 2 hands */, 2, Some(0)))
            .build_();

        event_verbose!(Event::CreateEntity(entity_id(entity)));
        Ok(entity)
    }

    pub fn build_food_item(self, nutrition: u16, condition: f32) -> Result<Entity, &'static str> {
        self.build().map(|b| {
            b.with_(BaseItemComponent::new(
                "Food",
                ItemCondition::with_proportion(Proportion::with_proportion(condition, nutrition)),
                0.5,
                ItemClass::Food,
                1,
                1,
            ))
            .with_(EdibleItemComponent::new(nutrition))
            .with_(ThrowableItemComponent::default()) // like all good food should be
            .build_()
        })
    }

    pub fn build(self) -> Result<W::Builder, &'static str> {
        let shape = self.shape.ok_or("no shape")?;
        let transform = {
            let pos = self.block_pos.ok_or("no position")?;
            let height = self.height.ok_or("no height")?;

            let world = self.world.voxel_world();
            let pos = pos.resolve(&world.borrow())?;
            TransformComponent::new(pos.into(), shape.radius(), height)
        };

        let render = RenderComponent::new(self.color.ok_or("no color")?, shape);

        Ok(self.world.create_entity().with_(transform).with_(render))
    }
}

pub trait EntityPosition {
    fn resolve(&self, world: &InnerWorldRef) -> Result<WorldPosition, &'static str>;
}

impl EntityPosition for WorldPosition {
    fn resolve(&self, _: &InnerWorldRef) -> Result<WorldPosition, &'static str> {
        Ok(*self)
    }
}

impl EntityPosition for (i32, i32, Option<i32>) {
    fn resolve(&self, world: &InnerWorldRef) -> Result<WorldPosition, &'static str> {
        match *self {
            (x, y, Some(z)) => Ok(WorldPosition(x, y, z)),
            (x, y, None) => world
                .find_accessible_block_in_column(x, y)
                .ok_or("couldn't find highest safe point"),
        }
    }
}
