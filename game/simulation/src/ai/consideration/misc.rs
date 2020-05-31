use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};
use common::OrderedFloat;

pub struct ConstantConsideration(pub f32);

impl Consideration<AiContext> for ConstantConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::Constant(OrderedFloat(self.0))
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
    #[cfg(feature = "metrics")]
    fn log_metric(&self, _: &str, _: f32) {}
}
