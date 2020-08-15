mod activities;
mod activity;
mod subactivities;
mod system;

pub use activities::*;
pub use activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
pub use system::{
    ActivityComponent, ActivityEventSystem, ActivitySystem, BlockingActivityComponent,
};
// TODO move subactivity errors somewhere else
pub use subactivities::{EquipItemError, UseHeldItemError};

mod action_to_activity {
    use super::*;
    use crate::activity::Activity;
    use crate::ai::{AiAction, ItemsToPickUp};
    use crate::ComponentWorld;

    impl AiAction {
        pub fn into_activity<W: ComponentWorld>(self, activity: &mut Box<dyn Activity<W>>) {
            macro_rules! activity {
                ($act:expr) => {
                    Box::new($act) as Box<dyn Activity<W>>
                };
            }

            *activity = match self {
                AiAction::Nop => activity!(NopActivity),
                AiAction::Goto { target, reason } => activity!(GoToActivity::new(target, reason)),
                AiAction::GoPickUp(ItemsToPickUp(_, items)) => {
                    // TODO itemfilter should specify a static string describing itself
                    activity!(PickupItemsActivity::with_items(items, "iTeMs"))
                }
                AiAction::Wander => activity!(WanderActivity),
                AiAction::UseHeldItem(item) => activity!(UseHeldItemActivity::with_item(item)),
                AiAction::GoBreakBlock(pos) => activity!(GoBreakBlockActivity::new(pos)),
            }
        }
    }
}
