use crate::ai::consideration::{
    BlockTypeMatchesConsideration, MyProximityToConsideration, Proximity,
};
use crate::ai::input::BlockTypeMatch;
use crate::ai::{AiAction, AiContext};
use ai::{AiBox, Consideration, Context, DecisionWeightType, Dse};
use unit::world::WorldPosition;
use world::block::BlockType;

pub struct BreakBlockDse(pub WorldPosition);
pub struct BuildBlockDse(pub WorldPosition, pub BlockType);

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

impl Dse<AiContext> for BuildBlockDse {
    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            AiBox::new(MyProximityToConsideration {
                target: self.0.centred(),
                proximity: Proximity::Walkable,
            }),
            // assume the emitting job/command has checked the block is valid and buildable
        ]
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        AiAction::GoBuildBlock(self.0, self.1)
    }
}
