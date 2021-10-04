use async_trait::async_trait;

use common::NormalizedFloat;
use common::*;

use world::SearchGoal;

use crate::activity::activity::Activity;
use crate::activity::context::{ActivityContext, ActivityResult, InterruptResult};

use crate::activity::subactivity::GoingToStatus;
use crate::activity::HaulError;
use crate::{Entity, EntityEvent, HaulTarget};

use crate::event::{EntityEventSubscription, EventSubscription};

// TODO support for hauling multiple things at once to the same loc, if the necessary amount of hands are available
// TODO support hauling multiple things to multiple locations (or via multiple activities?)
// TODO haul target should hold pos+item radius, assigned once on creation

#[derive(Debug, Clone)]
pub struct GoHaulActivity {
    thing: Entity,
    source: HaulTarget,
    target: HaulTarget,
}

impl GoHaulActivity {
    pub fn new(entity: Entity, source: HaulTarget, target: HaulTarget) -> Self {
        Self {
            thing: entity,
            source,
            target,
        }
    }
}

#[async_trait]
impl Activity for GoHaulActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
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

impl Display for GoHaulActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO format the other entity better e.g. get item name. or do this in the ui layer?
        write!(f, "Hauling {} to {}", self.thing, self.target)
    }
}
