use common::*;

use crate::activity::activity::{ActivityFinish, ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::{Entity, E};
use crate::event::prelude::*;
use crate::item::{
    ContainedInComponent, EndHaulBehaviour, FoundSlot, HauledItemComponent, InventoryComponent,
    ItemFilter,
};
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

impl SubActivity for ItemEquipSubActivity {
    fn init(&self, ctx: &mut ActivityContext) -> ActivityResult {
        let holder = ctx.entity;
        let item = self.item;
        let extra_hands = self.extra_hands;
        let filter = ItemFilter::SpecificEntity(item);

        let inventory = ctx
            .world
            .component::<InventoryComponent>(holder)
            .map_err(|_| EquipItemError::NoInventory);

        // check if already equipped
        let result = inventory.and_then(move |inv| match inv.search(&filter, ctx.world) {
            None => Err(EquipItemError::NotInInventory),
            Some(FoundSlot::Equipped(_)) => {
                // already equipped

                // if its currently being hauled, don't drop it
                if let Ok(hauling) = ctx.world.component_mut::<HauledItemComponent>(self.item) {
                    hauling.interrupt_behaviour = EndHaulBehaviour::KeepEquipped;
                    debug!("item is currently being hauled, ensuring it is kept in inventory");
                }

                Ok(ActivityResult::Finished(ActivityFinish::Success))
            }
            Some(slot) => {
                // in inventory but not equipped
                debug!("found item"; "slot" => ?slot, "item" => E(self.item));

                ctx.updates.queue("equip item", move |world| {
                    let mut do_equip = || -> Result<Entity, EquipItemError> {
                        let inventory = world
                            .component_mut::<InventoryComponent>(holder)
                            .map_err(|_| EquipItemError::NoInventory)?;

                        // TODO inventory operations should not be immediate

                        let slot = inventory
                            .search_mut(&filter, world)
                            .ok_or(EquipItemError::NotInInventory)?;

                        if slot.equip(extra_hands, world) {
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

                // block on equip event
                ctx.subscribe_to(
                    item,
                    EventSubscription::Specific(EntityEventType::BeenEquipped),
                );
                Ok(ActivityResult::Blocked)
            }
        });

        match result {
            Ok(result) => result,
            Err(e) => ActivityResult::errored(e),
        }
    }

    fn on_finish(&self, _: &ActivityFinish, _: &mut ActivityContext) -> BoxedResult<()> {
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
