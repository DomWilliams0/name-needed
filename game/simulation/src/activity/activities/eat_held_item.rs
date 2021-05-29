use common::*;

use crate::activity::activity::{
    ActivityEventContext, ActivityFinish, ActivityResult, SubActivity,
};
use crate::activity::subactivities::{ItemEatSubActivity, ItemEquipSubActivity};
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::{Entity, E};
use crate::event::{EntityEvent, EntityEventPayload};
use crate::item::EdibleItemComponent;
use crate::unexpected_event;
use crate::{nop_subactivity, ComponentWorld};

#[derive(Debug)]
pub struct EatHeldItemActivity {
    item: Entity,
    state: EatHeldItemState,
    finished: Option<BoxedResult<()>>,
}

#[derive(Clone, Debug, Error)]
pub enum EatHeldItemError {
    #[error("Item is not edible")]
    NotEdible,

    #[error("Eater is invalid")]
    InvalidEater,
}

#[derive(Debug)]
enum EatHeldItemState {
    Init,
    Equipping(ItemEquipSubActivity),
    Eating(ItemEatSubActivity),
}

impl<W: ComponentWorld> Activity<W> for EatHeldItemActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        if let Some(result) = self.finished.take() {
            return ActivityResult::from(result);
        }

        match &mut self.state {
            EatHeldItemState::Init => {
                // ensure enough hands to eat it
                let extra_hands = match ctx.world.component::<EdibleItemComponent>(self.item) {
                    Ok(comp) => comp.extra_hands,
                    Err(_) => return ActivityResult::errored(EatHeldItemError::NotEdible),
                };

                let sub = ItemEquipSubActivity {
                    item: self.item,
                    extra_hands,
                };
                match sub.init(ctx) {
                    ActivityResult::Finished(ActivityFinish::Success) => {
                        trace!("item is already equipped, using immediately");

                        let sub = ItemEatSubActivity(self.item);
                        let result = sub.init(ctx);
                        self.state = EatHeldItemState::Eating(sub);
                        result
                    }
                    res => {
                        self.state = EatHeldItemState::Equipping(sub);
                        res
                    }
                }
            }
            EatHeldItemState::Eating(sub) => sub.init(ctx),
            _ => todo!(),
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        _: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        debug_assert_eq!(event.subject, self.item);

        match &event.payload {
            EntityEventPayload::BeenEquipped(result) => {
                match result {
                    Ok(_) => {
                        // TODO sanity check equipper is this entity
                        trace!("equipped food, time to eat");
                        self.state = EatHeldItemState::Eating(ItemEatSubActivity(self.item));
                    }
                    Err(err) => {
                        debug!("failed to equip food"; "error" => %err);
                        self.finished = Some(Err(err.clone().into()));
                    }
                };
                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
                )
            }
            EntityEventPayload::BeenEaten(result) => {
                self.finished = Some(if result.is_ok() {
                    trace!("finished eating food successfully");
                    Ok(())
                } else {
                    trace!("failed to eat");
                    Err(EatHeldItemError::InvalidEater.into())
                });

                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
                )
            }
            e => unexpected_event!(e),
        }
    }

    fn on_finish(&mut self, _: &ActivityFinish, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        match &self.state {
            EatHeldItemState::Equipping(sub) => sub,
            EatHeldItemState::Eating(sub) => sub,
            EatHeldItemState::Init => nop_subactivity!(),
        }
    }
}

impl EatHeldItemActivity {
    pub fn with_item(item: Entity) -> Self {
        Self {
            item,
            state: EatHeldItemState::Init,
            finished: None,
        }
    }
}

impl Display for EatHeldItemActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Eating {}", E(self.item))
    }
}
