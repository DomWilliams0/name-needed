//! Hacky things for dev that can be called from main and avoid the need to expose a bunch of sim
//! API for testing

use crate::ai::{AiAction, AiComponent};
use crate::ecs::{EcsWorld, Entity, E};

use crate::{
    ComponentWorld, Container, InventoryComponent, PhysicalComponent, TransformComponent,
};
use common::*;
use unit::length::Length3;
use unit::volume::Volume;
use unit::world::WorldPosition;

pub trait SimulationDevExt {
    fn give_bag(&mut self, lucky_holder: Entity) {
        let bag = Container::new(Volume::new(100), Length3::new(10, 10, 20));

        let inv = self
            .world()
            .component_mut::<InventoryComponent>(lucky_holder)
            .expect("no inventory");

        inv.give_container(bag);
    }

    fn put_food_in_container(&mut self, food: Entity, lucky_holder: Entity) {
        let inv = self
            .world()
            .component_mut::<InventoryComponent>(lucky_holder)
            .expect("no inventory");

        let bag = inv.containers_mut().next().expect("no container");

        let physical = self
            .world()
            .component::<PhysicalComponent>(food)
            .expect("bad food");

        bag.add_with(food, physical.volume, physical.half_dimensions)
            .expect("failed to add to bag");

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

    fn haul(&mut self, hauler: Entity, haulee: Entity, to: WorldPosition) {
        let ai = self
            .world_mut()
            .component_mut::<AiComponent>(hauler)
            .expect("no activity");

        ai.add_divine_command(AiAction::Haul(haulee, to));

        info!(
            "forcing {hauler} to haul {haulee}",
            hauler = E(hauler),
            haulee = E(haulee);
            "target" => %to,
        );
    }

    fn eat(&mut self, eater: Entity, food: Entity) {
        let ai = self
            .world_mut()
            .component_mut::<AiComponent>(eater)
            .expect("no activity");

        ai.add_divine_command(AiAction::EatHeldItem(food));

        info!(
            "forcing {eater} to eat {food}",
            eater = E(eater),
            food = E(food),
        );
    }

    fn world(&self) -> &EcsWorld;
    fn world_mut(&mut self) -> &mut EcsWorld;
}
