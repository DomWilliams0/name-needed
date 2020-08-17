use common::*;
use unit::world::WorldPoint;
use world::NavigationError;

use crate::activity::activity::{ActivityEventContext, ActivityResult, Finish, SubActivity};
use crate::activity::subactivities::{GoToSubActivity, PickupItemSubActivity};
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::{Entity, E};
use crate::event::{
    EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType, EventSubscription,
};
use crate::item::PickupItemError;
use crate::{nop_subactivity, unexpected_event};
use crate::{ComponentWorld, TransformComponent};

#[derive(Debug)]
enum PickupItemsState {
    Undecided,
    GoingTo(Entity, GoToSubActivity),
    PickingUp(PickupItemSubActivity),
}

#[derive(Debug)]
pub struct PickupItemsActivity {
    items: Vec<(Entity, WorldPoint)>,
    item_desc: &'static str,
    state: PickupItemsState,
    last_error: Option<PickupFailure>,
    complete: bool,
}

enum BestItem {
    Excellent {
        // index: usize,
        item: Entity,
        pos: WorldPoint,
    },
    NoneLeft(Finish),
}

#[derive(Debug)]
enum PickupFailure {
    PickupError(PickupItemError),
    NavigationError(NavigationError),
    Other,
}

impl<W: ComponentWorld> Activity<W> for PickupItemsActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        if self.complete {
            return ActivityResult::Finished(Finish::Success);
        }

        // try to update state
        match &self.state {
            PickupItemsState::Undecided => {
                // choose a new item to pickup
                match self.best_item(ctx.world) {
                    BestItem::Excellent { item, pos, .. } => {
                        my_trace!("new best item chosen"; "item" => E(item), "position" => %pos);

                        // subscribe to anything happening to the item
                        ctx.subscribe_to(item, EventSubscription::All);

                        // go to the item and subscribe to arrival
                        let goto = GoToSubActivity::new(pos, NormalizedFloat::new(0.8));
                        let result = goto.init(ctx);

                        // update state
                        self.state = PickupItemsState::GoingTo(item, goto);
                        result
                    }
                    BestItem::NoneLeft(finish) => {
                        // no more items left, we're done
                        my_trace!("no more items left"; "finish" => ?finish);
                        ActivityResult::Finished(finish)
                    }
                }
            }
            PickupItemsState::PickingUp(sub) => {
                // delegate to pick up subactivity
                sub.init(ctx)
            }
            PickupItemsState::GoingTo(_, _) => unreachable!("should be blocked until arrival"),
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        ctx: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        match &event.payload {
            EntityEventPayload::Arrived(token, result) => {
                debug_assert_eq!(event.subject, ctx.subscriber);

                // we have arrived at our item, change state and start the pickup in the next tick
                match &self.state {
                    PickupItemsState::GoingTo(item, sub) => {
                        // wrong token
                        if sub.token() != *token {
                            my_trace!("got arrival event for different token than expected, continuing to wait";
                                "expected" => ?sub.token(), "actual" => ?token
                            );

                            return (
                                EventUnblockResult::KeepBlocking,
                                EventUnsubscribeResult::StaySubscribed
                            );
                        }

                        // navigation error
                        if let Err(e) = result {
                            my_debug!("failed to navigate to item"; "error" => %e);
                            self.last_error = Some(PickupFailure::NavigationError(e.to_owned()));
                            self.state = PickupItemsState::Undecided;
                            return (
                                EventUnblockResult::Unblock,
                                EventUnsubscribeResult::UnsubscribeAll,
                            );
                        }

                        my_trace!("arrived at item, pick up next tick");
                        self.state = PickupItemsState::PickingUp(PickupItemSubActivity(*item));

                        // unsubscribe from our arrival but stay subscribed to item events
                        let unsubscribe = EntityEventSubscription(ctx.subscriber, EventSubscription::Specific(EntityEventType::Arrived));

                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::Unsubscribe(unsubscribe),
                        )
                    }
                    ref e => unreachable!("should only receive arrival event while going to item, but is in state {:?}", e),
                }
            }
            EntityEventPayload::PickedUp(result) => {
                // our item has been picked up, who was it?
                match (&self.state, result) {
                    (PickupItemsState::PickingUp(pickup), Ok((item, picker_upper)))
                        if *picker_upper == ctx.subscriber =>
                    {
                        debug_assert_eq!(*item, pickup.0);

                        // oh hey it was us, pickup complete!
                        my_trace!("completed pick up");
                        self.complete = true;
                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::UnsubscribeAll,
                        )
                    }
                    (_, err) => {
                        // something else happened, rip to this attempt. try again next tick
                        my_trace!("something happened to the item before we could pick it up"; "result" => ?err);

                        self.last_error = Some(if let Err(e) = err {
                            PickupFailure::PickupError(e.to_owned())
                        } else {
                            PickupFailure::Other
                        });

                        // TODO detect other destructive events e.g. entity removal
                        self.state = PickupItemsState::Undecided;
                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::UnsubscribeAll,
                        )
                    }
                }
            }
            e => unexpected_event!(e),
        }
    }

    fn on_finish(&mut self, _: Finish, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        match &self.state {
            PickupItemsState::GoingTo(_, sub) => sub,
            PickupItemsState::PickingUp(sub) => sub,
            PickupItemsState::Undecided => nop_subactivity!(),
        }
    }
}

impl PickupItemsActivity {
    pub fn with_items(items: Vec<(Entity, WorldPoint)>, what: &'static str) -> Self {
        Self {
            items,
            item_desc: what,
            state: PickupItemsState::Undecided,
            last_error: None,
            complete: false,
        }
    }

    fn best_item<W: ComponentWorld>(&mut self, world: &W) -> BestItem {
        let voxel_ref = world.voxel_world();
        let voxel_world = voxel_ref.borrow();

        let err = self.last_error.take();
        if let Some(err) = err.as_ref() {
            let last = self.items.pop();
            if let Some((last, _)) = last {
                my_debug!(
                    "removed last best item due to pickup failure";
                    "item" => E(last), "error" => ?err
                );
            } else {
                my_debug!(
                    "pickup failure occurred picking up the last candidate item";
                    "error" => ?err
                );
            }
        }

        // choose the best item that still exists
        let new_best_index = self.items.iter().rposition(|(item, known_pos)| {
            match world
                .component::<TransformComponent>(*item)
                .ok()
                .and_then(|transform| {
                    // still got a transform
                    voxel_world.area_for_point(transform.position)
                }) {
                Some((current_pos, _)) if current_pos == known_pos.floor() => {
                    // this item is good to path find to and still in the same place we expect
                    true
                }
                _ => false, // move onto next item because this one is not accessible anymore
            }
        });

        match (new_best_index, err) {
            (Some(idx), _) => {
                // any items after idx are to be discarded
                self.items.truncate(idx + 1);

                // safety: index returned from rposition
                let (item, pos) = unsafe { *self.items.get_unchecked(idx) };
                BestItem::Excellent { item, pos }
            }

            (None, Some(err)) => {
                let err: Box<dyn Error> = match err {
                    PickupFailure::PickupError(e) => Box::new(e),
                    PickupFailure::NavigationError(e) => Box::new(e),
                    PickupFailure::Other => Box::new(PickupItemError::NoLongerAvailable),
                };

                BestItem::NoneLeft(Finish::Failure(err))
            }
            (None, None) => BestItem::NoneLeft(Finish::Success),
        }
    }
}

impl Display for PickupItemsActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Picking up {}", self.item_desc)
    }
}
