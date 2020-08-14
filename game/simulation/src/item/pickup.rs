use crate::ecs::*;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventQueue};
use crate::item::{BaseItemComponent, InventoryError};
use crate::queued_update::QueuedUpdates;
use crate::{InventoryComponent, TransformComponent};
use common::*;
use std::collections::HashSet;

#[derive(Error, Debug, Clone)]
pub enum PickupItemError {
    #[error("Entity is not an item")]
    NotAnItem,

    #[error("Item has already been picked up")]
    AlreadyPickedUp,

    #[error("Picker-upper has no inventory")]
    NoInventory,

    #[error("Picker-upper couldn't pick up item")]
    InventoryError(#[from] InventoryError),

    #[error("Picker-upper is too far away from item (distance: {})", _0)]
    TooFar(f32),
}

/// Pick up the given item entity if in range
#[derive(Component, Debug)]
#[storage(HashMapStorage)]
pub struct PickupItemComponent(pub Entity);

pub struct PickupItemSystem;

impl<'a> System<'a> for PickupItemSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, QueuedUpdates>,
        Write<'a, EntityEventQueue>,
        ReadStorage<'a, PickupItemComponent>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, BaseItemComponent>,
        ReadStorage<'a, InventoryComponent>,
    );

    fn run(
        &mut self,
        (entities, updates, mut events, pickup, transforms, base_items, inventories): Self::SystemData,
    ) {
        // TODO store this in the system and reuse the allocation
        let mut picked_up_items = HashSet::new();

        for (holder, pickup, picker_upper_transform) in (&entities, &pickup, &transforms).join() {
            let item = pickup.0;

            let mut do_pickup = || {
                // entity should be an item
                let base_item = match base_items.get(item) {
                    Some(comp) => comp,
                    None => return Err(PickupItemError::NotAnItem),
                };

                // entity should have a transform i.e. not be picked up
                let item_transform = match transforms.get(item) {
                    Some(comp) => comp,
                    None => return Err(PickupItemError::AlreadyPickedUp),
                };

                // picker upper should have an inventory
                if inventories.get(holder).is_none() {
                    return Err(PickupItemError::NoInventory);
                }

                // we should be close enough to touch it
                if !picker_upper_transform.position.is_almost(
                    &item_transform.position,
                    item_transform.bounding_radius() + picker_upper_transform.bounding_radius(),
                ) {
                    let distance = picker_upper_transform
                        .position
                        .distance2(item_transform.position)
                        .sqrt();
                    return Err(PickupItemError::TooFar(distance));
                }

                // time to actually pick it up - reserve this item from other entities this tick
                picked_up_items.insert(item);

                // queue item pickup at the end of the frame. can't do it now because we're iterating
                // transforms
                let item_size = base_item.base_slots;
                updates.queue("pick up item", move |world| {
                    // pick it up by
                    // - putting it in the base inventory of the picker upper
                    // - removing transform from item
                    // - removing this component

                    let result = world
                        .component_mut::<InventoryComponent>(holder)
                        .map_err(|_| PickupItemError::NoInventory)
                        .and_then(|inv| {
                            inv.give_item(item, item_size as usize)
                                .map_err(PickupItemError::InventoryError)
                        });

                    let event_payload = if result.is_ok() {
                        let _ = world.remove_now::<TransformComponent>(item);

                        debug!("{:?} picked up item {:?}", holder, item);
                        EntityEventPayload::PickedUp(Ok(item))
                    } else {
                        EntityEventPayload::PickedUp(Err(PickupItemError::NoInventory))
                    };

                    // remove this component unconditionally
                    let _ = world.remove_now::<PickupItemComponent>(holder);

                    // post event
                    world.resource_mut::<EntityEventQueue>().post(EntityEvent {
                        subject: item,
                        payload: event_payload,
                    });

                    Ok(())
                });

                // wew we made it
                Ok(())
            };

            // only post event on failure - successful event will be posted in the queued update
            if let Err(err) = do_pickup() {
                debug!("item pickup failed: {}", err);

                events.post(EntityEvent {
                    subject: item,
                    payload: EntityEventPayload::PickedUp(Err(err)),
                });
            }
        }
    }
}
