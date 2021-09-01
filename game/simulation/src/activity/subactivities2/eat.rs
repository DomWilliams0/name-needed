use crate::activity::activity2::ActivityContext2;
use crate::activity::activity2::EventResult::{Consumed, Unconsumed};
use crate::ecs::*;
use crate::event::prelude::*;
use crate::item::{
    ContainedInComponent, EndHaulBehaviour, FoundSlot, HaulableItemComponent, HauledItemComponent,
    ItemFilter,
};
use crate::needs::{BeingEatenComponent, FoodEatingError};
use crate::queued_update::QueuedUpdates;
use crate::unexpected_event2;
use crate::{ComponentWorld, Entity, InventoryComponent, PhysicalComponent, TransformComponent};
use common::*;

/// Eats an item that's already equipped
pub struct EatItemSubactivity2;

#[derive(Error, Debug, Clone)]
pub enum EatItemError {
    #[error("Eating was cancelled")]
    Cancelled,

    #[error("{0}")]
    Food(#[from] FoodEatingError),
}

impl EatItemSubactivity2 {
    pub async fn eat(&self, ctx: &ActivityContext2, item: Entity) -> Result<(), EatItemError> {
        let eater = ctx.entity();
        ctx.world()
            .resource::<QueuedUpdates>()
            .queue("begin eating", move |world| {
                match world.component::<ContainedInComponent>(item) {
                    Ok(ContainedInComponent::InventoryOf(holder)) if *holder == eater => {
                        // success
                    }
                    other => {
                        debug!("cannot eat because food is not held"; "error" => ?other);
                        world.post_event(EntityEvent {
                            subject: item,
                            payload: EntityEventPayload::BeenEaten(Err(
                                FoodEatingError::NotEquipped,
                            )),
                        });
                        return Err(FoodEatingError::NotEquipped.into());
                    }
                }

                // start eating
                let insert_result = world.add_now(item, BeingEatenComponent { eater });
                debug_assert!(insert_result.is_ok());
                Ok(())
            });

        // wait for completion
        let mut eat_result = None;
        ctx.subscribe_to_until(
            EntityEventSubscription {
                subject: item,
                subscription: EventSubscription::Specific(EntityEventType::BeenEaten),
            },
            |evt| {
                match evt {
                    EntityEventPayload::BeenEaten(Ok(actual_eater)) if actual_eater != eater => {
                        // someone else ate it, damn
                        Unconsumed(evt)
                    }
                    EntityEventPayload::BeenEaten(result) => {
                        eat_result = Some(result);
                        Consumed
                    }
                    // calling activity can handle other destructive events
                    _ => unexpected_event2!(evt),
                }
            },
        )
        .await;

        match eat_result {
            Some(Ok(_)) => Ok(()),
            Some(Err(err)) => Err(EatItemError::Food(err)),
            None => Err(EatItemError::Cancelled),
        }
    }
}
