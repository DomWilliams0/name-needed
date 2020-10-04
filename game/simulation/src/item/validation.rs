use crate::ecs::*;
use crate::item::Inventory2Component;
use std::collections::HashMap;

pub struct InventoryValidationSystem;

impl<'a> System<'a> for InventoryValidationSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldFrameRef>,
        ReadStorage<'a, Inventory2Component>,
    );

    fn run(&mut self, (entities, ecs_world, invs): Self::SystemData) {
        let mut seen_items = HashMap::new();
        for (e, inventory) in (&entities, &invs).join() {
            inventory.validate(e, &**ecs_world, &mut seen_items);
        }
    }
}
