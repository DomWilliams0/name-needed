pub use activities::*;
pub use activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
// TODO move subactivity errors somewhere else
pub use event_logging::{EntityLoggingComponent, LoggedEntityDecision, LoggedEntityEvent};
pub use subactivities::{EquipItemError, HaulError, PickupItemError};
pub use system::{
    ActivityComponent, ActivityEventSystem, ActivitySystem, BlockingActivityComponent,
};

mod activities;
mod activity;
mod event_logging;
mod subactivities;
mod system;

mod action_to_activity {
    use crate::activity::Activity;
    use crate::ai::AiAction;
    use crate::item::ItemsToPickUp;
    use crate::ComponentWorld;

    use super::*;

    impl AiAction {
        pub fn into_activity<W: ComponentWorld>(self, activity: &mut Box<dyn Activity<W>>) {
            macro_rules! activity {
                ($act:expr) => {
                    Box::new($act) as Box<dyn Activity<W>>
                };
            }

            *activity = match self {
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
