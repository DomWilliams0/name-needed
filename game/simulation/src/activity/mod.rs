pub use activity::*;
pub use event_logging::{EntityLoggingComponent, LoggedEntityDecision, LoggedEntityEvent};
pub use status::{StatusReceiver, StatusUpdater};
pub use subactivity::{EquipItemError, HaulError, HaulPurpose, HaulSource, HaulTarget};
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

            use AiAction::*;

            match self {
                Wander => activity!(WanderActivity::default()),
                Nop => activity!(NopActivity::default()),
                GoBreakBlock(pos) => activity!(GoBreakBlockActivity::new(pos)),
                GoBuild { job, details } => activity!(GoBuildActivity::new(job, details)),
                GoEquip(e) => activity!(GoEquipActivity::new(e)),
                EatHeldItem(item) => activity!(EatHeldItemActivity::new(item)),
                Goto(target) => activity!(GoToActivity::new(
                    target,
                    NormalizedFloat::new(0.8),
                    SearchGoal::Arrive
                )),
                Follow { target, radius } => {
                    activity!(FollowActivity::new(target, radius))
                }
                Haul(thing, source, target, purpose) => {
                    activity!(GoHaulActivity::new_with_purpose(
                        thing, source, target, purpose
                    ))
                }
            }
        }
    }
}
