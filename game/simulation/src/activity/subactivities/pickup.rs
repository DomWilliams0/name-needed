use common::*;

use crate::{ComponentWorld, PhysicalComponent, TransformComponent};
// use common::derive_more::Error;
use crate::activity::activity::{ActivityFinish, ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::*;
use crate::event::prelude::*;
use crate::item::{ContainedInComponent, HaulableItemComponent, InventoryComponent};

/// Pick up the given item. Blocks on pick up event.
#[derive(Clone, Debug)]
pub struct PickupItemSubActivity(pub(crate) Entity);

#[derive(Error, Debug, Clone)]
pub enum PickupItemError {
    #[error("Item is invalid, non-existent or already picked up")]
    NotAvailable,

    #[error("Holder has no inventory")]
    NoInventory,

    #[error("Can't free up any/enough equip slots")]
    NoFreeHands,
}

impl SubActivity for PickupItemSubActivity {
    fn init(&self, ctx: &mut ActivityContext) -> ActivityResult {
        // assumes we are close enough to pick it up already

        let item = self.0;
        let holder = ctx.entity;

        ctx.updates.queue("pick up item", move |world| {
            let mut do_pickup = || -> Result<Entity, PickupItemError> {
                let mut shifted_items = Vec::new(); // TODO smallvec
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
                    let inventory = inventories
                        .get_mut(holder)
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
                Ok(holder)
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
                payload: EntityEventPayload::BeenPickedUp(result),
            });

            Ok(())
        });

        // subscribe to item pick up
        ctx.subscribe_to(
            self.0,
            EventSubscription::Specific(EntityEventType::BeenPickedUp),
        );

        ActivityResult::Blocked
    }

    fn on_finish(&self, _: &ActivityFinish, _: &mut ActivityContext) -> BoxedResult<()> {
        Ok(())
    }

    fn exertion(&self) -> f32 {
        // TODO exertion of picking up item depends on item weight
        0.2
    }
}

impl Display for PickupItemSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Picking up item")
    }
}
