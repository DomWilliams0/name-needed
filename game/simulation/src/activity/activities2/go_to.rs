use async_trait::async_trait;

use common::*;

use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::{Activity2, ActivityResult};
use crate::activity::status::Status;
use unit::world::WorldPoint;
use world::SearchGoal;

#[derive(Debug, Clone)]
pub struct GoToActivity2 {
    target: WorldPoint,
    reason: &'static str,
}

struct GoingToState;

#[async_trait]
impl Activity2 for GoToActivity2 {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it<'a>(&'a self, ctx: ActivityContext2<'a>) -> ActivityResult {
        ctx.update_status(GoingToState);
        ctx.go_to(self.target, NormalizedFloat::new(0.8), SearchGoal::Arrive)
            .await?;

        Ok(())
    }
}

impl GoToActivity2 {
    pub fn new(target: WorldPoint, reason: &'static str) -> Self {
        Self { target, reason }
    }
}

impl Display for GoToActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Going to {} because {}", self.target, self.reason)
    }
}

impl Display for GoingToState {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("Going to target")
    }
}

impl Status for GoingToState {
    fn exertion(&self) -> f32 {
        1.0
    }
}
