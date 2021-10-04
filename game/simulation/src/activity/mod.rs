pub use activity::*;
pub use event_logging::{EntityLoggingComponent, LoggedEntityDecision, LoggedEntityEvent};
pub use status::{StatusReceiver, StatusUpdater};
pub use subactivity::{EquipItemError, HaulError, HaulTarget};
pub use system::{ActivityComponent, ActivitySystem};

mod activity;
mod context;
mod event_logging;
mod status;
mod subactivity;
mod system;

mod action_to_activity {
    use crate::activity::activity::Activity;
    use crate::ai::AiAction;

    use super::*;
    use common::NormalizedFloat;
    use std::rc::Rc;
    use world::SearchGoal;

    impl AiAction {
        pub fn into_activity(self) -> Rc<dyn Activity> {
            macro_rules! activity {
                ($act:expr) => {
                    Rc::new($act) as Rc<dyn Activity>
                };
            }

            match self {
                AiAction::Wander => activity!(WanderActivity::default()),
                AiAction::Nop => activity!(NopActivity::default()),
                AiAction::GoBreakBlock(pos) => activity!(GoBreakBlockActivity::new(pos)),
                AiAction::GoEquip(e) => activity!(GoEquipActivity::new(e)),
                AiAction::EatHeldItem(item) => activity!(EatHeldItemActivity::new(item)),
                AiAction::Goto(target) => activity!(GoToActivity::new(
                    target,
                    NormalizedFloat::new(0.8),
                    SearchGoal::Arrive
                )),
                AiAction::Follow { target, radius } => {
                    activity!(FollowActivity::new(target, radius))
                }
                AiAction::Haul(thing, source, target) => {
                    activity!(GoHaulActivity::new(thing, source, target))
                }
            }
        }
    }
}
