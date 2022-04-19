use std::fmt::Display;

use async_trait::async_trait;

use common::*;
use world::SearchGoal;

use crate::activity::context::{ActivityContext, ActivityResult};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::activity::Activity;
use crate::interact::herd::HerdInfo;
use crate::{ComponentWorld, TransformComponent};

/// Returning to herd
#[derive(Debug, Default, Display)]
pub struct ReturnToHerdActivity;

struct State;

#[derive(Debug, Error)]
pub enum ReturnToHerdError {
    #[error("Missing or invalid herd")]
    InvalidHerd,
}

#[async_trait]
impl Activity for ReturnToHerdActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(Self)
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        let herd =
            HerdInfo::get(ctx.entity(), ctx.world()).ok_or(ReturnToHerdError::InvalidHerd)?;
        let target = herd.herd_centre(|e| {
            ctx.world()
                .component::<TransformComponent>(e)
                .ok()
                .map(|t| t.position)
        });
        ctx.go_to(
            target,
            NormalizedFloat::new(0.7),
            SearchGoal::Nearby(5),
            GoingToStatus::Custom(State),
        )
        .await?;

        Ok(())
    }
}

impl Status for State {
    fn exertion(&self) -> f32 {
        0.6
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Returning to herd")
    }
}
