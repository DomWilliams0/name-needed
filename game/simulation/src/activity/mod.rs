pub use activities::*;
pub(crate) use activity::ActivityFinish;
pub use activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
// TODO move subactivity errors somewhere else
pub use activities2::*;
pub use event_logging::{EntityLoggingComponent, LoggedEntityDecision, LoggedEntityEvent};
pub use subactivities::{EquipItemError, HaulError, PickupItemError};
pub use system::{
    ActivityComponent, ActivityEventSystem, ActivitySystem, BlockingActivityComponent,
};
pub use system2::{ActivityComponent2, ActivitySystem2};

#[deprecated]
mod activities;
mod activities2;
mod activity;
mod activity2;
mod event_logging;
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

    impl AiAction {
        pub fn into_activity2(self) -> Box<dyn Activity2> {
            macro_rules! activity {
                ($act:expr) => {
                    Box::new($act) as Box<dyn Activity2>
                };
            }

            match self {
                AiAction::Wander => activity!(WanderActivity2::default()),
                AiAction::Nop => activity!(NopActivity2::default()),
                _ => unreachable!(),
                // AiAction::Nop => activity!(NopActivity::default()),
                // AiAction::Goto { target, reason } => activity!(GoToActivity::new(target, reason)),
                // AiAction::GoPickUp(ItemsToPickUp(desc, _, items)) => {
                //     activity!(PickupItemsActivity::with_items(items, desc))
                // }
                // AiAction::Wander => activity!(WanderActivity::default()),
                // AiAction::GoBreakBlock(pos) => activity!(GoBreakBlockActivity::new(pos)),
                // AiAction::Follow { target, radius } => {
                //     activity!(FollowActivity::new(target, radius))
                // }
                // AiAction::Haul(thing, source, target) => {
                //     activity!(HaulActivity::new(thing, source, target))
                // }
                // AiAction::EatHeldItem(item) => activity!(EatHeldItemActivity::with_item(item)),
            }
        }
        pub fn into_activity(self) -> Box<dyn Activity> {
            macro_rules! activity {
                ($act:expr) => {
                    Box::new($act) as Box<dyn Activity>
                };
            }

            match self {
                AiAction::Nop => activity!(NopActivity::default()),
                AiAction::Goto { target, reason } => activity!(GoToActivity::new(target, reason)),
                AiAction::GoPickUp(ItemsToPickUp(desc, _, items)) => {
                    activity!(PickupItemsActivity::with_items(items, desc))
                }
                AiAction::Wander => activity!(WanderActivity::default()),
                AiAction::GoBreakBlock(pos) => activity!(GoBreakBlockActivity::new(pos)),
                AiAction::Follow { target, radius } => {
                    activity!(FollowActivity::new(target, radius))
                }
                AiAction::Haul(thing, source, target) => {
                    activity!(HaulActivity::new(thing, source, target))
                }
                AiAction::EatHeldItem(item) => activity!(EatHeldItemActivity::with_item(item)),
            }
        }
    }
}
