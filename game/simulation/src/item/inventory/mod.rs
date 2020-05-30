pub use component::{
    BaseSlotPolicy, InventoryComponent, InventoryError, ItemReference, LooseItemReference,
    SlotReference,
};
pub use contents::{Contents, ContentsGetError, ContentsPutError, ItemSlot};

mod component;
mod contents;

pub type SlotIndex = usize;
pub type MountedInventoryIndex = usize;

#[cfg(test)]
mod test {
    use crate::ecs::{ComponentBuilder, ComponentWorld, DummyComponentReceptacle, Entity};
    use crate::item::*;

    pub fn test_dummy_items() -> (DummyComponentReceptacle, [Entity; 3]) {
        let mut world = DummyComponentReceptacle::new();

        // add dummy items
        let food = world
            .create_entity()
            .with_(BaseItemComponent::new(
                "food",
                ItemCondition::new_perfect(100),
                1.0,
                ItemClass::Food,
                1,
                1,
            ))
            .with_(EdibleItemComponent::new(50))
            .build_();

        let weapon = world
            .create_entity()
            .with_(BaseItemComponent::new(
                "weapon",
                ItemCondition::new_perfect(100),
                1.0,
                ItemClass::Weapon,
                1,
                1,
            ))
            .build_();

        let other_weapon = world
            .create_entity()
            .with_(BaseItemComponent::new(
                "another weapon",
                ItemCondition::new_perfect(100),
                1.0,
                ItemClass::Weapon,
                1,
                1,
            ))
            .build_();

        (world, [food, weapon, other_weapon])
    }
}
