use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use common::*;

use crate::activity::subactivities2::{GoToSubactivity, GotoError};
use crate::activity::StatusUpdater;
use crate::event::{
    EntityEvent, EntityEventPayload, EntityEventQueue, EntityEventSubscription, RuntimeTimers,
};
use crate::runtime::{ManualFuture, TaskRef, TimerFuture};
use crate::{ComponentWorld, EcsWorld, Entity};
use unit::world::WorldPoint;
use world::SearchGoal;

pub type ActivityResult = Result<(), Box<dyn Error>>;

#[async_trait]
pub trait Activity2: Display + Debug {
    fn description(&self) -> Box<dyn Display>;
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> ActivityResult;
}

pub struct ActivityContext2<'a> {
    pub entity: Entity,
    // TODO ensure component refs cant be held across awaits
    pub world: Pin<&'a EcsWorld>,
    pub task: TaskRef,
    pub status: StatusUpdater,
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

    pub async fn go_to(&self, pos: WorldPoint, speed: NormalizedFloat) -> Result<(), GotoError> {
        GoToSubactivity
            .go_to(self, pos, speed, SearchGoal::Arrive)
            .await
    }

    /// Prefer using other helpers than direct event subscription e.g. [go_to].
    ///
    /// Subscribes to the given subscription, runs the filter against each event until it returns
    /// false, then unsubscribes from the given event
    pub async fn subscribe_to_until(
        &self,
        subscription: EntityEventSubscription,
        mut filter: impl FnMut(EntityEventPayload) -> bool,
    ) {
        // TODO other subscribe method to batch up a few subscriptions before adding to evt queue
        // register subscription
        let evts = self.world.resource_mut::<EntityEventQueue>();
        evts.subscribe(self.entity, once(subscription));

        loop {
            let evt = self.next_event().await;
            debug_assert_eq!(evt.subject, subscription.subject);
            if !filter(evt.payload) {
                break;
            }
        }

        // unsubscribe
        evts.unsubscribe(self.entity, subscription);
    }

    async fn next_event(&self) -> EntityEvent {
        // TODO event queue needs to be cleared of events after unsubscribing? or just consume them and ignore them?
        let mut n = 0;
        loop {
            match self.task.pop_event() {
                None => {
                    if n > 0 {
                        trace!("woken up {} times without an event", n; self.entity);
                        if n > 5 {
                            warn!("woken up {} times without an event!", n; self.entity);
                        }
                    }
                    // keep waiting until an event marks this as ready again
                    self.task.park_until_event().await;

                    n += 1;
                }
                Some(evt) => return evt,
            }
        }
    }

    pub fn update_status(&self, status: impl Display + 'static) {
        self.status.update(status);
    }

    // async fn yield_now(&self) {
    //     pub struct YieldNow(bool);
    //
    //     impl Future for YieldNow {
    //         type Output = ();
    //
    //         fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    //             if std::mem::replace(&mut self.0, true) {
    //                 Poll::Ready(())
    //             } else {
    //                 cx.waker().wake_by_ref();
    //                 Poll::Pending
    //             }
    //         }
    //     }
    //
    //     YieldNow(false).await
    // }
}
