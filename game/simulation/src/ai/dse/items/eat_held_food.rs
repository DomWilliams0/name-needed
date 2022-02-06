use ai::{Considerations, DecisionWeight, Dse};
use common::*;

use crate::ai::consideration::{
    CanUseHeldItemConsideration, HoldingItemConsideration, HungerConsideration,
};
use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};
use crate::item::ItemFilter;

/// Equips food in inventory and eats it
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EatHeldFoodDse;

const FOOD_FILTER: ItemFilter = ItemFilter::HasComponent("edible");

impl Dse<AiContext> for EatHeldFoodDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(HungerConsideration);
        out.add(HoldingItemConsideration(FOOD_FILTER));
        out.add(CanUseHeldItemConsideration(FOOD_FILTER));
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::BasicNeeds
    }

    fn action(&self, blackboard: &mut AiBlackboard, _: Option<AiTarget>) -> AiAction {
        let slot = blackboard
            .inventory_search_cache
            .get(&FOOD_FILTER)
            .expect("item search succeeded but missing result in cache");

        let inventory = blackboard.inventory.unwrap(); // certainly has an inv by now
        let food = slot.get(inventory, blackboard.world);

        AiAction::EatHeldItem(food)
    }
}
