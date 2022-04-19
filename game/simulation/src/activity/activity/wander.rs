use async_trait::async_trait;
use specs::WorldExt;

use common::rand::distributions::Uniform;
use common::*;
use unit::world::WorldPosition;
use world::{ExplorationFilter, ExplorationResult, SearchGoal};

use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::activity::Activity;
use crate::ecs::ComponentGetError;
use crate::path::WANDER_SPEED;
use crate::{ComponentWorld, HerdedComponent, Herds, TransformComponent};

/// Wandering aimlessly
#[derive(Debug, Default, Display)]
pub struct WanderActivity;

enum State {
    Wander,
    Loiter,
}

#[derive(Debug, Error)]
pub enum WanderError {
    #[error("Wanderer has no transform")]
    MissingTransform,
}

#[async_trait]
impl Activity for WanderActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(Self)
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        let distr_wander_distance = Uniform::new(2, 50);
        let distr_loiter_ticks = Uniform::new(5, 20);
        let world_ref = ctx.world().voxel_world();

        loop {
            let (wander_distance, loiter_ticks) = {
                let mut random = thread_rng();
                (
                    distr_wander_distance.sample(&mut random),
                    distr_loiter_ticks.sample(&mut random),
                )
            };

            ctx.update_status(State::Wander);

            let wander_destination = {
                let transforms = ctx.world().read_storage::<TransformComponent>();
                let explore_filter = ctx
                    .world()
                    .component::<HerdedComponent>(ctx.entity())
                    .ok()
                    .and_then(|comp| {
                        let herds = ctx.world().resource::<Herds>();
                        herds.get_info(comp.current().handle())
                    })
                    .map(|herd| {
                        let pos = herd
                            .herd_centre(|e| e.get(&transforms).map(|t| t.position))
                            .floor();
                        let max_distance2 = {
                            let (min, max) = herd.range().bounds();
                            let w = max.x() - min.x();
                            let h = max.y() - min.y();
                            let range = w.max(h).max(5.0);
                            ((range * 0.5) as i32).pow(2)
                        };
                        // TODO when in range, this produces a lot of single block paths, which look terrible
                        ExplorationFilter(Box::new(move |candidate: WorldPosition| {
                            if candidate.distance2(pos) < max_distance2 {
                                ExplorationResult::Continue
                            } else {
                                // too far away
                                ExplorationResult::Abort
                            }
                        }))
                    });

                let explore_src = ctx
                    .entity()
                    .get(&transforms)
                    .map(|t| t.accessible_position())
                    .ok_or(WanderError::MissingTransform)?;

                world_ref.borrow().find_exploratory_destination(
                    explore_src,
                    wander_distance,
                    explore_filter,
                )?
            };

            ctx.go_to(
                wander_destination.centred(),
                NormalizedFloat::new(WANDER_SPEED),
                SearchGoal::Nearby(1),
                GoingToStatus::Custom(State::Wander),
            )
            .await?;

            // loiter for a bit
            ctx.update_status(State::Loiter);
            ctx.wait(loiter_ticks).await;
        }
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            State::Wander => "Ambling about",
            State::Loiter => "Loitering",
        };

        f.write_str(s)
    }
}

impl Status for State {
    fn exertion(&self) -> f32 {
        match self {
            State::Wander => 0.6,
            State::Loiter => 0.2,
        }
    }
}
