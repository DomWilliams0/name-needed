use async_trait::async_trait;

use common::*;

use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::{Activity2, ActivityResult};
use crate::activity::status::{NopStatus, Status};
use crate::activity::subactivities2::GoingToStatus;
use crate::WorldPosition;
use world::SearchGoal;

#[derive(Debug, Clone)]
pub struct GoBreakBlockActivity2 {
    block: WorldPosition,
}

struct BreakBlockStatus;

#[async_trait]
impl Activity2 for GoBreakBlockActivity2 {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext2) -> ActivityResult {
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

impl GoBreakBlockActivity2 {
    pub fn new(block: WorldPosition) -> Self {
        Self { block }
    }
}

impl Display for GoBreakBlockActivity2 {
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
