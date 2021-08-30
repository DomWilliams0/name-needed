use common::*;

use crate::{ComponentWorld, EcsWorld};

use crate::ecs::Entity;
use crate::event::{
    EntityEvent, EntityEventSubscription, EventSubscription, RuntimeTimers, TimerToken,
};
use crate::path::FollowPathComponent;
use crate::queued_update::QueuedUpdates;

pub enum ActivityResult {
    Ongoing,
    /// `ctx.subscriptions` expected to be populated
    Blocked,
    Finished(ActivityFinish),
}

#[derive(Debug)]
pub enum EventUnblockResult {
    Unblock,
    KeepBlocking,
}

#[derive(Debug)]
pub enum EventUnsubscribeResult {
    UnsubscribeAll,
    Unsubscribe(EntityEventSubscription),
    StaySubscribed,
}

#[derive(Debug)]
pub enum ActivityFinish {
    Success,
    Failure(Box<dyn Error>),
    Interrupted,
}

pub struct ActivityContext<'a> {
    pub entity: Entity,
    /// Immutable getters only! Use lazy_updates for adding/removing components
    pub world: &'a EcsWorld,
    pub updates: &'a QueuedUpdates,
    pub subscriptions: &'a mut Vec<EntityEventSubscription>,
}

pub struct ActivityEventContext {
    pub subscriber: Entity,
}

#[derive(Error, Debug)]
#[error("Both activity and subactivity failed to finish. Activity: {activity}. Subactivity: {subactivity}")]
pub struct ColossalActivityFailure {
    activity: Box<dyn Error>,
    subactivity: Box<dyn Error>,
}

#[macro_export]
macro_rules! unexpected_event {
    ($event:expr) => {
        {
            debug!("ignoring unexpected event"; "event" => ?$event);
            (
                EventUnblockResult::KeepBlocking,
                EventUnsubscribeResult::StaySubscribed,
            )
        }
    };
}

#[macro_export]
macro_rules! unexpected_event2 {
    ($event:expr) => {{
        trace!("ignoring unexpected event"; "event" => ?$event);
        $crate::activity::activity2::EventResult::Unconsumed($event)
    }};
}

pub trait Activity: Display + Debug {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_>) -> ActivityResult;

    #[allow(unused_variables)]
    fn on_event(
        &mut self,
        event: &EntityEvent,
        ctx: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        // must be subscribed to an event to get here
        unreachable!("unexpected event {:?}", event);
    }

    fn on_finish(&mut self, finish: &ActivityFinish, ctx: &mut ActivityContext) -> BoxedResult<()>;

    // ---
    fn current_subactivity(&self) -> &dyn SubActivity;

    /// Calls on_finish on both activity and sub activity
    fn finish(&mut self, finish: &ActivityFinish, ctx: &mut ActivityContext) -> BoxedResult<()> {
        let a = self.current_subactivity().on_finish(finish, ctx);
        let b = self.on_finish(finish, ctx);

        match (a, b) {
            (err @ Err(_), Ok(_)) | (Ok(_), err @ Err(_)) => err,
            (Err(subactivity), Err(activity)) => Err(ColossalActivityFailure {
                activity,
                subactivity,
            }
            .into()),
            _ => Ok(()), // both ok
        }
    }
}

pub trait SubActivity: Display {
    fn init(&self, ctx: &mut ActivityContext) -> ActivityResult;
    fn on_finish(&self, finish: &ActivityFinish, ctx: &mut ActivityContext) -> BoxedResult<()>;

    fn exertion(&self) -> f32;
}

impl<'a> ActivityContext<'a> {
    pub fn subscribe_to(&mut self, subject_entity: Entity, subscription: EventSubscription) {
        self.subscriptions.push(EntityEventSubscription {
            subject: subject_entity,
            subscription: subscription,
        });
    }

    pub fn clear_path(&self) {
        if let Ok(comp) = self.world.component_mut::<FollowPathComponent>(self.entity) {
            comp.clear_path();
        }
    }

    #[deprecated]
    pub fn schedule_timer(&self, count: u32, subject: Entity) -> TimerToken {
        unreachable!()
        // self.world
        //     .resource_mut::<RuntimeTimers>()
        //     .schedule(count, subject)
    }
}

impl ActivityResult {
    pub fn errored<E: Error + 'static>(err: E) -> Self {
        Self::Finished(ActivityFinish::Failure(Box::new(err)))
    }
}

impl From<BoxedResult<()>> for ActivityResult {
    fn from(res: BoxedResult<()>) -> Self {
        let finish = match res {
            Ok(_) => ActivityFinish::Success,
            Err(err) => ActivityFinish::Failure(err),
        };

        Self::Finished(finish)
    }
}

// impl <A> slog::Value  for A where A: Activity<_> {
//     fn serialize(&self, _: &Record, key: &'static str, serializer: &mut dyn Serializer) -> SlogResult<()> {
//     }
// }

impl slog::Value for dyn Activity {
    fn serialize(
        &self,
        _: &Record,
        key: &'static str,
        serializer: &mut dyn Serializer,
    ) -> SlogResult<()> {
        serializer.emit_arguments(key, &format_args!("{:?}", self))
    }
}
