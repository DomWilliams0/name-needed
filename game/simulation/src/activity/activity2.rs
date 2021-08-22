use crate::activity::system2::ActivityContext2;
use crate::{ComponentWorld, TransformComponent};
use async_trait::async_trait;
use common::*;

#[async_trait]
pub trait Activity2: Display + Debug {
    // TODO need a context that can be stored forever
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> BoxedResult<()>;
}

// TODO temporary
#[derive(Default, Debug)]
pub struct TestActivity2;

// TODO temporary
#[derive(Default, Debug)]
pub struct NopActivity2;

#[async_trait]
impl Activity2 for TestActivity2 {
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> BoxedResult<()> {
        debug!("TODO wandering");
        let transform = ctx.world.component::<TransformComponent>(ctx.entity)?;
        // TODO ensure component refs cant be held across awaits
        Ok(())
    }
}

#[async_trait]
impl Activity2 for NopActivity2 {
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> BoxedResult<()> {
        // TODO reimplement nop
        Ok(())
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

// TODO ensure destructor runs when cancelled
