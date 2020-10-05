use common::*;

use crate::{ComponentWorld, PhysicalComponent, TransformComponent};
// use common::derive_more::Error;
use crate::activity::activity::{ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::*;
use crate::event::prelude::*;
use crate::item::{HaulableItemComponent, InventoryComponent};

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

impl<W: ComponentWorld> SubActivity<W> for PickupItemSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        // assumes we are close enough to pick it up already

        let item = self.0;
        let holder = ctx.entity;

        ctx.updates.queue("pick up item", move |world| {
            let do_pickup = || -> Result<(Entity, Entity), PickupItemError> {
                let mut transforms = world.write_storage::<TransformComponent>();
                let mut inventories = world.write_storage::<InventoryComponent>();
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
                        item,
                        haulable.extra_hands,
                        physical.volume,
                        physical.half_dimensions,
                    )
                    .ok_or(PickupItemError::NoFreeHands)?;

                // pickup success, remove transform from item
                transforms.remove(item);
                Ok((item, holder))
            };

            let result = do_pickup();
            world.post_event(EntityEvent {
                subject: item,
                payload: EntityEventPayload::PickedUp(result),
            });

            Ok(())
        });

        // subscribe to item pick up
        ctx.subscribe_to(
            self.0,
            EventSubscription::Specific(EntityEventType::PickedUp),
        );

        ActivityResult::Blocked
    }

    fn on_finish(&self, _: &mut ActivityContext<W>) -> BoxedResult<()> {
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
