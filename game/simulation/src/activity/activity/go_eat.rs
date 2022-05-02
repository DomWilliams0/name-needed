use async_trait::async_trait;

use common::derive_more::Display;
use common::*;
use unit::world::WorldPoint;
use world::SearchGoal;

use crate::activity::activity::Activity;
use crate::activity::context::{ActivityContext, ActivityResult, InterruptResult};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::ecs::ComponentGetError;
use crate::event::{EntityEvent, EntityEventSubscription, EventSubscription};

use crate::{ComponentWorld, Entity, TransformComponent};

/// Go eat a non-haulable food directly without picking it up first
#[derive(Debug, Clone, Display)]
#[display(fmt = "Going to eat {_0}")]
pub struct GoEatActivity(Entity);

#[derive(Debug, Error)]
pub enum EatError {
    #[error("Can't get item transform")]
    MissingTransform(#[source] ComponentGetError),
}

#[derive(Display)]
#[display(fmt = "Eating")]
struct EatingState;

#[async_trait]
impl Activity for GoEatActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        // cancel if any destructive event happens to the food
        ctx.subscribe_to(EntityEventSubscription {
            subject: self.0,
            subscription: EventSubscription::All,
        });

        // find and walk to the food
        // TODO destination depends on food size, not always adjacent block
        let pos = self.find_item(ctx)?;
        ctx.go_to(
            pos,
            NormalizedFloat::new(0.8),
            SearchGoal::Adjacent,
            GoingToStatus::target("food"),
        )
        .await?;

        // eat up
        ctx.update_status(EatingState);
        ctx.eat_nearby(self.0).await?;

        Ok(())
    }

    fn on_unhandled_event(&self, event: EntityEvent, me: Entity) -> InterruptResult {
        if event.subject == self.0 && event.payload.is_destructive_for(Some(me)) {
            debug!("food has been destroyed, cancelling eat");
            InterruptResult::Cancel
        } else {
            InterruptResult::Continue
        }
    }
}

impl GoEatActivity {
    pub fn new(food: Entity) -> Self {
        Self(food)
    }

    fn find_item(&self, ctx: &ActivityContext) -> Result<WorldPoint, EatError> {
        let transform = ctx
            .world()
            .component::<TransformComponent>(self.0)
            .map_err(EatError::MissingTransform)?;

        Ok(transform.position)
    }
}

impl Status for EatingState {
    fn exertion(&self) -> f32 {
        0.1
    }
}
