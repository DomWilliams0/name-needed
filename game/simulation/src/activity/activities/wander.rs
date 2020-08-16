use crate::activity::activity::{ActivityResult, Finish, SubActivity};
use crate::activity::{Activity, ActivityContext};
use crate::nop_subactivity;
use crate::path::WanderComponent;
use crate::ComponentWorld;
use common::*;

#[derive(Debug)]
pub struct WanderActivity;

impl<W: ComponentWorld> Activity<W> for WanderActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        // add wander marker component
        ctx.world.add_lazy(ctx.entity, WanderComponent);
        ActivityResult::Blocked
    }

    fn on_finish(&mut self, _: Finish, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        ctx.world.remove_lazy::<WanderComponent>(ctx.entity);
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        nop_subactivity!("Wandering", 0.2)
    }
}

impl Display for WanderActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Wandering aimlessly")
    }
}
