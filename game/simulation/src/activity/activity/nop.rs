use async_trait::async_trait;

use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::Activity;
use common::*;

const NOP_WARN_THRESHOLD: u32 = 60;

/// Thinking
#[derive(Default, Debug, Display)]
pub struct NopActivity;

#[async_trait]
impl Activity for NopActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(Self)
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        loop {
            ctx.wait(NOP_WARN_THRESHOLD).await;

            warn!(
                "{} has been stuck in nop activity for a while, possible infinite loop",
                ctx.entity()
            );
        }
    }
}