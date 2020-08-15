use crate::activity::activity::{ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::Entity;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventType, EventSubscription};
use crate::item::{ItemFilter, ItemReference, SlotReference, UsingItemComponent};
use crate::{BaseItemComponent, ComponentWorld, InventoryComponent};
use common::*;

/// Start using the given item, which must be equipped in the given base slot
pub struct ItemUseSubActivity(Entity, usize);

#[derive(Clone, Debug, Error)]
pub enum UseHeldItemError {
    #[error("Entity is not an item")]
    NotAnItem,

    #[error("Holder has no inventory")]
    NoInventory,

    #[error("Item was not found in the holder's inventory")]
    ItemNotFound,

    #[error("Item is not equipped")]
    NotEquipped,
}

impl<W: ComponentWorld> SubActivity<W> for ItemUseSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        let base_item = match ctx.world.component::<BaseItemComponent>(self.0) {
            Ok(base) => base,
            Err(_) => return ActivityResult::errored(UseHeldItemError::NotAnItem),
        };

        let condition = base_item.condition.value();
        let class = base_item.class;
        let holder = ctx.entity;
        let item = self.0;

        ctx.updates.queue("use held item", move |world| {
            let find_slot = || {
                let inventory = world
                    .component::<InventoryComponent>(holder)
                    .map_err(|_| UseHeldItemError::NoInventory)?;

                match inventory.search(&ItemFilter::SpecificEntity(item), world) {
                    Some(ItemReference(SlotReference::Base(slot), _)) => Ok(slot),
                    Some(_) => Err(UseHeldItemError::NotEquipped),
                    None => Err(UseHeldItemError::ItemNotFound),
                }
            };

            let result = find_slot().and_then(|base_slot| {
                if let Ok(Some(old)) = world.add_now(
                    holder,
                    UsingItemComponent {
                        left: condition,
                        base_slot,
                        class,
                    },
                ) {
                    warn!("overwriting previous item use: {:?}", old);
                };

                Ok(())
            });

            // only post event on error
            if result.is_err() {
                world.post_event(EntityEvent {
                    subject: item,
                    payload: EntityEventPayload::UsedUp(result.clone()),
                });
            }

            result?; // auto boxed
            Ok(())
        });

        // subscribe to finishing usage
        ctx.subscribe_to(self.0, EventSubscription::Specific(EntityEventType::UsedUp));

        ActivityResult::Blocked
    }

    fn on_finish(&self, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        // stop using item
        ctx.world.remove_lazy::<UsingItemComponent>(self.0);
        Ok(())
    }

    fn exertion(&self) -> f32 {
        // TODO per-item exertion
        0.5
    }
}

impl ItemUseSubActivity {
    pub fn new(item: Entity, slot: SlotReference) -> Self {
        let base_slot = match slot {
            SlotReference::Base(slot) => slot,
            _ => unreachable!("slot must be base slot"),
        };

        Self(item, base_slot)
    }
}

impl Display for ItemUseSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Using item")
    }
}
