//! Hacky things for dev that can be called from main and avoid the need to expose a bunch of sim
//! API for testing

use crate::ecs::{EcsWorld, Entity};
use crate::item::Contents;
use crate::{ComponentWorld, InventoryComponent, TransformComponent};

pub trait SimulationDevExt {
    fn make_food_bag_and_give_to(&mut self, food: Entity, lucky_holder: Entity) {
        let mut bag = Contents::with_size(8);
        bag.put_item(food, 0, 1).expect("cant put in bag");
        // TODO always make sure that putting an item into a contents removes its transform? only do this via a system

        let inv = self
            .world()
            .component_mut::<InventoryComponent>(lucky_holder)
            .expect("no inventory");
        inv.give_mounted(bag).expect("cant give bag");

        self.world_mut().remove_now::<TransformComponent>(food);
    }

    fn world(&self) -> &EcsWorld;
    fn world_mut(&mut self) -> &mut EcsWorld;
}
