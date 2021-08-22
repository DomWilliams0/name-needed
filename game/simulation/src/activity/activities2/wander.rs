use async_trait::async_trait;

use common::*;

use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::{Activity2, ActivityResult};
use crate::{ComponentWorld, TransformComponent};

#[derive(Default, Debug)]
pub struct WanderActivity2;

#[async_trait]
impl Activity2 for WanderActivity2 {
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> ActivityResult {
        debug!("TODO wandering");
        let transform = ctx.world.component::<TransformComponent>(ctx.entity);
        ctx.wait(10).await;
        // TODO ensure component refs cant be held across awaits
        Ok(())
    }
}

impl Display for WanderActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}
