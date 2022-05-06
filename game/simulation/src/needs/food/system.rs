use common::*;

use crate::ecs::*;
use crate::event::EntityEventQueue;
use crate::needs::food::component::{BeingEatenComponent, HungerComponent};
use crate::needs::food::EatType;
use crate::simulation::EcsWorldRef;
use crate::{
    ActivityComponent, ConditionComponent, EdibleItemComponent, EntityEvent, EntityEventPayload,
    InventoryComponent,
};

// amount gained when eating per tick
// const BASE_EAT_RATE: Fuel = 5;

#[derive(Error, Debug, Clone)]
pub enum FoodEatingError {
    #[error("Food is not equipped by the eater")]
    NotEquipped,
}

/// Decreases hunger over time
pub struct HungerSystem;

/// Food eating
pub struct EatingSystem;

impl<'a> System<'a> for HungerSystem {
    type SystemData = (
        WriteStorage<'a, HungerComponent>,
        ReadStorage<'a, ActivityComponent>, // for current exertion TODO moving average
    );

    fn run(&mut self, (mut hunger, activity): Self::SystemData) {
        for (hunger, activity) in (&mut hunger, &activity).join() {
            let metabolism = hunger.metabolism();
            hunger.hunger_mut().burn(metabolism, activity.exertion());
        }
    }
}

impl<'a> System<'a> for EatingSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldRef>,
        Write<'a, EntityEventQueue>,
        WriteStorage<'a, InventoryComponent>,
        ReadStorage<'a, BeingEatenComponent>,
        WriteStorage<'a, HungerComponent>,
        ReadStorage<'a, EdibleItemComponent>,
        WriteStorage<'a, ConditionComponent>,
    );

    fn run(
        &mut self,
        (
            entities,
            ecs_world,
            mut events,
            mut inv,
            eating,
            mut hunger,
            edible_item,
            mut condition,
        ): Self::SystemData,
    ) {
        for (item, being_eaten, edible, condition) in
            (&entities, &eating, &edible_item, &mut condition).join()
        {
            let item = item.into();
            log_scope!(o!("system" => "being-eaten", item));

            let mut do_eat = || {
                // get eater
                let (eater_inv, eater_hunger) = match ecs_world
                    .components(being_eaten.eater, ((&mut inv).maybe(), &mut hunger))
                {
                    Some(comps) => comps,
                    None => {
                        warn!("food eater doesn't have hunger component"; "eater" => being_eaten.eater);
                        return Some(Err(FoodEatingError::NotEquipped));
                    }
                };

                // do the eat
                // TODO variable speed for eating - hurried (fast) vs relaxed/idle (slow)
                let degradation = eater_hunger
                    .hunger_mut()
                    .eat(&edible.description, condition.0.value());
                condition.0 -= degradation;

                trace!("{eater} is eating", eater = being_eaten.eater;
                    "new_hunger" => ?eater_hunger.hunger().satiety(),
                    "food_degradation" => ?degradation,
                    "new_food_condition" => ?condition.0,
                );

                // TODO while eating/for a short time afterwards, add a hunger multiplier e.g. 0.2

                if condition.0.is_broken() {
                    debug!("food has been consumed");

                    // remove from eater's inventory if applicable
                    if let EatType::Held = being_eaten.ty {
                        if let Some(eater_inv) = eater_inv {
                            let remove_count = eater_inv.remove_item(item);
                            debug_assert!(remove_count > 0); // should have been in an equip slot
                        }
                    }

                    // queue food entity for deletion
                    let delete_result = entities.delete(item.into());
                    debug_assert!(delete_result.is_ok());

                    // do post event
                    Some(Ok(being_eaten.eater))
                } else {
                    // still eating
                    None
                }
            };

            if let Some(result) = do_eat() {
                events.post(EntityEvent {
                    subject: item,
                    payload: EntityEventPayload::BeenEaten(result.clone()),
                });

                if result.is_ok() {
                    events.post(EntityEvent {
                        subject: being_eaten.eater,
                        payload: EntityEventPayload::HasEaten(item),
                    });
                }
            }
        }
    }
}
