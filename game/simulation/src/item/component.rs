use common::derive_more::*;

use crate::ecs::*;
use crate::item::condition::ItemCondition;
use crate::needs::Fuel;

/// Common properties across all items
#[derive(Component, EcsComponent, Constructor, Clone, Debug)]
#[storage(DenseVecStorage)]
#[name("item")]
pub struct BaseItemComponent {
    pub name: String,
    pub condition: ItemCondition,
}

#[derive(Component, EcsComponent, Constructor, Clone, Debug)]
#[name("edible")]
#[storage(DenseVecStorage)]
pub struct EdibleItemComponent {
    /// All fuel available from this item - never changes, decrease base item condition instead
    pub total_nutrition: Fuel,
    // TODO proper nutritional value
    /// Extra number of hands needed to eat this
    pub extra_hands: u16,
    // TODO food debris - the last X fuel/proportion is inedible and has to be disposed of
    // TODO depending on their mood/personality this will be tossed to the ground or taken to a proper place
}

// TODO add aerodynamic-ness field
#[derive(Component, EcsComponent, Default, Debug)]
#[storage(NullStorage)]
#[name("throwable")]
pub struct ThrowableItemComponent;

// TODO drinkable
// TODO splatterable (after throw, if walked on)
// TODO weapon (damage to target per hit, damage to own condition per hit, attack speed, cooldown)

impl<V: Value> ComponentTemplate<V> for BaseItemComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        Ok(Box::new(Self {
            name: values.get_string("name")?,
            condition: ItemCondition::perfect(),
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }
}

impl<V: Value> ComponentTemplate<V> for EdibleItemComponent {
    fn construct(
        values: &mut Map<V>,
    ) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError> {
        Ok(Box::new(Self {
            total_nutrition: values.get_int("total_nutrition")?,
            extra_hands: values.get_int("extra_hands")?,
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }
}

impl<V: Value> ComponentTemplate<V> for ThrowableItemComponent {
    fn construct(_: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError> {
        Ok(Box::new(Self))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(ThrowableItemComponent)
    }
}

register_component_template!("item", BaseItemComponent);
register_component_template!("edible", EdibleItemComponent);
register_component_template!("throwable", ThrowableItemComponent);
