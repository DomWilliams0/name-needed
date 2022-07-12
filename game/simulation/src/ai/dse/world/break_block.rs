use crate::ai::consideration::{
    MyProximityToTargetConsideration, TargetBlockTypeMatchesConsideration,
};

use crate::ai::input::BlockTypeMatch;
use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};

use ai::{Considerations, DecisionWeight, Dse, TargetOutput, Targets};

use unit::world::WorldPosition;
use world_types::BlockType;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BreakBlockDse(pub WorldPosition);

impl Dse<AiContext> for BreakBlockDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        // for now, direct distance
        // TODO calculate path and use length, cache path which can be reused by movement system
        // TODO has the right tool/is the right tool nearby/close enough in society storage
        out.add(MyProximityToTargetConsideration);
        out.add(TargetBlockTypeMatchesConsideration(BlockTypeMatch::IsNot(
            BlockType::Air,
        )));
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn target(&self, targets: &mut Targets<AiContext>, _: &mut AiBlackboard) -> TargetOutput {
        targets.add(AiTarget::Block(self.0));
        TargetOutput::TargetsCollected
    }

    fn action(&self, _: &mut AiBlackboard, tgt: Option<AiTarget>) -> AiAction {
        debug_assert_eq!(tgt, Some(AiTarget::Block(self.0)));
        AiAction::GoBreakBlock(self.0)
    }
}
