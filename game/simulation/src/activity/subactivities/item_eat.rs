use common::*;

use crate::activity::activity::{ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::{Entity, E};
use crate::event::prelude::*;
use crate::needs::BeingEatenComponent;
use crate::ComponentWorld;

/// Eat the item, which should be equipped in an equip slot
#[derive(Debug)]
pub struct ItemEatSubActivity(pub Entity);

impl<W: ComponentWorld> SubActivity<W> for ItemEatSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        // start eating and block until complete

        let item = self.0;
        let eater = ctx.entity;
        ctx.updates.queue("begin eating", move |world| {
            let insert_result = world.add_now(item, BeingEatenComponent { eater });
            debug_assert!(insert_result.is_ok());
            Ok(())
        });

        ctx.subscribe_to(item, EventSubscription::Specific(EntityEventType::Eaten));
        ActivityResult::Blocked
    }

    fn on_finish(&self, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
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
        write!(f, "Eating {}", E(self.0))
    }
}
