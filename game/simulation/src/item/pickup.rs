use crate::ecs::*;
use crate::item::BaseItemComponent;
use crate::queued_update::QueuedUpdates;
use crate::{InventoryComponent, TransformComponent};
use common::*;
use std::collections::HashSet;

/// Pick up the given item entity if in range
#[derive(Component, Debug)]
#[storage(HashMapStorage)]
pub struct PickupItemComponent(pub Entity);

pub struct PickupItemSystem;

impl<'a> System<'a> for PickupItemSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, QueuedUpdates>,
        ReadStorage<'a, PickupItemComponent>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, BaseItemComponent>,
    );

    fn run(&mut self, (entities, updates, pickup, transforms, base_items): Self::SystemData) {
        // TODO store this in the system and reuse the allocation
        let mut picked_up_items = HashSet::new();

        for (holder, pickup, transform) in (&entities, &pickup, &transforms).join() {
            let item = pickup.0;

            let check_item = || {
                // check we havent picked up the item this tick
                if picked_up_items.contains(&item) {
                    return Err("item was picked up already this tick");
                }

                // check item still has its transform and is a valid item
                let (item_transform, base_item) = (&transforms, &base_items)
                    .join()
                    .get(item, &entities)
                    .ok_or("invalid item entity")?;

                // check we are in touching range
                let holder_pos = Vector2::from(transform.position);
                let item_pos = item_transform.position.into();
                let touching_radius2 =
                    (transform.bounding_radius() + item_transform.bounding_radius()).powi(2);

                // return base item ref only if within touching distance
                let base_item = if holder_pos.distance2(item_pos) < touching_radius2 {
                    Some(base_item)
                } else {
                    None
                };

                Ok(base_item)
            };

            let item_size = match check_item() {
                Err(err) => {
                    debug!("aborting item pickup: {}", err);
                    updates.queue("abort item pickup", move |world| {
                        world.remove_now::<PickupItemComponent>(holder);
                        Ok(())
                    });
                    continue;
                }
                Ok(None) => {
                    // not close enough to pickup yet, better luck next tick pal
                    continue;
                }
                Ok(Some(item)) => item.base_slots,
            };

            // reserve this item from other entities this tick
            picked_up_items.insert(item);

            // queue item pickup at the end of the frame
            updates.queue("pick up item", move |world| {
                let inventory = world.component_mut::<InventoryComponent>(holder)?;

                // pick it up by
                // - putting it in the base inventory of the picker upper
                // - removing transform from item
                // - removing this component

                inventory.give_item(item, item_size as usize)?;
                let _ = world.remove_now::<TransformComponent>(item);
                let _ = world.remove_now::<PickupItemComponent>(holder);

                debug!("{:?} picked up item {:?}", holder, item);

                Ok(())
            });
        }
    }
}
