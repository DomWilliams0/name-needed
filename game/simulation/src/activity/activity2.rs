use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use common::*;

use crate::event::RuntimeTimers;
use crate::runtime::{ManualFuture, TimerFuture};
use crate::{ComponentWorld, EcsWorld, Entity};

pub type ActivityResult = Result<(), Box<dyn Error>>;

#[async_trait]
pub trait Activity2: Display + Debug {
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> ActivityResult;
}

pub struct ActivityContext2<'a> {
    pub entity: Entity,
    pub world: Pin<&'a EcsWorld>,
}

// only used on the main thread
unsafe impl Sync for ActivityContext2<'_> {}
unsafe impl Send for ActivityContext2<'_> {}

impl<'a> ActivityContext2<'a> {
    pub fn wait(&self, ticks: u32) -> impl Future<Output = ()> + 'a {
        let timers = self.world.resource_mut::<RuntimeTimers>();
        let trigger = ManualFuture::default();
        let token = timers.schedule(ticks, trigger.clone());
        TimerFuture::new(trigger, token, self.world)
    }
}
