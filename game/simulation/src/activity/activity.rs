use common::*;

use crate::ComponentWorld;

use crate::activity::NopActivity;
use crate::ecs::Entity;
use crate::event::{EntityEvent, EntityEventSubscription, EventSubscription};
use crate::queued_update::QueuedUpdates;
use unit::world::WorldPoint;

pub enum ActivityResult {
    Ongoing,
    /// `ctx.subscriptions` expected to be populated
    Blocked,
    Finished(Finish),
}

#[derive(Debug)]
pub enum EventUnblockResult {
    Unblock,
    KeepBlocking,
}

#[derive(Debug)]
pub enum EventUnsubscribeResult {
    UnsubscribeAll,
    StaySubscribed,
}

#[derive(Debug)]
pub enum Finish {
    Success,
    Failure(Box<dyn Error>),
    Interrupted,
}

pub struct ActivityContext<'a, W: ComponentWorld> {
    pub entity: Entity,
    /// Immutable getters only! Use lazy_updates for adding/removing components
    pub world: &'a W,
    // TODO can queuedupdates be removed from activity context
    pub updates: &'a QueuedUpdates,
    pub subscriptions: &'a mut Vec<EntityEventSubscription>,
}

pub struct ActivityEventContext {
    pub subscriber: Entity,
}

pub trait Activity<W: ComponentWorld>: Display {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult;

    #[allow(unused_variables)]
    fn on_event(
        &mut self,
        event: &EntityEvent,
        ctx: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        // must be subscribed to an event to get here
        unreachable!()
    }

    fn on_finish(&mut self, finish: Finish, ctx: &mut ActivityContext<W>) -> BoxedResult<()>;

    // ---
    fn current_subactivity(&self) -> &dyn SubActivity<W>;

    /// Calls on_finish on both activity and sub activity
    fn finish(&mut self, finish: Finish, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        let a = self.current_subactivity().on_finish(ctx);
        let b = self.on_finish(finish, ctx);

        match (a, b) {
            (err @ Err(_), Ok(_)) | (Ok(_), err @ Err(_)) => err,
            (Err(a), Err(b)) => {
                // pass through activity failure and log subactivity
                error!("failed to finish subactivity as well as activity: {}", a);
                Err(b)
            }
            _ => Ok(()), // both ok
        }
    }
}

pub trait SubActivity<W: ComponentWorld>: Display {
    fn init(&self, ctx: &mut ActivityContext<W>) -> ActivityResult;
    fn on_finish(&self, ctx: &mut ActivityContext<W>) -> BoxedResult<()>;

    fn exertion(&self) -> f32;
}

impl<'a, W: ComponentWorld> ActivityContext<'a, W> {
    pub fn subscribe_to(&mut self, subject_entity: Entity, subscription: EventSubscription) {
        self.subscriptions
            .push(EntityEventSubscription(subject_entity, subscription));
    }
}

impl ActivityResult {
    pub fn errored<E: Error + 'static>(err: E) -> Self {
        Self::Finished(Finish::Failure(Box::new(err)))
    }
}
