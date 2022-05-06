use std::rc::Rc;

use common::derive_more::*;
use unit::food::Nutrition;

use crate::ecs::*;
use crate::item::condition::ItemCondition;
use crate::needs::food::{FoodDescription, FoodFlavours};
use crate::string::StringCache;

/// Condition/durability of an entity, e.g. a tool or food
#[derive(Component, EcsComponent, Constructor, Clone, Debug)]
#[storage(DenseVecStorage)]
#[name("condition")]
pub struct ConditionComponent(pub ItemCondition);

#[derive(Component, EcsComponent, Constructor, Clone, Debug)]
#[name("edible")]
#[storage(DenseVecStorage)]
pub struct EdibleItemComponent {
    /// Immutable description of food properties
    pub description: FoodDescription,

    /// Extra number of hands needed to eat this
    pub extra_hands: u16,

    // TODO food debris - the last X fuel/proportion is inedible and has to be disposed of
    // TODO depending on their mood/personality this will be tossed to the ground or taken to a proper place
    pub flavours: FoodFlavours,
}

// TODO add aerodynamic-ness field
#[derive(Component, EcsComponent, Default, Debug, Clone)]
#[storage(NullStorage)]
#[name("throwable")]
pub struct ThrowableItemComponent;

// TODO drinkable
// TODO splatterable (after throw, if walked on)
// TODO weapon (damage to target per hit, damage to own condition per hit, attack speed, cooldown)

impl<V: Value> ComponentTemplate<V> for ConditionComponent {
    fn construct(
        _: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        Ok(Rc::new(Self(ItemCondition::perfect())))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}

impl<V: Value> ComponentTemplate<V> for EdibleItemComponent {
    fn construct(
        values: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError> {
        let description = FoodDescription {
            total_nutrition: Nutrition::new(values.get_int("total_nutrition")?),
            consumption_rate: Nutrition::new(values.get_int("consumption_rate")?),
            efficiency: values.get_float("efficiency")?,
        };
        let extra_hands = values.get_int("extra_hands")?;
        let flavours = values.get_string("flavours")?.parse().map_err(|e| {
            ComponentBuildError::TemplateSpecific(format!("failed to parse flavours: {e}"))
        })?;

        Ok(Rc::new(Self {
            description,
            extra_hands,
            flavours,
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}

impl<V: Value> ComponentTemplate<V> for ThrowableItemComponent {
    fn construct(
        _: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError> {
        Ok(Rc::new(Self))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(ThrowableItemComponent)
    }

    crate::as_any!();
}

register_component_template!("breakable", ConditionComponent);
register_component_template!("edible", EdibleItemComponent);
register_component_template!("throwable", ThrowableItemComponent);
