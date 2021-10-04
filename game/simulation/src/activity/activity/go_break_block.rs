use async_trait::async_trait;

use common::*;

use crate::activity::activity::Activity;
use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::WorldPosition;
use world::SearchGoal;

/// Breaking block at {block}
#[derive(Debug, Clone, Display)]
pub struct GoBreakBlockActivity {
    block: WorldPosition,
}

/// Breaking block
#[derive(Display)]
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

impl Status for BreakBlockStatus {
    fn exertion(&self) -> f32 {
        1.3
    }
}
