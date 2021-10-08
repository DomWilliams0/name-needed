use common::newtype::AccumulativeInt;
use common::*;

use crate::ecs::*;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventQueue};
use crate::item::{EdibleItemComponent, InventoryComponent};
use crate::simulation::EcsWorldRef;
use crate::{ActivityComponent, ConditionComponent};

// TODO newtype for Fuel
pub type Fuel = u16;

// fuel used per tick TODO depends on time rate
// TODO species metabolism
const BASE_METABOLISM: f32 = 0.5;

// amount gained when eating per tick
const BASE_EAT_RATE: Fuel = 5;

// TODO generic needs component with hunger/thirst/toilet/social etc
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("hunger")]
pub struct HungerComponent {
    current_fuel: AccumulativeInt<Fuel>,
    max_fuel: Fuel,
}

/// A food item is being eaten by the given eater
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("being-eaten")]
pub struct BeingEatenComponent {
    pub eater: Entity,
}

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

            debug_assert!(fuel_used.is_sign_positive());
            hunger.current_fuel -= fuel_used;
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
            log_scope!(o!("system" => "being-eating", item));

            let mut do_eat = || {
                // get eater
                let (eater_inv, eater_hunger) = match ecs_world
                    .components(being_eaten.eater, (&mut inv, &mut hunger))
                {
                    Some(comps) => comps,
                    None => {
                        warn!("food eater doesn't have inventory or hunger component"; "eater" => being_eaten.eater);
                        return Some(Err(FoodEatingError::NotEquipped));
                    }
                };

                // calculate how much to consume this tick
                let fuel_to_consume = BASE_EAT_RATE; // TODO individual rate
                let proportion_to_eat = NormalizedFloat::clamped(
                    fuel_to_consume as f32 / edible.total_nutrition as f32,
                );

                // do the eat
                eater_hunger.current_fuel.add(fuel_to_consume);
                condition.0 -= proportion_to_eat;

                trace!("{eater} is eating", eater = being_eaten.eater;
                    "new_hunger" => ?eater_hunger.current_fuel,
                    "new_food_condition" => ?condition.0,
                );

                // TODO while eating/for a short time afterwards, add a hunger multiplier e.g. 0.2

                if condition.0.value().value() <= 0.0 {
                    debug!("food has been consumed");

                    // remove from eater's inventory
                    let remove_count = eater_inv.remove_item(item);
                    debug_assert!(remove_count > 0); // should have been in an equip slot

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

impl HungerComponent {
    pub fn new(max: Fuel) -> Self {
        Self {
            current_fuel: AccumulativeInt::new(max),
            max_fuel: max,
        }
    }

    pub fn hunger(&self) -> NormalizedFloat {
        NormalizedFloat::new(self.current_fuel.value() as f32 / self.max_fuel as f32)
    }

    /// (a, b) -> a/b fuel
    pub fn satiety(&self) -> (Fuel, Fuel) {
        (self.current_fuel.value(), self.max_fuel)
    }

    pub fn set_satiety(&mut self, proportion: NormalizedFloat) {
        let fuel = self.max_fuel as f64 * proportion.value() as f64;
        self.current_fuel = AccumulativeInt::new(fuel as Fuel)
    }
}

impl<V: Value> ComponentTemplate<V> for HungerComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let max = values.get_int("max")?;
        Ok(Box::new(Self::new(max)))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }
}

register_component_template!("hunger", HungerComponent);
