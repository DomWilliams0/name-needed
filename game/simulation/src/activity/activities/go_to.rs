use common::*;
use unit::world::WorldPoint;
use world::NavigationError;

use crate::activity::activity::{
    ActivityEventContext, ActivityFinish, ActivityResult, SubActivity,
};
use crate::activity::subactivities::GoToSubActivity;
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::event::{EntityEvent, EntityEventPayload};
use crate::unexpected_event;
use crate::ComponentWorld;

/// Simple wrapper around goto subactivity with a given reason
#[derive(Debug)]
pub struct GoToActivity {
    goto: GoToSubActivity,
    reason: &'static str,
    result: Option<Result<(), NavigationError>>,
}

impl<W: ComponentWorld> Activity<W> for GoToActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<W>) -> ActivityResult {
        match self.result.take() {
            Some(Ok(_)) => ActivityResult::Finished(ActivityFinish::Success),
            Some(Err(e)) => ActivityResult::Finished(ActivityFinish::Failure(Box::new(e))),
            None => self.goto.init(ctx),
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        _: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        // arrival
        match &event.payload {
            EntityEventPayload::Arrived(token, result) if *token == self.goto.token() => {
                self.result = Some(result.to_owned().map(|_| ()));
                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
                )
            }
            e => unexpected_event!(e),
        }
    }

    fn on_finish(&mut self, _: &ActivityFinish, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        &self.goto
    }
}

impl GoToActivity {
    pub fn new(target: WorldPoint, reason: &'static str) -> Self {
        Self {
            goto: GoToSubActivity::new(target, NormalizedFloat::new(0.8)),
            reason,
            result: None,
        }
    }
}

impl Display for GoToActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // TODO reason specification should be type level and used everywhere. ties into localization
        write!(f, "Going to target because {:?}", self.reason)
    }
}
