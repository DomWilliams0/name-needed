use crate::activity::activity::{ActivityResult, Finish, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::Entity;
use crate::event::prelude::*;
use crate::item::{BaseSlotPolicy, InventoryError, SlotReference};
use crate::{BaseItemComponent, ComponentWorld, InventoryComponent};
use common::*;

#[derive(Debug)]
pub struct ItemEquipSubActivity {
    slot: SlotReference,
    item: Entity,
}

#[derive(Clone, Debug, Error)]
pub enum EquipItemError {
    #[error("Entity is not an item")]
    NotAnItem,

    #[error("Holder does not have an inventory")]
    NoInventory,

    #[error("Failed to equip: {}", _0)]
    InventoryError(#[from] InventoryError),
}

impl<W: ComponentWorld> SubActivity<W> for ItemEquipSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        // TODO add ItemUseType which hints at which slot to use
        let policy = BaseSlotPolicy::AlwaysDominant;

        if !matches!(policy, BaseSlotPolicy::AlwaysDominant)
            && matches!(self.slot, SlotReference::Base(_))
        {
            // nothing to do, already equipped
            return ActivityResult::Finished(Finish::Success);
        }

        let base_item = match ctx.world.component::<BaseItemComponent>(self.item) {
            Ok(base) => base,
            Err(_) => return ActivityResult::errored(EquipItemError::NotAnItem),
        };

        // TODO equipping will depend on the item's size in base+mounted inventories, not yet implemented
        assert_eq!(base_item.base_slots, 1);
        assert_eq!(base_item.mounted_slots, 1);

        let holder = ctx.entity;
        let item = self.item;
        let slot = self.slot;

        // queue equip
        ctx.updates.queue("equip item", move |world| {
            // TODO inventory operations should not be immediate
            let result = world
                .component_mut::<InventoryComponent>(holder)
                .map_err(|_| EquipItemError::NoInventory)
                .and_then(|inventory| {
                    inventory
                        .equip(slot, policy)
                        .map_err(EquipItemError::InventoryError)
                })
                .map(SlotReference::Base);

            world.post_event(EntityEvent {
                subject: item,
                payload: EntityEventPayload::Equipped(result.clone()),
            });

            result?; // auto boxed
            Ok(())
        });

        // subscribe to finishing equipping item
        ctx.subscribe_to(item, EventSubscription::Specific(EntityEventType::Equipped));

        ActivityResult::Blocked
    }

    fn on_finish(&self, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn exertion(&self) -> f32 {
        0.2
    }
}

impl ItemEquipSubActivity {
    pub fn new(slot: SlotReference, item: Entity) -> Self {
        Self { slot, item }
    }

    pub fn slot(&self) -> SlotReference {
        self.slot
    }
}

impl Display for ItemEquipSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Equipping item from inventory")
    }
}
