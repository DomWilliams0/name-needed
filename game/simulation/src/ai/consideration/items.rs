use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};
use crate::ecs::Entity;
use crate::item::ItemFilter;
use crate::SocietyHandle;
use common::*;

declare_entity_metric!(HOLD_ITEM, "ai_item_holding", "Is holding an item", "filter");
declare_entity_metric!(
    FIND_ITEM,
    "ai_item_find",
    "Can find a local item",
    "filter",
    "radius"
);

/// Switch, 1 if holding an item matching the filter, otherwise 0
pub struct HoldingItemConsideration(pub ItemFilter);

/// Finds items matching filter nearby, preferring those in better condition if applicable. Will
/// avoid items reserved by the entity's society
///
/// TODO consider society stores before scanning the local area? or put society stores in a separate consideration
pub struct FindLocalGradedItemConsideration {
    pub filter: ItemFilter,
    pub max_radius: u32,
    /// Max input value to map to 1.0
    /// e.g. 5.0: 2 perfect items would have an input of 2.0, so this maps to less than 0.5
    ///  but 1.0 would map that to the full 1.0
    pub normalize_range: f32,
}

/// 1 if this entity has this number of extra hands available for hauling, or is already hauling
/// a matching entity, else 0
// TODO also count currently occupied hands as "available", could drop current item to haul this
pub struct HasExtraHandsForHaulingConsideration(pub u16, pub Option<ItemFilter>);

/// Same as `HoldingItemConsideration` but reduced if the item cannot be used immediately e.g. is
/// being hauled
pub struct CanUseHeldItemConsideration(pub ItemFilter);

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

impl Consideration<AiContext> for HasExtraHandsForHaulingConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::HasExtraHandsForHauling(self.0, self.1)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}

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
