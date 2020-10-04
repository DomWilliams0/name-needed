use common::*;

use crate::activity::activity::{ActivityResult, Finish, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::{Entity, E};
use crate::event::prelude::*;
use crate::item::{FoundSlot, Inventory2Component, ItemFilter};
use crate::ComponentWorld;

/// Equip the given item, given it's already somewhere in the holder's inventory
#[derive(Debug)]
pub struct ItemEquipSubActivity {
    pub item: Entity,
    pub extra_hands: u16,
}

#[derive(Clone, Debug, Error)]
pub enum EquipItemError {
    #[error("Entity is not an item")]
    NotAnItem,

    #[error("Holder does not have an inventory")]
    NoInventory,

    #[error("Item not found in inventory")]
    NotInInventory,

    #[error("Not enough space in inventory to equip item")]
    NotEnoughSpace,
}

impl<W: ComponentWorld> SubActivity<W> for ItemEquipSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        let holder = ctx.entity;
        let food = self.item;
        let extra_hands = self.extra_hands;
        let filter = ItemFilter::SpecificEntity(food);

        let inventory = ctx
            .world
            .component::<Inventory2Component>(holder)
            .map_err(|_| EquipItemError::NoInventory);

        // check if already equipped
        let result = inventory.and_then(move |inv| match inv.search(&filter, ctx.world) {
            None => Err(EquipItemError::NotInInventory),
            Some(FoundSlot::Equipped(_)) => {
                // already equipped
                Ok(ActivityResult::Finished(Finish::Success))
            }
            Some(slot) => {
                // in inventory but not equipped
                debug!("found food"; "slot" => ?slot);

                ctx.updates.queue("equip food", move |world| {
                    let do_equip = || -> Result<Entity, EquipItemError> {
                        let inventory = world
                            .component_mut::<Inventory2Component>(holder)
                            .map_err(|_| EquipItemError::NoInventory)?;

                        // TODO inventory operations should not be immediate

                        let slot = inventory
                            .search_mut(&filter, world)
                            .ok_or(EquipItemError::NotInInventory)?;

                        if slot.equip(extra_hands) {
                            Ok(holder)
                        } else {
                            Err(EquipItemError::NotEnoughSpace)
                        }
                    };

                    let result = do_equip();
                    world.post_event(EntityEvent {
                        subject: food,
                        payload: EntityEventPayload::Equipped(result),
                    });

                    Ok(())
                });

                // block on equip event
                ctx.subscribe_to(food, EventSubscription::Specific(EntityEventType::Equipped));
                Ok(ActivityResult::Blocked)
            }
        });

        match result {
            Ok(result) => result,
            Err(e) => ActivityResult::errored(e),
        }
    }

    fn on_finish(&self, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn exertion(&self) -> f32 {
        0.1
    }
}

impl Display for ItemEquipSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Equipping {}", E(self.item))
    }
}
