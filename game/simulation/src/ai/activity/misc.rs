use crate::ai::activity::{Activity, ActivityContext, ActivityResult, Finish};
use crate::ecs::ComponentWorld;

pub struct NopActivity;

impl<W: ComponentWorld> Activity<W> for NopActivity {
    fn on_start(&mut self, _: &ActivityContext<W>) {}

    fn on_tick(&mut self, _: &ActivityContext<W>) -> ActivityResult {
        ActivityResult::Ongoing
    }

    fn on_finish(&mut self, _: Finish, _: &ActivityContext<W>) {}

    fn exertion(&self) -> f32 {
        1.0
    }
}
