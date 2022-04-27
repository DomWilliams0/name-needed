use crate::activity::context::ActivityContext;

use crate::event::prelude::*;
use crate::item::ContainedInComponent;
use crate::needs::food::{BeingEatenComponent, FoodEatingError};
use crate::queued_update::QueuedUpdates;

use crate::{ComponentWorld, Entity};
use common::*;

/// Eats an item that's already equipped
pub struct EatItemSubactivity;

#[derive(Error, Debug, Clone)]
pub enum EatItemError {
    #[error("Eating was cancelled")]
    Cancelled,

    #[error("{0}")]
    Food(#[from] FoodEatingError),
}

impl EatItemSubactivity {
    pub async fn eat(&self, ctx: &ActivityContext, item: Entity) -> Result<(), EatItemError> {
        let eater = ctx.entity();
        ctx.world()
            .resource::<QueuedUpdates>()
            .queue("begin eating", move |world| {
                match world.component::<ContainedInComponent>(item).as_deref() {
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
        let eat_result = ctx
            .subscribe_to_specific_until(item, EntityEventType::BeenEaten, |evt| {
                match evt {
                    EntityEventPayload::BeenEaten(Ok(actual_eater)) if actual_eater != eater => {
                        // someone else ate it, damn
                        Err(evt)
                    }
                    EntityEventPayload::BeenEaten(result) => Ok(result),
                    // calling activity can handle other destructive events
                    _ => Err(evt),
                }
            })
            .await;

        match eat_result {
            Some(Ok(_)) => Ok(()),
            Some(Err(err)) => Err(EatItemError::Food(err)),
            None => Err(EatItemError::Cancelled),
        }
    }
}
