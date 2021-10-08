use crate::ecs::*;
use crate::event::prelude::*;
use crate::item::{
    ContainedInComponent, EndHaulBehaviour, FoundSlot, HaulableItemComponent, HauledItemComponent,
    ItemFilter,
};
use crate::queued_update::QueuedUpdates;

use crate::activity::context::{ActivityContext, DistanceCheckResult};
use crate::{ComponentWorld, Entity, InventoryComponent, PhysicalComponent, TransformComponent};
use common::*;

/// Pick up item off the ground, checks if close enough first
pub struct PickupSubactivity;

/// Equip item that's already in inventory
pub struct EquipSubActivity;

const MAX_DISTANCE: f32 = 4.0;

#[derive(Error, Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum EquipItemError {
    #[error("Item is invalid, non-existent or not pickupable")]
    NotAvailable,

    #[error("Holder has no inventory")]
    NoInventory,

    #[error("Can't free up any/enough equip slots")]
    NoFreeHands,

    #[error("Item not found in inventory")]
    NotInInventory,

    #[error("Not enough space in inventory to equip item")]
    NotEnoughSpace,

    #[error("Item is too far away to pick up")]
    TooFar,

    #[error("Equip was cancelled")]
    Cancelled,
}

impl PickupSubactivity {
    /// Checks if close enough to pick up
    pub async fn pick_up(&self, ctx: &ActivityContext, item: Entity) -> Result<(), EquipItemError> {
        // ensure close enough
        match ctx.check_entity_distance(item, MAX_DISTANCE.powi(2)) {
            DistanceCheckResult::NotAvailable => return Err(EquipItemError::NotAvailable),
            DistanceCheckResult::TooFar => return Err(EquipItemError::TooFar),
            DistanceCheckResult::InRange => {}
        }

        // queue pickup for next tick
        queue_pickup(ctx.world().resource(), ctx.entity(), item);

        // await event
        ctx.subscribe_to_specific_until(item, EntityEventType::BeenPickedUp, |evt| {
            match evt {
                EntityEventPayload::BeenPickedUp(picker_upper, result)
                    if picker_upper == ctx.entity() =>
                {
                    // it was us, and we tried
                    Ok(result)
                }
                // calling activity can handle other destructive events
                _ => Err(evt),
            }
        })
        .await
        .unwrap_or(Err(EquipItemError::Cancelled))
    }
}

impl EquipSubActivity {
    pub async fn equip(
        &self,
        ctx: &ActivityContext,
        item: Entity,
        extra_hands: u16,
    ) -> Result<(), EquipItemError> {
        let holder = ctx.entity();
        let filter = ItemFilter::SpecificEntity(item);

        {
            let inventory = ctx
                .world()
                .component::<InventoryComponent>(holder)
                .map_err(|_| EquipItemError::NoInventory)?;

            // check if already equipped
            let inv_slot = inventory.search(&filter, ctx.world());
            match inv_slot {
                None => return Err(EquipItemError::NotInInventory),
                Some(FoundSlot::Equipped(_)) => {
                    // already equipped

                    // if its currently being hauled, don't drop it
                    if let Ok(mut hauling) = ctx.world().component_mut::<HauledItemComponent>(item)
                    {
                        hauling.interrupt_behaviour = EndHaulBehaviour::KeepEquipped;
                        debug!("item is currently being hauled, ensuring it is kept in inventory");
                    }

                    return Ok(());
                }
                Some(slot) => {
                    // in inventory but not equipped
                    debug!("found item"; "slot" => ?slot, "item" => item);

                    // continue outside of match, to allow dropping of the inventory component ref
                }
            };
        }

        // equip next tick
        queue_equip(ctx.world().resource(), holder, item, extra_hands, filter);

        // wait for equip event
        ctx.subscribe_to_specific_until(item, EntityEventType::BeenEquipped, |evt| {
            if let EntityEventPayload::BeenEquipped(result) = evt {
                Ok(result)
            } else {
                Err(evt)
            }
        })
        .await
        .unwrap_or(Err(EquipItemError::Cancelled))
        .map(|_| {})
    }
}

#[inline]
fn queue_equip(
    updates: &QueuedUpdates,
    holder: Entity,
    item: Entity,
    extra_hands: u16,
    filter: ItemFilter,
) {
    updates.queue("equip item", move |world| {
        let do_equip = || -> Result<Entity, EquipItemError> {
            let mut inventory = world
                .component_mut::<InventoryComponent>(holder)
                .map_err(|_| EquipItemError::NoInventory)?;

            // TODO inventory operations should not be immediate

            let slot = inventory
                .search_mut(&filter, &*world)
                .ok_or(EquipItemError::NotInInventory)?;

            if slot.equip(extra_hands, &*world) {
                world
                    .helpers_comps()
                    .add_to_container(item, ContainedInComponent::InventoryOf(holder));
                Ok(holder)
            } else {
                Err(EquipItemError::NotEnoughSpace)
            }
        };

        let result = do_equip();
        if result.is_ok() {
            world.post_event(EntityEvent {
                subject: holder,
                payload: EntityEventPayload::HasEquipped(item),
            });
        }

        world.post_event(EntityEvent {
            subject: item,
            payload: EntityEventPayload::BeenEquipped(result),
        });

        Ok(())
    });
}

#[inline]
fn queue_pickup(updates: &QueuedUpdates, holder: Entity, item: Entity) {
    updates.queue("pick up item", move |world| {
        let do_pickup = || -> Result<(), EquipItemError> {
            let mut shifted_items = SmallVec::<[(Entity, Entity); 3]>::new();
            {
                let mut inventories = world.write_storage::<InventoryComponent>();
                let transforms = world.read_storage::<TransformComponent>();
                let haulables = world.read_storage::<HaulableItemComponent>();
                let physicals = world.read_storage::<PhysicalComponent>();

                // get item components and ensure transform i.e. not already picked up
                let (_, haulable, physical) = world
                    .components(item, (&transforms, &haulables, &physicals))
                    .ok_or(EquipItemError::NotAvailable)?;

                // get holder inventory, free up enough hands and try to fill em
                let inventory = holder
                    .get_mut(&mut inventories)
                    .ok_or(EquipItemError::NoInventory)?;

                inventory
                    .insert_item(
                        &*world,
                        item,
                        haulable.extra_hands,
                        physical.volume,
                        physical.size,
                        |item, container| {
                            // cant mutate world in this callback
                            shifted_items.push((item, container));
                        },
                    )
                    .ok_or(EquipItemError::NoFreeHands)?;
            }

            // update items that have been shifted into containers
            for (item, container) in shifted_items {
                world
                    .helpers_comps()
                    .add_to_container(item, ContainedInComponent::Container(container));
            }

            // pickup success
            world
                .helpers_comps()
                .add_to_container(item, ContainedInComponent::InventoryOf(holder));

            Ok(())
        };

        let result = do_pickup();
        if result.is_ok() {
            world.post_event(EntityEvent {
                subject: holder,
                payload: EntityEventPayload::HasPickedUp(item),
            });
        }

        world.post_event(EntityEvent {
            subject: item,
            payload: EntityEventPayload::BeenPickedUp(holder, result),
        });

        Ok(())
    });
}
