use async_trait::async_trait;

use common::*;

use crate::activity::activity::Activity;
use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::WorldPosition;
use world::SearchGoal;

#[derive(Debug, Clone)]
pub struct GoBreakBlockActivity {
    block: WorldPosition,
}

struct BreakBlockStatus;

#[async_trait]
impl Activity for GoBreakBlockActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        // walk to the block
        ctx.go_to(
            self.block.centred(),
            NormalizedFloat::new(0.8),
            SearchGoal::Adjacent,
            GoingToStatus::target("block"),
        )
        .await?;

        // breaky breaky
        ctx.update_status(BreakBlockStatus);
        ctx.break_block(self.block).await?;

        Ok(())
    }
}

impl GoBreakBlockActivity {
    pub fn new(block: WorldPosition) -> Self {
        Self { block }
    }
}

impl Display for GoBreakBlockActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Breaking block at {}", self.block)
    }
}

impl Display for BreakBlockStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("Breaking block")
    }
}

impl Status for BreakBlockStatus {
    fn exertion(&self) -> f32 {
        1.3
    }
}
