use std::collections::HashMap;

use crate::ecs::*;

use crate::item::stack::ItemStackComponent;
use crate::item::{ContainedInComponent, ContainerComponent, InventoryComponent};
use crate::simulation::EcsWorldRef;

pub struct InventoryValidationSystem;

impl<'a> System<'a> for InventoryValidationSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldRef>,
        ReadStorage<'a, InventoryComponent>,
        ReadStorage<'a, ContainerComponent>,
        ReadStorage<'a, ItemStackComponent>,
        ReadStorage<'a, ContainedInComponent>,
    );

    fn run(
        &mut self,
        (entities, ecs_world, invs, containers, stacks, contained): Self::SystemData,
    ) {
        let mut seen_items = HashMap::new();
        for (e, inventory) in (&entities, &invs).join() {
            inventory.validate(e.into(), &**ecs_world, &mut seen_items);
        }

        for (e, container) in (&entities, &containers).join() {
            container.validate(e.into(), &**ecs_world, &mut seen_items);
        }

        for (e, stack) in (&entities, &stacks).join() {
            stack.validate(e.into(), &**ecs_world, &mut seen_items);
        }

        for (e, contained) in (&entities, contained.maybe()).join() {
            let e = Entity::from(e);
            let held_by = seen_items.get(&e);
            assert_eq!(contained.is_some(), held_by.is_some(),
                       "{} is in invalid contained state (contained = {:?}, seen in inv, container or stack = {:?})",
                       e,
                       contained,
                       held_by);
        }
    }
}

/// Panics if any container/inventory components are invalid
pub fn validate_all_inventories(world: &EcsWorld) {
    InventoryValidationSystem.run_now(world);
}
