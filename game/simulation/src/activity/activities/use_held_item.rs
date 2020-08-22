use common::*;

use crate::activity::activity::{ActivityEventContext, ActivityResult, Finish, SubActivity};
use crate::activity::subactivities::{ItemEquipSubActivity, ItemUseSubActivity};
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::Entity;
use crate::event::{EntityEvent, EntityEventPayload};
use crate::item::{ItemReference, LooseItemReference};
use crate::unexpected_event;
use crate::ComponentWorld;

// TODO str to describe item, and pass through to subactivities
#[derive(Debug)]
pub struct UseHeldItemActivity {
    item: Entity,
    state: UseHeldItemState,
    finished: Option<BoxedResult<()>>,
}

#[derive(Debug)]
enum UseHeldItemState {
    Equipping(ItemEquipSubActivity),
    Using(ItemUseSubActivity),
}

impl<W: ComponentWorld> Activity<W> for UseHeldItemActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        if let Some(result) = self.finished.take() {
            let finish = match result {
                Ok(_) => Finish::Success,
                Err(e) => Finish::Failure(e),
            };

            return ActivityResult::Finished(finish);
        }

        match &mut self.state {
            UseHeldItemState::Equipping(sub) => match sub.init(ctx) {
                ActivityResult::Finished(Finish::Success) => {
                    trace!("item is already equipped, using immediately");

                    let sub = ItemUseSubActivity::new(self.item, sub.slot());
                    let result = sub.init(ctx);
                    self.state = UseHeldItemState::Using(sub);
                    result
                }
                res => res,
            },
            UseHeldItemState::Using(sub) => sub.init(ctx),
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        _: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        debug_assert_eq!(event.subject, self.item);

        match &event.payload {
            EntityEventPayload::Equipped(result) => match result {
                Ok(slot) => {
                    trace!("equipped item successfully"; "slot" => ?slot);

                    // upgrade state to using equipped item
                    self.state = UseHeldItemState::Using(ItemUseSubActivity::new(
                        self.item,
                        slot.to_owned(),
                    ));
                    (
                        EventUnblockResult::Unblock,
                        EventUnsubscribeResult::UnsubscribeAll,
                    )
                }

                Err(err) => {
                    debug!("failed to equip item"; "error" => %err);
                    self.finished = Some(Err(Box::new(err.to_owned())));
                    (
                        EventUnblockResult::Unblock,
                        EventUnsubscribeResult::UnsubscribeAll,
                    )
                }
            },

            EntityEventPayload::UsedUp(result) => {
                trace!("item is used up");
                debug_assert!(matches!(self.state, UseHeldItemState::Using(_)));

                let result = result.to_owned().map_err(|e| Box::new(e) as Box<dyn Error>);
                self.finished = Some(result);
                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
                )
            }
            e => unexpected_event!(e),
        }
    }

    fn on_finish(&mut self, _: Finish, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        match &self.state {
            UseHeldItemState::Equipping(sub) => sub,
            UseHeldItemState::Using(sub) => sub,
        }
    }
}

impl UseHeldItemActivity {
    pub fn with_item(item: LooseItemReference) -> Self {
        let LooseItemReference(ItemReference(slot, item)) = item;
        Self {
            item,
            state: UseHeldItemState::Equipping(ItemEquipSubActivity::new(slot, item)),
            finished: None,
        }
    }
}

impl Display for UseHeldItemActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Using held item")
    }
}
