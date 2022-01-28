use ai::{Considerations, Context, DecisionWeight, Dse};
use common::*;

use crate::ai::consideration::{FindLocalGradedItemConsideration, HungerConsideration};
use crate::ai::{AiAction, AiContext};
use crate::item::ItemFilter;
/// Finds food nearby to pick up
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FindLocalFoodDse;

const FOOD_FILTER: ItemFilter = ItemFilter::HasComponent("edible");
const FOOD_MAX_RADIUS: u32 = 20;

impl Dse<AiContext> for FindLocalFoodDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(HungerConsideration);
        // TODO "I can/want to move" consideration
        out.add(FindLocalGradedItemConsideration {
            filter: FOOD_FILTER,
            max_radius: FOOD_MAX_RADIUS,
            normalize_range: 2.0, // 2 perfect food nearby is enough for a 1
        });
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn action(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
    ) -> <AiContext as Context>::Action {
        let food = blackboard.inventory_search_cache
            .get(&FOOD_FILTER)
            .map(|slot| slot.get(blackboard.inventory.expect("inventory expected"), blackboard.world))
            .or_else(|| {
                blackboard
                    .local_area_search_cache
                    .get(&FOOD_FILTER)
                    .map(|(_, found_items)| {
                        let (best_item, item_pos, _, condition) = found_items
                            .iter()
                            .max_by_key(|(_, _, distance, condition)| {
                                // flip distance so closer == higher score
                                let distance = FOOD_MAX_RADIUS as f32 - distance;
                                OrderedFloat(condition.value() * distance)
                            })
                            .expect("food search is empty");

                        debug!("chose best item to pick up"; "item" => best_item, "pos" => %item_pos, "condition" => ?condition);
                        *best_item
                    })

            })
        .expect("local food search succeeded but missing result in both inventory and local cache");

        AiAction::GoEquip(food)
    }
}
