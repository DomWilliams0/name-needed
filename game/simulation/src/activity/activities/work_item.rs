use common::*;

use crate::activity::activity::{
    ActivityEventContext, ActivityFinish, ActivityResult, SubActivity,
};

use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::event::EntityEvent;
use crate::unexpected_event;
use crate::ComponentWorld;

#[derive(Debug)]
pub struct WorkOnWorkItemActivity {}

impl<W: ComponentWorld> Activity<W> for WorkOnWorkItemActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<W>) -> ActivityResult {
        todo!()
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        _: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        match &event.payload {
            e => unexpected_event!(e),
        }
    }

    fn on_finish(&mut self, _: &ActivityFinish, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        todo!()
    }
}

impl WorkOnWorkItemActivity {
    pub fn new() -> Self {
        todo!()
    }
}

impl Display for WorkOnWorkItemActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        todo!()
    }
}
