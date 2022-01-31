use crate::activity::{HaulPurpose, HaulSource};
use crate::ai::consideration::{
    FindLocalGradedItemConsideration, HasExtraHandsForHaulingConsideration,
    MyProximityToConsideration,
};
use std::fmt::Debug;

use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};
use crate::build::BuildMaterial;
use crate::ecs::*;
use crate::item::{ItemFilter, ItemFilterable};
use crate::job::SocietyJobHandle;
use crate::{HaulTarget, ItemStackComponent};
use ai::{Considerations, Context, DecisionWeight, Dse};
use common::OrderedFloat;

use unit::world::WorldPosition;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GatherMaterialsDse {
    pub target: WorldPosition,
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
        out.add(HasExtraHandsForHaulingConsideration(
            self.extra_hands_for_haul,
            Some(self.filter()),
        ));
        out.add(MyProximityToConsideration(self.target.centred()));
        out.add(FindLocalGradedItemConsideration {
            filter: self.filter(),
            max_radius: 20,
            normalize_range: 1.0,
        });
        // TODO check society containers
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn action(&self, blackboard: &mut AiBlackboard, _: Option<AiTarget>) -> AiAction {
        // if we are already hauling a matching item, choose that

        if let Some((best_item, source)) = self.choose_best_item(blackboard) {
            // TODO separate HaulTarget to drop nearby/adjacent
            return AiAction::Haul(
                best_item,
                source,
                HaulTarget::Drop(self.target.centred()),
                HaulPurpose::MaterialGathering(self.job),
            );
        }

        todo!("gather materials from a different source")
    }

    fn as_debug(&self) -> Option<&dyn Debug> {
        Some(self)
    }
}

impl GatherMaterialsDse {
    fn choose_best_item(&self, blackboard: &AiBlackboard) -> Option<(Entity, HaulSource)> {
        let filter = self.filter();
        if let AiAction::Haul(haulee, source, ..) = blackboard.ai.last_action() {
            if (*haulee, Some(blackboard.world)).matches(filter) {
                // use this thing in inventory for hauling. even if the stack is too big, it will be
                // split when it arrives at the build site
                return Some((*haulee, *source));
            }
        }

        if let Some((_, found_items)) = blackboard.local_area_search_cache.get(&filter) {
            // choose the nearest out of found local items
            // TODO take the stack size into account too, choose the biggest
            if let Some(found) = found_items
                .iter()
                .min_by_key(|(_, _, distance, _)| OrderedFloat(*distance))
                .map(|(e, _, _, _)| *e)
            {
                let src = match blackboard.world.component::<ItemStackComponent>(found) {
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
                return Some((found, src));
            }
        }

        None
    }
}
