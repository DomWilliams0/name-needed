//! Hacky things for dev that can be called from main and avoid the need to expose a bunch of sim
//! API for testing

use crate::ai::{AiAction, AiComponent};
use crate::ecs::{EcsWorld, Entity, E};
use crate::item::Contents;
use crate::{ComponentWorld, InventoryComponent, TransformComponent};
use common::*;

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

    fn follow(&mut self, follower: Entity, followee: Entity) {
        let ai = self
            .world_mut()
            .component_mut::<AiComponent>(follower)
            .expect("no activity");

        ai.add_divine_command(AiAction::Follow {
            target: followee,
            radius: 3,
        });

        info!(
            "forcing {follower} to follow {followee}",
            follower = E(follower),
            followee = E(followee)
        );
    }

    fn world(&self) -> &EcsWorld;
    fn world_mut(&mut self) -> &mut EcsWorld;
}
