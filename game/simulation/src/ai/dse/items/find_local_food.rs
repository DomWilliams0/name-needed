use specs::WorldExt;

use ai::{Considerations, DecisionWeight, Dse, TargetOutput, Targets};
use common::*;

use crate::ai::consideration::{
    HasFreeHandsToHoldTargetConsideration, HungerConsideration, MyProximityToTargetConsideration,
};
use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};
use crate::item::ItemFilter;
use crate::{ComponentWorld, EdibleItemComponent, HungerComponent};

/// Finds food nearby to pick up
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FindLocalEquippableFoodDse;

/// Finds food nearby to graze on
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FindLocalGrazingFoodDse;

const FOOD_FILTER: ItemFilter = ItemFilter::HasComponent("edible");
const FOOD_MAX_RADIUS: u32 = 20;

fn common_considerations(out: &mut Considerations<AiContext>) {
    out.add(HungerConsideration);
    // TODO food interests
    out.add(MyProximityToTargetConsideration);
    // TODO target food condition consideration
    // TODO "I can/want to move" consideration
}

fn find_targets(targets: &mut Targets<AiContext>, blackboard: &mut AiBlackboard) -> TargetOutput {
    if let Ok(hunger) = blackboard
        .world
        .component::<HungerComponent>(blackboard.entity)
    {
        let interests = &hunger.food_interest;
        let edibles = blackboard.world.read_component::<EdibleItemComponent>();
        blackboard.search_local_entities(FOOD_FILTER, FOOD_MAX_RADIUS as f32, 10, |item| {
            let edible = item
                .entity
                .get(&edibles)
                .expect("found food expected to be edible");

            // only consider food as a target if it matches the interests at all
            interests
                .eats(&edible.flavours)
                .tap(|_| targets.add(AiTarget::Entity(item.entity)))
        });
    }

    TargetOutput::TargetsCollected
}

impl Dse<AiContext> for FindLocalEquippableFoodDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        common_considerations(out);

        // needs to pick up
        out.add(HasFreeHandsToHoldTargetConsideration);
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn target(
        &self,
        targets: &mut Targets<AiContext>,
        blackboard: &mut AiBlackboard,
    ) -> TargetOutput {
        find_targets(targets, blackboard)
    }

    fn action(&self, _: &mut AiBlackboard, target: Option<AiTarget>) -> AiAction {
        let target = target.and_then(|t| t.entity()).expect("bad target");
        AiAction::GoEquip(target)
    }
}

impl Dse<AiContext> for FindLocalGrazingFoodDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        common_considerations(out);
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn target(
        &self,
        targets: &mut Targets<AiContext>,
        blackboard: &mut AiBlackboard,
    ) -> TargetOutput {
        find_targets(targets, blackboard)
    }

    fn action(&self, _: &mut AiBlackboard, target: Option<AiTarget>) -> AiAction {
        let target = target.and_then(|t| t.entity()).expect("bad target");
        AiAction::GoEat(target)
    }
}
