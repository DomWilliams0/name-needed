use crate::activity::activity::Activity;
use crate::activity::context::{ActivityContext, ActivityResult, InterruptResult};
use crate::activity::status::Status;
use crate::activity::subactivity::GoingToStatus;
use crate::ecs::ComponentGetError;
use crate::event::{EntityEvent, EntityEventSubscription, EventSubscription};
use crate::item::HaulableItemComponent;
use crate::{ComponentWorld, Entity, TransformComponent};
use async_trait::async_trait;
use common::*;
use unit::world::WorldPoint;
use world::SearchGoal;

/// Picking up {0}
#[derive(Debug, Clone, Display)]
pub struct GoEquipActivity(Entity);

#[derive(Debug, Error)]
pub enum EquipError {
    #[error("Can't get item transform")]
    MissingTransform(#[source] ComponentGetError),

    #[error("Item can not be picked up or equipped ({0})")]
    NotHaulable(#[source] ComponentGetError),
}

/// Equipping
#[derive(Display)]
struct EquippingState;

#[async_trait]
impl Activity for GoEquipActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        // cancel if any destructive event happens to the item
        ctx.subscribe_to(EntityEventSubscription {
            subject: self.0,
            subscription: EventSubscription::All,
        });

        // check if the item exists in the world
        if let Ok(item_pos) = self.find_item(ctx) {
            // go to the item
            ctx.go_to(
                item_pos,
                NormalizedFloat::new(0.8),
                SearchGoal::Arrive,
                GoingToStatus::target("item"),
            )
            .await?;

            // picky uppy
            ctx.update_status(EquippingState);
            ctx.pick_up(self.0).await?;
        } else {
            // it must be held by someone, try equipping instead
            let extra_hands = match ctx.world().component::<HaulableItemComponent>(self.0) {
                Ok(comp) => comp.extra_hands,
                Err(err) => return Err(EquipError::NotHaulable(err).into()),
            };

            ctx.update_status(EquippingState);
            ctx.equip(self.0, extra_hands).await?;
        }

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

impl GoEquipActivity {
    pub fn new(item: Entity) -> Self {
        Self(item)
    }

    fn find_item(&self, ctx: &ActivityContext) -> Result<WorldPoint, EquipError> {
        let transform = ctx
            .world()
            .component::<TransformComponent>(self.0)
            .map_err(EquipError::MissingTransform)?;

        Ok(transform.position)
    }
}

impl Status for EquippingState {
    fn exertion(&self) -> f32 {
        0.6
    }
}
