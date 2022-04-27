use common::*;

use crate::activity::context::{ActivityContext, DistanceCheckResult};
use crate::event::prelude::*;
use crate::item::ContainedInComponent;
use crate::needs::food::{BeingEatenComponent, FoodEatingError};
use crate::queued_update::QueuedUpdates;
use crate::{ComponentWorld, Entity};

/// Eats an item that's already equipped
pub struct EatItemSubactivity;

const MAX_EAT_DISTANCE: f32 = 2.0;

#[derive(Error, Debug, Clone)]
pub enum EatItemError {
    #[error("Eating was cancelled")]
    Cancelled,

    #[error("{0}")]
    Food(#[from] FoodEatingError),

    #[error("Food entity is missing transform")]
    BadItemEntity,

    #[error("Too far from food to eat")]
    TooFarFromFood,
}

impl EatItemSubactivity {
    pub async fn eat_held(&self, ctx: &ActivityContext, item: Entity) -> Result<(), EatItemError> {
        let eater = ctx.entity();
        ctx.world()
            .resource::<QueuedUpdates>()
            .queue("begin eating held food", move |world| {
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
                let insert_result = world.add_now(
                    item,
                    BeingEatenComponent {
                        eater,
                        is_equipped: true,
                    },
                );
                debug_assert!(insert_result.is_ok());
                Ok(())
            });

        self.wait_for_event(ctx, eater, item).await
    }

    async fn wait_for_event(
        &self,
        ctx: &ActivityContext,
        eater: Entity,
        item: Entity,
    ) -> Result<(), EatItemError> {
        let eat_result = ctx
            .subscribe_to_specific_until(item, EntityEventType::BeenEaten, |evt| {
                match evt {
                    EntityEventPayload::BeenEaten(Ok(actual_eater)) if actual_eater == eater => {
                        Ok(Ok(actual_eater))
                    }
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

    pub async fn eat_nearby(
        &self,
        ctx: &ActivityContext,
        item: Entity,
    ) -> Result<(), EatItemError> {
        let eater = ctx.entity();
        // TODO any need to queue this to the next tick?

        // ensure close enough
        match ctx.check_entity_distance(item, MAX_EAT_DISTANCE.powi(2)) {
            DistanceCheckResult::NotAvailable => return Err(EatItemError::BadItemEntity),
            DistanceCheckResult::TooFar => return Err(EatItemError::TooFarFromFood),
            DistanceCheckResult::InRange => {} // good
        };

        // start eating
        ctx.world()
            .resource::<QueuedUpdates>()
            .queue("begin eating nearby food", move |world| {
                let _ = world.add_now(
                    item,
                    BeingEatenComponent {
                        eater,
                        is_equipped: false,
                    },
                );
                Ok(())
            });

        self.wait_for_event(ctx, eater, item).await
    }
}
