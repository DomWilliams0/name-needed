pub use activities::*;
pub(crate) use activity::ActivityFinish;
pub use activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
// TODO move subactivity errors somewhere else
pub use activities2::*;
pub use event_logging::{EntityLoggingComponent, LoggedEntityDecision, LoggedEntityEvent};
pub use status::{StatusReceiver, StatusUpdater};
pub use subactivities2::{EquipItemError, HaulError, HaulTarget};
pub use system::{
    ActivityComponent, ActivityEventSystem, ActivitySystem, BlockingActivityComponent,
};
pub use system2::{ActivityComponent2, ActivitySystem2};

#[deprecated]
mod activities;
mod activities2;
#[deprecated]
mod activity;
mod activity2;
mod event_logging;
mod status;
#[deprecated]
mod subactivities;
mod subactivities2;
#[deprecated]
mod system;
mod system2;

mod action_to_activity {
    use crate::activity::activity2::Activity2;
    use crate::activity::Activity;
    use crate::ai::AiAction;
    use crate::item::ItemsToPickUp;

    use super::*;
    use common::NormalizedFloat;
    use std::rc::Rc;
    use world::SearchGoal;

    impl AiAction {
        pub fn into_activity2(self) -> Rc<dyn Activity2> {
            macro_rules! activity {
                ($act:expr) => {
                    Rc::new($act) as Rc<dyn Activity2>
                };
            }

            match self {
                AiAction::Wander => activity!(WanderActivity2::default()),
                AiAction::Nop => activity!(NopActivity2::default()),
                AiAction::GoBreakBlock(pos) => activity!(GoBreakBlockActivity2::new(pos)),
                AiAction::GoEquip(e) => activity!(GoEquipActivity2::new(e)),
                AiAction::EatHeldItem(item) => activity!(EatHeldItemActivity2::new(item)),
                AiAction::Goto(target) => activity!(GoToActivity2::new(
                    target,
                    NormalizedFloat::new(0.8),
                    SearchGoal::Arrive
                )),
                AiAction::Follow { target, radius } => {
                    activity!(FollowActivity2::new(target, radius))
                }
                AiAction::Haul(thing, source, target) => {
                    activity!(GoHaulActivity2::new(thing, source, target))
                }
                AiAction::GoPickUp(_) => unreachable!("replaced by GoEquip"),
            }
        }
        #[deprecated]
        pub fn into_activity(self) -> Box<dyn Activity> {
            macro_rules! activity {
                ($act:expr) => {
                    Box::new($act) as Box<dyn Activity>
                };
            }

            unreachable!()
        }
    }
}
