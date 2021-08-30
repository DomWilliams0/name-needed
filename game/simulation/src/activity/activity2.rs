use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use common::*;

use crate::activity::subactivities2::{
    BreakBlockError, BreakBlockSubactivity, GoToSubactivity, GotoError, PickupSubactivity,
};
use crate::activity::{PickupItemError, StatusUpdater};
use crate::event::{
    EntityEvent, EntityEventPayload, EntityEventQueue, EntityEventSubscription, RuntimeTimers,
};
use crate::runtime::{ManualFuture, TaskRef, TimerFuture};
use crate::{ComponentWorld, EcsWorld, Entity, FollowPathComponent, WorldPosition};
use std::cell::{Cell, RefCell};
use std::task::{Context, Poll};
use unit::world::WorldPoint;
use world::SearchGoal;

pub type ActivityResult = Result<(), Box<dyn Error>>;

#[async_trait]
pub trait Activity2: Debug {
    fn description(&self) -> Box<dyn Display>;
    async fn dew_it<'a>(&'a self, ctx: ActivityContext2<'a>) -> ActivityResult;
}

pub struct ActivityContext2<'a> {
    entity: Entity,
    // TODO ensure component refs cant be held across awaits
    world: Pin<&'a EcsWorld>,
    task: TaskRef,
    status: StatusUpdater,
}

pub struct ScopedSubscription<'a> {
    subscription: EntityEventSubscription,
    ctx: &'a ActivityContext2<'a>,
}

pub enum EventResult {
    Consumed,
    /// Give it back
    Unconsumed(EntityEventPayload),
}

// only used on the main thread
unsafe impl Sync for ActivityContext2<'_> {}
unsafe impl Send for ActivityContext2<'_> {}

impl<'a> ActivityContext2<'a> {
    pub fn new(
        entity: Entity,
        world: Pin<&'a EcsWorld>,
        task: TaskRef,
        status: StatusUpdater,
    ) -> Self {
        Self {
            entity,
            world,
            task,
            status,
        }
    }

    pub const fn entity(&self) -> Entity {
        self.entity
    }

    pub fn world(&self) -> &EcsWorld {
        &self.world
    }

    pub fn wait(&self, ticks: u32) -> impl Future<Output = ()> + 'a {
        let timers = self.world.resource_mut::<RuntimeTimers>();
        let trigger = ManualFuture::default();
        let token = timers.schedule(ticks, trigger.clone());
        TimerFuture::new(trigger, token, self.world)
    }

    /// Does not update activity status
    pub async fn go_to(
        &'a self,
        pos: WorldPoint,
        speed: NormalizedFloat,
        goal: SearchGoal,
    ) -> Result<(), GotoError> {
        GoToSubactivity::new(self).go_to(pos, speed, goal).await
    }

    pub fn clear_path(&self) {
        if let Ok(comp) = self.world.component_mut::<FollowPathComponent>(self.entity) {
            comp.clear_path();
        }
    }

    /// Must be close enough
    pub async fn break_block(&self, block: WorldPosition) -> Result<(), BreakBlockError> {
        BreakBlockSubactivity::default()
            .break_block(self, block)
            .await
    }

    /// Pick up item off the ground, checks if close enough first
    pub async fn pick_up(&self, item: Entity) -> Result<(), PickupItemError> {
        PickupSubactivity.pick_up(self, item).await
    }

    /// Prefer using other helpers than direct event subscription e.g. [go_to].
    ///
    /// Subscribes to the given subscription, runs the filter against each event until it returns
    /// false, then unsubscribes from the given event
    pub async fn subscribe_to_until(
        &self,
        subscription: EntityEventSubscription,
        mut filter: impl FnMut(EntityEventPayload) -> EventResult,
    ) {
        // TODO other subscribe method to batch up a few subscriptions before adding to evt queue
        // register subscription
        let evts = self.world.resource_mut::<EntityEventQueue>();
        evts.subscribe(self.entity, once(subscription));

        loop {
            let mut evt = self.next_event().await;
            if evt.subject == subscription.subject {
                match filter(evt.payload) {
                    EventResult::Consumed => break,
                    EventResult::Unconsumed(payload) => evt.payload = payload,
                }
            }

            // event is unconsumed
            unreachable!("event {:?}", evt);
        }

        // unsubscribe
        evts.unsubscribe(self.entity, subscription);
    }

    pub fn subscribe_to_scoped(
        &'a self,
        subscription: EntityEventSubscription,
    ) -> ScopedSubscription<'_> {
        let evts = self.world.resource_mut::<EntityEventQueue>();
        evts.subscribe(self.entity, once(subscription));
        ScopedSubscription {
            subscription,
            ctx: self,
        }
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

    /// Ready next tick
    pub async fn yield_now(&self) {
        pub struct YieldNow(bool);

        impl Future for YieldNow {
            type Output = ();

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if std::mem::replace(&mut self.0, true) {
                    Poll::Ready(())
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
        }

        YieldNow(false).await
    }
}

impl Drop for ScopedSubscription<'_> {
    fn drop(&mut self) {
        let evts = self.ctx.world.resource_mut::<EntityEventQueue>();
        evts.unsubscribe(self.ctx.entity, self.subscription);
    }
}
