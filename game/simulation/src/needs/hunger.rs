use common::newtype::AccumulativeInt;
use common::*;

use crate::ai::ActivityComponent;
use crate::ecs::*;
use crate::item::{
    BaseItemComponent, EdibleItemComponent, InventoryComponent, ItemClass, SlotReference,
    UsingItemComponent,
};

pub type Fuel = u16;

// fuel used per tick TODO depends on time rate
// TODO species metabolism
const BASE_METABOLISM: f32 = 0.5;

// amount gained when eating per tick
const BASE_EAT_RATE: Fuel = 5;

// TODO generic needs component with hunger/thirst/toilet/social etc
#[derive(Component)]
#[storage(VecStorage)]
pub struct HungerComponent {
    current_fuel: AccumulativeInt<Fuel>,
    max_fuel: Fuel,
}

pub struct HungerSystem;

pub struct EatingSystem;

enum EatResult {
    NoItem,
    /// Unconditionally delete item entity because something has gone wrong
    Errored(Entity),
    /// Delete item entity if food is finished
    Success(Entity),
}

impl<'a> System<'a> for HungerSystem {
    type SystemData = (
        WriteStorage<'a, HungerComponent>,
        ReadStorage<'a, ActivityComponent>, // for current exertion TODO moving average
    );

    fn run(&mut self, (mut hunger, activity): Self::SystemData) {
        for (hunger, activity) in (&mut hunger, &activity).join() {
            // TODO individual metabolism rate
            // TODO compensate multipliers
            let metabolism = 1.0;
            let fuel_used = BASE_METABOLISM * metabolism * activity.current.exertion();

            debug_assert!(fuel_used.is_sign_positive());
            hunger.current_fuel -= fuel_used;
        }
    }
}

impl<'a> System<'a> for EatingSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        WriteStorage<'a, InventoryComponent>,
        WriteStorage<'a, UsingItemComponent>,
        WriteStorage<'a, HungerComponent>,
        ReadStorage<'a, EdibleItemComponent>,
        WriteStorage<'a, BaseItemComponent>,
    );

    fn run(
        &mut self,
        (entities, mut inv, mut using, mut hunger, edible_item, mut base_item): Self::SystemData,
    ) {
        for (inv, using, hunger) in (&mut inv, &mut using, &mut hunger).join() {
            if let ItemClass::Food = using.class {
                let val = using.left.value();

                // food is already no more
                if val <= 0.0 {
                    continue;
                }

                let result = do_eat(inv, using, hunger, &edible_item, &mut base_item, &entities);

                if !matches!(result, EatResult::Success(_)) {
                    // ensure that this disaster ends soon by killing the item now
                    // one could say it's an ex food
                    using.left = NormalizedFloat::zero();
                }

                // food has ceased to be
                if using.left.value().is_zero() {
                    let _ = inv.remove_item(SlotReference::Base(using.base_slot));

                    // queue item entity for deletion
                    if let EatResult::Errored(item) | EatResult::Success(item) = result {
                        trace!("deleting food item {:?}", item);
                        if let Err(e) = entities.delete(item) {
                            warn!("failed to delete item: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

#[inline]
fn do_eat(
    inv: &InventoryComponent,
    using: &mut UsingItemComponent,
    hunger: &mut HungerComponent,
    edible_item: &ReadStorage<EdibleItemComponent>,
    base_item: &mut WriteStorage<BaseItemComponent>,
    entities: &Read<EntitiesRes>,
) -> EatResult {
    // get item from inventory
    let slot = SlotReference::Base(using.base_slot);
    let item = match inv.get(slot) {
        Ok(e) => e,
        Err(e) => {
            warn!(
                "failed to get item in use from inventory: {:?} - {:?}",
                slot, e
            );
            return EatResult::NoItem;
        }
    };

    let (base_item, edible) = match (base_item, edible_item).join().get(item, entities) {
        None => {
            warn!("food item missing base or edible components ({:?})", item);
            return EatResult::Errored(item);
        }
        Some(tup) => tup,
    };

    // calculate how much to consume this tick
    let fuel_to_consume = BASE_EAT_RATE; // TODO individual rate
    let proportion_to_eat =
        NormalizedFloat::clamped(fuel_to_consume as f32 / edible.total_nutrition as f32);

    // eat the damn thing
    hunger.current_fuel.add(fuel_to_consume);
    using.left -= proportion_to_eat.value();
    base_item.condition.set(using.left);

    // TODO while eating/for a short time afterwards, add a hunger multiplier e.g. 0.2

    EatResult::Success(item)
}

impl HungerComponent {
    pub fn new(starting: NormalizedFloat, max: Fuel) -> Self {
        let starting = starting.value() * max as f32;
        Self {
            current_fuel: AccumulativeInt::new(starting as Fuel),
            max_fuel: max,
        }
    }

    pub fn hunger(&self) -> NormalizedFloat {
        NormalizedFloat::new(self.current_fuel.value() as f32 / self.max_fuel as f32)
    }
}
