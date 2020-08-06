use crate::ai::activity::{Activity, ActivityContext, ActivityResult, Finish};
use crate::ai::AiComponent;
use crate::ecs::ComponentWorld;
use common::derive_more::Display;

#[derive(Display)]
#[display(fmt = "Doing nothing")]
pub struct NopActivity;

impl<W: ComponentWorld> Activity<W> for NopActivity {
    fn on_start(&mut self, _: &ActivityContext<W>) {}

    fn on_tick(&mut self, _: &ActivityContext<W>) -> ActivityResult {
        common::warn!("ticking nop activity, possible infinite loop");
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

#[derive(Display)]
#[display(fmt = "Following divine command: {}", _0)]
pub struct DivineCommandActivity<W: ComponentWorld>(Box<dyn Activity<W>>);

impl<W: ComponentWorld> Activity<W> for DivineCommandActivity<W> {
    fn on_start(&mut self, ctx: &ActivityContext<W>) {
        self.0.on_start(ctx)
    }

    fn on_tick(&mut self, ctx: &ActivityContext<W>) -> ActivityResult {
        self.0.on_tick(ctx)
    }

    fn on_finish(&mut self, finish: Finish, ctx: &ActivityContext<W>) {
        self.0.on_finish(finish, ctx);

        // remove divine dse
        if let Ok(ai) = ctx.world.component_mut::<AiComponent>(ctx.entity) {
            ai.remove_divine_command()
        }
    }

    fn exertion(&self) -> f32 {
        self.0.exertion()
    }
}
