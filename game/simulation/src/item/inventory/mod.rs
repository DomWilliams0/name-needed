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
    use crate::ecs::{EcsWorld, Entity};
    use crate::{definitions, ComponentWorld};

    pub fn test_dummy_items() -> (impl ComponentWorld, [Entity; 3]) {
        let registry = definitions::load_from_str(
            r#"
[
	(
		uid: "test_dummy_item",
		components: [],
	),
]
        "#,
        )
        .expect("bad definitions");
        let mut world = EcsWorld::test_new();

        let mut dummies = (0..3).map(|_| {
            registry
                .instantiate("test_dummy_item", &mut world)
                .expect("bad definition")
                .spawn()
                .expect("failed to spawn item")
        });

        let a = dummies.next().expect("no entity");
        let b = dummies.next().expect("no entity");
        let c = dummies.next().expect("no entity");
        (world, [a, b, c])
    }
}
