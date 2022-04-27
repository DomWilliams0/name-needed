use common::*;

use crate::ecs::*;
use crate::event::EntityEventQueue;
use crate::needs::food::component::{BeingEatenComponent, Fuel, HungerComponent};
use crate::simulation::EcsWorldRef;
use crate::{
    ActivityComponent, ConditionComponent, EdibleItemComponent, EntityEvent, EntityEventPayload,
    InventoryComponent,
};

// fuel used per tick TODO depends on time rate
// TODO species metabolism
const BASE_METABOLISM: f32 = 0.5;

// amount gained when eating per tick
const BASE_EAT_RATE: Fuel = 5;

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
            // TODO individual metabolism rate
            // TODO elaborate and specify metabolism rate
            // TODO take into account general movement speed in addition to this
            let metabolism = 1.0;
            let fuel_used = BASE_METABOLISM * metabolism * activity.exertion();

            hunger.consume_fuel(fuel_used);
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

                // calculate how much to consume this tick
                // TODO individual rate
                // TODO depends on food type/consistency
                let fuel_to_consume = BASE_EAT_RATE;
                let proportion_to_eat = NormalizedFloat::new(
                    (fuel_to_consume as f32 / edible.total_nutrition as f32).min(0.2),
                );

                // do the eat
                eater_hunger.add_fuel(fuel_to_consume);
                condition.0 -= proportion_to_eat;

                trace!("{eater} is eating", eater = being_eaten.eater;
                    "new_hunger" => ?eater_hunger.satiety(),
                    "new_food_condition" => ?condition.0,
                );

                // TODO while eating/for a short time afterwards, add a hunger multiplier e.g. 0.2

                if condition.0.value().value() <= 0.0 {
                    debug!("food has been consumed");

                    // remove from eater's inventory if applicable
                    if being_eaten.is_equipped {
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
