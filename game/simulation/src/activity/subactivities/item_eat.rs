use common::*;

use crate::activity::activity::{ActivityFinish, ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::Entity;
use crate::ComponentWorld;

use crate::event::prelude::*;
use crate::item::ContainedInComponent;
use crate::needs::BeingEatenComponent;

/// Eat the item, which should be equipped in an equip slot
#[derive(Debug)]
pub struct ItemEatSubActivity(pub Entity);

#[derive(Clone, Debug, Error)]
pub enum ItemEatError {
    #[error("Food is not in the eater's inventory")]
    NotInInventory,
}

impl SubActivity for ItemEatSubActivity {
    fn init(&self, ctx: &mut ActivityContext) -> ActivityResult {
        // start eating and block until complete

        let item = self.0;
        let eater = ctx.entity;
        ctx.updates.queue("begin eating", move |world| {
            match world.component::<ContainedInComponent>(item) {
                Ok(ContainedInComponent::InventoryOf(holder)) if *holder == eater => {}
                other => {
                    debug!("cannot eat because food is not held"; "error" => ?other);
                    world.post_event(EntityEvent {
                        subject: item,
                        payload: EntityEventPayload::BeenEaten(Err(())),
                    });
                    return Err(ItemEatError::NotInInventory.into());
                }
            }

            let insert_result = world.add_now(item, BeingEatenComponent { eater });
            debug_assert!(insert_result.is_ok());
            Ok(())
        });

        ctx.subscribe_to(
            item,
            EventSubscription::Specific(EntityEventType::BeenEaten),
        );
        ActivityResult::Blocked
    }

    fn on_finish(&self, _: &ActivityFinish, ctx: &mut ActivityContext) -> BoxedResult<()> {
        let item = self.0;
        ctx.updates.queue("end eating", move |world| {
            let _ = world.remove_now::<BeingEatenComponent>(item);
            Ok(())
        });
        Ok(())
    }

    fn exertion(&self) -> f32 {
        // TODO varying exertion per food
        0.5
    }
}

impl Display for ItemEatSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Eating {}", self.0)
    }
}
