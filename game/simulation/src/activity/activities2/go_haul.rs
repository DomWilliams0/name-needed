use async_trait::async_trait;

use common::*;

use world::SearchGoal;

use crate::activity::activity2::{Activity2, ActivityResult};
use crate::activity::activity2::{ActivityContext2, InterruptResult};

use crate::activity::subactivities2::{GoingToStatus, HaulError};
use crate::activity::HaulTarget;

use crate::event::{EntityEventSubscription, EventSubscription};

use crate::{Entity, EntityEvent, EntityEventPayload, TransformComponent, WorldPosition};

// TODO support for hauling multiple things at once to the same loc, if the necessary amount of hands are available
// TODO support hauling multiple things to multiple locations (or via multiple activities?)
// TODO haul target should hold pos+item radius, assigned once on creation

#[derive(Debug, Clone)]
pub struct GoHaulActivity2 {
    thing: Entity,
    source: HaulTarget,
    target: HaulTarget,
}

impl GoHaulActivity2 {
    pub fn new(entity: Entity, source: HaulTarget, target: HaulTarget) -> Self {
        Self {
            thing: entity,
            source,
            target,
        }
    }
}

#[async_trait]
impl Activity2 for GoHaulActivity2 {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext2) -> ActivityResult {
        // cancel if any destructive event happens to the hauled thing
        // TODO destructive events on items should include moving/falling
        // TODO destructive events on the container? society job handles this but not always the source
        ctx.subscribe_to(EntityEventSubscription {
            subject: self.thing,
            subscription: EventSubscription::All,
        });

        // go to it
        // TODO arrival radius depends on the size of the item
        let pos = self.source.source_position(ctx.world(), self.thing)?;
        ctx.go_to(
            pos,
            NormalizedFloat::new(0.8),
            SearchGoal::Adjacent,
            GoingToStatus::target("haul target"),
        )
        .await?;

        // pick it up
        let mut hauling = ctx.haul(self.thing, self.source).await?;

        // go to destination
        let pos = self
            .target
            .target_position(ctx.world())
            .ok_or(HaulError::BadTargetContainer)?;

        ctx.go_to(
            pos,
            NormalizedFloat::new(0.8),
            SearchGoal::Adjacent,
            GoingToStatus::target("destination"),
        )
        .await?;

        // put it down
        hauling.end_haul(self.target).await?;
        Ok(())
    }

    fn on_unhandled_event(&self, event: EntityEvent) -> InterruptResult {
        if event.subject == self.thing && event.payload.is_destructive() {
            // the expected haul event will be handled before this handler
            debug!("thing has been destroyed, cancelling haul");
            InterruptResult::Cancel
        } else {
            InterruptResult::Continue
        }
    }
}

impl Display for GoHaulActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO format the other entity better e.g. get item name. or do this in the ui layer?
        write!(f, "Hauling {} to {}", self.thing, self.target)
    }
}
