use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};
use crate::item::ItemFilter;

/// Same as `HoldingItemConsideration` but reduced if the item cannot be used immediately e.g. is
/// being hauled
pub struct CanUseHeldItemConsideration(pub ItemFilter);

impl Consideration<AiContext> for CanUseHeldItemConsideration {
    fn curve(&self) -> Curve {
        Curve::Linear(1.0, 0.3)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::CanUseHeldItem(self.0)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}
