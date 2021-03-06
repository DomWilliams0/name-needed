use std::collections::HashMap;

use crate::ecs::*;
use crate::item::{ContainedInComponent, ContainerComponent, InventoryComponent};

pub struct InventoryValidationSystem;

impl<'a> System<'a> for InventoryValidationSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldFrameRef>,
        ReadStorage<'a, InventoryComponent>,
        ReadStorage<'a, ContainerComponent>,
        ReadStorage<'a, ContainedInComponent>,
    );

    fn run(&mut self, (entities, ecs_world, invs, containers, contained): Self::SystemData) {
        let mut seen_items = HashMap::new();
        for (e, inventory) in (&entities, &invs).join() {
            inventory.validate(e, &**ecs_world, &mut seen_items);
        }

        for (e, container) in (&entities, &containers).join() {
            container.validate(e, &**ecs_world, &mut seen_items);
        }

        for (e, contained) in (&entities, contained.maybe()).join() {
            let held_by = seen_items.get(&e);
            assert_eq!(contained.is_some(), held_by.is_some(),
                       "{} is in invalid contained state (contained = {:?}, seen in inv or container = {:?})",
                       E(e),
                       contained,
                       held_by);
        }
    }
}
