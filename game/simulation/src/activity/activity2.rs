use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use common::*;

use crate::activity::activity2::EventResult::Unconsumed;
use crate::activity::status::Status;
use crate::activity::subactivities2::{
    BreakBlockError, BreakBlockSubactivity, EatItemError, EatItemSubactivity2, EquipSubActivity2,
    GoToSubactivity, GotoError, PickupSubactivity,
};
use crate::activity::{EquipItemError, StatusUpdater};
use crate::event::{
    EntityEvent, EntityEventPayload, EntityEventQueue, EntityEventSubscription, RuntimeTimers,
};
use crate::runtime::{ManualFuture, TaskRef, TimerFuture};
use crate::{ComponentWorld, EcsWorld, Entity, FollowPathComponent, WorldPosition};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::task::{Context, Poll};
use unit::world::WorldPoint;
use world::SearchGoal;

pub type ActivityResult = Result<(), Box<dyn Error>>;

pub enum InterruptResult {
    Continue,
    Cancel,
}

#[async_trait]
pub trait Activity2: Debug {
    fn description(&self) -> Box<dyn Display>;
    async fn dew_it<'a>(&'a self, ctx: ActivityContext2<'a>) -> ActivityResult;

    fn on_unhandled_event(&self, event: EntityEvent) -> InterruptResult {
        InterruptResult::Continue
    }
}

pub struct ActivityContext2<'a> {
    entity: Entity,
    // TODO ensure component refs cant be held across awaits
    world: Pin<&'a EcsWorld>,
    task: TaskRef,
    status: StatusUpdater,
    activity: Rc<dyn Activity2>,
}

pub enum EventResult {
    Consumed,
    /// Give it back
    Unconsumed(EntityEventPayload),
}

pub enum GenericEventResult {
    Consumed,
    /// Give it back
    Unconsumed(EntityEvent),
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
        activity: Rc<dyn Activity2>,
    ) -> Self {
        Self {
            entity,
            world,
            task,
            status,
            activity,
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
    pub async fn pick_up(&self, item: Entity) -> Result<(), EquipItemError> {
        PickupSubactivity.pick_up(self, item).await
    }

    /// Equip item that's already in inventory
    pub async fn equip(&self, item: Entity, extra_hands: u16) -> Result<(), EquipItemError> {
        EquipSubActivity2.equip(self, item, extra_hands).await
    }

    /// Item should already be equipped
    pub async fn eat(&self, item: Entity) -> Result<(), EatItemError> {
        EatItemSubactivity2.eat(self, item).await
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

        self.consume_events(|mut evt| {
            if evt.subject == subscription.subject {
                match filter(evt.payload) {
                    EventResult::Consumed => GenericEventResult::Consumed,
                    EventResult::Unconsumed(payload) => {
                        evt.payload = payload;
                        GenericEventResult::Unconsumed(evt)
                    }
                }
            } else {
                GenericEventResult::Unconsumed(evt)
            }
        })
        .await;

        // unsubscribe
        evts.unsubscribe(self.entity, subscription);
    }

    /// Hangs in event loop running filter against all of them until unconsumed is returned
    pub async fn consume_events(&self, mut filter: impl FnMut(EntityEvent) -> GenericEventResult) {
        loop {
            let evt = self.next_event().await;
            let evt = match filter(evt) {
                GenericEventResult::Consumed => break,
                GenericEventResult::Unconsumed(returned_evt) => returned_evt,
            };

            // event is unconsumed
            trace!("handling unhandled event"; "event" => ?evt);
            match self.activity.on_unhandled_event(evt) {
                InterruptResult::Continue => continue,
                InterruptResult::Cancel => {
                    trace!("handler requested cancel");
                    break;
                }
            }
        }
    }

    /// Must manually unsubscribe or wait until activity end
    pub fn subscribe_to(&'a self, subscription: EntityEventSubscription) {
        let evts = self.world.resource_mut::<EntityEventQueue>();
        evts.subscribe(self.entity, once(subscription));
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

    pub fn update_status(&self, status: impl Status + 'static) {
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
