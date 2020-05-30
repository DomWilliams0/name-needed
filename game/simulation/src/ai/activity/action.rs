use crate::ai::activity::ItemsToPickUp;
use crate::item::LooseItemReference;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum AiAction {
    Nop,

    Wander,

    GoPickUp(ItemsToPickUp),

    UseHeldItem(LooseItemReference),
}

impl Default for AiAction {
    fn default() -> Self {
        AiAction::Nop
    }
}
