use ai::{Consideration, ConsiderationParameter, Context, Curve};
use common::*;

use crate::ai::{AiContext, AiInput};
use crate::item::ItemFilter;

declare_entity_metric!(HOLD_ITEM, "ai_item_holding", "Is holding an item", "filter");

/// Switch, 1 if holding an item matching the filter, otherwise 0
pub struct HoldingItemConsideration(pub ItemFilter);

impl Consideration<AiContext> for HoldingItemConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::HasInInventory(self.0)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop // bounded already
    }

    #[cfg(feature = "metrics")]
    fn log_metric(&self, entity: &str, value: f32) {
        entity_metric!(HOLD_ITEM, entity, value, &format!("{}", self.0));
    }
}
