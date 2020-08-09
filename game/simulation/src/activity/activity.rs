use common::*;

use crate::ai::AiAction;
use crate::ComponentWorld;

use crate::activity::activities::NopActivity;
use crate::ecs::Entity;
use crate::event::{EntityEvent, EntityEventSubscription};
use crate::queued_update::QueuedUpdates;

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

#[derive(Debug, Copy, Clone)]
pub enum Finish {
    Succeeded,
    Failed,
    // TODO failure/interrupt reason
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

pub trait Activity<W: ComponentWorld>: Display {
    // fn on_start(&mut self, ctx: &ActivityContext<W>);
    fn on_finish(&mut self, finish: Finish, ctx: &mut ActivityContext<W>);

    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult;

    fn on_event(&mut self, _event: &EntityEvent) -> (EventUnblockResult, EventUnsubscribeResult) {
        unreachable!()
    }

    fn exertion(&self) -> f32 {
        0.5 // TODO get from current sub activity
    }
}

impl AiAction {
    pub fn into_activity<W: ComponentWorld>(self, activity: &mut Box<dyn Activity<W>>) {
        macro_rules! activity {
            ($act:expr) => {
                Box::new($act) as Box<dyn Activity<W>>
            };
        }

        *activity = match self {
            AiAction::Nop => activity!(NopActivity),
            // AiAction::Goto(pos) => activity!(GotoThenNop::new(pos)),
            // AiAction::GoPickUp(ItemsToPickUp(_, items)) => {
            //     activity!(PickupItemsActivity::with_items(items))
            // }
            _ => todo!(),
        }
    }
}
