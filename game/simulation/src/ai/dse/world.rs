use crate::activity::{HaulPurpose, HaulSource};
use crate::ai::consideration::{
    BlockTypeMatchesConsideration, FindLocalGradedItemConsideration,
    HasExtraHandsForHaulingConsideration, MyProximityToConsideration, Proximity,
};
use std::fmt::Debug;

use crate::ai::input::BlockTypeMatch;
use crate::ai::{AiAction, AiContext};
use crate::build::BuildMaterial;
use crate::ecs::*;
use crate::item::{ItemFilter, ItemFilterable};
use crate::job::{BuildDetails, SocietyJobHandle};
use crate::{HaulTarget, ItemStackComponent};
use ai::{AiBox, Consideration, Considerations, Context, DecisionWeightType, Dse};
use common::OrderedFloat;

use unit::world::WorldPosition;
use world::block::BlockType;

pub struct BreakBlockDse(pub WorldPosition);

pub struct BuildDse {
    pub job: SocietyJobHandle,
    pub details: BuildDetails,
}

pub struct GatherMaterialsDse {
    pub target: WorldPosition,
    pub material: BuildMaterial,
    pub job: SocietyJobHandle,
    pub extra_hands_for_haul: u16,
}

impl Dse<AiContext> for BreakBlockDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        // for now, direct distance
        // TODO calculate path and use length, cache path which can be reused by movement system
        // TODO has the right tool/is the right tool nearby/close enough in society storage
        out.add(MyProximityToConsideration {
            target: self.0.centred(),
            proximity: Proximity::Walkable,
        });
        out.add(BlockTypeMatchesConsideration(
            self.0,
            BlockTypeMatch::IsNot(BlockType::Air),
        ));
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        AiAction::GoBreakBlock(self.0)
    }
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
        out.add(MyProximityToConsideration {
            target: self.target.centred(),
            proximity: Proximity::Walkable,
        });
        out.add(FindLocalGradedItemConsideration {
            filter: self.filter(),
            max_radius: 20,
            normalize_range: 1.0,
        });
        // TODO check society containers
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
    ) -> <AiContext as Context>::Action {
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
}

impl GatherMaterialsDse {
    fn choose_best_item(
        &self,
        blackboard: &<AiContext as Context>::Blackboard,
    ) -> Option<(Entity, HaulSource)> {
        let filter = self.filter();
        if let AiAction::Haul(haulee, source, ..) = blackboard.ai.last_action() {
            if (*haulee, Some(blackboard.world)).matches(filter) {
                // use this thing in inventory for hauling
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

impl Dse<AiContext> for BuildDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        // TODO wants to work, can work
        // TODO has tool
        out.add(MyProximityToConsideration {
            target: self.details.pos.centred(),
            proximity: Proximity::Walkable,
        });
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(&self, _blackboard: &mut <AiContext as Context>::Blackboard) -> AiAction {
        AiAction::GoBuild {
            job: self.job,
            details: self.details.clone(),
        }
    }
}
