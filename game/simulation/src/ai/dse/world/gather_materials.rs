use crate::activity::{HaulPurpose, HaulSource};
use crate::ai::consideration::{
    HasExtraHandsForHaulingConsideration, MyProximityToConsideration,
    MyProximityToTargetConsideration,
};
use std::fmt::Debug;

use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};
use crate::build::BuildMaterial;
use crate::ecs::*;
use crate::item::ItemFilter;
use crate::job::SocietyJobHandle;
use crate::{HaulTarget, ItemStackComponent};
use ai::{Considerations, DecisionWeight, Dse, TargetOutput, Targets};

use unit::world::WorldPosition;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GatherMaterialsDse {
    pub build_pos: WorldPosition,
    pub material: BuildMaterial,
    pub job: SocietyJobHandle,
    pub extra_hands_for_haul: u16,
}

impl GatherMaterialsDse {
    fn filter(&self) -> ItemFilter {
        ItemFilter::MatchesDefinition(self.material.definition())
    }
}

impl Dse<AiContext> for GatherMaterialsDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(HasExtraHandsForHaulingConsideration {
            extra_hands: self.extra_hands_for_haul,
            target: None, // target entity instead
        });
        out.add(MyProximityToTargetConsideration); // distance to material
        out.add(MyProximityToConsideration(AiTarget::Block(self.build_pos))); // distance to build
                                                                              // TODO consider item stack size and condition
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn target(
        &self,
        targets: &mut Targets<AiContext>,
        blackboard: &mut AiBlackboard,
    ) -> TargetOutput {
        let filter = self.filter();

        // TODO search range could depend on entity senses
        // TODO share search range with food searching
        blackboard.search_local_entities(filter, 50.0, 5, |item| {
            targets.add(AiTarget::Entity(item.entity));
            true
        });

        // TODO check society containers too

        TargetOutput::TargetsCollected
    }

    fn action(&self, blackboard: &mut AiBlackboard, tgt: Option<AiTarget>) -> AiAction {
        let item = tgt.and_then(|t| t.entity()).expect("invalid target");

        let src = match blackboard.world.component::<ItemStackComponent>(item) {
            Ok(stack) => {
                // only take as much of the stack as is needed
                let n = self
                    .material
                    .quantity()
                    .get()
                    .min(stack.stack.total_count());
                HaulSource::PickUpSplitStack(n)
            }
            _ => HaulSource::PickUp,
        };

        AiAction::Haul(
            item,
            src,
            HaulTarget::Drop(self.build_pos.centred()),
            HaulPurpose::MaterialGathering(self.job),
        )
    }

    fn as_debug(&self) -> Option<&dyn Debug> {
        Some(self)
    }
}
