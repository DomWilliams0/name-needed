use async_trait::async_trait;

use common::*;

use crate::activity::activity::Activity;
use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::job::{BuildDetails, SocietyJobHandle};

use world::SearchGoal;

#[derive(Debug, Clone)]
pub struct GoBuildActivity {
    job: SocietyJobHandle,
    details: BuildDetails,
}

/// Building
#[derive(Display)]
struct BuildStatus;

#[async_trait]
impl Activity for GoBuildActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        // walk to the block
        ctx.go_to(
            self.details.pos.centred(),
            NormalizedFloat::new(0.8),
            SearchGoal::Adjacent,
            GoingToStatus::target("block"),
        )
        .await?;

        // buildy buildy
        ctx.update_status(BuildStatus);
        ctx.build_block(self.job, &self.details).await?;

        Ok(())
    }
}

impl GoBuildActivity {
    pub fn new(job: SocietyJobHandle, details: BuildDetails) -> Self {
        Self { job, details }
    }
}

impl Status for BuildStatus {
    // TODO depends on build type
    fn exertion(&self) -> f32 {
        1.3
    }
}

impl Display for GoBuildActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Building {:?}", self.details.target)
    }
}
