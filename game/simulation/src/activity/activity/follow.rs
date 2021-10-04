use crate::activity::activity::{Activity};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::ecs::*;

use crate::{Entity, TransformComponent};
use async_trait::async_trait;
use common::*;
use futures::future::Either;
use futures::pin_mut;
use std::fmt::Formatter;
use unit::world::WorldPoint;
use world::SearchGoal;
use crate::activity::context::{ActivityContext, ActivityResult};

#[derive(Debug, Clone)]
pub struct FollowActivity {
    target: Entity,
    radius: u8,
}

enum State {
    Follow,
    Loiter,
}

#[derive(Debug, Error)]
pub enum FollowError {
    #[error("Either entity missing transform for following")]
    MissingTransform,
}

#[async_trait]
impl Activity for FollowActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        let mut loitering = false;
        loop {
            let (distance2, target_pos) = self.distance(ctx)?;

            if distance2 < self.radius.pow(2) as f32 {
                // loiter in range

                if !loitering {
                    // do this once the first time to avoid spamming with clear requests
                    loitering = true;
                    ctx.clear_path();
                }

                ctx.update_status(State::Loiter);
                ctx.wait(20).await;
                continue;
            }

            loitering = false;

            // move up until timer expires
            // TODO this can generate path requests that are immediately complete, leading to a lot of path spam
            let goto_fut = ctx.go_to(
                target_pos,
                // TODO specify follow speed in activity too
                NormalizedFloat::new(0.3),
                SearchGoal::Nearby(self.radius),
                GoingToStatus::Custom(State::Follow),
            );

            let timeout_fut = ctx.wait(20);
            pin_mut!(goto_fut);
            pin_mut!(timeout_fut);

            // select on both, and bubble up errs from goto
            if let Either::Left((Err(err), _)) =
                futures::future::select(goto_fut, timeout_fut).await
            {
                return Err(err.into());
            }
        }
    }
}

impl FollowActivity {
    pub fn new(target: Entity, radius: u8) -> Self {
        Self { target, radius }
    }

    fn distance(&self, ctx: &ActivityContext) -> Result<(f32, WorldPoint), FollowError> {
        let me = ctx.world().component::<TransformComponent>(ctx.entity());
        let you = ctx.world().component::<TransformComponent>(self.target);

        match me.ok().zip(you.ok()) {
            Some((me, you)) => Ok((me.position.distance2(you.position), you.position)),
            None => Err(FollowError::MissingTransform),
        }
    }
}

impl Display for FollowActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Following {}", self.target)
    }
}

//noinspection DuplicatedCode
impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            State::Follow => "Following target",
            State::Loiter => "Hanging around",
        })
    }
}

impl Status for State {
    fn exertion(&self) -> f32 {
        match self {
            State::Follow => 0.8,
            State::Loiter => 0.2,
        }
    }
}
