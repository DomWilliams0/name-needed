use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};

pub struct HasFreeHandsToHoldTargetConsideration;

impl Consideration<AiContext> for HasFreeHandsToHoldTargetConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::HasFreeHandsToHoldTarget
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}
