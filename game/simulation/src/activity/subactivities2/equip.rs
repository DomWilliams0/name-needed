use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::EventResult::Consumed;
use crate::ecs::*;
use crate::event::prelude::*;
use crate::item::{ContainedInComponent, HaulableItemComponent};
use crate::queued_update::QueuedUpdates;
use crate::unexpected_event2;
use crate::{ComponentWorld, Entity, InventoryComponent, PhysicalComponent, TransformComponent};
use common::*;

/// Pick up item off the ground, checks if close enough first
pub struct PickupSubactivity;

const MAX_DISTANCE: f32 = 4.0;

#[derive(Error, Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum PickupItemError {
    #[error("Item is invalid, non-existent or already picked up")]
    NotAvailable,

    #[error("Holder has no inventory")]
    NoInventory,

    #[error("Can't free up any/enough equip slots")]
    NoFreeHands,

    #[error("Too far away to pick up")]
    TooFar,

    #[error("Pickup was cancelled")]
    Cancelled,
}

impl PickupSubactivity {
    /// Checks if close enough to pick up
    pub async fn pick_up(
        &mut self,
        ctx: &ActivityContext2<'_>,
        item: Entity,
    ) -> Result<(), PickupItemError> {
        // ensure close enough
        self.check_distance(ctx, item)?;

        // queue pickup for next tick
        self.queue_pickup(ctx, item);

        // await event
        let subscription = EntityEventSubscription {
            subject: item,
            subscription: EventSubscription::Specific(EntityEventType::BeenPickedUp),
        };
        let mut pickup_result = None;
        ctx.subscribe_to_until(subscription, |evt| match evt {
            EntityEventPayload::BeenPickedUp(picker_upper, result)
                if picker_upper == ctx.entity() =>
            {
                // it was us, and we tried
                pickup_result = Some(result);
                Consumed
            }
            // calling activity can handle other destructive events
            _ => unexpected_event2!(evt),
        })
        .await;

        pickup_result.unwrap_or(Err(PickupItemError::Cancelled))
    }

    fn check_distance(&self, ctx: &ActivityContext2, item: Entity) -> Result<(), PickupItemError> {
        let transforms = ctx.world().read_storage::<TransformComponent>();
        let my_pos = transforms.get(ctx.entity().into());
        let item_pos = transforms.get(item.into());

        my_pos
            .zip(item_pos)
            .ok_or(PickupItemError::NotAvailable)
            .and_then(|(me, item)| {
                if me.position.distance2(item.position) < MAX_DISTANCE.powi(2) {
                    Ok(())
                } else {
                    Err(PickupItemError::TooFar)
                }
            })
    }

    fn queue_pickup(&self, ctx: &ActivityContext2, item: Entity) {
        let holder = ctx.entity();
        ctx.world()
            .resource::<QueuedUpdates>()
            .queue("pick up item", move |world| {
                let do_pickup = || -> Result<(), PickupItemError> {
                    let mut shifted_items = SmallVec::<[(Entity, Entity); 3]>::new();
                    {
                        let mut inventories = world.write_storage::<InventoryComponent>();
                        let transforms = world.read_storage::<TransformComponent>();
                        let haulables = world.read_storage::<HaulableItemComponent>();
                        let physicals = world.read_storage::<PhysicalComponent>();

                        // get item components and ensure transform i.e. not already picked up
                        let (_, haulable, physical) = world
                            .components(item, (&transforms, &haulables, &physicals))
                            .ok_or(PickupItemError::NotAvailable)?;

                        // get holder inventory, free up enough hands and try to fill em
                        let inventory = holder
                            .get_mut(&mut inventories)
                            .ok_or(PickupItemError::NoInventory)?;

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
                            .ok_or(PickupItemError::NoFreeHands)?;
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
}
