use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};

/// Takes into account target food flavours and the interests of the entity
pub struct LikesToEatTargetConsideration;

impl Consideration<AiContext> for LikesToEatTargetConsideration {
    fn curve(&self) -> Curve {
        // interest above ~0.7 is 1.0
        Curve::Linear(1.3, 0.0)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::FoodInterestInTarget
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop // already normalized
    }
}
