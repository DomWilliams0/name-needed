use crate::activity::{HaulPurpose, HaulSource};
use crate::ai::consideration::{
    BlockTypeMatchesConsideration, FindLocalGradedItemConsideration,
    HasExtraHandsForHaulingConsideration, HoldingItemConsideration, MyProximityToConsideration,
    Proximity,
};
use crate::ai::input::AiInput::HasInInventory;
use crate::ai::input::{BlockTypeMatch, LocalAreaSearch};
use crate::ai::{AiAction, AiContext};
use crate::build::BuildMaterial;
use crate::ecs::*;
use crate::item::{HauledItemComponent, ItemFilter, ItemFilterable};
use crate::job::{SocietyJobHandle, SocietyJobRef};
use crate::HaulTarget;
use ai::{AiBox, Consideration, Context, DecisionWeightType, Dse};
use common::OrderedFloat;
use unit::world::WorldPosition;
use world::block::BlockType;

pub struct BreakBlockDse(pub WorldPosition);

pub struct GatherMaterialsDse {
    pub target: WorldPosition,
    pub material: BuildMaterial,
    pub job: SocietyJobHandle,
    pub extra_hands_for_haul: u16,
}

impl Dse<AiContext> for BreakBlockDse {
    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            // for now, direct distance
            // TODO calculate path and use length, cache path which can be reused by movement system
            // TODO has the right tool/is the right tool nearby/close enough in society storage
            AiBox::new(MyProximityToConsideration {
                target: self.0.centred(),
                proximity: Proximity::Walkable,
            }),
            AiBox::new(BlockTypeMatchesConsideration(
                self.0,
                BlockTypeMatch::IsNot(BlockType::Air),
            )),
        ]
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
    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            AiBox::new(HasExtraHandsForHaulingConsideration(
                self.extra_hands_for_haul,
                Some(self.filter()),
            )),
            AiBox::new(MyProximityToConsideration {
                target: self.target.centred(),
                proximity: Proximity::Walkable,
            }),
            AiBox::new(FindLocalGradedItemConsideration {
                filter: self.filter(),
                max_radius: 20,
                normalize_range: 1.0,
            }),
            // TODO check society containers
        ]
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
    ) -> <AiContext as Context>::Action {
        // if we are already hauling a matching item, choose that

        if let Some(best_item) = choose_best_item(blackboard, self.filter()) {
            // TODO separate HaulTarget to drop nearby/adjacent
            return AiAction::Haul(
                best_item,
                HaulSource::PickUp,
                HaulTarget::Drop(self.target.centred()),
                HaulPurpose::MaterialGathering(self.job),
            );
        }

        todo!("gather materials from a different source")
    }
}

fn choose_best_item(
    blackboard: &<AiContext as Context>::Blackboard,
    filter: ItemFilter,
) -> Option<Entity> {
    // if we are already hauling a matching item, choose that
    let inventory = blackboard.inventory.unwrap(); // definitely has an inventory by now
    for equipped in inventory.all_equipped_items() {
        if (equipped, Some(blackboard.world)).matches(filter) {
            // haul this
            return Some(equipped);
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
            return Some(found);
        }
    }

    None
}
