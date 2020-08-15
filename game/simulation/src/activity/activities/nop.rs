use crate::activity::activity::{ActivityResult, Finish, SubActivity};
use crate::activity::{Activity, ActivityContext};
use crate::nop_subactivity;
use crate::ComponentWorld;
use common::*;

pub struct NopActivity;

impl<W: ComponentWorld> Activity<W> for NopActivity {
    fn on_tick<'a>(&mut self, _: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        warn!("ticking nop activity, possible infinite loop");
        ActivityResult::Ongoing
    }

    fn on_finish(&mut self, _: Finish, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        nop_subactivity!()
    }
}

impl Display for NopActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Doing nothing")
    }
}
