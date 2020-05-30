use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};
use crate::item::ItemFilter;

pub struct HoldingItemConsideration(pub ItemFilter);

pub struct FindLocalItemConsideration {
    pub filter: ItemFilter,
    pub max_radius: u32,
    /// Max input value to map to 1.0
    /// e.g. 5.0: 2 perfect items would have an input of 2.0, so this maps to less than 0.5
    ///  but 1.0 would map that to the full 1.0
    pub normalize_range: f32,
}

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
}

impl Consideration<AiContext> for FindLocalItemConsideration {
    fn curve(&self) -> Curve {
        Curve::Exponential(2.0, -4.0, 0.0, -1.0, 1.0)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::CanFindLocally {
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
}
