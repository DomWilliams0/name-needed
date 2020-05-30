use ai::{AiBox, Consideration, Context, DecisionWeight, Dse};

use crate::ai::activity::{AiAction, ItemsToPickUp};
use crate::ai::consideration::{
    FindLocalItemConsideration, HoldingItemConsideration, HungerConsideration,
};
use crate::ai::AiContext;
use crate::item::{ItemClass, ItemFilter, LooseItemReference};
use common::*;

pub struct UseHeldFoodDse;
pub struct FindLocalFoodDse;

const FOOD_FILTER: ItemFilter = ItemFilter::Class(ItemClass::Food);
const FOOD_MAX_RADIUS: u32 = 20;

impl Dse<AiContext> for UseHeldFoodDse {
    fn name(&self) -> &str {
        "Use Held Item - Food"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            AiBox::new(HungerConsideration),
            AiBox::new(HoldingItemConsideration(FOOD_FILTER)),
        ]
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::BasicNeeds
    }

    fn action(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
    ) -> <AiContext as Context>::Action {
        let item = blackboard
            .inventory_search_cache
            .get(&FOOD_FILTER)
            .expect("item search succeeded but missing result in cache");

        AiAction::UseHeldItem(LooseItemReference(*item))
    }
}

impl Dse<AiContext> for FindLocalFoodDse {
    fn name(&self) -> &str {
        "Find Local Item - Food"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            AiBox::new(HungerConsideration),
            // TODO "I can/want to move" consideration
            AiBox::new(FindLocalItemConsideration {
                filter: FOOD_FILTER,
                max_radius: FOOD_MAX_RADIUS,
                normalize_range: 2.0, // 2 perfect food nearby is enough for a 1
            }),
        ]
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::BasicNeeds
    }

    fn action(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
    ) -> <AiContext as Context>::Action {
        let (_, found_items) = blackboard
            .local_area_search_cache
            .get(&FOOD_FILTER)
            .expect("local food search succeeded but missing result in cache");

        debug_assert!(!found_items.is_empty());

        // sort items in reverse desirability - best items are at the end so we can pop them efficiently
        let desired_items = found_items
            .iter()
            .sorted_by_key(|(_, _, distance, condition)| {
                // flip distance so closer == higher score
                let distance = FOOD_MAX_RADIUS as f32 - distance;
                OrderedFloat(condition.value() * distance)
            })
            .map(|&(item, pos, _, _)| (item, pos))
            .collect();

        AiAction::GoPickUp(ItemsToPickUp(FOOD_FILTER, desired_items))
    }
}
