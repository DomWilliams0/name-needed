use std::cell::Cell;

use common::*;
use unit::world::WorldPoint;

use crate::activity::activity::{ActivityFinish, ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::event::prelude::*;
use crate::path::{FollowPathComponent, PathToken};
use crate::ComponentWorld;
use world::SearchGoal;

/// Assigns path to navigate to given pos. Blocks on arrival event
#[derive(Clone, Debug)]
pub struct GoToSubActivity {
    target: WorldPoint,
    speed: NormalizedFloat,
    goal: SearchGoal,
    token: Cell<Option<PathToken>>,
}

impl GoToSubActivity {
    pub fn new(target: WorldPoint, speed: NormalizedFloat) -> Self {
        Self::with_goal(target, speed, SearchGoal::Arrive)
    }

    pub fn with_goal(target: WorldPoint, speed: NormalizedFloat, goal: SearchGoal) -> Self {
        Self {
            target,
            speed,
            goal,
            token: Cell::new(None),
        }
    }

    /// Panics if called before `init` finishes successfully
    pub fn token(&self) -> PathToken {
        self.token.get().expect("path token not set")
    }
}

impl<W: ComponentWorld> SubActivity<W> for GoToSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        let follow_path = match ctx.world.component_mut::<FollowPathComponent>(ctx.entity) {
            Ok(comp) => comp,
            Err(e) => {
                error!("can't follow path"; "error" => %e);
                return ActivityResult::errored(e);
            }
        };

        // assign path
        let token = follow_path.new_path_with_goal(self.target, self.goal, self.speed);
        self.token.set(Some(token));

        // await arrival
        ctx.subscribe_to(
            ctx.entity,
            EventSubscription::Specific(EntityEventType::Arrived),
        );

        ActivityResult::Blocked
    }

    fn on_finish(&self, _: &ActivityFinish, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        // TODO helper on ctx to get component

        if let Ok(comp) = ctx.world.component_mut::<FollowPathComponent>(ctx.entity) {
            let token = self.token.get();
            if token.is_some() && comp.current_token() == token {
                comp.clear_path();
            }
        }
        Ok(())
    }

    fn exertion(&self) -> f32 {
        // TODO better exertion calculation for movement speed
        self.speed.value()
    }
}

impl Display for GoToSubActivity {
    // TODO use movement speed enum for display e.g. wandering to, running to
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Going to {}", self.target)
    }
}
