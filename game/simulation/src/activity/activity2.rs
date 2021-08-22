use crate::activity::system2::ActivityContext2;
use crate::{ComponentWorld, TransformComponent};
use async_trait::async_trait;
use common::*;

pub type ActivityResult = Result<(), Box<dyn Error>>;

#[async_trait]
pub trait Activity2: Display + Debug {
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> ActivityResult;
}

// TODO temporary
#[derive(Default, Debug)]
pub struct TestActivity2;

const NOP_WARN_THRESHOLD: u32 = 60;

#[derive(Default, Debug)]
pub struct NopActivity2;

#[async_trait]
impl Activity2 for TestActivity2 {
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> ActivityResult {
        debug!("TODO wandering");
        let transform = ctx.world.component::<TransformComponent>(ctx.entity);
        ctx.wait(10).await;
        // TODO ensure component refs cant be held across awaits
        Ok(())
    }
}

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

impl Display for TestActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}
