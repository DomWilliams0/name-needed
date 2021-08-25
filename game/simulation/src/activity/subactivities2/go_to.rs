use crate::activity::activity2::ActivityContext2;
use crate::activity::EventUnsubscribeResult;
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
}

pub struct GoToSubactivity;

impl GoToSubactivity {
    pub async fn go_to<'a>(
        &mut self,
        ctx: &ActivityContext2<'a>,
        dest: WorldPoint,
        speed: NormalizedFloat,
        goal: SearchGoal,
    ) -> Result<(), GotoError> {
        let follow_path = ctx
            .world
            .component_mut::<FollowPathComponent>(ctx.entity)
            .map_err(GotoError::MissingComponent)?;

        // assign path
        let path_token = follow_path.new_path_with_goal(dest, goal, speed);

        // await arrival
        let mut goto_result = None;
        let subscription = EntityEventSubscription {
            subject: ctx.entity,
            subscription: EventSubscription::Specific(EntityEventType::Arrived),
        };

        ctx.subscribe_to_until(subscription, |evt| match evt {
            EntityEventPayload::Arrived(token, result) if token == path_token => {
                goto_result = Some(result);
                false
            }
            _ => unexpected_event2!(evt),
        })
        .await;

        let result = goto_result.expect("did not get goto event?"); // TODO possible?
        match result {
            Ok(_) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}
