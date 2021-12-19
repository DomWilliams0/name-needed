use std::future::Future;
use std::pin::Pin;

use common::*;

use crate::activity::status::Status;
use crate::activity::subactivity::{
    BreakBlockError, BreakBlockSubactivity, BuildBlockError, BuildBlockSubactivity, EatItemError,
    EatItemSubactivity, EquipSubActivity, GoToSubactivity, GoingToStatus, GotoError, HaulSource,
    HaulSubactivity, PickupSubactivity,
};
use crate::activity::{Activity, EquipItemError, HaulError, StatusUpdater};
use crate::ecs::*;
use crate::event::prelude::*;
use crate::event::{EntityEventQueue, RuntimeTimers};
use crate::runtime::{TaskRef, TimerFuture};
use crate::{
    ComponentWorld, EcsWorld, Entity, FollowPathComponent, TransformComponent, WorldPosition,
};

use crate::activity::context::EventResult::{Consumed, Unconsumed};
use crate::job::{BuildDetails, SocietyJobHandle};
use std::rc::Rc;
use std::task::{Context, Poll};
use unit::world::WorldPoint;
use world::SearchGoal;

pub type ActivityResult = Result<(), Box<dyn Error>>;

pub enum InterruptResult {
    Continue,
    Cancel,
}

#[macro_export]
macro_rules! unexpected_event2 {
    ($event:expr) => {{
        trace!("ignoring unexpected event"; "event" => ?$event);
        $crate::activity::activity::EventResult::Unconsumed($event)
    }};
}

#[derive(Clone)]
pub struct ActivityContext {
    entity: Entity,
    // TODO ensure component refs cant be held across awaits
    world: Pin<&'static EcsWorld>,
    task: TaskRef,
    status: StatusUpdater,
    activity: Rc<dyn Activity>,
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

pub enum DistanceCheckResult {
    /// No transform or invalid entity
    NotAvailable,
    TooFar,
    InRange,
}

// only used on the main thread
unsafe impl Sync for ActivityContext {}
unsafe impl Send for ActivityContext {}

impl ActivityContext {
    pub fn new(
        entity: Entity,
        world: Pin<&'static EcsWorld>,
        task: TaskRef,
        status: StatusUpdater,
        activity: Rc<dyn Activity>,
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

    pub fn wait(&self, ticks: u32) -> TimerFuture {
        let timers = self.world.resource_mut::<RuntimeTimers>();
        let (end_tick, timer) = timers.schedule(ticks, self.task.weak());
        TimerFuture::new(end_tick, timer, self.world)
    }

    /// Updates status
    pub async fn go_to<S: Status + 'static>(
        &self,
        pos: WorldPoint,
        speed: NormalizedFloat,
        goal: SearchGoal,
        status: GoingToStatus<S>,
    ) -> Result<(), GotoError> {
        GoToSubactivity::new(self)
            .go_to(pos, speed, goal, status)
            .await
    }

    pub fn clear_path(&self) {
        if let Ok(mut comp) = self.world.component_mut::<FollowPathComponent>(self.entity) {
            comp.clear_path();
        }
    }

    /// Must be close enough
    pub async fn break_block(&self, block: WorldPosition) -> Result<(), BreakBlockError> {
        BreakBlockSubactivity.break_block(self, block).await
    }

    /// Must be close enough
    pub async fn build_block(
        &self,
        job: SocietyJobHandle,
        details: &BuildDetails,
    ) -> Result<(), BuildBlockError> {
        BuildBlockSubactivity.build_block(self, job, details).await
    }

    /// Pick up item off the ground, checks if close enough first
    pub async fn pick_up(&self, item: Entity) -> Result<(), EquipItemError> {
        PickupSubactivity.pick_up(self, item).await
    }

    /// Equip item that's already in inventory
    pub async fn equip(&self, item: Entity, extra_hands: u16) -> Result<(), EquipItemError> {
        EquipSubActivity.equip(self, item, extra_hands).await
    }

    /// Item should already be equipped
    pub async fn eat(&self, item: Entity) -> Result<(), EatItemError> {
        EatItemSubactivity.eat(self, item).await
    }

    /// Picks up thing for hauling, checks if close enough first
    pub async fn haul(
        &self,
        thing: Entity,
        source: HaulSource,
    ) -> Result<HaulSubactivity<'_>, HaulError> {
        HaulSubactivity::start_hauling(self, thing, source).await
    }

    pub fn check_entity_distance(&self, entity: Entity, max_dist_2: f32) -> DistanceCheckResult {
        let transforms = self.world().read_storage::<TransformComponent>();
        let my_pos = transforms.get(self.entity().into());
        let entity_pos = transforms.get(entity.into());

        if let Some((me, entity)) = my_pos.zip(entity_pos) {
            if me.position.distance2(entity.position) < max_dist_2 {
                DistanceCheckResult::InRange
            } else {
                DistanceCheckResult::TooFar
            }
        } else {
            DistanceCheckResult::NotAvailable
        }
    }

    /// Prefer using other helpers than direct event subscription e.g. [go_to].
    ///
    /// Subscribes to the given subscriptions, runs the filter against each event until it returns
    /// false, then unsubscribes from the given event
    pub async fn subscribe_to_many_until(
        &self,
        subject: Entity,
        subscriptions: impl Iterator<Item = EntityEventType>,
        mut filter: impl FnMut(EntityEventPayload) -> EventResult,
    ) {
        let subscriptions = subscriptions
            .map(|ty| EntityEventSubscription {
                subject,
                subscription: EventSubscription::Specific(ty),
            })
            .collect::<SmallVec<[_; 4]>>();

        // register subscription
        let evts = self.world.resource_mut::<EntityEventQueue>();
        evts.subscribe(self.entity, subscriptions.iter().copied());

        self.consume_events(|mut evt| {
            if evt.subject == subject {
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
        for sub in subscriptions {
            evts.unsubscribe(self.entity, sub);
        }
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

    /// Common pattern using [subscribe_to_specific] that waits for a single event type, and returns
    /// the payload on success.
    pub async fn subscribe_to_specific_until<T>(
        &self,
        subject: Entity,
        event_type: EntityEventType,
        mut filter_map: impl FnMut(EntityEventPayload) -> Result<T, EntityEventPayload>,
    ) -> Option<T> {
        let sub = EntityEventSubscription {
            subject,
            subscription: EventSubscription::Specific(event_type),
        };

        let mut final_result = None;
        self.subscribe_to_until(sub, |evt| {
            // TODO possible to compare std::mem::discriminants instead of converting to evt type enum?

            match filter_map(evt) {
                Ok(result) => {
                    final_result = Some(result);
                    Consumed
                }
                Err(evt) => Unconsumed(evt),
            }
        })
        .await;

        final_result
    }

    /// Hangs in event loop running filter against all of them until consumed is returned
    pub async fn consume_events(&self, mut filter: impl FnMut(EntityEvent) -> GenericEventResult) {
        loop {
            let evt = self.next_event().await;
            let evt = match filter(evt) {
                GenericEventResult::Consumed => break,
                GenericEventResult::Unconsumed(returned_evt) => returned_evt,
            };

            // event is unconsumed
            trace!("handling unhandled event"; "event" => ?evt);
            match self.activity.on_unhandled_event(evt, self.entity) {
                InterruptResult::Continue => continue,
                InterruptResult::Cancel => {
                    trace!("handler requested cancel");
                    break;
                }
            }
        }
    }

    /// Must manually unsubscribe or wait until activity end
    pub fn subscribe_to(&self, subscription: EntityEventSubscription) {
        let evts = self.world.resource_mut::<EntityEventQueue>();
        evts.subscribe(self.entity, once(subscription));
    }

    async fn next_event(&self) -> EntityEvent {
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
                    self.task.park_until_triggered().await;

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
