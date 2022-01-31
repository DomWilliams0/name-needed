use ai::{Considerations, DecisionWeight, Dse, TargetOutput, Targets};
use common::*;

use crate::ai::consideration::{
    HasFreeHandsToHoldTargetConsideration, HungerConsideration, MyProximityToConsideration,
    MyProximityToTargetConsideration,
};
use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};
use crate::item::ItemFilter;
/// Finds food nearby to pick up
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FindLocalFoodDse;

const FOOD_FILTER: ItemFilter = ItemFilter::HasComponent("edible");
const FOOD_MAX_RADIUS: u32 = 20;

impl Dse<AiContext> for FindLocalFoodDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(HungerConsideration);
        out.add(MyProximityToTargetConsideration);
        out.add(HasFreeHandsToHoldTargetConsideration);
        // TODO "I can/want to move" consideration
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn target(
        &self,
        targets: &mut Targets<AiContext>,
        blackboard: &mut AiBlackboard,
    ) -> TargetOutput {
        blackboard.search_local_entities(FOOD_FILTER, FOOD_MAX_RADIUS as f32, 10, |item| {
            targets.add(AiTarget::Entity(item.entity));
            true
        });

        TargetOutput::TargetsCollected
    }

    fn action(&self, _: &mut AiBlackboard, target: Option<AiTarget>) -> AiAction {
        let target = target.and_then(|t| t.entity()).expect("bad target");
        AiAction::GoEquip(target)
    }
}
