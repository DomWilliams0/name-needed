use async_trait::async_trait;

use common::*;

use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::{Activity2, ActivityResult};
use crate::activity::status::Status;
use crate::activity::subactivities2::GoingToStatus;
use crate::ecs::ComponentGetError;
use crate::path::WANDER_SPEED;
use crate::{ComponentWorld, TransformComponent, WorldPosition};
use world::SearchGoal;

#[derive(Debug, Default)]
pub struct WanderActivity2;

enum State {
    Wander,
    Loiter,
}

#[derive(Debug, Error)]
pub enum WanderError {
    #[error("Wanderer has no transform: {0}")]
    MissingTransform(#[from] ComponentGetError),

    #[error("Can't find an accessible wander destination, possibly stuck")]
    Inaccessible,
}

const WANDER_RADIUS: u16 = 10;

#[async_trait]
impl Activity2 for WanderActivity2 {
    fn description(&self) -> Box<dyn Display> {
        Box::new(Self)
    }

    async fn dew_it(&self, ctx: &ActivityContext2) -> ActivityResult {
        loop {
            // wander to a new target
            let tgt = find_target(&ctx)?;
            trace!("wandering to {:?}", tgt);
            ctx.go_to(
                tgt.centred(),
                NormalizedFloat::new(WANDER_SPEED),
                SearchGoal::Arrive,
                GoingToStatus::Custom(State::Wander),
            )
            .await?;

            // loiter for a bit
            ctx.update_status(State::Loiter);
            let loiter_ticks = random::get().gen_range(5, 60);
            ctx.wait(loiter_ticks).await;
        }
    }
}

fn find_target(ctx: &ActivityContext2) -> Result<WorldPosition, WanderError> {
    // TODO special SearchGoal for wandering instead of randomly choosing an accessible target
    let transform = ctx
        .world()
        .component::<TransformComponent>(ctx.entity())
        .map_err(WanderError::MissingTransform)?;

    let world = ctx.world().voxel_world();
    let world = world.borrow();

    world
        .choose_random_accessible_block_in_radius(
            transform.accessible_position(),
            WANDER_RADIUS,
            20,
        )
        .ok_or(WanderError::Inaccessible)
}

impl Display for WanderActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Wandering aimlessly")
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
