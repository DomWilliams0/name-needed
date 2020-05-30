use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};

pub struct ConstantConsideration(pub f32);

impl Consideration<AiContext> for ConstantConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::Constant(self.0)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}
