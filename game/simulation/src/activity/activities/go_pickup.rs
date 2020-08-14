use crate::activity::activity::{ActivityEventContext, ActivityResult, Finish, SubActivity};
use crate::activity::subactivities::{GoToSubActivity, PickupItemSubActivity, ThinkingSubActivity};
use crate::activity::{
    Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult, NopActivity,
};
use crate::ecs::Entity;
use crate::event::{EntityEvent, EntityEventPayload, EventSubscription};
use crate::{ComponentWorld, TransformComponent};
use common::*;
use unit::world::WorldPoint;

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
}

impl<W: ComponentWorld> Activity<W> for PickupItemsActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        // try to update state
        match &self.state {
            PickupItemsState::Undecided => {
                // choose a new item to pickup
                if let Some((_, (item, pos))) = self.best_item(ctx.world) {
                    // go to the item
                    let goto = GoToSubActivity::new(pos, NormalizedFloat::new(0.8));
                    self.state = PickupItemsState::GoingTo(item, goto.clone());

                    // subscribe to anything happening to the item too
                    ctx.subscribe_to(item, EventSubscription::All);

                    goto.init(ctx)
                } else {
                    // no more items left, we're done
                    debug_assert!(self.items.is_empty(), "should have exhausted all items");
                    ActivityResult::Finished(Finish::Success)
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
            EntityEventPayload::Arrived(_) if event.subject == ctx.subscriber => {
                // we have arrived at our item, change state and start the pickup in the next tick
                match self.state {
                    PickupItemsState::GoingTo(item, _) => {
                        self.state = PickupItemsState::PickingUp(PickupItemSubActivity(item));
                        return (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::UnsubscribeAll,
                        );
                    }
                    ref e => unreachable!("should only receive arrival event while going to item, but is in state {:?}", e),
                }
            }
            EntityEventPayload::PickedUp(result) => {
                // our item has been picked up, who was it?
                return match (&self.state, result) {
                    (PickupItemsState::PickingUp(pickup), Ok(picked_up))
                        if pickup.0 == *picked_up =>
                    {
                        // oh hey it was us, pickup complete!
                        self.state = PickupItemsState::Complete;
                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::UnsubscribeAll,
                        )
                    }
                    _ => {
                        // something else happened, rip to this attempt. try again next tick
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
        }
    }

    fn best_item<W: ComponentWorld>(&mut self, world: &W) -> Option<(usize, (Entity, WorldPoint))> {
        let voxel_ref = world.voxel_world();
        let voxel_world = voxel_ref.borrow();

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

        new_best_index.map(|idx| {
            // any items after idx are to be discarded
            self.items.truncate(idx + 1);

            // safety: index returned from rposition
            let item = unsafe { *self.items.get_unchecked(idx) };
            (idx, item)
        })
    }
}

impl Display for PickupItemsActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Picking up {}", self.item_desc)
    }
}
