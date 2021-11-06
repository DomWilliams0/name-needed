pub use activity_trait::Activity;

pub use eat_held_item::EatHeldItemActivity;
pub use follow::FollowActivity;
pub use go_break_block::GoBreakBlockActivity;
pub use go_build::GoBuildActivity;
pub use go_equip::GoEquipActivity;
pub use go_haul::GoHaulActivity;
pub use go_to::GoToActivity;
pub use nop::NopActivity;
pub use wander::WanderActivity;

mod eat_held_item;
mod follow;
mod go_break_block;
mod go_build;
mod go_equip;
mod go_haul;
mod go_to;
mod nop;
mod wander;

mod activity_trait {
    use crate::activity::context::{ActivityContext, ActivityResult, InterruptResult};
    use crate::{Entity, EntityEvent};
    use async_trait::async_trait;
    use std::fmt::{Debug, Display};

    #[async_trait]
    pub trait Activity: Debug {
        fn description(&self) -> Box<dyn Display>;
        async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult;

        /// me is the entity with the activity
        fn on_unhandled_event(&self, event: EntityEvent, me: Entity) -> InterruptResult {
            #![allow(unused_variables)]
            InterruptResult::Continue
        }
    }
}
