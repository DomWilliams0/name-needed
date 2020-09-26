use common::derive_more::*;
use common::NormalizedFloat;

use crate::ecs::*;
use crate::item::condition::ItemCondition;
use crate::item::SlotIndex;
use crate::needs::Fuel;
use specs::{Builder, EntityBuilder};

/// Common properties across all items
#[derive(Component, EcsComponent, Constructor, Clone, Debug)]
#[storage(DenseVecStorage)]
#[name("item")]
pub struct BaseItemComponent {
    pub name: String,
    pub condition: ItemCondition,

    // Kilograms
    pub mass: f32,

    /// Number of base inventory slots this takes up e.g. in hand
    pub base_slots: u8,

    /// Number of mounted inventory slots this takes up e.g. in bag
    pub mounted_slots: u8,

    pub stack_size: u16,
}

#[derive(Component, EcsComponent, Constructor, Clone, Debug)]
#[name("edible")]
#[storage(DenseVecStorage)]
pub struct EdibleItemComponent {
    /// All fuel available from this item - never changes, decrease base item condition instead
    pub total_nutrition: Fuel,
    // TODO proper nutritional value

    // TODO food debris - the last X fuel/proportion is inedible and has to be disposed of
    // TODO depending on their mood/personality this will be tossed to the ground or taken to a proper place
}

// TODO use item mass to determine how far it flies? or also aerodynamic-ness
#[derive(Component, EcsComponent, Default, Debug)]
#[storage(NullStorage)]
#[name("throwable")]
pub struct ThrowableItemComponent;

// TODO drinkable
// TODO splatterable (after throw, if walked on)
// TODO weapon (damage to target per hit, damage to own condition per hit, attack speed, cooldown)

/// Holding an item in base inventory and using it
#[derive(Component, EcsComponent, Debug)]
#[storage(HashMapStorage)]
#[name("using-item")]
pub struct UsingItemComponent {
    /// Amount of item left to use, if this reaches 0 this component will be removed and the
    /// activity finished gracefully.
    pub left: NormalizedFloat,

    /// Amount to reduce `left` by each tick
    // pub increment: NormalizedFloat,

    /// Item must be in base inventory to use TODO is this needed?
    pub base_slot: SlotIndex,
}

impl<V: Value> ComponentTemplate<V> for BaseItemComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        Ok(Box::new(Self {
            name: values.get_string("name")?,
            condition: ItemCondition::perfect(),
            mass: values.get_float("mass")?,
            base_slots: values.get_int("base_slots")?,
            mounted_slots: values.get_int("mounted_slots")?,
            stack_size: match values.get_string("stacking")?.as_str() {
                "single" => 1,
                "item_default" => 150,
                s => {
                    return Err(ComponentBuildError::InvalidEnumVariant(
                        s.to_owned(),
                        "item stacking",
                    ))
                }
            },
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
