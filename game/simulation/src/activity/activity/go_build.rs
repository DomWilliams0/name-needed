use async_trait::async_trait;

use common::*;
use world::block::BlockType;

use crate::activity::activity::Activity;
use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::WorldPosition;
use world::SearchGoal;

/// Building {bt} at {block}
#[derive(Debug, Clone, Display)]
pub struct GoBuildActivity {
    block: WorldPosition,
    bt: BlockType,
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
            self.block.centred(),
            NormalizedFloat::new(0.8),
            SearchGoal::Adjacent,
            GoingToStatus::target("block"),
        )
        .await?;

        // buildy buildy
        ctx.update_status(BuildStatus);
        ctx.build_block(self.block, self.bt).await?;

        Ok(())
    }
}

impl GoBuildActivity {
    pub fn new(block: WorldPosition, bt: BlockType) -> Self {
        Self { block, bt }
    }
}

impl Status for BuildStatus {
    fn exertion(&self) -> f32 {
        1.3
    }
}
