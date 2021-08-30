use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::EventResult::Consumed;
use crate::ecs::ComponentGetError;
use crate::event::prelude::*;
use crate::unexpected_event2;
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
    context: &'a ActivityContext2<'a>,
    complete: bool,
}

impl<'a> GoToSubactivity<'a> {
    pub fn new(context: &'a ActivityContext2<'a>) -> Self {
        Self {
            context,
            complete: false,
        }
    }

    pub async fn go_to(
        &mut self,
        dest: WorldPoint,
        speed: NormalizedFloat,
        goal: SearchGoal,
    ) -> Result<(), GotoError> {
        let ctx = self.context;

        let follow_path = ctx
            .world()
            .component_mut::<FollowPathComponent>(ctx.entity())
            .map_err(GotoError::MissingComponent)?;

        // assign path
        let path_token = follow_path.new_path_with_goal(dest, goal, speed);

        // await arrival
        let mut goto_result = None;
        let subscription = EntityEventSubscription {
            subject: ctx.entity(),
            subscription: EventSubscription::Specific(EntityEventType::Arrived),
        };

        ctx.subscribe_to_until(subscription, |evt| {
            debug!("goto is waiting for arrival of token {:?}", path_token);
            match evt {
                EntityEventPayload::Arrived(token, result) if token == path_token => {
                    goto_result = Some(result);
                    Consumed
                }
                _ => unexpected_event2!(evt),
            }
        })
        .await;

        self.complete = true;

        match goto_result {
            None => Err(GotoError::Cancelled),
            Some(Ok(_)) => Ok(()),
            Some(Err(err)) => Err(err.into()),
        }
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
