use crate::activity::activity::{ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::{Entity, WorldExt, E};
use crate::event::{EntityEvent, EntityEventPayload};
use crate::item::{HaulType, HaulableItemComponent, HauledItemComponent};
use crate::{ComponentWorld, InventoryComponent, PhysicalComponent};
use common::*;

/// Handles holding of an item in the hauler's hands. No moving
#[derive(Debug)]
pub struct HaulSubActivity {
    thing: Entity,
}

#[derive(Debug, Error, Clone)]
pub enum HaulError {
    #[error("Item destroyed/moved by a destructive event")]
    Interrupted,

    #[error("Hauler has no inventory")]
    NoInventory,

    #[error("Hauler doesn't have enough free hands")]
    NotEnoughFreeHands,

    #[error("Item is not alive, haulable or physical")]
    BadItem,
}

impl HaulSubActivity {
    pub fn new(entity: Entity) -> Self {
        HaulSubActivity { thing: entity }
    }
}

impl<W: ComponentWorld> SubActivity<W> for HaulSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        let hauler = ctx.entity;
        let item = self.thing;

        // validate inventory space now
        // TODO move this check to the DSE, doing it here only saves 1 tick
        // if let Err(e) = ctx
        //     .world
        //     .component::<InventoryComponent>(hauler)
        //     .map_err(|_| HaulError::NoInventory)
        //     .and_then(|inv| {
        //         inv.has_hauling_slots(extra_hands)
        //             .ok_or(HaulError::NotEnoughFreeHands)
        //     })
        // {
        //     debug!("not enough hands");
        //     return ActivityResult::errored(e);
        // }

        ctx.updates.queue("haul item", move |world| {
            let mut do_haul = || -> Result<(Entity, Entity), HaulError> {
                // check item is alive first, to ensure .insert() succeeds below
                if !world.is_entity_alive(item) {
                    return Err(HaulError::BadItem);
                }

                // get item properties
                let (extra_hands, volume, size) = {
                    let haulables = world.read_storage::<HaulableItemComponent>();
                    let physicals = world.read_storage::<PhysicalComponent>();
                    match world.components(item, (&haulables, &physicals)) {
                        Some((haulable, physical)) => (
                            haulable.extra_hands,
                            physical.volume,
                            physical.half_dimensions,
                        ),
                        None => {
                            warn!("item is not haulable"; "item" => E(item));
                            return Err(HaulError::BadItem);
                        }
                    }
                };

                debug!(
                    "{entity} wants to haul {item} which needs {extra_hands} extra hands",
                    entity = E(hauler),
                    item = E(item),
                    extra_hands = extra_hands
                );

                // get hauler inventory
                let inventory = world
                    .component_mut::<InventoryComponent>(hauler)
                    .map_err(|_| HaulError::NoInventory)?;

                // ensure hauler has enough free hands
                let mut slots = inventory
                    .get_hauling_slots(extra_hands)
                    .ok_or(HaulError::NotEnoughFreeHands)?;

                // everything has been checked, no more errors past this point

                // fill equip slots
                slots.fill(item, volume, size);

                // add haul component to item
                world
                    .add_now(item, HauledItemComponent::new(hauler, HaulType::CarryAlone))
                    .expect("item was asserted to be alive");

                // TODO apply slowness effect to holder
                Ok((item, hauler))
            };

            let result = do_haul();
            world.post_event(EntityEvent {
                subject: item,
                payload: EntityEventPayload::Hauled(result),
            });

            Ok(())
        });

        // nothing else to do here, goto sub activity will handle the actual movement
        ActivityResult::Blocked
    }

    fn on_finish(&self, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        let hauler = ctx.entity;
        let item = self.thing;
        ctx.updates.queue("stop hauling item", move |world| {
            // remove haul component from item
            let _ = world.remove_now::<HauledItemComponent>(item);

            // free holder's hands
            let inventory = world
                .component_mut::<InventoryComponent>(hauler)
                .map_err(|_| HaulError::NoInventory)?;

            let count = inventory.remove_item(item);
            debug!(
                "{hauler} stopped hauling {item}, removed from {slots} slots",
                hauler = E(hauler),
                item = E(item),
                slots = count
            );

            // TODO remove slowness effect if any

            Ok(())
        });

        Ok(())
    }

    fn exertion(&self) -> f32 {
        // TODO depends on the weight of the item(s)
        1.5
    }
}

impl Display for HaulSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Hauling {}", E(self.thing))
    }
}
