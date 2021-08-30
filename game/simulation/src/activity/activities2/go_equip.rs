use crate::activity::activity2::{
    Activity2, ActivityContext2, ActivityResult, EventResult, InterruptResult,
};
use crate::activity::status::Status;
use crate::ecs::ComponentGetError;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventSubscription, EventSubscription};
use crate::{ComponentWorld, Entity, TransformComponent};
use async_trait::async_trait;
use common::*;
use unit::world::WorldPoint;
use world::SearchGoal;

#[derive(Debug, Clone)]
pub struct GoEquipActivity2(Entity);

#[derive(Debug, Error)]
pub enum EquipError {
    #[error("Can't get item transform")]
    MissingTransform(#[from] ComponentGetError),
}

pub enum State {
    Going,
    PickingUp,
}

#[async_trait]
impl Activity2 for GoEquipActivity2 {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it<'a>(&'a self, ctx: ActivityContext2<'a>) -> ActivityResult {
        // cancel if any destructive event happens to the item
        ctx.subscribe_to(EntityEventSubscription {
            subject: self.0,
            subscription: EventSubscription::All,
        });

        // go to the item
        ctx.update_status(State::Going);
        let item_pos = self.find_item(&ctx)?;
        ctx.go_to(item_pos, NormalizedFloat::new(0.8), SearchGoal::Arrive)
            .await?;

        // picky uppy
        ctx.update_status(State::PickingUp);
        ctx.pick_up(self.0).await?;

        Ok(())
    }

    fn on_unhandled_event(&self, event: EntityEvent) -> InterruptResult {
        if event.subject == self.0 && event.payload.is_destructive() {
            debug!("item has been destroyed, cancelling equip");
            InterruptResult::Cancel
        } else {
            InterruptResult::Continue
        }
    }
}

impl GoEquipActivity2 {
    pub fn new(item: Entity) -> Self {
        Self(item)
    }

    fn find_item(&self, ctx: &ActivityContext2) -> Result<WorldPoint, EquipError> {
        let transform = ctx
            .world()
            .component::<TransformComponent>(self.0)
            .map_err(EquipError::MissingTransform)?;

        Ok(transform.position)
    }
}

impl Display for GoEquipActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picking up {}", self.0)
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            State::Going => "Going to item",
            State::PickingUp => "Picking up item",
        })
    }
}

impl Status for State {
    fn exertion(&self) -> f32 {
        match self {
            State::Going => 1.0,
            State::PickingUp => 0.6,
        }
    }
}
