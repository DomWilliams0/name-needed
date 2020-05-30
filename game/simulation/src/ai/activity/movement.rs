use crate::ai::activity::{Activity, ActivityContext, ActivityResult, Finish};
use crate::ecs::ComponentWorld;
use crate::path::{FollowPathComponent, WanderComponent, WANDER_SPEED};
use crate::steer::SteeringComponent;

pub struct WanderActivity;

impl<W: ComponentWorld> Activity<W> for WanderActivity {
    fn on_start(&mut self, ctx: &ActivityContext<W>) {
        // add wander marker component
        ctx.world.add_lazy(ctx.entity, WanderComponent);
    }

    fn on_tick(&mut self, _: &ActivityContext<W>) -> ActivityResult {
        ActivityResult::Ongoing
    }

    fn on_finish(&mut self, _: Finish, ctx: &ActivityContext<W>) {
        // remove wander marker component
        ctx.world.remove_lazy::<WanderComponent>(ctx.entity);

        // clear wander goals and reset steering to the default Stop behaviour
        let entity = ctx.entity;
        ctx.updates.queue("clear wander movement", move |world| {
            if let Ok(c) = world.component_mut::<SteeringComponent>(entity) {
                *c = SteeringComponent::default()
            }
            if let Ok(c) = world.component_mut::<FollowPathComponent>(entity) {
                *c = FollowPathComponent::default()
            }

            Ok(())
        })
    }

    fn exertion(&self) -> f32 {
        // TODO wander *activity* exertion should be 0, but added to the exertion of walking at X speed
        // TODO remove WANDER_SPEED constant when this is done
        WANDER_SPEED
    }
}
