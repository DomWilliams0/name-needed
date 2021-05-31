use crate::activity::activity::{
    ActivityEventContext, ActivityFinish, ActivityResult, SubActivity,
};
use crate::activity::subactivities::GoToSubActivity;
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::ComponentGetError;
use crate::event::{
    EntityEvent, EntityEventPayload, EntityEventType, EntityTimers, EventSubscription, TimerToken,
};
use crate::path::WANDER_SPEED;
use crate::{nop_subactivity, unexpected_event, ComponentWorld, TransformComponent};
use common::*;

// TODO add additional DSEs while wandering and loitering e.g. whistling, waving, humming

#[derive(Debug)]
enum WanderState {
    /// Number of arrivals
    Wandering(u8),
    /// Ticks to loiter
    Loitering(u8),
}

#[derive(Debug)]
enum WanderSubActivity {
    Uninit,
    Wandering(GoToSubActivity),
    Loitering(TimerToken),
}

#[derive(Debug)]
pub struct WanderActivity {
    subactivity: WanderSubActivity,
    state: WanderState,
}

#[derive(Debug, Error)]
pub enum WanderError {
    #[error("Wanderer has no transform: {0}")]
    MissingTransform(#[from] ComponentGetError),

    #[error("Can't find an accessible wander destination, possibly stuck")]
    Inaccessible,
}

const WANDER_RADIUS: u16 = 10;

impl<W: ComponentWorld> Activity<W> for WanderActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        match self.state {
            WanderState::Wandering(count) => {
                trace!(
                    "wandering to {count} random places until next loiter",
                    count = count
                );

                let pos = match ctx.world.component::<TransformComponent>(ctx.entity) {
                    Ok(t) => t.accessible_position(),
                    Err(e) => return ActivityResult::errored(WanderError::MissingTransform(e)),
                };

                let world_ref = ctx.world.voxel_world();
                let world = world_ref.borrow();

                let target = world.choose_random_accessible_block_in_radius(pos, WANDER_RADIUS, 20);
                if let Some(target) = target {
                    let goto =
                        GoToSubActivity::new(target.centred(), NormalizedFloat::new(WANDER_SPEED));
                    trace!("new wander target"; "target" => %target);

                    let result = goto.init(ctx);
                    self.subactivity = WanderSubActivity::Wandering(goto);
                    result
                } else {
                    warn!(
                        "failed to find wander destination";
                        "position" => %pos,
                    );

                    ActivityResult::errored(WanderError::Inaccessible)
                }
            }
            WanderState::Loitering(count) => {
                trace!("loitering for {count} ticks", count = count);

                let token = ctx
                    .world
                    .resource_mut::<EntityTimers>()
                    .schedule(count as u32, ctx.entity);
                self.subactivity = WanderSubActivity::Loitering(token);

                ctx.subscribe_to(
                    ctx.entity,
                    EventSubscription::Specific(EntityEventType::TimerElapsed),
                );
                ActivityResult::Blocked
            }
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        _: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        match &event.payload {
            EntityEventPayload::Arrived(_, result) => {
                if result.is_ok() {
                    // ignore path token
                    if let WanderState::Wandering(counter) = &mut self.state {
                        *counter = counter.saturating_sub(1);
                        if *counter == 0 {
                            // stop wandering and loiter
                            self.state = WanderState::loiter();
                        }

                        // unblock for new wander target
                        return (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::StaySubscribed,
                        );
                    }
                }
                (
                    EventUnblockResult::KeepBlocking,
                    EventUnsubscribeResult::StaySubscribed,
                )
            }
            EntityEventPayload::TimerElapsed(token) => {
                let unblock = match self.subactivity {
                    WanderSubActivity::Loitering(my_token) if my_token == *token => {
                        // stop loitering
                        self.state = WanderState::wander();
                        EventUnblockResult::Unblock
                    }
                    _ => EventUnblockResult::KeepBlocking,
                };

                (unblock, EventUnsubscribeResult::StaySubscribed)
            }
            _ => unexpected_event!(event),
        }
    }

    fn on_finish(&mut self, _: &ActivityFinish, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        ctx.clear_path();

        if let WanderSubActivity::Loitering(timer) = self.subactivity {
            ctx.world.resource_mut::<EntityTimers>().cancel(timer);
        }
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        match &self.subactivity {
            WanderSubActivity::Wandering(goto) => goto,
            _ => nop_subactivity!("Loitering", 0.15),
        }
    }
}

impl Display for WanderActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Wandering aimlessly")
    }
}

impl Default for WanderActivity {
    fn default() -> Self {
        Self {
            subactivity: WanderSubActivity::Uninit,
            state: WanderState::wander(),
        }
    }
}

impl WanderState {
    fn wander() -> Self {
        WanderState::Wandering(random::get().gen_range(1, 4))
    }

    fn loiter() -> Self {
        WanderState::Loitering(random::get().gen_range(10, 60))
    }
}
