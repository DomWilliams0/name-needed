use ai::{AiBox, Consideration, Context, DecisionWeightType, Dse};

use crate::ai::consideration::{
    CanUseHeldItemConsideration, FindLocalGradedItemConsideration, HoldingItemConsideration,
    HungerConsideration,
};
use crate::ai::{AiAction, AiContext};
use crate::item::{ItemFilter, ItemsToPickUp};
use common::*;

/// Equips food in inventory and eats it
pub struct EatHeldFoodDse;

/// Finds food nearby to pick up
pub struct FindLocalFoodDse;

const FOOD_FILTER: ItemFilter = ItemFilter::HasComponent("edible");
const FOOD_MAX_RADIUS: u32 = 20;

impl Dse<AiContext> for EatHeldFoodDse {
    fn name(&self) -> &'static str {
        "Use Held Item - Food"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            AiBox::new(HungerConsideration),
            AiBox::new(HoldingItemConsideration(FOOD_FILTER)),
            AiBox::new(CanUseHeldItemConsideration(FOOD_FILTER)),
        ]
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::BasicNeeds
    }

    fn action(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
    ) -> <AiContext as Context>::Action {
        let slot = blackboard
            .inventory_search_cache
            .get(&FOOD_FILTER)
            .expect("item search succeeded but missing result in cache");

        let inventory = blackboard.inventory.unwrap(); // certainly has an inv by now
        let food = slot.get(inventory, blackboard.world);

        AiAction::EatHeldItem(food)
    }
}

impl Dse<AiContext> for FindLocalFoodDse {
    fn name(&self) -> &'static str {
        "Find Local Item - Food"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            AiBox::new(HungerConsideration),
            // TODO "I can/want to move" consideration
            AiBox::new(FindLocalGradedItemConsideration {
                filter: FOOD_FILTER,
                max_radius: FOOD_MAX_RADIUS,
                normalize_range: 2.0, // 2 perfect food nearby is enough for a 1
            }),
        ]
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
    ) -> <AiContext as Context>::Action {
        let (_, found_items) = blackboard
            .local_area_search_cache
            .get(&FOOD_FILTER)
            .expect("local food search succeeded but missing result in cache");

        let (best_item, item_pos, _, condition) = found_items
            .iter()
            .max_by_key(|(_, _, distance, condition)| {
                // flip distance so closer == higher score
                let distance = FOOD_MAX_RADIUS as f32 - distance;
                OrderedFloat(condition.value() * distance)
            })
            .expect("food search is empty");

        debug!("chose best item to pick up"; "item" => best_item, "pos" => %item_pos, "condition" => ?condition);
        AiAction::GoEquip(*best_item)
    }
}
