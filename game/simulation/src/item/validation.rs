use crate::ecs::*;
use crate::InventoryComponent;

pub struct InventoryValidationSystem<'a>(pub &'a EcsWorld);

impl<'a> System<'a> for InventoryValidationSystem<'a> {
    type SystemData = (ReadStorage<'a, InventoryComponent>,);

    fn run(&mut self, (invs,): Self::SystemData) {
        for (inventory,) in (&invs,).join() {
            inventory.validate(self.0);
        }
    }
}
