use crate::activity::activity::{ActivityResult, Finish, SubActivity};
use crate::activity::subactivities::ThinkingSubActivity;
use crate::activity::{Activity, ActivityContext};
use crate::ComponentWorld;
use common::{derive_more::Display, warn, BoxedResult};

#[derive(Display)]
#[display("Doing nothing")]
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
        &ThinkingSubActivity
    }
}

/*#[derive(Copy, Clone, Debug)]
enum GotoThenNopState {
    GoingTo(WorldPoint),
    Done,
}

pub struct GotoThenNop {
    state: GotoThenNopState,
}

impl<W: ComponentWorld> Activity<W> for GotoThenNop {
    fn on_finish(&mut self, _finish: Finish, _ctx: &mut ActivityContext<W>) {
        // TODO remove path here? or is it up to the new activity to cancel path finding if it wants
    }

    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult<'a> {
        match self.state {
            GotoThenNopState::GoingTo(pos) => {
                // trigger go to
                let follow = ctx
                    .world
                    .component_mut::<FollowPathComponent>(ctx.entity)
                    .unwrap();
                follow.new_path(pos, SearchGoal::Arrive, NormalizedFloat::new(0.7));

                // block on arrive event
                // TODO specify entity specifically, either Self or Other(e)
                ctx.subscriptions.push(EntityEventSubscription(
                    ctx.entity,
                    EventSubscription::Specific(EntityEventType::Arrived),
                ));
                ActivityResult::Blocked(ctx.subscriptions)
            }
            GotoThenNopState::Done => {
                // nice
                ActivityResult::Finished(Finish::Succeeded)
            }
        }
    }

    fn on_event(&mut self, event: &EntityEvent) -> (EventUnblockResult, EventUnsubscribeResult) {
        match event.1 {
            EntityEventPayload::Arrived(_) => {
                self.state = GotoThenNopState::Done;
                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
                )
            }
            _ => unreachable!(),
        }
    }
}

impl GotoThenNop {
    pub fn new(pos: WorldPoint) -> Self {
        Self {
            state: GotoThenNopState::GoingTo(pos),
        }
    }
}

impl Display for GotoThenNop {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Going to a place then nop'ing")
    }
}
*/
