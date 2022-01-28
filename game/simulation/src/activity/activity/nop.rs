use async_trait::async_trait;

use common::*;

use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::status::Status;
use crate::activity::Activity;

/// Thinking
#[derive(Default, Debug, Display)]
pub struct NopActivity;

/// Pondering
#[derive(Display)]
struct NopStatus;

#[async_trait]
impl Activity for NopActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(Self)
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        ctx.update_status(NopStatus);
        loop {
            ctx.park_indefinitely().await;
        }
    }
}

impl Status for NopStatus {
    fn exertion(&self) -> f32 {
        0.1
    }
}
