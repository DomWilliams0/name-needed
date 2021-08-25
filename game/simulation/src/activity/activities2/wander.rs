use async_trait::async_trait;

use common::*;

use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::{Activity2, ActivityResult};
use crate::ecs::ComponentGetError;
use crate::path::WANDER_SPEED;
use crate::{ComponentWorld, TransformComponent, WorldPosition};

#[derive(Debug)]
pub struct WanderActivity2(State);

#[derive(Debug)]
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
    async fn dew_it<'a>(&'a mut self, ctx: ActivityContext2<'a>) -> ActivityResult {
        loop {
            // wander to a new target
            self.0 = State::Wander;

            let tgt = find_target(&ctx)?;
            trace!("wandering to {:?}", tgt);
            ctx.go_to(tgt.centred(), NormalizedFloat::new(WANDER_SPEED))
                .await?;

            // loiter for a bit
            self.0 = State::Loiter;
            let loiter_ticks = random::get().gen_range(5, 60);
            ctx.wait(loiter_ticks).await;
        }
    }
}

fn find_target(ctx: &ActivityContext2) -> Result<WorldPosition, WanderError> {
    let transform = ctx
        .world
        .component::<TransformComponent>(ctx.entity)
        .map_err(WanderError::MissingTransform)?;

    let world = ctx.world.voxel_world();
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
        Debug::fmt(self, f)
    }
}

impl Default for WanderActivity2 {
    fn default() -> Self {
        Self(State::Wander)
    }
}
