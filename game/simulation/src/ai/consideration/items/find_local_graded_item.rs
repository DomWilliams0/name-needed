use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};

use crate::item::ItemFilter;

use common::*;

declare_entity_metric!(
    FIND_ITEM,
    "ai_item_find",
    "Can find a local item",
    "filter",
    "radius"
);

/// Finds items matching filter in inventory and nearby, preferring those in better condition if applicable. Will
/// avoid items reserved by the entity's society
///
/// TODO search society stores as well
pub struct FindLocalGradedItemConsideration {
    pub filter: ItemFilter,
    pub max_radius: u32,
    /// Max input value to map to 1.0
    /// e.g. 5.0: 2 perfect items would have an input of 2.0, so this maps to less than 0.5
    ///  but 1.0 would map that to the full 1.0
    pub normalize_range: f32,
}

impl Consideration<AiContext> for FindLocalGradedItemConsideration {
    fn curve(&self) -> Curve {
        Curve::Exponential(2.0, -4.0, 0.0, -1.0, 1.0)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::CanFindGradedItems {
            filter: self.filter,
            max_radius: self.max_radius,
            // a tad arbitrary - assumes the average item condition is 0.25 so 4x range would
            // be enough to score a 1.0
            max_count: (self.normalize_range * 4.0) as u32,
        }
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Range {
            min: 0.0,
            max: self.normalize_range,
        }
    }

    #[cfg(feature = "metrics")]
    fn log_metric(&self, entity: &str, value: f32) {
        entity_metric!(
            FIND_ITEM,
            entity,
            value,
            &format!("{}", self.filter),
            &format!("{}", self.max_radius)
        );
    }
}
