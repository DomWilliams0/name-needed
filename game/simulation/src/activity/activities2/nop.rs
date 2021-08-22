use async_trait::async_trait;

use common::*;

use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::{Activity2, ActivityResult};

const NOP_WARN_THRESHOLD: u32 = 60;

#[derive(Default, Debug)]
pub struct NopActivity2;

#[async_trait]
impl Activity2 for NopActivity2 {
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> ActivityResult {
        loop {
            ctx.wait(NOP_WARN_THRESHOLD).await;

            warn!(
                "{} has been stuck in nop activity for a while, possible infinite loop",
                ctx.entity
            );
        }
    }
}

impl Display for NopActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}
