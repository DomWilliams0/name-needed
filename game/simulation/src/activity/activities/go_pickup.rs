use common::*;
use unit::world::WorldPoint;

use crate::activity::activity::{ActivityEventContext, ActivityResult, Finish, SubActivity};
use crate::activity::subactivities::{GoToSubActivity, PickupItemSubActivity, ThinkingSubActivity};
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::Entity;
use crate::event::{
    EntityEvent, EntityEventPayload, EntityEventSubscription, EntityEventType, EventSubscription,
};
use crate::item::PickupItemError;
use crate::{ComponentWorld, TransformComponent};

#[derive(Debug)]
enum PickupItemsState {
    Undecided,
    GoingTo(Entity, GoToSubActivity),
    PickingUp(PickupItemSubActivity),
    Complete,
}

pub struct PickupItemsActivity {
    items: Vec<(Entity, WorldPoint)>,
    item_desc: &'static str,
    state: PickupItemsState,
    last_error: Option<PickupFailure>,
}

enum BestItem {
    Excellent {
        index: usize,
        item: Entity,
        pos: WorldPoint,
    },
    NoneLeft(Finish),
}

#[derive(Debug)]
enum PickupFailure {
    Error(PickupItemError),
    Other,
}

impl<W: ComponentWorld> Activity<W> for PickupItemsActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        // try to update state
        match &self.state {
            PickupItemsState::Undecided => {
                // choose a new item to pickup
                match self.best_item(ctx.world) {
                    BestItem::Excellent { item, pos, .. } => {
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
                        ActivityResult::Finished(finish)
                    }
                }
            }
            PickupItemsState::PickingUp(sub) => {
                // delegate to pick up subactivity
                sub.init(ctx)
            }
            PickupItemsState::GoingTo(_, _) => unreachable!("should be blocked until arrival"),
            PickupItemsState::Complete => ActivityResult::Finished(Finish::Success),
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        ctx: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        match &event.payload {
            EntityEventPayload::Arrived(_) => {
                debug_assert_eq!(event.subject, ctx.subscriber);

                // we have arrived at our item, change state and start the pickup in the next tick
                match self.state {
                    PickupItemsState::GoingTo(item, _) => {
                        self.state = PickupItemsState::PickingUp(PickupItemSubActivity(item));

                        // unsubscribe from our arrival but stay subscribed to item events
                        let unsubscribe = EntityEventSubscription(ctx.subscriber, EventSubscription::Specific(EntityEventType::Arrived));

                        return (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::Unsubscribe(unsubscribe),
                        );
                    }
                    ref e => unreachable!("should only receive arrival event while going to item, but is in state {:?}", e),
                }
            }
            EntityEventPayload::PickedUp(result) => {
                // our item has been picked up, who was it?
                return match (&self.state, result) {
                    (PickupItemsState::PickingUp(pickup), Ok((item, picker_upper)))
                        if *picker_upper == ctx.subscriber =>
                    {
                        debug_assert_eq!(*item, pickup.0);

                        // oh hey it was us, pickup complete!
                        self.state = PickupItemsState::Complete;
                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::UnsubscribeAll,
                        )
                    }
                    (_, err) => {
                        // something else happened, rip to this attempt. try again next tick

                        self.last_error = Some(if let Err(e) = err {
                            debug!("failed to pickup item: {}", e);
                            PickupFailure::Error(e.to_owned())
                        } else {
                            debug!("aborting item pickup");
                            PickupFailure::Other
                        });

                        // TODO detect other destructive events e.g. entity removal
                        self.state = PickupItemsState::Undecided;
                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::UnsubscribeAll,
                        )
                    }
                };
            }
            _ => {
                // unknown event
                debug!("ignoring event {:?}", event);
            }
        };

        (
            EventUnblockResult::KeepBlocking,
            EventUnsubscribeResult::StaySubscribed,
        )
    }

    fn on_finish(&mut self, _: Finish, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        match &self.state {
            PickupItemsState::GoingTo(_, sub) => sub,
            PickupItemsState::PickingUp(sub) => sub,
            _ => &ThinkingSubActivity, // intermediate states
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
        }
    }

    fn best_item<W: ComponentWorld>(&mut self, world: &W) -> BestItem {
        let voxel_ref = world.voxel_world();
        let voxel_world = voxel_ref.borrow();

        let err = self.last_error.take();
        if let Some(err) = err.as_ref() {
            let last = self.items.pop();
            debug!(
                "removed last best item {:?} due to pickup failure: {:?}",
                last, err
            );
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
                BestItem::Excellent {
                    index: idx,
                    item,
                    pos,
                }
            }

            (None, Some(err)) => {
                let err = match err {
                    PickupFailure::Error(e) => e,
                    PickupFailure::Other => PickupItemError::NoLongerAvailable,
                };

                BestItem::NoneLeft(Finish::Failure(Box::new(err)))
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
