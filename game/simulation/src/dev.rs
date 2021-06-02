//! Hacky things for dev that can be called from main and avoid the need to expose a bunch of sim
//! API for testing

use common::*;
use unit::world::WorldPosition;

use crate::activity::HaulTarget;
use crate::ai::{AiAction, AiComponent};
use crate::ecs::{EcsWorld, Entity};
use crate::item::{ContainedInComponent, ContainerComponent};
use crate::job::SocietyJob;
use crate::queued_update::QueuedUpdates;
use crate::simulation::AssociatedBlockData;
use crate::society::job::HaulJob;
use crate::{
    ComponentWorld, InventoryComponent, PhysicalComponent, Societies, SocietyHandle,
    TransformComponent,
};
use std::pin::Pin;

#[derive(common::derive_more::Deref, common::derive_more::DerefMut)]
pub struct EcsExtDev<'w>(&'w EcsWorld);

impl EcsWorld {
    pub fn helpers_dev(&self) -> EcsExtDev {
        EcsExtDev(self)
    }
}

impl EcsExtDev<'_> {
    pub fn give_bag(&self, lucky_holder: Entity) {
        let bag = self
            .build_entity("core_storage_backpack")
            .expect("no backpack")
            .spawn()
            .expect("cant make backpack");

        let inv = self
            .component_mut::<InventoryComponent>(lucky_holder)
            .expect("no inventory");

        info!("giving bag {} to {}", bag, lucky_holder);

        inv.give_container(bag);
        self.helpers_comps()
            .add_to_container(bag, ContainedInComponent::InventoryOf(lucky_holder));
    }

    pub fn put_food_in_container(&mut self, food: Entity, lucky_holder: Entity) {
        let inv = self
            .component_mut::<InventoryComponent>(lucky_holder)
            .expect("no inventory");

        let (bag, container) = inv.containers_mut(self.0).next().expect("no container");

        let physical = self.component::<PhysicalComponent>(food).expect("bad food");

        info!(
            "putting {} into container {} in inventory of {}",
            food, bag, lucky_holder
        );

        container
            .add_with(food, physical.volume, physical.size)
            .expect("failed to add to bag");

        self.helpers_comps()
            .add_to_container(food, ContainedInComponent::Container(bag));
    }

    pub fn follow(&mut self, follower: Entity, followee: Entity) {
        let ai = self
            .component_mut::<AiComponent>(follower)
            .expect("no activity");

        ai.add_divine_command(AiAction::Follow {
            target: followee,
            radius: 3,
        });

        info!(
            "forcing {follower} to follow {followee}",
            follower = follower,
            followee = followee
        );
    }

    pub fn make_container_communal(
        &mut self,
        container_pos: WorldPosition,
        society: Option<SocietyHandle>,
    ) {
        self.resource::<QueuedUpdates>()
            .queue("make container communal", move |world| {
                let w = world.voxel_world();
                let w = w.borrow();
                if let Some(AssociatedBlockData::Container(e)) =
                    w.associated_block_data(container_pos)
                {
                    info!(
                        "forcing container to be communal";
                        "container" => e,
                        "society" => ?society,
                    );

                    world
                        .helpers_containers()
                        .set_container_communal(*e, society)
                        .expect("failed to set communal");
                } else {
                    panic!("no container");
                }

                Ok(())
            });
    }

    pub fn haul_from_container(
        &mut self,
        hauler: Entity,
        haulee: Entity,
        container_pos: WorldPosition,
        haul_to: WorldPosition,
    ) {
        self.resource::<QueuedUpdates>()
            .queue("force haul from container", move |world| {
                let w = world.voxel_world();
                let w = w.borrow();
                if let Some(AssociatedBlockData::Container(container)) =
                    w.associated_block_data(container_pos)
                {
                    let ai = world
                        .component_mut::<AiComponent>(hauler)
                        .expect("no activity");

                    let from = HaulTarget::Container(*container);
                    let to = HaulTarget::Position(haul_to);

                    info!(
                        "forcing {hauler} to haul {haulee}",
                        hauler = hauler,
                        haulee = haulee;
                        "source" => %from,
                        "target" => %to,
                    );

                    ai.add_divine_command(AiAction::Haul(haulee, from, to));

                    // teehee add the haulee to the container too
                    let phys = world
                        .component::<PhysicalComponent>(haulee)
                        .expect("no physical");

                    world
                        .component_mut::<ContainerComponent>(*container)
                        .unwrap()
                        .container
                        .add_with(haulee, phys.volume, phys.size)
                        .expect("failed to add");

                    world
                        .helpers_comps()
                        .add_to_container(haulee, ContainedInComponent::Container(*container));
                } else {
                    panic!("no container");
                }

                Ok(())
            });
    }

    pub fn do_with_placed_container(
        &mut self,
        wat: &'static str,
        container_pos: WorldPosition,
        mut f: impl FnMut(Pin<&mut EcsWorld>, Entity) + 'static,
    ) {
        self.resource::<QueuedUpdates>().queue(wat, move |world| {
            let w = world.voxel_world();
            let w = w.borrow();
            if let Some(AssociatedBlockData::Container(container)) =
                w.associated_block_data(container_pos)
            {
                f(world, *container);
            } else {
                panic!("no container");
            }

            Ok(())
        });
    }

    pub fn haul_to_container_via_divine(
        &mut self,
        hauler: Entity,
        haulee: Entity,
        container_pos: WorldPosition,
    ) {
        self.do_with_placed_container(
            "force haul to container",
            container_pos,
            move |world, container| {
                let food_pos = world
                    .component::<TransformComponent>(haulee)
                    .unwrap()
                    .accessible_position();

                let ai = world
                    .component_mut::<AiComponent>(hauler)
                    .expect("no activity");

                let from = HaulTarget::Position(food_pos);
                let to = HaulTarget::Container(container);

                info!(
                    "forcing {hauler} to haul {haulee}",
                    hauler = hauler,
                    haulee = haulee;
                    "source" => %from,
                    "target" => %to,
                );

                ai.add_divine_command(AiAction::Haul(haulee, from, to));
            },
        );
    }

    pub fn haul_to_container_via_society(
        &mut self,
        society: SocietyHandle,
        haulee: Entity,
        container_pos: WorldPosition,
    ) {
        self.do_with_placed_container(
            "queue society haul to container job",
            container_pos,
            move |world, container| {
                let job = HaulJob::with_target_container(haulee, container, &*world)
                    .expect("cant create job");

                world
                    .resource_mut::<Societies>()
                    .society_by_handle_mut(society)
                    .expect("bad society")
                    .jobs_mut()
                    .submit(SocietyJob::create(&*world, job));

                info!(
                    "adding society job to haul item to container";
                    "society" => ?society,
                    "haulee" => haulee,
                    "container" => %container_pos,
                );
            },
        );
    }

    pub fn eat(&mut self, eater: Entity, food: Entity) {
        self.force_activity(eater, AiAction::EatHeldItem(food));
    }

    pub fn force_activity(&mut self, slave: Entity, action: AiAction) {
        let ai = self
            .component_mut::<AiComponent>(slave)
            .expect("no activity");

        info!(
            "forcing {entity} to follow divine command",
            entity = slave;
            "action" => ?action,
        );

        ai.add_divine_command(action);
    }
}
