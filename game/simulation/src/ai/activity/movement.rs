use common::derive_more::*;
use common::*;
use unit::world::WorldPoint;
use world::SearchGoal;

use crate::ai::activity::misc::OneShotNopActivity;
use crate::ai::activity::{Activity, ActivityContext, ActivityResult, Finish};
use crate::ecs::ComponentWorld;
use crate::path::{
    ArrivedAtTargetEventComponent, FollowPathComponent, WanderComponent, WANDER_SPEED,
};

#[derive(Display)]
#[display(fmt = "Wandering aimlessly")]
pub struct WanderActivity;

/// Helper activity that wraps another, calling it only after path finding to and arriving at the
/// given location
pub struct GotoThen<W: ComponentWorld, A: Activity<W> + GoingToActivity> {
    activity: A,
    target: WorldPoint,
    goal: SearchGoal,
    initialized: bool,
    arrived: bool,
    speed: NormalizedFloat,
    _phantom: PhantomData<W>,
}

pub type GotoActivity<W> = GotoThen<W, OneShotNopActivity>;

pub trait GoingToActivity {
    fn display_with_target(&self, f: &mut Formatter<'_>, target: WorldPoint) -> FmtResult;
}

impl<W: ComponentWorld, A: Activity<W> + GoingToActivity> Activity<W> for GotoThen<W, A> {
    fn on_start(&mut self, _: &ActivityContext<W>) {}

    fn on_tick(&mut self, ctx: &ActivityContext<W>) -> ActivityResult {
        let target = self.target;
        let on_missing_comp = || {
            warn!(
                "entity {:?} cannot go to target {:?} because it has no FollowPathComponent",
                ctx.entity, target
            );
            ActivityResult::Finished(Finish::Failed)
        };

        if !self.initialized {
            // first time initialization: assign path target
            self.initialized = true;
            return if let Ok(c) = ctx.world.component_mut::<FollowPathComponent>(ctx.entity) {
                c.new_path(target, self.goal, self.speed);
                ActivityResult::Ongoing
            } else {
                on_missing_comp()
            };
        }

        if !self.arrived {
            // not arrived yet: check progress
            if let Ok(ArrivedAtTargetEventComponent(dest)) = ctx
                .world
                .component::<ArrivedAtTargetEventComponent>(ctx.entity)
            {
                if dest.is_almost(&target, 1.5) {
                    // we've arrived - now we can start our sub action
                    debug!("arrived at activity target, beginning activity '{}'", self);
                    self.arrived = true;
                    self.activity.on_start(ctx);
                    return self.activity.on_tick(ctx);
                }
            }

            // check we actually found a path
            return match ctx.world.component::<FollowPathComponent>(ctx.entity) {
                Ok(follow) if follow.target().map(|t| t.is_almost(&target, 1.5)) == Some(true) => {
                    // still path finding
                    ActivityResult::Ongoing
                }
                Ok(_) => {
                    warn!(
                        "failed to find path to location {} to do '{}'",
                        self.target, self.activity
                    );
                    ActivityResult::Finished(Finish::Failed)
                }
                Err(e) => {
                    warn!(
                        "failed to find path to location {} to do '{}' ({})",
                        self.target, self.activity, e
                    );
                    ActivityResult::Finished(Finish::Failed)
                }
            };
        }

        // keep ticking sub activity
        self.activity.on_tick(ctx)
    }

    fn on_finish(&mut self, finish: Finish, ctx: &ActivityContext<W>) {
        if self.arrived {
            self.activity.on_finish(finish, ctx);
        }
    }

    fn exertion(&self) -> f32 {
        if self.arrived {
            self.activity.exertion()
        } else {
            // TODO exertion depends on speed
            1.0
        }
    }
}
impl<W: ComponentWorld, A: Activity<W> + GoingToActivity> GotoThen<W, A> {
    pub(crate) fn new(
        target: WorldPoint,
        goal: SearchGoal,
        speed: NormalizedFloat,
        activity: A,
    ) -> Self {
        Self {
            activity,
            target,
            goal,
            initialized: false,
            arrived: false,
            speed,
            _phantom: PhantomData,
        }
    }
}

impl<W: ComponentWorld, A: Activity<W> + GoingToActivity> Display for GotoThen<W, A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.activity.display_with_target(f, self.target)
    }
}

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
    }

    fn exertion(&self) -> f32 {
        // TODO wander *activity* exertion should be 0, but added to the exertion of walking at X speed
        // TODO remove WANDER_SPEED constant when this is done
        WANDER_SPEED
    }
}

pub fn goto<W: ComponentWorld>(target: WorldPoint) -> GotoActivity<W> {
    GotoThen::new(
        target,
        SearchGoal::Arrive,
        NormalizedFloat::one(),
        OneShotNopActivity,
    )
}

impl GoingToActivity for OneShotNopActivity {
    fn display_with_target(&self, f: &mut Formatter<'_>, target: WorldPoint) -> FmtResult {
        write!(f, "Going to {}", target)
    }
}
