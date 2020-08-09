use common::derive_more::Display;
use common::*;
use unit::world::WorldPoint;
use world::SearchGoal;

use crate::ai::AiAction;
use crate::ComponentWorld;

use crate::ecs::Entity;
use crate::event::{
    EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType, EventSubscription,
};
use crate::path::FollowPathComponent;
use crate::queued_update::QueuedUpdates;

// #[derive(Clone)]
pub enum ActivityResult<'a> {
    Ongoing,
    Blocked(&'a mut Vec<EntityEventSubscription>),
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
    pub updates: &'a QueuedUpdates,
    pub subscriptions: &'a mut Vec<EntityEventSubscription>,
}

// TODO display too
pub trait Activity<W: ComponentWorld>: Display {
    // fn on_start(&mut self, ctx: &ActivityContext<W>);
    fn on_finish(&mut self, finish: Finish, ctx: &mut ActivityContext<W>);

    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult<'a>;

    fn on_event(&mut self, _event: &EntityEvent) -> (EventUnblockResult, EventUnsubscribeResult) {
        unreachable!()
    }

    fn exertion(&self) -> f32 {
        0.5 // TODO get from current sub activity
    }
}
// TODO for testing, eventually have a submodule per activity
#[derive(Display)]
#[display("Doing nothing")]
pub struct NopActivity;

// #[derive(Copy, Clone)]
// enum PickupItemsState {
//     Uninit,
//     GoingTo(Entity, WorldPoint),
//     PickingUp(Entity),
// }
// struct PickupItemsActivity {
//     items: Vec<(Entity, WorldPoint)>,
//     state: PickupItemsState,
// }
//
// impl PickupItemsActivity {
//     fn with_items(items: Vec<(Entity, WorldPoint)>) -> Self {
//         Self {
//             items,
//             state: PickupItemsState::Uninit,
//         }
//     }
//
//     fn best_item<W: ComponentWorld>(&mut self, world: &W) -> Option<(usize, (Entity, WorldPoint))> {
//         // TODO
//         None
//     }
// }

#[derive(Copy, Clone, Debug)]
enum GotoThenNopState {
    GoingTo(WorldPoint),
    Done,
}

pub struct GotoThenNop {
    state: GotoThenNopState,
}

impl<W: ComponentWorld> Activity<W> for GotoThenNop {
    fn on_finish(&mut self, _finish: Finish, _ctx: &mut ActivityContext<W>) {
        // TODO remove path here? or is it up to the new activity to cancel path finding if it wants
    }

    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult<'a> {
        match self.state {
            GotoThenNopState::GoingTo(pos) => {
                // trigger go to
                let follow = ctx
                    .world
                    .component_mut::<FollowPathComponent>(ctx.entity)
                    .unwrap();
                follow.new_path(pos, SearchGoal::Arrive, NormalizedFloat::new(0.7));

                // block on arrive event
                // TODO specify entity specifically, either Self or Other(e)
                ctx.subscriptions.push(EntityEventSubscription(
                    ctx.entity,
                    EventSubscription::Specific(EntityEventType::Arrived),
                ));
                ActivityResult::Blocked(ctx.subscriptions)
            }
            GotoThenNopState::Done => {
                // nice
                ActivityResult::Finished(Finish::Succeeded)
            }
        }
    }

    fn on_event(&mut self, event: &EntityEvent) -> (EventUnblockResult, EventUnsubscribeResult) {
        match event.1 {
            EntityEventPayload::Arrived(_) => {
                self.state = GotoThenNopState::Done;
                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
                )
            }
            _ => unreachable!(),
        }
    }
}

impl GotoThenNop {
    pub fn new(pos: WorldPoint) -> Self {
        Self {
            state: GotoThenNopState::GoingTo(pos),
        }
    }
}

impl Display for GotoThenNop {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Going to a place then nop'ing")
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
            // AiAction::Nop => activity!(NopActivity),
            AiAction::Goto(pos) => activity!(GotoThenNop::new(pos)),
            // AiAction::GoPickUp(ItemsToPickUp(_, items)) => {
            //     activity!(PickupItemsActivity::with_items(items))
            // }
            _ => todo!(),
        }
    }
}
impl<W: ComponentWorld> Activity<W> for NopActivity {
    fn on_finish(&mut self, _: Finish, _: &mut ActivityContext<W>) {}

    fn on_tick<'a>(&mut self, _: &'a mut ActivityContext<'_, W>) -> ActivityResult<'a> {
        warn!("ticking nop activity, possible infinite loop");
        ActivityResult::Ongoing
    }
}
