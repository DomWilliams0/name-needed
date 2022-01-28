use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};
use crate::item::ItemFilter;

/// 1 if this entity has this number of extra hands available for hauling, or is already hauling
/// a matching entity, else 0
// TODO also count currently occupied hands as "available", could drop current item to haul this
pub struct HasExtraHandsForHaulingConsideration(pub u16, pub Option<ItemFilter>);

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
