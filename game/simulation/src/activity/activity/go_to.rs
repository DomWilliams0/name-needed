use async_trait::async_trait;

use common::*;

use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::subactivity::GoingToStatus;
use crate::activity::Activity;
use unit::world::WorldPoint;
use world::SearchGoal;

/// Going to {target}
#[derive(Debug, Clone, Display)]
pub struct GoToActivity {
    target: WorldPoint,
    speed: NormalizedFloat,
    goal: SearchGoal,
}

/// Going to {0}
#[derive(Display)]
struct GoingToDescription(WorldPoint);

#[async_trait]
impl Activity for GoToActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(GoingToDescription(self.target))
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        ctx.go_to(self.target, self.speed, self.goal, GoingToStatus::default())
            .await?;
        Ok(())
    }
}

impl GoToActivity {
    pub fn new(target: WorldPoint, speed: NormalizedFloat, goal: SearchGoal) -> Self {
        Self {
            target,
            speed,
            goal,
        }
    }
}
