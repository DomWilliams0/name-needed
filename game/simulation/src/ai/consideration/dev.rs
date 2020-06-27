use crate::ai::{AiContext, AiInput};
use ai::{Consideration, ConsiderationParameter, Context, Curve};

pub struct HasDivineCommandConsideration;

impl Consideration<AiContext> for HasDivineCommandConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::DivineCommand
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}
