use async_trait::async_trait;

use common::*;

use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::{Activity2, ActivityResult};
use crate::activity::subactivities2::GoingToStatus;
use unit::world::WorldPoint;
use world::SearchGoal;

#[derive(Debug, Clone)]
pub struct GoToActivity2 {
    target: WorldPoint,
    speed: NormalizedFloat,
    goal: SearchGoal,
}

struct GoingToDescription(WorldPoint);

#[async_trait]
impl Activity2 for GoToActivity2 {
    fn description(&self) -> Box<dyn Display> {
        Box::new(GoingToDescription(self.target))
    }

    async fn dew_it(&self, ctx: &ActivityContext2) -> ActivityResult {
        ctx.go_to(self.target, self.speed, self.goal, GoingToStatus::default())
            .await?;
        Ok(())
    }
}

impl GoToActivity2 {
    pub fn new(target: WorldPoint, speed: NormalizedFloat, goal: SearchGoal) -> Self {
        Self {
            target,
            speed,
            goal,
        }
    }
}

impl Display for GoToActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Going to {}", self.target)
    }
}

impl Display for GoingToDescription {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Going to {}", self.0)
    }
}
