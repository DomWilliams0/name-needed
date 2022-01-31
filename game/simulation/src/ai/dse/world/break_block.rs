use crate::ai::consideration::{BlockTypeMatchesConsideration, MyProximityToConsideration};

use crate::ai::input::BlockTypeMatch;
use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};

use ai::{Considerations, DecisionWeight, Dse};

use unit::world::WorldPosition;
use world::block::BlockType;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BreakBlockDse(pub WorldPosition);

impl Dse<AiContext> for BreakBlockDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        // for now, direct distance
        // TODO calculate path and use length, cache path which can be reused by movement system
        // TODO has the right tool/is the right tool nearby/close enough in society storage
        out.add(MyProximityToConsideration(self.0.centred()));
        out.add(BlockTypeMatchesConsideration(
            self.0,
            BlockTypeMatch::IsNot(BlockType::Air),
        ));
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn action(&self, _: &mut AiBlackboard, _: Option<AiTarget>) -> AiAction {
        AiAction::GoBreakBlock(self.0)
    }
}
