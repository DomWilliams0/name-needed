use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::input::BlockTypeMatch;
use crate::ai::{AiContext, AiInput};

pub struct TargetBlockTypeMatchesConsideration(pub BlockTypeMatch);

impl Consideration<AiContext> for TargetBlockTypeMatchesConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::TargetBlockTypeMatches(self.0)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}
