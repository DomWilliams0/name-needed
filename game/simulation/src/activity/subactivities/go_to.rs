use crate::activity::activity::{ActivityResult, Finish, SubActivity};
use crate::activity::ActivityContext;
use crate::event::{EntityEventType, EventSubscription};
use crate::path::FollowPathComponent;
use crate::ComponentWorld;
use common::*;
use unit::world::WorldPoint;
use world::SearchGoal;

/// Assigns path to navigate to given pos. Blocks on arrival event
#[derive(Clone, Debug)]
pub struct GoToSubActivity {
    target: WorldPoint,
    speed: NormalizedFloat,
}

impl GoToSubActivity {
    pub fn new(target: WorldPoint, speed: NormalizedFloat) -> Self {
        Self { target, speed }
    }
}

impl<W: ComponentWorld> SubActivity<W> for GoToSubActivity {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult {
        let follow_path = match ctx.world.component_mut::<FollowPathComponent>(ctx.entity) {
            Ok(comp) => comp,
            Err(e) => {
                error!("{:?} can't follow path: {}", ctx.entity, e);
                return ActivityResult::errored(e);
            }
        };

        // assign path
        follow_path.new_path(self.target, SearchGoal::Arrive, self.speed);

        // await arrival
        ctx.subscribe_to(
            ctx.entity,
            EventSubscription::Specific(EntityEventType::Arrived),
        );

        ActivityResult::Blocked
    }

    fn on_finish(&self, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        // TODO clear path from followpath if it matches this path assignment token
        Ok(())
    }

    fn exertion(&self) -> f32 {
        // TODO better exertion calculation for movement speed
        self.speed.value()
    }
}

impl Display for GoToSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Going to {}", self.target)
    }
}
