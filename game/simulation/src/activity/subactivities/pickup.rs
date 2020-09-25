use common::*;
// use common::derive_more::Error;
use crate::activity::activity::{ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ecs::Entity;
use crate::event::prelude::*;
use crate::item::PickupItemComponent;
use crate::ComponentWorld;

/// Pick up the given item if close enough. Blocks on pick up event.
#[derive(Clone, Debug)]
pub struct PickupItemSubActivity(pub(crate) Entity);

impl<W: ComponentWorld> SubActivity<W> for PickupItemSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        // picking up is done in another system, kick that off and wait on the result
        ctx.world.add_lazy(ctx.entity, PickupItemComponent(self.0));

        // subscribe to item pick up
        ctx.subscribe_to(
            self.0,
            EventSubscription::Specific(EntityEventType::PickedUp),
        );

        ActivityResult::Blocked
    }

    fn on_finish(&self, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        let _ = ctx.world.remove_lazy::<PickupItemComponent>(ctx.entity);
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
