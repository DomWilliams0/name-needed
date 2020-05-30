use common::derive_more::*;
use common::NormalizedFloat;

use crate::ecs::*;
use crate::item::condition::ItemCondition;
use crate::item::{ItemClass, SlotIndex};
use crate::needs::Fuel;

/// Common properties across all items
#[derive(Component, Constructor, Clone)]
#[storage(DenseVecStorage)]
pub struct BaseItemComponent {
    // TODO this could do with a builder
    pub name: &'static str,
    pub condition: ItemCondition,
    // Kilograms
    pub mass: f32,

    /// Never changes
    pub class: ItemClass, // TODO possible for an item to have multiple classes?

    /// Number of base inventory slots this takes up e.g. in hand
    pub base_slots: u8,

    /// Number of mounted inventory slots this takes up e.g. in bag
    pub mounted_slots: u8,
}

#[derive(Component, Constructor)]
#[storage(DenseVecStorage)]
pub struct EdibleItemComponent {
    /// All fuel available from this item - never changes, decrease base item condition instead
    pub total_nutrition: Fuel,
    // TODO proper nutritional value

    // TODO food debris - the last X fuel/proportion is inedible and has to be disposed of
    // TODO depending on their mood/personality this will be tossed to the ground or taken to a proper place
}

// TODO use item mass to determine how far it flies? or also aerodynamic-ness
#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct ThrowableItemComponent;

// TODO drinkable
// TODO splatterable (after throw, if walked on)
// TODO weapon (damage to target per hit, damage to own condition per hit, attack speed, cooldown)

/// Holding an item in base inventory and using it
#[derive(Component, Debug)]
#[storage(HashMapStorage)]
pub struct UsingItemComponent {
    /// Amount of item left to use, if this reaches 0 this component will be removed and the
    /// activity finished gracefully.
    pub left: NormalizedFloat,

    /// Amount to reduce `left` by each tick
    // pub increment: NormalizedFloat,

    /// Item must be in base inventory to use TODO is this needed?
    pub base_slot: SlotIndex,

    pub class: ItemClass,
}
