use crate::ai::activity::{Activity, ActivityContext, ActivityResult, Finish};
use crate::ecs::ComponentWorld;
use common::derive_more::Display;

#[derive(Display)]
#[display(fmt = "Doing nothing")]
pub struct NopActivity;

impl<W: ComponentWorld> Activity<W> for NopActivity {
    fn on_start(&mut self, _: &ActivityContext<W>) {}

    fn on_tick(&mut self, _: &ActivityContext<W>) -> ActivityResult {
        ActivityResult::Ongoing
    }

    fn on_finish(&mut self, _: Finish, _: &ActivityContext<W>) {}

    fn exertion(&self) -> f32 {
        0.0
    }
}

/// Nop but ends immediately
#[derive(Display)]
#[display(fmt = "Doing nothing")]
pub struct OneShotNopActivity;

impl<W: ComponentWorld> Activity<W> for OneShotNopActivity {
    fn on_start(&mut self, _: &ActivityContext<W>) {}

    fn on_tick(&mut self, _: &ActivityContext<W>) -> ActivityResult {
        ActivityResult::Finished(Finish::Succeeded)
    }

    fn on_finish(&mut self, _: Finish, _: &ActivityContext<W>) {}

    fn exertion(&self) -> f32 {
        0.0
    }
}
