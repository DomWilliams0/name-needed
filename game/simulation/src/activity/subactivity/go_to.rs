use crate::activity::context::ActivityContext;

use crate::activity::status::{NopStatus, Status};
use crate::ecs::ComponentGetError;
use crate::event::prelude::*;

use crate::{ComponentWorld, FollowPathComponent};
use common::*;
use unit::world::WorldPoint;
use world::{NavigationError, SearchGoal};

#[derive(Debug, Error)]
pub enum GotoError {
    #[error("Can't get FollowPathComponent: {0}")]
    MissingComponent(#[from] ComponentGetError),

    #[error("Failed to navigate: {0}")]
    Navigation(#[from] NavigationError),

    #[error("Goto was cancelled")]
    Cancelled,
}

pub struct GoToSubactivity<'a> {
    context: &'a ActivityContext,
    complete: bool,
}

pub enum GoingToStatus<T: Status + 'static = NopStatus> {
    Default,
    Target(&'static str),
    Custom(T),
}

impl GoingToStatus {
    pub fn default() -> Self {
        GoingToStatus::Default
    }

    pub fn target(target: &'static str) -> Self {
        GoingToStatus::Target(target)
    }
}

impl<'a> GoToSubactivity<'a> {
    pub fn new(context: &'a ActivityContext) -> Self {
        Self {
            context,
            complete: false,
        }
    }

    pub async fn go_to<T: Status + 'static>(
        &mut self,
        dest: WorldPoint,
        speed: NormalizedFloat,
        goal: SearchGoal,
        status: GoingToStatus<T>,
    ) -> Result<(), GotoError> {
        let ctx = self.context;

        ctx.update_status(status);

        let path_token;
        {
            let mut follow_path = ctx
                .world()
                .component_mut::<FollowPathComponent>(ctx.entity())
                .map_err(GotoError::MissingComponent)?;

            // assign path
            path_token = follow_path.new_path_with_goal(dest, goal, speed);
        }

        // await arrival
        let goto_result = ctx
            .subscribe_to_specific_until(ctx.entity(), EntityEventType::Arrived, |evt| match evt {
                EntityEventPayload::Arrived(token, result) if token == path_token => Ok(result),
                _ => Err(evt),
            })
            .await;

        let result = match goto_result {
            None => return Err(GotoError::Cancelled),
            Some(Ok(_)) => Ok(()),
            Some(Err(err)) => Err(err.into()),
        };

        self.complete = true;
        result
    }
}

impl Drop for GoToSubactivity<'_> {
    fn drop(&mut self) {
        if !self.complete {
            debug!("aborting incomplete goto"; self.context.entity());
            self.context.clear_path();
        }
    }
}

impl<T: Status> Display for GoingToStatus<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GoingToStatus::Default => f.write_str("Going to target"),
            GoingToStatus::Target(target) => write!(f, "Going to {}", target),
            GoingToStatus::Custom(custom) => Display::fmt(custom, f),
        }
    }
}

impl<T: Status> Status for GoingToStatus<T> {
    fn exertion(&self) -> f32 {
        // TODO use target moving speed or get the actual speed when applying exertion in other system?
        1.0
    }
}
