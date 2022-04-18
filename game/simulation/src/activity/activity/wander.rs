use async_trait::async_trait;
use std::rc::Rc;

use common::rand::distributions::Uniform;
use common::*;
use unit::world::WorldPosition;
use world::{ExplorationFilter, ExplorationResult};

use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::status::Status;

use crate::activity::Activity;
use crate::ecs::ComponentGetError;
use crate::path::WANDER_SPEED;
use crate::{ComponentWorld, HerdedComponent, Herds};

/// Wandering aimlessly
#[derive(Debug, Default, Display)]
pub struct WanderActivity;

enum State {
    Wander,
    Loiter,
}

#[derive(Debug, Error)]
pub enum WanderError {
    #[error("Wanderer has no transform: {0}")]
    MissingTransform(#[from] ComponentGetError),
}

#[async_trait]
impl Activity for WanderActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(Self)
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        let distr_wander_distance = Uniform::new(2, 50);
        let distr_loiter_ticks = Uniform::new(5, 20);

        let explore_filter = ctx
            .world()
            .component::<HerdedComponent>(ctx.entity())
            .ok()
            .and_then(|comp| {
                let herds = ctx.world().resource::<Herds>();
                herds.get_info(comp.current().handle())
            })
            .map(|herd| {
                let pos = herd.median_pos.floor();
                const MAX_DISTANCE: i32 = 10; // TODO depends on size of herd
                ExplorationFilter(Rc::new(move |candidate: WorldPosition| {
                    if candidate.distance2(pos) < MAX_DISTANCE.pow(2) {
                        ExplorationResult::Continue
                    } else {
                        // too far away
                        ExplorationResult::Abort
                    }
                }))
            });

        loop {
            let (wander_distance, loiter_ticks) = {
                let mut random = thread_rng();
                (
                    distr_wander_distance.sample(&mut random),
                    distr_loiter_ticks.sample(&mut random),
                )
            };

            ctx.update_status(State::Wander);
            ctx.explore(
                wander_distance,
                NormalizedFloat::new(WANDER_SPEED),
                explore_filter.clone(),
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
