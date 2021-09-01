use common::*;
use world::SearchGoal;

use crate::activity::activity::{
    ActivityEventContext, ActivityFinish, ActivityResult, SubActivity,
};
use crate::activity::subactivities::GoToSubActivity;
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::Entity;
use crate::ComponentWorld;
use crate::{unexpected_event, TransformComponent};

use crate::event::prelude::*;
use crate::event::TimerToken;
use crate::nop_subactivity;

const FOLLOW_CHECK_SCHEDULE: u32 = 35;

// TODO will probably need porting to a follow subactivity

/// Follow the given entity
#[derive(Debug)]
pub struct FollowActivity {
    target: Entity,
    radius: u8,

    goto: Option<GoToSubActivity>,
    timer: Option<TimerToken>,
    subscribed: bool,
}

#[derive(Debug, Error)]
pub enum FollowError {
    #[error("Either entity missing transform for following")]
    MissingTransform,
}

impl Activity for FollowActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext) -> ActivityResult {
        ctx.clear_path();

        let (distance2, target_pos) = {
            let me = ctx.world.component::<TransformComponent>(ctx.entity);
            let you = ctx.world.component::<TransformComponent>(self.target);

            match me.ok().zip(you.ok()) {
                Some((me, you)) => (me.position.distance2(you.position), you.position),
                None => return ActivityResult::errored(FollowError::MissingTransform),
            }
        };

        if distance2 < self.radius.pow(2) as f32 {
            // in range still, loiter
            trace!("target in range, doing nothing and waiting"; "distance" => distance2.sqrt());
            ActivityResult::Ongoing
        } else {
            let goto = GoToSubActivity::with_goal(
                target_pos,
                NormalizedFloat::new(0.3),
                SearchGoal::Nearby(self.radius),
            );

            let result = goto.init(ctx);
            self.goto = Some(goto);

            // also schedule timer to periodically check target position again
            self.timer = Some(ctx.schedule_timer(FOLLOW_CHECK_SCHEDULE, ctx.entity));

            // only subscribe once
            if !std::mem::replace(&mut self.subscribed, true) {
                ctx.subscribe_to(ctx.entity, EventSubscription::Specific(unreachable!()));
            }

            result
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        _: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        match &event.payload {
            EntityEventPayload::Arrived(token, _) => {
                if self
                    .goto
                    .as_ref()
                    .map(|goto| goto.token() == *token)
                    .unwrap_or(false)
                {
                    // arrived, reconsider
                    self.goto = None;

                    // stay subscribed to all anyway
                    (
                        EventUnblockResult::Unblock,
                        EventUnsubscribeResult::StaySubscribed,
                    )
                } else {
                    (
                        EventUnblockResult::KeepBlocking,
                        EventUnsubscribeResult::StaySubscribed,
                    )
                }
            }

            _ => unexpected_event!(event),
        }
    }

    fn on_finish(&mut self, _: &ActivityFinish, _: &mut ActivityContext) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity {
        if let Some(goto) = self.goto.as_ref() {
            goto
        } else {
            nop_subactivity!("Loitering", 0.2)
        }
    }
}

impl FollowActivity {
    pub fn new(target: Entity, radius: u8) -> Self {
        Self {
            target,
            radius,
            goto: None,
            timer: None,
            subscribed: false,
        }
    }
}

impl Display for FollowActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Following {}", self.target)
    }
}
