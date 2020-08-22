use crate::ai::consideration::{BlockTypeMatchesConsideration, MyProximityToConsideration};
use crate::ai::input::BlockTypeMatch;
use crate::ai::{AiAction, AiContext};
use ai::{AiBox, Consideration, Context, DecisionWeight, Dse};
use unit::world::WorldPosition;
use world::block::BlockType;

pub struct BreakBlockDse(pub WorldPosition);

impl Dse<AiContext> for BreakBlockDse {
    fn name(&self) -> &'static str {
        "Break Block"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            // for now, direct distance
            // TODO calculate path and use length, cache path which can be reused by movement system
            // TODO has the right tool/is the right tool nearby/close enough in society storage
            AiBox::new(MyProximityToConsideration {
                target: self.0,
                max_distance: 400.0,
            }),
            AiBox::new(BlockTypeMatchesConsideration(
                self.0,
                BlockTypeMatch::IsNot(BlockType::Air),
            )),
        ]
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        AiAction::GoBreakBlock(self.0)
    }
}
