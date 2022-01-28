use ai::{Consideration, ConsiderationParameter, Context, Curve};
use unit::world::WorldPosition;

use crate::ai::input::BlockTypeMatch;
use crate::ai::{AiContext, AiInput};

pub struct BlockTypeMatchesConsideration(pub WorldPosition, pub BlockTypeMatch);

impl Consideration<AiContext> for BlockTypeMatchesConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::BlockTypeMatches(self.0, self.1)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}
