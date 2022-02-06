use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};
use crate::Entity;

/// 0.95 if this entity has this number of extra hands available for hauling, or is already hauling
/// the item (or target entity if None), else 0
// TODO also count currently occupied hands as "available", could drop current item to haul this
pub struct HasExtraHandsForHaulingConsideration {
    pub extra_hands: u16,
    /// Defaults to target entity
    pub target: Option<Entity>,
}

impl Consideration<AiContext> for HasExtraHandsForHaulingConsideration {
    fn curve(&self) -> Curve {
        Curve::Identity
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::HasExtraHandsForHauling(self.extra_hands, self.target)
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Nop
    }
}
